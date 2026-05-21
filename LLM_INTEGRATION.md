# LLM Integration Guide

## Architecture Overview

NexaCode provides a flexible LLM integration system that supports multiple AI providers:

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (React)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   useLLM     │  │  LLMService  │  │ ChatExample  │  │
│  │    Hook      │  │   (API)      │  │  Component   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└───────────────────────┬─────────────────────────────────┘
                        │ Tauri IPC
┌───────────────────────▼─────────────────────────────────┐
│                Backend (Rust/Tauri)                      │
│  ┌──────────────────────────────────────────────────┐  │
│  │              LLMManager                           │  │
│  │  - Manage multiple providers                      │  │
│  │  - Switch between providers                       │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │              Tauri Commands                       │  │
│  │  - add_provider, remove_provider                  │  │
│  │  - chat, chat_stream                              │  │
│  │  - list_models, list_providers                    │  │
│  └──────────────────────────────────────────────────┘  │
└───────────────────────┬─────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────┐
│              nexacode-core (Library)                     │
│  ┌──────────────────────────────────────────────────┐  │
│  │           LLMProvider Trait                       │  │
│  │  - chat() / chat_stream()                         │  │
│  │  - list_models()                                  │  │
│  └──────────────────────────────────────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │ OpenAI       │  │ Anthropic    │  │   Future    │ │
│  │ Provider     │  │ Provider     │  │  Providers  │ │
│  └──────────────┘  └──────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## Supported Providers

## Configuration Storage

### File Location
- **macOS/Linux**: `~/.nexacode/config.toml`
- **Windows**: `C:\Users\<username>\.nexacode\config.toml`

### File Format (TOML)
```toml
[providers.openai]
config = { provider_type = "openai", api_key = "sk-...", default_model = "gpt-4" }
is_active = true

[providers.claude]
config = { provider_type = "anthropic", api_key = "sk-ant-...", default_model = "claude-3-5-sonnet-20241022" }
is_active = false

[providers.ollama]
config = { provider_type = "openai_compatible", api_key = "ollama", base_url = "http://localhost:11434/v1", default_model = "llama2" }
is_active = false
```

### Auto-Loading
- Configuration is automatically loaded on application startup
- If the file doesn't exist, it will be created when you add your first provider
- Invalid providers are logged but don't prevent the app from starting
- Changes are automatically saved when you add/remove/modify providers

### 1. OpenAI
- GPT-4, GPT-3.5 Turbo, GPT-4 Vision
- Full API support with streaming

### 2. OpenAI-Compatible Services
- Ollama (local models)
- vLLM
- LocalAI
- Any service with OpenAI-compatible API

### 3. Anthropic
- Claude 3.5 Sonnet
- Claude 3.5 Haiku
- Claude 3 Opus

## Quick Start

### 1. Setup Provider (Frontend)

```typescript
import { LLMService } from './services/llm';

// Add OpenAI
await LLMService.addProvider(
  'openai',
  'openai',
  'sk-your-api-key',
  'gpt-4'
);

// Add Claude
await LLMService.addProvider(
  'claude',
  'anthropic',
  'sk-ant-your-api-key',
  'claude-3-5-sonnet-20241022'
);

// Add Ollama (local)
await LLMService.addProvider(
  'ollama',
  'openai_compatible',
  'ollama',
  'llama2',
  'http://localhost:11434/v1'
);
```

### 2. Send Chat Messages

```typescript
// Simple chat
const response = await LLMService.chat(
  [
    { role: 'system', content: 'You are a helpful assistant.' },
    { role: 'user', content: 'Hello!' }
  ],
  'gpt-4'
);

console.log(response.content);
```

### 3. Streaming Response

```typescript
await LLMService.chatStream(
  messages,
  'gpt-4',
  (chunk) => {
    console.log('Received:', chunk.delta);
  },
  (error) => {
    console.error('Error:', error);
  },
  () => {
    console.log('Stream ended');
  }
);
```

### 4. Using React Hook

```typescript
import { useLLM } from './hooks/useLLM';

function MyComponent() {
  const { chat, chatStream, streamingContent, isLoading } = useLLM();

  const handleSend = async () => {
    await chatStream(
      [{ role: 'user', content: 'Hello!' }],
      'gpt-4'
    );
    // streamingContent will be updated automatically
  };

  return (
    <div>
      <button onClick={handleSend} disabled={isLoading}>
        Send
      </button>
      <p>{streamingContent}</p>
    </div>
  );
}
```

## Tauri Commands Reference

### Provider Management

```rust
// Add a new provider
add_provider(
    name: String,              // "openai", "claude", etc.
    provider_type: String,     // "openai", "anthropic", "openai_compatible"
    api_key: String,
    default_model: String,
    base_url: Option<String>   // For custom endpoints
)

// Remove a provider
remove_provider(name: String)

// Set active provider
set_active_provider(name: String)

// List all providers
list_providers() -> Vec<String>

// Get active provider name
get_active_provider() -> Option<String>
```

### Chat Operations

```rust
// Non-streaming chat
chat(
    messages: Vec<ChatMessage>,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>
) -> ChatResponse

// Streaming chat (emits events)
chat_stream(
    messages: Vec<ChatMessage>,
    model: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>
)
// Events: "chat-chunk", "chat-error", "chat-end"

// List available models
list_models() -> Vec<ModelInfo>
```

## Configuration Storage

Provider configurations can be persisted:

```typescript
// Save configuration
const config = {
  name: 'openai',
  providerType: 'openai',
  apiKey: 'sk-xxx',
  defaultModel: 'gpt-4'
};
localStorage.setItem('llm-config', JSON.stringify(config));

// Load configuration
const saved = JSON.parse(localStorage.getItem('llm-config'));
await LLMService.addProvider(
  saved.name,
  saved.providerType,
  saved.apiKey,
  saved.defaultModel
);
```

## Extending with New Providers

To add a new LLM provider:

1. Implement `LLMProvider` trait in `nexacode-core/src/llm/providers/`
2. Add provider type to `ProviderType` enum
3. Update `LLMClient::new()` to handle new type
4. No frontend changes needed!

Example:

```rust
// crates/nexacode-core/src/llm/providers/google.rs
pub struct GoogleProvider {
    config: ProviderConfig,
    base: BaseProvider,
}

#[async_trait]
impl LLMProvider for GoogleProvider {
    async fn chat_stream(...) -> Result<StreamingResponse> {
        // Implement Google Gemini API
    }
    
    // ... other trait methods
}
```

## Error Handling

All commands return `Result<T, String>` where errors are converted to strings for Tauri IPC.

Frontend should handle errors:

```typescript
try {
  const response = await LLMService.chat(messages, model);
} catch (error) {
  console.error('Chat failed:', error);
  // Show error to user
}
```

## Performance Tips

1. **Reuse providers**: Add providers once, reuse for multiple chats
2. **Streaming**: Use streaming for better UX with long responses
3. **Model selection**: Choose appropriate model for task (e.g., Haiku for simple tasks)
4. **Temperature**: Lower temperature (0.2-0.5) for code, higher (0.7-0.9) for creative tasks

## Security Notes

- API keys are stored in memory only (not persisted by default)
- Consider using Tauri's secure storage for API key persistence
- Never log or expose API keys in error messages
