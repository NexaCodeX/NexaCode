import { useState } from 'react';
import { LucideIcon } from './LucideIcon';
import { ToolCallView } from './ToolCallView';
import { DiffPreview } from './DiffPreview';
import { MarkdownRenderer } from './MarkdownRenderer';
import type { AgentStep } from '../hooks/useAgent';

/** Check if a tool call involves file editing (should show diff preview) */
function isEditTool(name: string): boolean {
  return name === 'Edit' || name === 'MultiEdit' || name === 'Write';
}



interface AgentStepViewProps {
  step: AgentStep;
  stepIndex: number;
  isAgentRunning: boolean;
  sessionId?: string;
}

export function AgentStepView({ step, stepIndex, sessionId }: AgentStepViewProps) {
  const [thinkingExpanded, setThinkingExpanded] = useState(true);
  const isRunning = step.status === 'thinking' || step.status === 'calling_tool' || step.status === 'tool_running';

  return (
    <div className={`agent-step ${isRunning ? 'running' : ''} ${step.toolResult?.is_error ? 'error' : ''}`}>
      {/* Step number indicator */}
      <div className="agent-step-indicator">
        <span className="agent-step-number">{stepIndex + 1}</span>
      </div>

      {/* Step content */}
      <div className="agent-step-content">
        {/* Thinking block */}
        {step.thinking && (
          <div className={`agent-thinking ${thinkingExpanded ? 'open' : ''}`}>
            <button
              className="agent-thinking-header"
              onClick={() => setThinkingExpanded(!thinkingExpanded)}
            >
              <LucideIcon name="brain" size={14} color="var(--text-tertiary)" />
              <span>Thinking</span>
              <LucideIcon
                name={thinkingExpanded ? 'chevron-down' : 'chevron-right'}
                size={14}
                color="var(--text-tertiary)"
              />
            </button>
            {thinkingExpanded && (
              <div className="agent-thinking-body">
                <MarkdownRenderer content={step.thinking} disableThinkingWrapper={true} />
              </div>
            )}
          </div>
        )}

        {/* Tool call view */}
        {step.toolCall && (
          <>
            {isEditTool(step.toolCall.name) ? (
              <DiffPreview
                filePath={step.toolCall.arguments.path ? String(step.toolCall.arguments.path) : 'unknown'}
                arguments={step.toolCall.arguments}
                result={step.toolResult ? { output: step.toolResult.output, is_error: step.toolResult.is_error } : undefined}
                showActions={false}
                sessionId={sessionId}
              />
            ) : (
              <ToolCallView
                name={step.toolCall.name}
                arguments={step.toolCall.arguments}
                result={step.toolResult ? { output: step.toolResult.output, is_error: step.toolResult.is_error } : undefined}
                isRunning={isRunning}
              />
            )}
          </>
        )}

        {/* Tool result only (no tool call associated) */}
        {!step.toolCall && step.toolResult && (
          <div className={`agent-tool-result-only ${step.toolResult.is_error ? 'error' : ''}`}>
            <pre>{step.toolResult.output}</pre>
          </div>
        )}
      </div>
    </div>
  );
}
