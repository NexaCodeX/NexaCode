use serde::{Deserialize, Serialize};
use std::sync::Arc;
use futures::StreamExt;

use crate::llm::types::{ChatOptions, Message};
use crate::llm::LLMClient;
use crate::session::SessionLogger;
use crate::tools::{ToolContext, ToolRegistry, ToolResult};

/// Events emitted during Agent Loop execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// Agent is thinking (received text from LLM)
    Thinking {
        content: String,
    },
    /// Agent wants to call a tool
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
        /// Whether this tool call requires user confirmation
        requires_confirmation: bool,
    },
    /// A tool has been executed
    ToolResult {
        tool_call_id: String,
        name: String,
        result: ToolResult,
    },
    /// Agent completed its task with a final text response
    Completed {
        content: String,
    },
    /// Agent reached the maximum iteration limit
    MaxIterationsReached {
        iterations: usize,
        partial_response: String,
    },
    /// An error occurred
    Error {
        message: String,
    },
}

/// Configuration for the Agent Loop
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of Think-Act-Observe iterations
    pub max_iterations: usize,
    /// The system prompt to use
    pub system_prompt: Option<String>,
    /// Temperature for LLM calls
    pub temperature: Option<f32>,
    /// Max tokens for LLM responses
    pub max_tokens: Option<u32>,
    /// Whether to auto-approve safe tool calls (skip user confirmation)
    pub auto_approve_safe: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            auto_approve_safe: true,
        }
    }
}

impl AgentConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }
}

/// The Agent Loop engine
///
/// Implements the core Think-Act-Observe cycle:
/// 1. Send messages + tool definitions to LLM
/// 2. If LLM responds with text → emit Completed event, done
/// 3. If LLM responds with tool calls → execute tools, add results to messages, go back to step 1
/// 4. Repeat until LLM gives final text response or max iterations reached
pub struct AgentLoop {
    client: Arc<LLMClient>,
    registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
    config: AgentConfig,
}

impl AgentLoop {
    pub fn new(
        client: Arc<LLMClient>,
        registry: Arc<ToolRegistry>,
        context: Arc<ToolContext>,
    ) -> Self {
        Self {
            client,
            registry,
            context,
            config: AgentConfig::default(),
        }
    }

    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Run the Agent Loop for a user message.
    ///
    /// Returns a vector of events that describe the full execution trace.
    /// The caller can process these events to update the UI, log activity, etc.
    pub async fn run(
        &self,
        user_message: &str,
        model: &str,
    ) -> Vec<AgentEvent> {
        let mut events = Vec::new();

        // Build initial message list
        let mut messages = Vec::new();

        // Add system prompt
        if let Some(prompt) = &self.config.system_prompt {
            messages.push(Message::system(prompt));
        }

        // Add user message
        messages.push(Message::user(user_message));

        // Get tool definitions
        let tool_definitions = self.registry.definitions();

        // Build chat options
        let mut options = ChatOptions::new(model);
        if let Some(temp) = self.config.temperature {
            options = options.with_temperature(temp);
        }
        if let Some(tokens) = self.config.max_tokens {
            options = options.with_max_tokens(tokens);
        }

        // Agent Loop: Think → Act → Observe → ...
        for iteration in 0..self.config.max_iterations {
            log::info!(
                "Agent Loop iteration {}/{}",
                iteration + 1,
                self.config.max_iterations
            );

            // Call LLM with tools
            let response = match self
                .client
                .chat_with_tools(messages.clone(), options.clone(), tool_definitions.clone())
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    events.push(AgentEvent::Error {
                        message: format!("LLM call failed: {}", e),
                    });
                    return events;
                }
            };

            // Check if LLM responded with text (no tool calls)
            if !response.has_tool_calls() {
                let content = response.content.unwrap_or_default();
                events.push(AgentEvent::Completed {
                    content: content.clone(),
                });

                // Add assistant message to history
                messages.push(Message::assistant(&content));
                return events;
            }

