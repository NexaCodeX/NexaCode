import { useState, useCallback, useRef } from 'react';
import type { ChatMessage, ChatResponse, StreamChunk, ProviderConfigResponse } from '../services/llm';
import { LLMService } from '../services/llm';

export function useLLM() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [streamingContent, setStreamingContent] = useState('');

  // Internal buffer for batching stream updates
  const bufferRef = useRef('');
  const rafIdRef = useRef<number | null>(null);

  // Flush buffer to state via requestAnimationFrame
  const flushBuffer = useCallback(() => {
    const content = bufferRef.current;
    setStreamingContent(content);
    rafIdRef.current = null;
  }, []);

  // Append chunk to buffer and schedule a flush
  const appendToBuffer = useCallback((delta: string) => {
    bufferRef.current += delta;
    if (rafIdRef.current === null) {
      rafIdRef.current = requestAnimationFrame(flushBuffer);
    }
  }, [flushBuffer]);

  const chat = useCallback(
    async (
      messages: ChatMessage[],
      model: string,
      options?: {
        temperature?: number;
        maxTokens?: number;
      }
    ): Promise<ChatResponse | null> => {
      setIsLoading(true);
      setError(null);

      try {
        const response = await LLMService.chat(messages, model, options);
        setIsLoading(false);
        return response;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        setIsLoading(false);
        return null;
      }
    },
    []
  );

  const chatStream = useCallback(
    async (
      messages: ChatMessage[],
      model: string,
      options?: {
        temperature?: number;
        maxTokens?: number;
      }
    ): Promise<void> => {
      setIsLoading(true);
      setError(null);
      setStreamingContent('');
      bufferRef.current = '';

      return new Promise((resolve) => {
        LLMService.chatStream(
          messages,
          model,
          (chunk: StreamChunk) => {
            appendToBuffer(chunk.delta);
          },
          (err: string) => {
            // Flush any remaining buffer before handling error
            if (rafIdRef.current !== null) {
              cancelAnimationFrame(rafIdRef.current);
              rafIdRef.current = null;
            }
            setStreamingContent(bufferRef.current);
            setError(err);
            setIsLoading(false);
            resolve();
          },
          () => {
            // Flush any remaining buffer
            if (rafIdRef.current !== null) {
              cancelAnimationFrame(rafIdRef.current);
              rafIdRef.current = null;
            }
            setStreamingContent(bufferRef.current);
            setIsLoading(false);
            resolve();
          },
          options
        );
      });
    },
    [appendToBuffer]
  );

  const addProvider = useCallback(
    async (
      name: string,
      providerType: 'openai' | 'openai_compatible' | 'anthropic',
      apiKey: string,
      models: string[],
      baseUrl?: string
    ): Promise<boolean> => {
      try {
        await LLMService.addProvider(name, providerType, apiKey, models, baseUrl);
        return true;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        return false;
      }
    },
    []
  );

  const setActiveProvider = useCallback(async (name: string): Promise<boolean> => {
    try {
      await LLMService.setActiveProvider(name);
      return true;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      return false;
    }
  }, []);

  const listProviders = useCallback(async (): Promise<string[]> => {
    try {
      return await LLMService.listProviders();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      return [];
    }
  }, []);

  const getActiveProvider = useCallback(async (): Promise<string | null> => {
    try {
      return await LLMService.getActiveProvider();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      return null;
    }
  }, []);

  const removeProvider = useCallback(async (name: string): Promise<void> => {
    try {
      await LLMService.removeProvider(name);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
    }
  }, []);

  const getProviderConfig = useCallback(async (name: string): Promise<ProviderConfigResponse | null> => {
    try {
      return await LLMService.getProviderConfig(name);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      return null;
    }
  }, []);

  const updateProvider = useCallback(
    async (
      name: string,
      providerType: 'openai' | 'openai_compatible' | 'anthropic',
      apiKey: string,
      models: string[],
      baseUrl?: string
    ): Promise<boolean> => {
      try {
        await LLMService.updateProvider(name, providerType, apiKey, models, baseUrl);
        return true;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        return false;
      }
    },
    []
  );

  const listModels = useCallback(async () => {
    try {
      return await LLMService.listModels();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      return [];
    }
  }, []);

  return {
    isLoading,
    error,
    streamingContent,
    chat,
    chatStream,
    addProvider,
    setActiveProvider,
    listProviders,
    getActiveProvider,
    removeProvider,
    getProviderConfig,
    updateProvider,
    listModels,
    clearError: () => setError(null),
  };
}
