import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './styles/main.scss';
import logo from './assets/logo.png';
import { LucideIcon } from './components/LucideIcon';
import { Settings } from './components/Settings';
import { MarkdownRenderer } from './components/MarkdownRenderer';
import { useLLM } from './hooks/useLLM';
import type { ChatMessage } from './services/llm';

interface Message {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

interface ChatItem {
  id: string;
  title: string;
  date: string;
  messages: Message[];
}

function App() {
  const [chats, setChats] = useState<ChatItem[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [model, setModel] = useState('');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [customModel, setCustomModel] = useState('');
  const [showCustomModel, setShowCustomModel] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [isResizing, setIsResizing] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // 从后端加载聊天记录
  useEffect(() => {
    const loadChats = async () => {
      try {
        const loadedChats = await invoke<ChatItem[]>('load_chats');
        setChats(loadedChats);
      } catch (e) {
        console.error('Failed to load chats from disk:', e);
      }
    };
    loadChats();
  }, []);

  // 当 chats 改变时保存到后端
  useEffect(() => {
    const saveChats = async () => {
      try {
        await invoke('save_chats', { chats });
      } catch (e) {
        console.error('Failed to save chats to disk:', e);
      }
    };
    saveChats();
  }, [chats]);
  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return;
      // 限制最小宽度 200px，最大宽度 400px
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

  // 页面加载后，如果有激活的对话 ID，加载对应消息
  useEffect(() => {
    if (activeChatId && chats.length > 0) {
      const activeChat = chats.find(c => c.id === activeChatId);
      if (activeChat) {
        setMessages(activeChat.messages);
      }
    }
  }, [activeChatId, chats]);

  // 自动保存对话列表到 localStorage
  useEffect(() => {
    localStorage.setItem('nexacode-chats', JSON.stringify(chats));
  }, [chats]);

  // 自动保存当前激活的对话 ID 到 localStorage
  useEffect(() => {
    localStorage.setItem('nexacode-active-chat', JSON.stringify(activeChatId));
  }, [activeChatId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, streamingContent]);
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
        
        if (activeChatId) {
          setChats((prevChats) =>
            prevChats.map((chat) =>
              chat.id === activeChatId ? { ...chat, messages: newMessages } : chat
            )
          );
        }
        
        return newMessages;
      });
    }
  }, [streamingContent, isLoading, activeChatId]);

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
    console.log('=== handleSendMessage called ===');
    console.log('inputValue:', inputValue);
    console.log('isLoading:', isLoading);
    console.log('model:', model);
    console.log('messages count:', messages.length);
    
    if (!inputValue.trim() || isLoading) {
      console.log('Early return: no input or already loading');
      return;
    }

    const userMessage: Message = { role: 'user', content: inputValue.trim() };
    const newMessages = [...messages, userMessage];
    console.log('New messages:', newMessages);
    
    setMessages(newMessages);
    setInputValue('');

    if (!activeChatId) {
      const newChatId = Date.now().toString();
      const newChat: ChatItem = {
        id: newChatId,
        title: inputValue.trim().slice(0, 30) + (inputValue.trim().length > 30 ? '...' : ''),
        date: 'Today',
        messages: [userMessage],
      };
      console.log('Creating new chat:', newChat);
      setChats((prev) => [newChat, ...prev]);
      setActiveChatId(newChatId);
    } else {
      console.log('Updating existing chat:', activeChatId);
      setChats((prev) =>
        prev.map((chat) =>
          chat.id === activeChatId ? { ...chat, messages: newMessages } : chat
        )
      );
    }

    const apiMessages: ChatMessage[] = newMessages.map(m => ({
      role: m.role,
      content: m.content,
    }));

    console.log('Calling chatStream with model:', model);
    try {
      await chatStream(apiMessages, model);
      console.log('chatStream completed');
    } catch (err) {
      console.error('chatStream error:', err);
    }
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

  const handleSelectChat = (chatId: string) => {
    const chat = chats.find((c) => c.id === chatId);
    if (chat) {
      setActiveChatId(chatId);
      setMessages(chat.messages || []);
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

  return (
    <div className="app">
      <Settings isOpen={showSettings} onClose={() => setShowSettings(false)} />
      
      {/* Sidebar */}
      <aside className="sidebar" style={{ width: `${sidebarWidth}px` }}>
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
                onClick={() => handleSelectChat(chat.id)}
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
                          <MarkdownRenderer content={msg.content} />
                        </div>
                      </div>
                    )}
                  </div>
                ))}
                {isLoading && streamingContent && (
                  <div className="message assistant streaming">
                    <div className="message-row assistant-row">
                      <div className="message-avatar assistant-avatar">
                        <LucideIcon name="zap" size={16} color="#FFFFFF" />
                      </div>
                      <div className="message-body assistant-body">
                        <MarkdownRenderer content={streamingContent} />
                        <span className="streaming-cursor" />
                      </div>
                    </div>
                  </div>
                )}
                {isLoading && !streamingContent && (
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