            // LLM requested tool calls
            // First, emit the text content if any (thinking)
            if let Some(thinking) = &response.content {
                if !thinking.is_empty() {
                    events.push(AgentEvent::Thinking {
                        content: thinking.clone(),
                    });
                }
            }

            // Add assistant's tool call message to conversation
            let tool_calls = response.tool_calls.clone();
            messages.push(Message::assistant_tool_calls(tool_calls.clone()));

            // Execute each tool call
            for tc in &tool_calls {
                // Check if confirmation is required
                let requires_confirmation = self.registry.requires_confirmation(&tc.name, &tc.arguments);

                // Emit ToolCall event
                events.push(AgentEvent::ToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    requires_confirmation,
                });

                // Execute the tool
                let result = match self.registry.execute(&tc.name, tc.arguments.clone(), &self.context).await {
                    Ok(r) => r,
                    Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
                };

                // Emit ToolResult event
                events.push(AgentEvent::ToolResult {
                    tool_call_id: tc.id.clone(),
                    name: tc.name.clone(),
                    result: result.clone(),
                });

                // Add tool result to conversation
                messages.push(Message::tool_result(
                    &tc.id,
                    &tc.name,
                    &result.output,
                    result.is_error,
                ));
            }
        }

        // Max iterations reached
        events.push(AgentEvent::MaxIterationsReached {
            iterations: self.config.max_iterations,
            partial_response: String::from(
                "Agent reached the maximum number of iterations without completing.",
            ),
        });

        events
    }

    /// Run the Agent Loop with a streaming event callback.
    ///
    /// Each event is emitted immediately as it happens (instead of collecting all
    /// events into a Vec first). This is the preferred method for UI-driven scenarios
    /// where the frontend needs real-time progress updates.
    ///
    /// A small delay (`event_delay`) is inserted after each event emission so that
    /// the frontend has time to render the update before the next event arrives.
    /// This prevents the UI from appearing to "stutter" when many events fire in
    /// rapid succession (e.g. when multiple fast tools finish within the same
    /// millisecond).
    ///
    /// If a `SessionLogger` is provided, every LLM request and response is
    /// appended to `~/.nexacode/logs/{session_id}.log` for debugging.
    pub async fn run_streaming<F>(
        &self,
        user_message: &str,
        model: &str,
        mut on_event: F,
        logger: Option<SessionLogger>,
    ) where
        F: FnMut(AgentEvent),
    {
        // Minimum delay between consecutive events sent to the frontend.
        // 50 ms is a good balance: fast enough to feel responsive, slow enough
        // for the browser to paint each update individually.
        let event_delay = std::time::Duration::from_millis(50);

        // Log the start of this agent run
        if let Some(ref l) = logger {
            l.log_run_start(user_message, model).await;
        }

        // Build initial message list
        let mut messages = Vec::new();

        // Add system prompt
        if let Some(prompt) = &self.config.system_prompt {
            messages.push(Message::system(prompt));
        }

        // Add user message
        messages.push(Message::user(user_message));

        // Get tool definitions
        let tool_definitions = self.registry.definitions();

        // Build chat options
        let mut options = ChatOptions::new(model).with_stream(true);
        if let Some(temp) = self.config.temperature {
            options = options.with_temperature(temp);
        }
        if let Some(tokens) = self.config.max_tokens {
            options = options.with_max_tokens(tokens);
        }

        // Agent Loop: Think → Act → Observe → ...
        for iteration in 0..self.config.max_iterations {
            log::info!(
                "[Agent] Loop iteration {}/{}",
                iteration + 1,
                self.config.max_iterations
            );

            // Log request to session log
            if let Some(ref l) = logger {
                l.log_request(&messages, model, iteration + 1).await;
            }

            // Call LLM with tools in streaming mode
            let mut stream = match self
                .client
                .chat_stream_with_tools(messages.clone(), options.clone(), tool_definitions.clone())
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    log::error!("[Agent] LLM call failed: {}", e);
                    on_event(AgentEvent::Error {
                        message: format!("LLM call failed: {}", e),
                    });
                    return;
                }
            };

            let mut response_content = String::new();
            let mut tool_calls_builder: std::collections::BTreeMap<usize, (Option<String>, Option<String>, String)> =
                std::collections::BTreeMap::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // 1. Handle text content (thinking/reasoning delta)
                        if !chunk.delta.is_empty() {
                            response_content.push_str(&chunk.delta);
                            on_event(AgentEvent::Thinking {
                                content: chunk.delta.clone(),
                            });
                        }

                        // 2. Handle tool call deltas
                        if let Some(tc_delta) = chunk.tool_call_delta {
                            let entry = tool_calls_builder
                                .entry(tc_delta.index)
                                .or_insert((None, None, String::new()));
                            if let Some(id) = tc_delta.id {
                                entry.0 = Some(id);
                            }
                            if let Some(name) = tc_delta.name {
                                entry.1 = Some(name);
                            }
                            if let Some(arg_delta) = tc_delta.arguments_delta {
                                entry.2.push_str(&arg_delta);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("[Agent] LLM stream error: {}", e);
                        on_event(AgentEvent::Error {
                            message: format!("LLM stream error: {}", e),
                        });
                        return;
                    }
                }
            }

            // Build the final response and tool calls list from accumulated data
            let has_tool_calls = !tool_calls_builder.is_empty();
            let mut tool_calls = Vec::new();
            for (_index, (id, name, args_str)) in tool_calls_builder {
                let id = id.unwrap_or_default();
                let name = name.unwrap_or_default();
                let arguments: serde_json::Value = serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null);
                tool_calls.push(crate::llm::types::ToolCall {
                    id,
                    name,
                    arguments,
                });
            }

            let response = crate::llm::types::ToolAwareResponse {
                content: if response_content.is_empty() {
                    None
                } else {
                    Some(response_content.clone())
                },
                tool_calls: tool_calls.clone(),
                model: model.to_string(),
                usage: None,
                stop_reason: None,
            };

            log::info!("[Agent] LLM responded: has_tool_calls={}, content_len={}",
                has_tool_calls,
                response_content.len()
            );

            // Log response to session log
            if let Some(ref l) = logger {
                l.log_response(&response, iteration + 1).await;
            }

            // Check if LLM responded with text (no tool calls)
            if !response.has_tool_calls() {
                log::info!("[Agent] Completed with content: {} chars", response_content.len());

                // Log the final completion
                if let Some(ref l) = logger {
                    l.log_run_completed(&response_content, iteration + 1).await;
                }

                on_event(AgentEvent::Completed {
                    content: response_content.clone(),
                });
                tokio::time::sleep(event_delay).await;

                // Add assistant message to history
                messages.push(Message::assistant(&response_content));
                return;
            }

            // Add assistant's tool call message to conversation
            log::info!("[Agent] Tool calls: {}", tool_calls.len());
            messages.push(Message::assistant_tool_calls(tool_calls.clone()));

            // Execute each tool call
            for tc in &tool_calls {
                // Check if confirmation is required
                let requires_confirmation = self.registry.requires_confirmation(&tc.name, &tc.arguments);

                log::info!("[Agent] ToolCall: id={}, name={}", tc.id, tc.name);

                // Emit ToolCall event immediately
                on_event(AgentEvent::ToolCall {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                    requires_confirmation,
                });
                tokio::time::sleep(event_delay).await;

                // Execute the tool
                let result = match self.registry.execute(&tc.name, tc.arguments.clone(), &self.context).await {
                    Ok(r) => r,
                    Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
                };

                log::info!(
                    "[Agent] ToolResult: id={}, name={}, is_error={}, output_len={}",
                    tc.id, tc.name, result.is_error, result.output.len()
                );

                // Log tool result to session log
                if let Some(ref l) = logger {
                    l.log_tool_result(&tc.id, &tc.name, &tc.arguments, &result.output, result.is_error, iteration + 1).await;
                }

                // Emit ToolResult event immediately
                on_event(AgentEvent::ToolResult {
                    tool_call_id: tc.id.clone(),
                    name: tc.name.clone(),
                    result: result.clone(),
                });
                tokio::time::sleep(event_delay).await;

                // Add tool result to conversation
                messages.push(Message::tool_result(
                    &tc.id,
                    &tc.name,
                    &result.output,
                    result.is_error,
                ));
            }
        }

        // Max iterations reached
        log::warn!("[Agent] Max iterations reached: {}", self.config.max_iterations);
        on_event(AgentEvent::MaxIterationsReached {
            iterations: self.config.max_iterations,
            partial_response: String::from(
                "Agent reached the maximum number of iterations without completing.",
            ),
        });
        tokio::time::sleep(event_delay).await;
    }

    /// Run a single step of the Agent Loop (for interactive mode)
    ///
    /// Given a list of messages, call the LLM once and return:
    /// - The LLM's response (text or tool calls)
    /// - The tool results if any were executed
    pub async fn step(
        &self,
        messages: Vec<Message>,
        model: &str,
    ) -> Result<AgentStepResult, anyhow::Error> {
        let tool_definitions = self.registry.definitions();

        let mut options = ChatOptions::new(model);
        if let Some(temp) = self.config.temperature {
            options = options.with_temperature(temp);
        }
        if let Some(tokens) = self.config.max_tokens {
            options = options.with_max_tokens(tokens);
        }

        let response = self
            .client
            .chat_with_tools(messages, options, tool_definitions)
            .await?;

        if !response.has_tool_calls() {
            return Ok(AgentStepResult::Text {
                content: response.content.unwrap_or_default(),
                usage: response.usage,
            });
        }

        Ok(AgentStepResult::ToolCalls {
            thinking: response.content,
            tool_calls: response.tool_calls,
            usage: response.usage,
        })
    }

    /// Execute a set of tool calls and return the results
    pub async fn execute_tools(
        &self,
        tool_calls: &[(String, String, serde_json::Value)], // (id, name, arguments)
    ) -> Vec<(String, String, ToolResult)> {
        let mut results = Vec::new();

        for (id, name, arguments) in tool_calls {
            let result = match self.registry.execute(name, arguments.clone(), &self.context).await {
                Ok(r) => r,
                Err(e) => ToolResult::error(format!("Tool execution error: {}", e)),
            };
            results.push((id.clone(), name.clone(), result));
        }

        results
    }
}

