import { useState } from 'react';
import { LucideIcon } from './LucideIcon';
import { ToolCallView } from './ToolCallView';
import { DiffPreview } from './DiffPreview';
import { MarkdownRenderer, parseThinking } from './MarkdownRenderer';
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

  // The agent stream lumps real reasoning ([THINKING]...[/THINKING]) and the
  // model's user-facing narration into a single `thinking` field. Split them so
  // only the reasoning lives in the collapsible block and the narration renders
  // as normal visible prose.
  const { thinkingContent, mainContent } = step.thinking
    ? parseThinking(step.thinking)
    : { thinkingContent: '', mainContent: '' };

  return (
    <div className={`agent-step ${isRunning ? 'running' : ''} ${step.toolResult?.is_error ? 'error' : ''}`}>
      {/* Step number indicator */}
      <div className="agent-step-indicator">
        <span className="agent-step-number">{stepIndex + 1}</span>
      </div>

      {/* Step content */}
      <div className="agent-step-content">
        {/* Thinking block — only the actual reasoning ([THINKING] content) */}
        {thinkingContent && (
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
                <MarkdownRenderer content={thinkingContent} disableThinkingWrapper={true} />
              </div>
            )}
          </div>
        )}

        {/* Narration / user-facing prose the model emitted before this step */}
        {mainContent && (
          <div className="agent-step-text">
            <MarkdownRenderer content={mainContent} disableThinkingWrapper={true} />
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
