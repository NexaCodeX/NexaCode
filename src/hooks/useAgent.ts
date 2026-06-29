import { useState, useCallback, useRef } from 'react';
import { AgentService } from '../services/llm';
import type { AgentEventInfo, AgentRunRequest } from '../services/llm';

/** A single step in the agent's execution trace */
export interface AgentStep {
  /** Unique ID for this step */
  id: string;
  /** Thinking text (if any) before or during this step */
  thinking?: string;
  /** Tool call details (if this step involves a tool) */
  toolCall?: {
    id: string;
    name: string;
    arguments: Record<string, unknown>;
    requires_confirmation: boolean;
  };
  /** Tool result (if this step has a completed tool call) */
  toolResult?: {
    tool_call_id: string;
    name: string;
    output: string;
    is_error: boolean;
  };
  /** Status of this step */
  status: 'thinking' | 'calling_tool' | 'tool_running' | 'tool_done' | 'done' | 'error';
}

/** Final response from the agent */
export interface AgentFinalResponse {
  content: string;
  isCompleted: boolean;
  isError: boolean;
  iterations: number;
}

export function useAgent() {
  const [isRunning, setIsRunning] = useState(false);
  const [steps, setSteps] = useState<AgentStep[]>([]);
  const [finalResponse, setFinalResponse] = useState<AgentFinalResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Track step IDs for linking tool results to tool calls
  const stepCounterRef = useRef(0);
  const toolCallToStepMap = useRef<Map<string, string>>(new Map());
  const resolveRef = useRef<(() => void) | null>(null);

  // ✅ Use ref to track running state — avoids stale closure in event callbacks
  const isRunningRef = useRef(false);
  const handleEventRef = useRef<((event: AgentEventInfo) => void) | null>(null);

  const nextStepId = useCallback(() => {
    stepCounterRef.current += 1;
    return `step-${stepCounterRef.current}`;
  }, []);

  /** Reset agent state for a new run */
  const reset = useCallback(() => {
    setSteps([]);
    setFinalResponse(null);
    setError(null);
    stepCounterRef.current = 0;
    toolCallToStepMap.current.clear();
  }, []);

  /** Handle a single agent event, updating the steps state */
  const handleEvent = useCallback(
    (event: AgentEventInfo) => {
      switch (event.type) {
        case 'thinking': {
          setSteps((prev) => {
            const lastStep = prev[prev.length - 1];
            if (lastStep && lastStep.status === 'thinking') {
              return [
                ...prev.slice(0, -1),
                {
                  ...lastStep,
                  thinking: (lastStep.thinking || '') + event.content,
                },
              ];
            }
            const stepId = nextStepId();
            return [
              ...prev,
              {
                id: stepId,
                thinking: event.content,
                status: 'thinking',
              },
            ];
          });
          break;
        }

        case 'tool_call': {
          const stepId = nextStepId();
          toolCallToStepMap.current.set(event.id, stepId);

          setSteps((prev) => {
            const lastStep = prev[prev.length - 1];
            if (lastStep && lastStep.status === 'thinking') {
              return [
                ...prev.slice(0, -1),
                {
                  ...lastStep,
                  id: stepId,
                  status: 'calling_tool',
                  toolCall: {
                    id: event.id,
                    name: event.name,
                    arguments: event.arguments,
                    requires_confirmation: event.requires_confirmation,
                  },
                },
              ];
            }
            return [
              ...prev,
              {
                id: stepId,
                status: 'tool_running',
                toolCall: {
                  id: event.id,
                  name: event.name,
                  arguments: event.arguments,
                  requires_confirmation: event.requires_confirmation,
                },
              },
            ];
          });
          break;
        }

        case 'tool_result': {
          setSteps((prev) => {
            const stepId = toolCallToStepMap.current.get(event.tool_call_id);
            if (stepId) {
              return prev.map((step) =>
                step.id === stepId
                  ? {
                      ...step,
                      status: 'tool_done' as const,
                      toolResult: {
                        tool_call_id: event.tool_call_id,
                        name: event.name,
                        output: event.output,
                        is_error: event.is_error,
                      },
                    }
                  : step,
              );
            }
            const newStepId = nextStepId();
            return [
              ...prev,
              {
                id: newStepId,
                status: 'tool_done' as const,
                toolResult: {
                  tool_call_id: event.tool_call_id,
                  name: event.name,
                  output: event.output,
                  is_error: event.is_error,
                },
              },
            ];
          });
          break;
        }

        case 'completed': {
          setSteps((prev) => {
            const lastStep = prev[prev.length - 1];
            if (lastStep && lastStep.status === 'thinking') {
              return prev.slice(0, -1);
            }
            return prev;
          });

          setFinalResponse({
            content: event.content,
            isCompleted: true,
            isError: false,
            iterations: steps.length,
          });
          break;
        }

        case 'max_iterations_reached': {
          setFinalResponse({
            content: `Agent reached maximum iterations (${event.iterations})`,
            isCompleted: false,
            isError: true,
            iterations: event.iterations,
          });
          break;
        }

        case 'error': {
          setError(event.message);
          setFinalResponse({
            content: event.message,
            isCompleted: false,
            isError: true,
            iterations: steps.length,
          });
          break;
        }
      }
    },
    [nextStepId, steps],
  );

  // Keep ref up to date with the latest handleEvent function
  handleEventRef.current = handleEvent;

  /** Run the agent loop */
  const run = useCallback(
    async (request: AgentRunRequest) => {
      reset();
      isRunningRef.current = true;
      setIsRunning(true);

      return new Promise<void>((resolve) => {
        resolveRef.current = resolve;
        AgentService.run(
          request,
          (event: AgentEventInfo) => {
            if (!isRunningRef.current) return;
            if (handleEventRef.current) {
              handleEventRef.current(event);
            }
          },
          () => {
            isRunningRef.current = false;
            setIsRunning(false);
            resolveRef.current = null;
            resolve();
          },
        );
      });
    },
    [reset],
  );

  /** Stop the agent */
  const stop = useCallback(async () => {
    isRunningRef.current = false;
    setIsRunning(false);

    // Map any running/thinking steps to a finished state so they don't show as loading in history
    setSteps((prev) => {
      const updated = prev.map((step) => {
        if (step.status === 'thinking') {
          return { ...step, status: 'done' as const };
        } else if (step.status === 'calling_tool' || step.status === 'tool_running') {
          return {
            ...step,
            status: 'error' as const,
            toolResult: step.toolResult || {
              tool_call_id: step.toolCall?.id || 'unknown',
              name: step.toolCall?.name || 'unknown',
              output: 'Cancelled by user',
              is_error: true,
            },
          };
        }
        return step;
      });

      setFinalResponse({
        content: 'Session terminated by user.',
        isCompleted: false,
        isError: false,
        iterations: updated.length,
      });

      return updated;
    });

    if (resolveRef.current) {
      resolveRef.current();
      resolveRef.current = null;
    }

    try {
      await AgentService.cancel();
    } catch (err) {
      console.error('Failed to cancel agent on backend:', err);
    }
  }, []);

  return {
    isRunning,
    steps,
    finalResponse,
    error,
    run,
    stop,
    reset,
  };
}
