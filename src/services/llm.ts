import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type UnlistenFn = () => void;

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

// Active stream listeners — stored so we can unlisten on cancel
let activeListeners: UnlistenFn[] = [];

function clearActiveListeners() {
  for (const unlisten of activeListeners) {
    unlisten();
  }
  activeListeners = [];
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
      sessionId?: string;
      temperature?: number;
      maxTokens?: number;
    }
  ): Promise<void> {
    console.log('=== LLMService.chatStream called ===');
    console.log('messages:', messages);
    console.log('model:', model);
    console.log('options:', options);

    // Clear any leftover listeners from a previous stream
    clearActiveListeners();

    const unlistenChunk = await listen<StreamChunk>('chat-chunk', (event) => {
      onChunk(event.payload);
    });

    const unlistenError = await listen<string>('chat-error', (event) => {
      onError(event.payload);
      clearActiveListeners();
    });

    const unlistenEnd = await listen<void>('chat-end', () => {
      console.log('Event chat-end received');
      onEnd();
      clearActiveListeners();
    });

    // Store listeners so cancelStream can unlisten them
    activeListeners = [unlistenChunk, unlistenError, unlistenEnd];

    console.log('Invoking chat_stream command...');
    try {
      await invoke('chat_stream', {
        messages,
        model,
        sessionId: options?.sessionId,
        temperature: options?.temperature,
        maxTokens: options?.maxTokens,
      });
      console.log('chat_stream command invoked successfully');
    } catch (err) {
      console.error('chat_stream invoke error:', err);
      clearActiveListeners();
      onError(String(err));
    }
  }

  static async cancelStream(): Promise<boolean> {
    try {
      const result = await invoke<boolean>('chat_stream_cancel');
      // Immediately remove all listeners so late-arriving events are ignored
      clearActiveListeners();
      return result;
    } catch (err) {
      console.error('cancel_stream error:', err);
      clearActiveListeners();
      return false;
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

// ==========================================
// Agent Types & Service
// ==========================================

/** Agent event types matching the Rust backend AgentEventInfo */
export type AgentEventInfo =
  | { type: 'thinking'; content: string }
  | { type: 'tool_call'; id: string; name: string; arguments: Record<string, unknown>; requires_confirmation: boolean }
  | { type: 'tool_result'; tool_call_id: string; name: string; output: string; is_error: boolean }
  | { type: 'completed'; content: string }
  | { type: 'max_iterations_reached'; iterations: number }
  | { type: 'error'; message: string };

export interface AgentRunRequest {
  session_id?: string;
  message?: string;
  messages?: ChatMessage[];
  model: string;
  system_prompt?: string;
  max_iterations?: number;
  temperature?: number;
  max_tokens?: number;
}

// Active agent listeners
let activeAgentListeners: UnlistenFn[] = [];

function clearAgentListeners() {
  for (const unlisten of activeAgentListeners) {
    unlisten();
  }
  activeAgentListeners = [];
}

export class AgentService {
  /**
   * Run the agent loop, streaming events to the frontend.
   * The backend emits `agent-event` for each step and `agent-end` when done.
   */
  static async run(
    request: AgentRunRequest,
    onEvent: (event: AgentEventInfo) => void,
    onEnd: () => void,
  ): Promise<void> {
    console.log('[AgentService] run() called with request:', request);

    // Clear any leftover listeners
    clearAgentListeners();

    const unlistenEvent = await listen<AgentEventInfo>('agent-event', (e) => {
      console.log('[AgentService] Received agent-event:', e.payload.type, e.payload);
      onEvent(e.payload);
    });

    const unlistenEnd = await listen<void>('agent-end', () => {
      console.log('[AgentService] Received agent-end');
      clearAgentListeners();
      onEnd();
    });

    activeAgentListeners = [unlistenEvent, unlistenEnd];

    console.log('[AgentService] Invoking agent_run command...');
    try {
      await invoke('agent_run', { request });
      console.log('[AgentService] agent_run command invoked successfully (running in background)');
    } catch (err) {
      console.error('[AgentService] agent_run invoke error:', err);
      clearAgentListeners();
      onEvent({ type: 'error', message: String(err) });
      onEnd();
    }
  }

  /** Cancel any running agent execution and listeners */
  static async cancel(): Promise<void> {
    console.log('[AgentService] cancel() called');
    clearAgentListeners();
    try {
      await invoke('agent_cancel');
    } catch (err) {
      console.error('[AgentService] Failed to cancel agent on backend:', err);
    }
  }
}
