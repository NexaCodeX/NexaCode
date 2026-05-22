import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export interface ChatResponse {
  content: string;
  model: string;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

export interface ModelInfo {
  id: string;
  name?: string;
  description?: string;
}

export interface StreamChunk {
  delta: string;
  finish_reason?: string;
}

export class LLMService {
  static async addProvider(
    name: string,
    providerType: 'openai' | 'openai_compatible' | 'anthropic',
    apiKey: string,
    models: string[],
    baseUrl?: string
  ): Promise<void> {
    await invoke('add_provider', {
      name,
      providerType,
      apiKey,
      models,
      baseUrl,
    });
  }

  static async removeProvider(name: string): Promise<void> {
    await invoke('remove_provider', { name });
  }

  static async setActiveProvider(name: string): Promise<void> {
    await invoke('set_active_provider', { name });
  }

  static async listProviders(): Promise<string[]> {
    return await invoke('list_providers');
  }

  static async getActiveProvider(): Promise<string | null> {
    return await invoke('get_active_provider');
  }

  static async chat(
    messages: ChatMessage[],
    model: string,
    options?: {
      temperature?: number;
      maxTokens?: number;
    }
  ): Promise<ChatResponse> {
    return await invoke('chat', {
      messages,
      model,
      temperature: options?.temperature,
      maxTokens: options?.maxTokens,
    });
  }

  static async chatStream(
    messages: ChatMessage[],
    model: string,
    onChunk: (chunk: StreamChunk) => void,
    onError: (error: string) => void,
    onEnd: () => void,
    options?: {
      temperature?: number;
      maxTokens?: number;
    }
  ): Promise<void> {
    console.log('=== LLMService.chatStream called ===');
    console.log('messages:', messages);
    console.log('model:', model);
    console.log('options:', options);
    
    const unlistenChunk = await listen<StreamChunk>('chat-chunk', (event) => {
      console.log('Event chat-chunk:', event.payload);
      onChunk(event.payload);
    });

    const unlistenError = await listen<string>('chat-error', (event) => {
      console.error('Event chat-error:', event.payload);
      onError(event.payload);
    });

    const unlistenEnd = await listen<void>('chat-end', () => {
      console.log('Event chat-end received');
      onEnd();
      unlistenChunk();
      unlistenError();
      unlistenEnd();
    });

    console.log('Invoking chat_stream command...');
    try {
      await invoke('chat_stream', {
        messages,
        model,
        temperature: options?.temperature,
        maxTokens: options?.maxTokens,
      });
      console.log('chat_stream command invoked successfully');
    } catch (err) {
      console.error('chat_stream invoke error:', err);
      onError(String(err));
    }
  }

  static async listModels(): Promise<ModelInfo[]> {
    return await invoke('list_models');
  }

  static async getProviderConfig(name: string): Promise<ProviderConfigResponse> {
    return await invoke('get_provider_config', { name });
  }

  static async updateProvider(
    name: string,
    providerType: 'openai' | 'openai_compatible' | 'anthropic',
    apiKey: string,
    models: string[],
    baseUrl?: string
  ): Promise<void> {
    await invoke('update_provider', {
      name,
      providerType,
      apiKey,
      models,
      baseUrl,
    });
  }
}

export interface ProviderConfigResponse {
  provider_type: string;
  api_key: string;
  base_url?: string;
  models: string[];
}

export async function setupDefaultProviders() {
  try {
    const providers = await LLMService.listProviders();
    
    if (providers.length === 0) {
      console.log('No providers configured. Please add a provider using LLMService.addProvider()');
    }
  } catch (error) {
    console.error('Failed to check providers:', error);
  }
}
