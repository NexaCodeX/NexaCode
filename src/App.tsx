import React, { useState } from 'react';
import './styles/main.scss';
import logo from './assets/logo.png';
import { LucideIcon } from './components/LucideIcon';

interface ChatItem {
  id: string;
  title: string;
  date: string;
}

interface SkillItem {
  id: string;
  icon: string;
  name: string;
}

interface SuggestionItem {
  id: string;
  icon: string;
  iconColor: string;
  title: string;
  description: string;
}

function App() {
  const [chats, setChats] = useState<ChatItem[]>([
    { id: '1', title: 'React component help', date: 'Today' },
    { id: '2', title: 'Python API design', date: 'Yesterday' },
    { id: '3', title: 'Database optimization', date: '3 days ago' },
  ]);

  const [activeChatId, setActiveChatId] = useState<string>('1');

  const handleTextareaInput = (e: React.FormEvent<HTMLTextAreaElement>) => {
    const textarea = e.currentTarget;
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
  };

  const skills: SkillItem[] = [];

  const suggestions: SuggestionItem[] = [
    {
      id: '1',
      icon: 'code-2',
      iconColor: 'var(--accent-primary)',
      title: 'Write code',
      description: 'Get help with programming problems',
    },
    {
      id: '2',
      icon: 'file-text',
      iconColor: 'var(--accent-coral)',
      title: 'Explain concepts',
      description: 'Learn about technical topics',
    },
    {
      id: '3',
      icon: 'lightbulb',
      iconColor: 'var(--accent-warning)',
      title: 'Brainstorm ideas',
      description: 'Generate creative solutions',
    },
  ];

  return (
    <div className="app">
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
            <button className="new-chat-btn">
              <LucideIcon name="plus" size={16} color="var(--text-secondary)" />
              <span>New Chat</span>
            </button>
            <div className="skills-divider" />
            <div className="skills-header">
              <LucideIcon name="zap" size={16} color="var(--text-tertiary)" />
              <span className="skills-header-text">Skills</span>
            </div>
            {skills.map((skill) => (
              <div key={skill.id} className="skill-item">
                <LucideIcon name={skill.icon} size={16} color="var(--text-secondary)" />
                <span className="skill-item-text">{skill.name}</span>
              </div>
            ))}
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

          <button className="settings-btn">
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
          <div className="welcome-area">
            <div className="welcome-icon">
              <LucideIcon name="sparkles" size={36} color="var(--accent-primary)" />
            </div>
            <h1 className="welcome-title">Welcome to NexaCode</h1>
            <p className="welcome-subtitle">How can I help you today?</p>

            {/* <div className="suggestions">
              {suggestions.map((suggestion) => (
                <div key={suggestion.id} className="suggestion-card">
                  <LucideIcon name={suggestion.icon} size={24} color={suggestion.iconColor} />
                  <span className="suggestion-title">{suggestion.title}</span>
                  <span className="suggestion-desc">{suggestion.description}</span>
                </div>
              ))}
            </div> */}

            <div className="input-area">
              <div className="input-container">
                <textarea
                  className="input-field"
                  placeholder="Message NexaCode..."
                  rows={1}
                  onInput={handleTextareaInput}
                />
                <div className="input-actions">
                  <button className="attachment-btn">
                    <LucideIcon name="plus" size={20} color="var(--text-tertiary)" />
                  </button>
                  <div className="right-actions">
                    <select className="model-select">
                      <option value="gpt-4">GPT-4</option>
                      <option value="gpt-3.5">GPT-3.5</option>
                      <option value="claude-3">Claude 3</option>
                      <option value="claude-2">Claude 2</option>
                    </select>
                    <button className="send-btn">
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
        </div>
      </main>
    </div>
  );
}

export default App;
