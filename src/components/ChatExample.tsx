/* eslint-disable react-hooks/set-state-in-effect */
import { useState, useEffect, useCallback } from 'react';
import type { ChatMessage } from '../services/llm';
import { useLLM } from '../hooks/useLLM';

export function ChatExample() {
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [model, setModel] = useState('gpt-4');
  const [providers, setProviders] = useState<string[]>([]);
  
  const {
    isLoading,
    error,
    streamingContent,
    chat,
    chatStream,
    addProvider,
    setActiveProvider,
    listProviders,
    clearError,
  } = useLLM();

  const loadProviders = useCallback(async () => {
    const providerList = await listProviders();
    setProviders(providerList);
  }, [listProviders]);

  useEffect(() => {
    loadProviders();
  }, [loadProviders]);

  const handleSetupOpenAI = async () => {
    const apiKey = prompt('Enter your OpenAI API key:');
    if (!apiKey) return;

    const success = await addProvider('openai', 'openai', apiKey, ['gpt-4', 'gpt-4o', 'gpt-3.5-turbo']);
    if (success) {
      await loadProviders();
      alert('OpenAI provider added successfully!');
    }
  };

  const handleSetupClaude = async () => {
    const apiKey = prompt('Enter your Anthropic API key:');
    if (!apiKey) return;

    const success = await addProvider('claude', 'anthropic', apiKey, ['claude-3-5-sonnet-20241022', 'claude-3-5-haiku-20241022']);
    if (success) {
      await loadProviders();
      alert('Claude provider added successfully!');
    }
  };

  const handleSendMessage = async (useStream: boolean) => {
    if (!input.trim()) return;

    const userMessage: ChatMessage = { role: 'user', content: input };
    const newMessages = [...messages, userMessage];
    setMessages(newMessages);
    setInput('');

    if (useStream) {
      await chatStream(newMessages, model);
    } else {
      const response = await chat(newMessages, model);
      if (response) {
        setMessages([...newMessages, { role: 'assistant', content: response.content }]);
      }
    }
  };

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

  return (
    <div className="chat-example">
      <div className="provider-setup">
        <h3>Setup Providers</h3>
        <button onClick={handleSetupOpenAI}>Add OpenAI</button>
        <button onClick={handleSetupClaude}>Add Claude</button>
        
        <div className="providers-list">
          <h4>Available Providers:</h4>
          {providers.map((provider) => (
            <div key={provider}>
              <span>{provider}</span>
              <button onClick={() => setActiveProvider(provider)}>Set Active</button>
            </div>
          ))}
        </div>
      </div>

      <div className="model-select">
        <select value={model} onChange={(e) => setModel(e.target.value)}>
          <option value="gpt-4">GPT-4</option>
          <option value="gpt-3.5-turbo">GPT-3.5 Turbo</option>
          <option value="claude-3-5-sonnet-20241022">Claude 3.5 Sonnet</option>
          <option value="claude-3-5-haiku-20241022">Claude 3.5 Haiku</option>
        </select>
      </div>

      <div className="messages">
        {messages.map((msg, idx) => (
          <div key={idx} className={`message ${msg.role}`}>
            <strong>{msg.role}:</strong> {msg.content}
          </div>
        ))}
        {isLoading && streamingContent && (
          <div className="message assistant">
            <strong>assistant:</strong> {streamingContent}
          </div>
        )}
      </div>

      {error && (
        <div className="error">
          {error}
          <button onClick={clearError}>Dismiss</button>
        </div>
      )}

      <div className="input-area">
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Type your message..."
          rows={3}
        />
        <button onClick={() => handleSendMessage(false)} disabled={isLoading}>
          Send
        </button>
        <button onClick={() => handleSendMessage(true)} disabled={isLoading}>
          Send (Stream)
        </button>
      </div>
    </div>
  );
}
