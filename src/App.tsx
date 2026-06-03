/* eslint-disable react-hooks/set-state-in-effect */
/* eslint-disable react-hooks/purity */
import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './styles/main.scss';
import logo from './assets/logo.png';
import { LucideIcon } from './components/LucideIcon';
import { Settings } from './components/Settings';
import { MarkdownRenderer } from './components/MarkdownRenderer';
import { AgentStepView } from './components/AgentStep';
import { useLLM } from './hooks/useLLM';
import { useAgent } from './hooks/useAgent';
import type { ChatMessage } from './services/llm';
import type { AgentStep } from './hooks/useAgent';

// Types matching Rust backend Session/SessionMeta
interface SessionMeta {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  message_count: number;
}

interface Session {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  messages: SessionMessage[];
}

/** A message as stored in the session JSON — steps use camelCase (matching Rust #[serde(rename_all = "camelCase")]) */
interface SessionMessage {
  role: string;
  content: string;
  steps?: SessionStepData[];
}

/** Step data in the session JSON (camelCase from Rust) */
interface SessionStepData {
  id: string;
  thinking?: string;
  toolCall?: {
    id: string;
    name: string;
    arguments: Record<string, unknown>;
    requiresConfirmation: boolean;
  };
  toolResult?: {
    toolCallId: string;
    name: string;
    output: string;
    isError: boolean;
  };
  status: string;
}

/** Chat mode: Build = Agent loop with tools, Chat = simple streaming */
type ChatMode = 'build' | 'chat';

/** The runtime message type used in the UI */
interface Message {
  role: 'system' | 'user' | 'assistant';
  content: string;
  /** Agent execution steps (only on assistant messages in Build mode) */
  steps?: AgentStep[];
}

/** Convert an AgentStep (runtime) to SessionStepData (for saving) */
function stepToSessionData(s: AgentStep): SessionStepData {
  return {
    id: s.id,
    thinking: s.thinking,
    toolCall: s.toolCall ? {
      id: s.toolCall.id,
      name: s.toolCall.name,
      arguments: s.toolCall.arguments,
      requiresConfirmation: s.toolCall.requires_confirmation,
    } : undefined,
    toolResult: s.toolResult ? {
      toolCallId: s.toolResult.tool_call_id,
      name: s.toolResult.name,
      output: s.toolResult.output,
      isError: s.toolResult.is_error,
    } : undefined,
    status: s.status,
  };
}

/** Convert a SessionStepData (from JSON) back to AgentStep (runtime) */
function sessionDataToStep(s: SessionStepData): AgentStep {
  return {
    id: s.id,
    thinking: s.thinking,
    toolCall: s.toolCall ? {
      id: s.toolCall.id,
      name: s.toolCall.name,
      arguments: s.toolCall.arguments,
      requires_confirmation: s.toolCall.requiresConfirmation,
    } : undefined,
    toolResult: s.toolResult ? {
      tool_call_id: s.toolResult.toolCallId,
      name: s.toolResult.name,
      output: s.toolResult.output,
      is_error: s.toolResult.isError,
    } : undefined,
    status: s.status as AgentStep['status'],
  };
}

