import { useState, useCallback } from 'react';
import type { ChatMessage, ChatResponse, StreamChunk, ProviderConfigResponse } from '../services/llm';
import { LLMService } from '../services/llm';

export function useLLM() {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [streamingContent, setStreamingContent] = useState('');

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

      return new Promise((resolve) => {
        LLMService.chatStream(
          messages,
          model,
          (chunk: StreamChunk) => {
            setStreamingContent((prev) => prev + chunk.delta);
          },
          (err: string) => {
            setError(err);
            setIsLoading(false);
            resolve();
          },
          () => {
            setIsLoading(false);
            resolve();
          },
          options
        );
      });
    },
    []
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