/// Result of a single Agent step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentStepResult {
    /// LLM responded with text (conversation is complete)
    Text {
        content: String,
        usage: Option<crate::llm::types::Usage>,
    },
    /// LLM wants to call tools
    ToolCalls {
        /// Any text the LLM produced alongside the tool calls (thinking)
        thinking: Option<String>,
        /// The tool calls to execute
        tool_calls: Vec<crate::llm::types::ToolCall>,
        usage: Option<crate::llm::types::Usage>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_iterations, 50);
        assert!(config.system_prompt.is_none());
        assert!(config.auto_approve_safe);
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new()
            .with_max_iterations(10)
            .with_system_prompt("You are a coding assistant")
            .with_temperature(0.7)
            .with_max_tokens(4096);

        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.system_prompt, Some("You are a coding assistant".to_string()));
        assert_eq!(config.temperature, Some(0.7));
        assert_eq!(config.max_tokens, Some(4096));
    }

    #[test]
    fn test_agent_event_serialization() {
        let event = AgentEvent::ToolCall {
            id: "call_1".to_string(),
            name: "Read".to_string(),
            arguments: serde_json::json!({"path": "test.rs"}),
            requires_confirmation: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("tool_call"));
        assert!(json.contains("Read"));
    }

    #[test]
    fn test_agent_event_thinking() {
        let event = AgentEvent::Thinking {
            content: "Let me analyze this code...".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("thinking"));
    }

    #[test]
    fn test_agent_event_completed() {
        let event = AgentEvent::Completed {
            content: "I've fixed the bug!".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("completed"));
    }

    #[test]
    fn test_agent_event_max_iterations() {
        let event = AgentEvent::MaxIterationsReached {
            iterations: 50,
            partial_response: "timeout".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("max_iterations_reached"));
    }

    #[test]
    fn test_agent_event_error() {
        let event = AgentEvent::Error {
            message: "LLM call failed".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("error"));
    }

    #[test]
    fn test_agent_step_result_text() {
        let result = AgentStepResult::Text {
            content: "Hello!".to_string(),
            usage: None,
        };
        match result {
            AgentStepResult::Text { content, .. } => assert_eq!(content, "Hello!"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_agent_step_result_tool_calls() {
        let result = AgentStepResult::ToolCalls {
            thinking: Some("Let me read the file".to_string()),
            tool_calls: vec![crate::llm::types::ToolCall::new(
                "call_1",
                "Read",
                serde_json::json!({"path": "main.rs"}),
            )],
            usage: None,
        };
        match result {
            AgentStepResult::ToolCalls { tool_calls, thinking, .. } => {
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(thinking, Some("Let me read the file".to_string()));
            }
            _ => panic!("Expected ToolCalls variant"),
        }
    }

    #[test]
    fn test_agent_step_result_serialization() {
        let result = AgentStepResult::Text {
            content: "Done!".to_string(),
            usage: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("text"));
        assert!(json.contains("Done!"));
    }

    #[test]
    fn test_agent_step_result_tool_calls_serialization() {
        let result = AgentStepResult::ToolCalls {
            thinking: None,
            tool_calls: vec![crate::llm::types::ToolCall::new(
                "call_1",
                "Read",
                serde_json::json!({"path": "test.rs"}),
            )],
            usage: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("tool_calls"));
    }

    #[test]
    fn test_agent_event_all_variants_roundtrip() {
        let events = vec![
            AgentEvent::Thinking { content: "thinking...".to_string() },
            AgentEvent::ToolCall {
                id: "call_1".to_string(),
                name: "Read".to_string(),
                arguments: serde_json::json!({"path": "test.rs"}),
                requires_confirmation: false,
            },
            AgentEvent::ToolResult {
                tool_call_id: "call_1".to_string(),
                name: "Read".to_string(),
                result: ToolResult::success("file contents"),
            },
            AgentEvent::Completed { content: "Done!".to_string() },
            AgentEvent::MaxIterationsReached {
                iterations: 50,
                partial_response: "partial".to_string(),
            },
            AgentEvent::Error { message: "error".to_string() },
        ];

        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let decoded: AgentEvent = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&decoded).unwrap();
            assert_eq!(json, json2, "Roundtrip failed for event");
        }
    }

    #[test]
    fn test_agent_event_tool_result_with_error() {
        let event = AgentEvent::ToolResult {
            tool_call_id: "call_err".to_string(),
            name: "Bash".to_string(),
            result: ToolResult::error("command failed with exit code 1"),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("tool_result"));
        let decoded: AgentEvent = serde_json::from_str(&json).unwrap();
        if let AgentEvent::ToolResult { result, .. } = decoded {
            assert!(result.is_error);
            assert_eq!(result.output, "command failed with exit code 1");
        } else {
            panic!("Expected ToolResult variant");
        }
    }

    #[test]
    fn test_agent_config_zero_iterations() {
        let config = AgentConfig::new().with_max_iterations(0);
        assert_eq!(config.max_iterations, 0);
    }

    // ==========================================
    // Integration-like tests with MockProvider
    // ==========================================

    use crate::llm::traits::{LLMProvider, StreamingResponse};
    use crate::llm::types::{ChatResponse, ModelInfo, ToolAwareResponse};
    use crate::llm::LLMClient;
    use crate::tools::ToolContext;
    use crate::tools::default_registry;

    /// A mock LLM provider for testing the Agent Loop
    struct MockProvider {
        responses: std::sync::Mutex<Vec<ToolAwareResponse>>,
    }

    impl MockProvider {
        fn new(responses: Vec<ToolAwareResponse>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        async fn chat_stream(
            &self,
            _messages: Vec<Message>,
            _options: crate::llm::types::ChatOptions,
        ) -> Result<StreamingResponse, anyhow::Error> {
            Err(anyhow::anyhow!("Not implemented for mock"))
        }

        async fn chat(
            &self,
            _messages: Vec<Message>,
            _options: crate::llm::types::ChatOptions,
        ) -> Result<ChatResponse, anyhow::Error> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Ok(ChatResponse {
                    content: "No more responses".to_string(),
                    model: "mock".to_string(),
                    usage: None,
                });
            }
            let resp = responses.remove(0);
            Ok(ChatResponse {
                content: resp.content.unwrap_or_default(),
                model: resp.model.clone(),
                usage: resp.usage.clone(),
            })
        }

        async fn list_models(&self) -> Result<Vec<ModelInfo>, anyhow::Error> {
            Ok(vec![ModelInfo {
                id: "mock-model".to_string(),
                name: Some("Mock Model".to_string()),
                description: None,
            }])
        }

        fn name(&self) -> &str {
            "Mock"
        }

        async fn chat_with_tools(
            &self,
            _messages: Vec<Message>,
            _options: crate::llm::types::ChatOptions,
            _tools: Vec<crate::tools::ToolDefinition>,
        ) -> Result<ToolAwareResponse, anyhow::Error> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Ok(ToolAwareResponse {
                    content: Some("No more responses".to_string()),
                    tool_calls: vec![],
                    model: "mock".to_string(),
                    usage: None,
                    stop_reason: Some("stop".to_string()),
                });
            }
            Ok(responses.remove(0))
        }

        async fn chat_stream_with_tools(
            &self,
            _messages: Vec<Message>,
            _options: crate::llm::types::ChatOptions,
            _tools: Vec<crate::tools::ToolDefinition>,
        ) -> Result<StreamingResponse, anyhow::Error> {
            Err(anyhow::anyhow!("Not implemented for mock"))
        }
    }

    /// Helper to create an AgentLoop with a mock provider
    fn create_test_agent(responses: Vec<ToolAwareResponse>) -> AgentLoop {
        let mock = MockProvider::new(responses);
        let provider: Arc<dyn LLMProvider> = Arc::new(mock);

        // Create a client manually (bypassing the config-based constructor)
        let client = LLMClient::from_provider(provider);
        let registry = Arc::new(default_registry());
        let context = Arc::new(ToolContext::new(std::env::temp_dir()));

        AgentLoop::new(Arc::new(client), registry, context)
            .with_config(AgentConfig::new().with_max_iterations(10))
    }

    #[tokio::test]
    async fn test_agent_loop_immediate_text_response() {
        // LLM immediately responds with text (no tool calls)
        let agent = create_test_agent(vec![ToolAwareResponse {
            content: Some("Hello! I can help you with that.".to_string()),
            tool_calls: vec![],
            model: "mock".to_string(),
            usage: None,
            stop_reason: Some("stop".to_string()),
        }]);

        let events = agent.run("Hi there", "mock-model").await;

        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Completed { content } => {
                assert_eq!(content, "Hello! I can help you with that.");
            }
            _ => panic!("Expected Completed event, got {:?}", events[0]),
        }
    }

    #[tokio::test]
    async fn test_agent_loop_one_tool_call_then_text() {
        // LLM calls a tool, then responds with text
        let agent = create_test_agent(vec![
            // First call: LLM wants to call a tool
            ToolAwareResponse {
                content: Some("Let me read the file.".to_string()),
                tool_calls: vec![crate::llm::types::ToolCall::new(
                    "call_1",
                    "Bash",
                    serde_json::json!({"command": "echo hello"}),
                )],
                model: "mock".to_string(),
                usage: None,
                stop_reason: Some("tool_calls".to_string()),
            },
            // Second call: LLM responds with text
            ToolAwareResponse {
                content: Some("The file says hello!".to_string()),
                tool_calls: vec![],
                model: "mock".to_string(),
                usage: None,
                stop_reason: Some("stop".to_string()),
            },
        ]);

        let events = agent.run("Read the file", "mock-model").await;

        // Should have: Thinking, ToolCall, ToolResult, Completed
        assert!(events.len() >= 3);

        // Check for Thinking event
        let thinking_events: Vec<_> = events.iter().filter(|e| matches!(e, AgentEvent::Thinking { .. })).collect();
        assert_eq!(thinking_events.len(), 1);

        // Check for ToolCall event
        let tool_call_events: Vec<_> = events.iter().filter(|e| matches!(e, AgentEvent::ToolCall { .. })).collect();
        assert_eq!(tool_call_events.len(), 1);

        // Check for ToolResult event
        let tool_result_events: Vec<_> = events.iter().filter(|e| matches!(e, AgentEvent::ToolResult { .. })).collect();
        assert_eq!(tool_result_events.len(), 1);

        // Check for Completed event
        let completed_events: Vec<_> = events.iter().filter(|e| matches!(e, AgentEvent::Completed { .. })).collect();
        assert_eq!(completed_events.len(), 1);

        // Verify the Bash tool actually executed (it should have "hello" in the output)
        if let AgentEvent::ToolResult { result, .. } = &tool_result_events[0] {
            assert!(!result.is_error);
            assert!(result.output.contains("hello"));
        }
    }

    #[tokio::test]
    async fn test_agent_loop_max_iterations() {
        // LLM keeps calling tools forever — should hit max iterations
        let infinite_tool_call = ToolAwareResponse {
            content: None,
            tool_calls: vec![crate::llm::types::ToolCall::new(
                "call_loop",
                "Bash",
                serde_json::json!({"command": "echo loop"}),
            )],
            model: "mock".to_string(),
            usage: None,
            stop_reason: Some("tool_calls".to_string()),
        };

        // Create agent with max 3 iterations
        let agent = create_test_agent(vec![infinite_tool_call; 10])
            .with_config(AgentConfig::new().with_max_iterations(3));

        let events = agent.run("Keep going", "mock-model").await;

        // Should have: 3x (ToolCall + ToolResult) + MaxIterationsReached
        let tool_call_count = events.iter().filter(|e| matches!(e, AgentEvent::ToolCall { .. })).count();
        assert_eq!(tool_call_count, 3);

        let max_iter_events: Vec<_> = events.iter()
            .filter(|e| matches!(e, AgentEvent::MaxIterationsReached { .. }))
            .collect();
        assert_eq!(max_iter_events.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_step_text() {
        let agent = create_test_agent(vec![ToolAwareResponse {
            content: Some("Direct answer".to_string()),
            tool_calls: vec![],
            model: "mock".to_string(),
            usage: None,
            stop_reason: Some("stop".to_string()),
        }]);

        let result = agent.step(vec![Message::user("test")], "mock-model").await.unwrap();

        match result {
            AgentStepResult::Text { content, .. } => {
                assert_eq!(content, "Direct answer");
            }
            _ => panic!("Expected Text result"),
        }
    }

    #[tokio::test]
    async fn test_agent_step_tool_calls() {
        let agent = create_test_agent(vec![ToolAwareResponse {
            content: Some("Let me check".to_string()),
            tool_calls: vec![crate::llm::types::ToolCall::new(
                "call_1",
                "Bash",
                serde_json::json!({"command": "ls"}),
            )],
            model: "mock".to_string(),
            usage: None,
            stop_reason: Some("tool_calls".to_string()),
        }]);

        let result = agent.step(vec![Message::user("test")], "mock-model").await.unwrap();

        match result {
            AgentStepResult::ToolCalls { thinking, tool_calls, .. } => {
                assert_eq!(thinking, Some("Let me check".to_string()));
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name, "Bash");
            }
            _ => panic!("Expected ToolCalls result"),
        }
    }

    #[tokio::test]
    async fn test_agent_execute_tools() {
        let registry = Arc::new(default_registry());
        let context = Arc::new(ToolContext::new(std::env::temp_dir()));
        let mock = MockProvider::new(vec![]);
        let provider: Arc<dyn LLMProvider> = Arc::new(mock);
        let client = Arc::new(LLMClient::from_provider(provider));

        let agent = AgentLoop::new(client, registry, context);

        let results = agent.execute_tools(&[
            ("call_1".to_string(), "Bash".to_string(), serde_json::json!({"command": "echo test123"})),
        ]).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "call_1");
        assert_eq!(results[0].1, "Bash");
        assert!(results[0].2.output.contains("test123"));
        assert!(!results[0].2.is_error);
    }

    #[tokio::test]
    async fn test_agent_execute_tools_unknown_tool() {
        let registry = Arc::new(default_registry());
        let context = Arc::new(ToolContext::new(std::env::temp_dir()));
        let mock = MockProvider::new(vec![]);
        let provider: Arc<dyn LLMProvider> = Arc::new(mock);
        let client = Arc::new(LLMClient::from_provider(provider));

        let agent = AgentLoop::new(client, registry, context);

        let results = agent.execute_tools(&[
            ("call_1".to_string(), "NonExistentTool".to_string(), serde_json::json!({})),
        ]).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].2.is_error);
        assert!(results[0].2.output.contains("not found") || results[0].2.output.contains("Unknown"));
    }

    #[tokio::test]
    async fn test_agent_loop_with_system_prompt() {
        let agent = create_test_agent(vec![ToolAwareResponse {
            content: Some("System prompt was set".to_string()),
            tool_calls: vec![],
            model: "mock".to_string(),
            usage: None,
            stop_reason: Some("stop".to_string()),
        }])
        .with_config(AgentConfig::new().with_system_prompt("You are a coding assistant"));

        let events = agent.run("Hello", "mock-model").await;
        match &events[0] {
            AgentEvent::Completed { content } => {
                assert_eq!(content, "System prompt was set");
            }
            _ => panic!("Expected Completed event"),
        }
    }

    #[tokio::test]
    async fn test_agent_loop_multiple_tool_calls_in_one_turn() {
        // LLM calls multiple tools in a single turn
        let agent = create_test_agent(vec![
            ToolAwareResponse {
                content: None,
                tool_calls: vec![
                    crate::llm::types::ToolCall::new("call_1", "Bash", serde_json::json!({"command": "echo first"})),
                    crate::llm::types::ToolCall::new("call_2", "Bash", serde_json::json!({"command": "echo second"})),
                ],
                model: "mock".to_string(),
                usage: None,
                stop_reason: Some("tool_calls".to_string()),
            },
            ToolAwareResponse {
                content: Some("Both commands executed!".to_string()),
                tool_calls: vec![],
                model: "mock".to_string(),
                usage: None,
                stop_reason: Some("stop".to_string()),
            },
        ]);

        let events = agent.run("Run two commands", "mock-model").await;

        let tool_call_count = events.iter().filter(|e| matches!(e, AgentEvent::ToolCall { .. })).count();
        assert_eq!(tool_call_count, 2);

        let tool_result_count = events.iter().filter(|e| matches!(e, AgentEvent::ToolResult { .. })).count();
        assert_eq!(tool_result_count, 2);

        // Verify both results
        let tool_results: Vec<_> = events.iter()
            .filter_map(|e| if let AgentEvent::ToolResult { result, .. } = e { Some(result.clone()) } else { None })
            .collect();
        assert!(tool_results[0].output.contains("first"));
        assert!(tool_results[1].output.contains("second"));

        // Final event should be Completed
        if let Some(AgentEvent::Completed { content }) = events.last() {
            assert_eq!(content, "Both commands executed!");
        } else {
            panic!("Expected Completed as last event");
        }
    }
}
