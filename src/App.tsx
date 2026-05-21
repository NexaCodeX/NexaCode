import React, { useState, useEffect, useRef } from 'react';
import './styles/main.scss';
import logo from './assets/logo.png';
import { LucideIcon } from './components/LucideIcon';
import { Settings } from './components/Settings';
import { useLLM } from './hooks/useLLM';
import type { ChatMessage } from './services/llm';

interface ChatItem {
  id: string;
  title: string;
  date: string;
}

function App() {
  const [chats, setChats] = useState<ChatItem[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [model, setModel] = useState('');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [customModel, setCustomModel] = useState('');
  const [showCustomModel, setShowCustomModel] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const {
    isLoading,
    error,
    streamingContent,
    chatStream,
    listProviders,
    getActiveProvider,
    getProviderConfig,
    clearError,
  } = useLLM();

  useEffect(() => {
    loadProviders();
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamingContent]);

  useEffect(() => {
    if (streamingContent && !isLoading) {
      setMessages((prev) => {
        const lastMessage = prev[prev.length - 1];
        if (lastMessage?.role === 'assistant') {
          return [...prev.slice(0, -1), { ...lastMessage, content: streamingContent }];
        }
        return [...prev, { role: 'assistant', content: streamingContent }];
      });
    }
  }, [streamingContent, isLoading]);

  const loadProviders = async () => {
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
  };

  const handleTextareaInput = (e: React.FormEvent<HTMLTextAreaElement>) => {
    const textarea = e.currentTarget;
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
  };

  const handleSendMessage = async () => {
    if (!inputValue.trim() || isLoading) return;

    const userMessage: ChatMessage = { role: 'user', content: inputValue.trim() };
    const newMessages = [...messages, userMessage];
    setMessages(newMessages);
    setInputValue('');

    if (!activeChatId) {
      const newChatId = Date.now().toString();
      const newChat: ChatItem = {
        id: newChatId,
        title: inputValue.trim().slice(0, 30) + (inputValue.trim().length > 30 ? '...' : ''),
        date: 'Today',
      };
      setChats((prev) => [newChat, ...prev]);
      setActiveChatId(newChatId);
    }

    await chatStream(newMessages, model);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSendMessage();
    }
  };

  const handleNewChat = () => {
    setActiveChatId(null);
    setMessages([]);
  };

  const handleAddCustomModel = () => {
    if (customModel.trim() && !availableModels.includes(customModel.trim())) {
      setAvailableModels([...availableModels, customModel.trim()]);
      setModel(customModel.trim());
      setCustomModel('');
      setShowCustomModel(false);
    }
  };

  return (
    <div className="app">
      <Settings isOpen={showSettings} onClose={() => setShowSettings(false)} />
      
      {/* Sidebar */}
      <aside className="sidebar">
        {/* Drag region - aligns with macOS traffic lights */}
        <div className="sidebar-titlebar" data-tauri-drag-region />

        {/* Logo */}
        <div className="logo">
          <img src={logo} alt="Logo" className="logo-img" />
          <span className="logo-text">NexaCode</span>
        </div>

        {/* Sidebar body */}
        <div className="sidebar-body">
          <div className="skills-menu">
            <button className="new-chat-btn" onClick={handleNewChat}>
              <LucideIcon name="plus" size={16} color="var(--text-secondary)" />
              <span>New Chat</span>
            </button>
          </div>

          <div className="chat-list-container">
            {chats.map((chat) => (
              <div
                key={chat.id}
                className={`chat-item ${activeChatId === chat.id ? 'active' : ''}`}
                onClick={() => setActiveChatId(chat.id)}
              >
                <span className="chat-item-title">{chat.title}</span>
                <span className="chat-item-date">{chat.date}</span>
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

      {/* Main Content */}
      <main className="main-content">
        {/* Drag region for content area */}
        <div className="content-titlebar" data-tauri-drag-region />

        <div className="content-body">
          {messages.length === 0 ? (
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
                    disabled={isLoading || availableModels.length === 0}
                  />
                  <div className="input-actions">
                    <button className="attachment-btn">
                      <LucideIcon name="plus" size={20} color="var(--text-tertiary)" />
                    </button>
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
                      <button 
                        className="send-btn"
                        onClick={handleSendMessage}
                        disabled={isLoading || !inputValue.trim() || availableModels.length === 0}
                      >
                        <LucideIcon name="send" size={18} color="#FFFFFF" />
                      </button>
                    </div>
                  </div>
                </div>
                <div className="options-container">
                  <div className="options-left">
                    <select className="option-select">
                      <option value="build">Build</option>
                      <option value="plan">Plan</option>
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
              <div className="messages-container">
                {messages.map((msg, idx) => (
                  <div key={idx} className={`message ${msg.role}`}>
                    <div className="message-role">{msg.role === 'user' ? 'You' : 'Assistant'}</div>
                    <div className="message-content">{msg.content}</div>
                  </div>
                ))}
                {isLoading && streamingContent && (
                  <div className="message assistant">
                    <div className="message-role">Assistant</div>
                    <div className="message-content">{streamingContent}</div>
                  </div>
                )}
                {isLoading && !streamingContent && (
                  <div className="message assistant loading">
                    <div className="message-role">Assistant</div>
                    <div className="message-content">Thinking...</div>
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
                    disabled={isLoading}
                  />
                  <div className="input-actions">
                    <button className="attachment-btn">
                      <LucideIcon name="plus" size={20} color="var(--text-tertiary)" />
                    </button>
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
                      <button 
                        className="send-btn"
                        onClick={handleSendMessage}
                        disabled={isLoading || !inputValue.trim()}
                      >
                        <LucideIcon name="send" size={18} color="#FFFFFF" />
                      </button>
                    </div>
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