function App() {
  const [sessions, setSessions] = useState<SessionMeta[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [model, setModel] = useState('');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [customModel, setCustomModel] = useState('');
  const [showCustomModel, setShowCustomModel] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [isResizing, setIsResizing] = useState(false);
  const [chatMode, setChatMode] = useState<ChatMode>('build');
  const [userHasScrolledUp, setUserHasScrolledUp] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Debounce timer for saving session
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Composition state for IME
  const isComposingRef = useRef(false);
  const isComposingJustEndedRef = useRef(false);

  const handleCompositionStart = () => {
    isComposingRef.current = true;
    isComposingJustEndedRef.current = false;
  };

  const handleCompositionEnd = () => {
    isComposingRef.current = false;
    isComposingJustEndedRef.current = true;
    setTimeout(() => {
      isComposingJustEndedRef.current = false;
    }, 50);
  };

  const {
    isLoading,
    error,
    streamingContent,
    chatStream,
    cancelStream,
    listProviders,
    getActiveProvider,
    getProviderConfig,
    clearError,
  } = useLLM();

  const agent = useAgent();

  // Combined loading state
  const isAnyLoading = isLoading || agent.isRunning;

  // ==========================================
  // Session persistence
  // ==========================================

  // Refresh session list from backend
  const refreshSessions = useCallback(async () => {
    try {
      const list = await invoke<SessionMeta[]>('list_sessions');
      setSessions(list);
    } catch (e) {
      console.error('Failed to load sessions:', e);
    }
  }, []);

  // Load session list on mount
  useEffect(() => {
    refreshSessions();
  }, [refreshSessions]);

  // Load a session's messages from backend
  const loadSessionMessages = async (sessionId: string) => {
    try {
      const session = await invoke<Session>('load_session', { sessionId });
      const loaded: Message[] = session.messages.map(m => ({
        role: m.role as Message['role'],
        content: m.content,
        steps: m.steps?.map(sessionDataToStep),
      }));
      setMessages(loaded);

      // If any message has steps, this was a Build mode session
      const hasSteps = loaded.some(m => m.steps && m.steps.length > 0);
      if (hasSteps) {
        setChatMode('build');
      }

      agent.reset();
    } catch (e) {
      console.error('Failed to load session:', e);
    }
  };

  // Save current session to disk (with debounce)
  const saveCurrentSession = useCallback((sessionId: string, sessionMessages: Message[], title?: string) => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(async () => {
      try {
        const now = Date.now();
        const session: Session = {
          id: sessionId,
          title: title || sessionMessages[0]?.content.slice(0, 30) + (sessionMessages[0]?.content.length > 30 ? '...' : '') || 'New Chat',
          created_at: now,
          updated_at: now,
          messages: sessionMessages.map(m => ({
            role: m.role,
            content: m.content,
            steps: m.steps?.map(stepToSessionData),
          })),
        };
        await invoke('save_session', { session });
        await refreshSessions();
      } catch (e) {
        console.error('Failed to save session:', e);
      }
    }, 500); // 500ms debounce
  }, [refreshSessions]);

  // ==========================================
  // Sidebar resize
  // ==========================================

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;
      const newWidth = Math.max(200, Math.min(400, e.clientX));
      setSidebarWidth(newWidth);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
    };

    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizing]);

  // ==========================================
  // Auto-scroll & streaming completion
  // ==========================================

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const container = e.currentTarget;
    const isAtBottom = container.scrollHeight - container.scrollTop - container.clientHeight <= 50;
    setUserHasScrolledUp(!isAtBottom);
  };

  // Reset scroll lock when switching sessions
  useEffect(() => {
    setUserHasScrolledUp(false);
  }, [activeSessionId]);

  useEffect(() => {
    if (!userHasScrolledUp) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, streamingContent, agent.steps, userHasScrolledUp]);

  // When streaming completes (Chat mode), add assistant message to messages and save
  useEffect(() => {
    if (streamingContent && !isLoading) {
      setMessages((prev) => {
        const lastMessage = prev[prev.length - 1];

        const newMessage: Message = {
          role: 'assistant',
          content: streamingContent,
        };

        let newMessages: Message[];
        if (lastMessage?.role === 'assistant') {
          newMessages = [...prev.slice(0, -1), newMessage];
        } else {
          newMessages = [...prev, newMessage];
        }

        if (activeSessionId) {
          saveCurrentSession(activeSessionId, newMessages);
        }

        return newMessages;
      });
    }
  }, [streamingContent, isLoading, activeSessionId, saveCurrentSession]);

  // When agent completes, create a single assistant message with steps + final content
  const { isRunning: isAgentRunning, finalResponse: agentFinalResponse, steps: agentSteps, reset: resetAgent } = agent;
  useEffect(() => {
    if (!isAgentRunning && agentFinalResponse && activeSessionId) {
      const content = agentFinalResponse.content;

      setMessages((prev) => {
        // Build the assistant message with embedded steps
        const assistantMsg: Message = {
          role: 'assistant',
          content,
          steps: agentSteps.length > 0 ? agentSteps : undefined,
        };

        const newMessages = [...prev, assistantMsg];
        saveCurrentSession(activeSessionId, newMessages);
        return newMessages;
      });

      resetAgent();
    }
  }, [isAgentRunning, agentFinalResponse, activeSessionId, saveCurrentSession, agentSteps, resetAgent]);

  // ==========================================
  // Provider loading
  // ==========================================

  const loadProviders = useCallback(async () => {
    const providerList = await listProviders();
    if (providerList.length === 0) {
      setShowSettings(true);
    } else {
      const active = await getActiveProvider();
      if (active) {
        const config = await getProviderConfig(active);
        if (config && config.models.length > 0) {
          setAvailableModels(config.models);
          setModel(config.models[0]);
        }
      }
    }
  }, [listProviders, getActiveProvider, getProviderConfig]);

  useEffect(() => {
    loadProviders();
  }, [loadProviders]);

  // ==========================================
  // Chat actions
  // ==========================================

  const handleTextareaInput = (e: React.FormEvent<HTMLTextAreaElement>) => {
    const textarea = e.currentTarget;
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
  };

  const handleSendMessage = async () => {
    if (!inputValue.trim() || isAnyLoading) return;

    if (!model) {
      console.error('[App] No model selected');
      return;
    }

    console.log('[App] handleSendMessage called, chatMode:', chatMode, 'model:', model);

    setUserHasScrolledUp(false);

    const userMessage: Message = { role: 'user', content: inputValue.trim() };
    const newMessages = [...messages, userMessage];

    setMessages(newMessages);
    setInputValue('');

    let sessionId = activeSessionId;
    if (!sessionId) {
      sessionId = Date.now().toString();
      setActiveSessionId(sessionId);
    }

    // Save session with user message
    const title = messages.length === 0
      ? inputValue.trim().slice(0, 30) + (inputValue.trim().length > 30 ? '...' : '')
      : undefined;
    saveCurrentSession(sessionId, newMessages, title);

    if (chatMode === 'build') {
      // Agent mode — run the agent loop
      console.log('[App] Running agent in build mode...');
      try {
        await agent.run({
          session_id: sessionId,
          message: inputValue.trim(),
          model,
        });
        console.log('[App] Agent run completed');
      } catch (err) {
        console.error('[App] agent_run error:', err);
      }
    } else {
      // Chat mode — simple streaming
      console.log('[App] Running chat stream...');
      const apiMessages: ChatMessage[] = newMessages.map(m => ({
        role: m.role,
        content: m.content,
      }));

      try {
        await chatStream(apiMessages, model, { sessionId });
        console.log('[App] Chat stream completed');
      } catch (err) {
        console.error('[App] chatStream error:', err);
      }
    }
  };

  const handleStop = () => {
    if (agent.isRunning) {
      agent.stop();
    }
    if (isLoading) {
      cancelStream();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // If the user is composing text with an IME (e.g. Chinese pinyin input),
    // or composition just ended, ignore Enter so the IME can handle candidate
    // selection instead of accidentally sending the message.
    if (
      isComposingRef.current ||
      isComposingJustEndedRef.current ||
      e.nativeEvent.isComposing ||
      e.keyCode === 229
    ) {
      return;
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const handleNewChat = () => {
    setActiveSessionId(null);
    setMessages([]);
    agent.reset();
  };

  const handleSelectSession = async (sessionId: string) => {
    setActiveSessionId(sessionId);
    await loadSessionMessages(sessionId);
  };

  const handleDeleteSession = async (e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation();
    try {
      await invoke('delete_session', { sessionId });
      if (activeSessionId === sessionId) {
        setActiveSessionId(null);
        setMessages([]);
      }
      await refreshSessions();
    } catch (err) {
      console.error('Failed to delete session:', err);
    }
  };

  const handleAddCustomModel = () => {
    if (customModel.trim() && !availableModels.includes(customModel.trim())) {
      setAvailableModels([...availableModels, customModel.trim()]);
      setModel(customModel.trim());
      setCustomModel('');
      setShowCustomModel(false);
    }
  };

  // Format timestamp to relative date
  const formatDate = (ts: number): string => {
    const now = Date.now();
    const diff = now - ts;
    if (diff < 86400000) return 'Today';
    if (diff < 172800000) return 'Yesterday';
    const d = new Date(ts);
    return `${d.getMonth() + 1}/${d.getDate()}`;
  };

  // ==========================================
  // Shared: model selector + send/stop buttons
  // ==========================================

  const renderInputActions = () => (
    <div className="right-actions">
      {showCustomModel ? (
        <div className="custom-model-input">
          <input
            type="text"
            value={customModel}
            onChange={(e) => setCustomModel(e.target.value)}
            placeholder="model-name"
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                handleAddCustomModel();
              }
              if (e.key === 'Escape') {
                setShowCustomModel(false);
                setCustomModel('');
              }
            }}
          />
          <button onClick={handleAddCustomModel} className="add-model-btn">
            <LucideIcon name="check" size={16} color="var(--accent-primary)" />
          </button>
          <button onClick={() => { setShowCustomModel(false); setCustomModel(''); }} className="cancel-model-btn">
            <LucideIcon name="x" size={16} color="var(--text-tertiary)" />
          </button>
        </div>
      ) : (
        <>
          <select
            className="model-select"
            value={model}
            onChange={(e) => setModel(e.target.value)}
          >
            {availableModels.length > 0 ? (
              availableModels.map((modelId) => (
                <option key={modelId} value={modelId}>{modelId}</option>
              ))
            ) : (
              <>
                <option value="gpt-4">GPT-4</option>
                <option value="gpt-4o">GPT-4o</option>
                <option value="gpt-3.5-turbo">GPT-3.5</option>
                <option value="claude-3-5-sonnet-20241022">Claude 3.5 Sonnet</option>
                <option value="claude-3-5-haiku-20241022">Claude 3.5 Haiku</option>
              </>
            )}
          </select>
          <button
            onClick={() => setShowCustomModel(true)}
            className="custom-model-btn"
            title="Add custom model"
          >
            <LucideIcon name="plus" size={16} color="var(--text-secondary)" />
          </button>
        </>
      )}
      {isAnyLoading ? (
        <button className="stop-btn" onClick={handleStop}>
          <LucideIcon name="square" size={16} color="#FFFFFF" />
        </button>
      ) : (
        <button
          className="send-btn"
          onClick={handleSendMessage}
          disabled={!inputValue.trim()}
        >
          <LucideIcon name="send" size={18} color="#FFFFFF" />
        </button>
      )}
    </div>
  );

  // ==========================================
  // Render
  // ==========================================

  return (
    <div className="app">
      <Settings isOpen={showSettings} onClose={() => setShowSettings(false)} />

      {/* Sidebar */}
      <aside className="sidebar" style={{ width: `${sidebarWidth}px` }}>
        <div className="sidebar-titlebar" data-tauri-drag-region />

        <div className="logo">
          <img src={logo} alt="Logo" className="logo-img" />
          <span className="logo-text">NexaCode</span>
        </div>

        <div className="sidebar-body">
          <div className="skills-menu">
            <button className="new-chat-btn" onClick={handleNewChat}>
              <LucideIcon name="plus" size={16} color="var(--text-secondary)" />
              <span>New Chat</span>
            </button>
          </div>

          <div className="chat-list-container">
            {sessions.map((session) => (
              <div
                key={session.id}
                className={`chat-item ${activeSessionId === session.id ? 'active' : ''}`}
                onClick={() => handleSelectSession(session.id)}
              >
                <span className="chat-item-title">{session.title}</span>
                <span className="chat-item-date">{formatDate(session.updated_at)}</span>
                <button
                  className="chat-item-delete"
                  onClick={(e) => handleDeleteSession(e, session.id)}
                  title="Delete"
                >
                  <LucideIcon name="x" size={12} color="var(--text-tertiary)" />
                </button>
              </div>
            ))}
          </div>

          <div className="spacer" />

          <button className="settings-btn" onClick={() => setShowSettings(true)}>
            <LucideIcon name="cog" size={16} color="var(--text-secondary)" />
            <span>Settings</span>
          </button>
        </div>
      </aside>

      {/* Resize handle */}
      <div
        className="resize-handle"
        style={{ left: `${sidebarWidth}px` }}
        onMouseDown={handleMouseDown}
      />

      {/* Main Content */}
      <main className="main-content">
        <div className="content-titlebar" data-tauri-drag-region />

        <div className="content-body">
          {messages.length === 0 && !agent.isRunning && agent.steps.length === 0 ? (
            <div className="welcome-area">
              <div className="welcome-icon">
                <LucideIcon name="sparkles" size={36} color="var(--accent-primary)" />
              </div>
              <h1 className="welcome-title">Welcome to NexaCode</h1>
              <p className="welcome-subtitle">How can I help you today?</p>

              {availableModels.length === 0 && (
                <div className="setup-notice">
                  <p>No LLM provider configured.</p>
                  <button onClick={() => setShowSettings(true)}>Configure Provider</button>
                </div>
              )}

              <div className="input-area">
                <div className="input-container">
                  <textarea
                    className="input-field"
                    placeholder="Message NexaCode..."
                    rows={1}
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onInput={handleTextareaInput}
                    onKeyDown={handleKeyDown}
                    onCompositionStart={handleCompositionStart}
                    onCompositionEnd={handleCompositionEnd}
                    disabled={isAnyLoading}
                  />
                  <div className="input-actions">
                    <button className="attachment-btn">
                      <LucideIcon name="plus" size={20} color="var(--text-tertiary)" />
                    </button>
                    {renderInputActions()}
                  </div>
                </div>
                <div className="options-container">
                  <div className="options-left">
                    <select
                      className="option-select"
                      value={chatMode}
                      onChange={(e) => setChatMode(e.target.value as ChatMode)}
                    >
                      <option value="build">Build (Agent)</option>
                      <option value="chat">Chat</option>
                    </select>
                    <button className="folder-btn">
                      <LucideIcon name="folder" size={16} color="var(--text-secondary)" />
                      <span>Select Folder</span>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="chat-area">
              <div className="messages-container" onScroll={handleScroll}>
                {/* Render all messages in order — steps are embedded in assistant messages */}
                {messages.map((msg, idx) => (
                  <div key={idx} className={`message ${msg.role} ${msg.steps ? 'agent' : ''}`}>
                    {msg.role === 'user' ? (
                      <div className="message-row user-row">
                        <div className="message-body user-body">
                          <div className="message-content">
                            {msg.content}
                          </div>
                        </div>
                        <div className="message-avatar user-avatar">
                          <span>U</span>
                        </div>
                      </div>
                    ) : (
                      <div className="message-row assistant-row">
                        <div className="message-avatar assistant-avatar">
                          <LucideIcon name="zap" size={16} color="#FFFFFF" />
                        </div>
                        <div className="message-body assistant-body">
                          {/* Agent steps (from saved data) */}
                          {msg.steps && msg.steps.length > 0 && (
                            <div className="agent-steps-container">
                              {msg.steps.map((step, sIdx) => (
                                <AgentStepView
                                  key={step.id}
                                  step={step}
                                  stepIndex={sIdx}
                                  isAgentRunning={false}
                                />
                              ))}
                            </div>
                          )}
                          {/* Final text content */}
                          <MarkdownRenderer content={msg.content} />
                        </div>
                      </div>
                    )}
                  </div>
                ))}

                {/* Live agent steps (while agent is running) */}
                {agent.isRunning && chatMode === 'build' && (
                  <div className="message assistant agent">
                    <div className="message-row assistant-row">
                      <div className="message-avatar assistant-avatar">
                        <LucideIcon name="zap" size={16} color="#FFFFFF" />
                      </div>
                      <div className="message-body assistant-body">
                        <div className="agent-steps-container">
                          {agent.steps
                            .filter((step, idx) => !(idx === agent.steps.length - 1 && step.status === 'thinking'))
                            .map((step, idx) => (
                              <AgentStepView
                                key={step.id}
                                step={step}
                                stepIndex={idx}
                                isAgentRunning={agent.isRunning}
                              />
                            ))}
                        </div>

                        {(() => {
                          const lastStep = agent.steps[agent.steps.length - 1];
                          if (lastStep && lastStep.status === 'thinking' && lastStep.thinking) {
                            return (
                              <div className="agent-live-content" style={{ marginTop: '12px' }}>
                                <MarkdownRenderer content={lastStep.thinking} />
                              </div>
                            );
                          }
                          return null;
                        })()}

                        <div className="agent-running-indicator">
                          <div className="agent-running-dots">
                            <span></span>
                            <span></span>
                            <span></span>
                          </div>
                          <span>Agent is working...</span>
                        </div>
                      </div>
                    </div>
                  </div>
                )}

                {/* Streaming content (chat mode) */}
                {streamingContent && chatMode === 'chat' && (
                  <div className={`message assistant ${isLoading ? 'streaming' : ''}`}>
                    <div className="message-row assistant-row">
                      <div className="message-avatar assistant-avatar">
                        <LucideIcon name="zap" size={16} color="#FFFFFF" />
                      </div>
                      <div className="message-body assistant-body">
                        <MarkdownRenderer content={streamingContent} />
                        {isLoading && <span className="streaming-cursor" />}
                      </div>
                    </div>
                  </div>
                )}

                {/* Thinking indicator (chat mode) */}
                {isLoading && !streamingContent && chatMode === 'chat' && (
                  <div className="message assistant loading">
                    <div className="message-row assistant-row">
                      <div className="message-avatar assistant-avatar">
                        <LucideIcon name="zap" size={16} color="#FFFFFF" />
                      </div>
                      <div className="message-body assistant-body">
                        <div className="thinking-indicator">
                          <div className="thinking-dots">
                            <span></span>
                            <span></span>
                            <span></span>
                          </div>
                          <span className="thinking-text">Thinking...</span>
                        </div>
                      </div>
                    </div>
                  </div>
                )}

                <div ref={messagesEndRef} />
              </div>

              {error && (
                <div className="error-banner">
                  <span>{error}</span>
                  <button onClick={clearError}>✕</button>
                </div>
              )}

              <div className="input-area">
                <div className="input-container">
                  <textarea
                    className="input-field"
                    placeholder="Message NexaCode..."
                    rows={1}
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    onInput={handleTextareaInput}
                    onKeyDown={handleKeyDown}
                    onCompositionStart={handleCompositionStart}
                    onCompositionEnd={handleCompositionEnd}
                    disabled={isAnyLoading}
                  />
                  <div className="input-actions">
                    <button className="attachment-btn">
                      <LucideIcon name="plus" size={20} color="var(--text-tertiary)" />
                    </button>
                    {renderInputActions()}
                  </div>
                </div>
                <div className="options-container">
                  <div className="options-left">
                    <select
                      className="option-select"
                      value={chatMode}
                      onChange={(e) => setChatMode(e.target.value as ChatMode)}
                    >
                      <option value="build">Build (Agent)</option>
                      <option value="chat">Chat</option>
                    </select>
                    <button className="folder-btn">
                      <LucideIcon name="folder" size={16} color="var(--text-secondary)" />
                      <span>Select Folder</span>
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}

export default App;
