import { useState } from 'react';
import { LucideIcon } from './LucideIcon';

/** Map tool names to display icons and colors */
const TOOL_CONFIG: Record<string, { icon: string; color: string; label: string }> = {
  Read: { icon: 'file-text', color: '#3B82F6', label: 'Reading file' },
  Write: { icon: 'file-plus', color: '#10B981', label: 'Writing file' },
  Edit: { icon: 'file-edit', color: '#F59E0B', label: 'Editing file' },
  MultiEdit: { icon: 'file-edit', color: '#F59E0B', label: 'Multi-editing files' },
  LS: { icon: 'folder', color: '#8B5CF6', label: 'Listing directory' },
  Grep: { icon: 'search', color: '#6366F1', label: 'Searching content' },
  Glob: { icon: 'search', color: '#6366F1', label: 'Finding files' },
  Bash: { icon: 'terminal', color: '#10B981', label: 'Running command' },
  WebFetch: { icon: 'globe', color: '#3B82F6', label: 'Fetching URL' },
  Diagnostic: { icon: 'bug', color: '#EF4444', label: 'Reading diagnostics' },
  Task: { icon: 'layers', color: '#8B5CF6', label: 'Managing tasks' },
  Git: { icon: 'git-branch', color: '#F97316', label: 'Git operation' },
};

interface ToolCallViewProps {
  name: string;
  arguments: Record<string, unknown>;
  result?: {
    output: string;
    is_error: boolean;
  };
  isRunning?: boolean;
}

/** Get a short summary of the tool call for the header */
function getToolSummary(name: string, args: Record<string, unknown>): string {
  switch (name) {
    case 'Read':
      return args.path ? String(args.path) : '';
    case 'Write':
      return args.path ? String(args.path) : '';
    case 'Edit':
    case 'MultiEdit':
      return args.path ? String(args.path) : '';
    case 'LS':
      return args.path ? String(args.path) : '.';
    case 'Grep':
      return args.pattern ? String(args.pattern) : '';
    case 'Glob':
      return args.pattern ? String(args.pattern) : '';
    case 'Bash':
      return args.command ? String(args.command) : '';
    case 'WebFetch':
      return args.url ? String(args.url) : '';
    case 'Diagnostic':
      return 'Checking errors';
    case 'Task':
      return args.operation ? String(args.operation) : '';
    default:
      return '';
  }
}

/** Truncate a string for display */
function truncate(str: string, maxLen: number): string {
  if (str.length <= maxLen) return str;
  return str.slice(0, maxLen) + '...';
}

export function ToolCallView({ name, arguments: args, result, isRunning }: ToolCallViewProps) {
  const [expanded, setExpanded] = useState(false);
  const config = TOOL_CONFIG[name] || { icon: 'wrench', color: '#6B7280', label: name };
  const summary = getToolSummary(name, args);

  return (
    <div className="tool-call-view">
      {/* Header row */}
      <button
        className={`tool-call-header ${expanded ? 'expanded' : ''}`}
        onClick={() => setExpanded(!expanded)}
      >
        <div className="tool-call-header-left">
          {isRunning ? (
            <div className="tool-call-spinner" style={{ borderColor: config.color }}>
              <div className="tool-call-spinner-inner" style={{ borderTopColor: config.color }} />
            </div>
          ) : result?.is_error ? (
            <LucideIcon name="alert-circle" size={14} color="#EF4444" />
          ) : result ? (
            <LucideIcon name="check-circle" size={14} color={config.color} />
          ) : (
            <LucideIcon name={config.icon} size={14} color={config.color} />
          )}
          <span className="tool-call-label" style={{ color: config.color }}>
            {config.label}
          </span>
          {summary && (
            <span className="tool-call-summary">{truncate(summary, 60)}</span>
          )}
        </div>
        <LucideIcon
          name={expanded ? 'chevron-down' : 'chevron-right'}
          size={14}
          color="var(--text-tertiary)"
        />
      </button>

      {/* Expanded details */}
      {expanded && (
        <div className="tool-call-details">
          {/* Arguments */}
          <div className="tool-call-section">
            <div className="tool-call-section-label">Parameters</div>
            <pre className="tool-call-args">{JSON.stringify(args, null, 2)}</pre>
          </div>

          {/* Result */}
          {result && (
            <div className="tool-call-section">
              <div className="tool-call-section-label">
                {result.is_error ? 'Error' : 'Result'}
              </div>
              <pre className={`tool-call-result ${result.is_error ? 'error' : ''}`}>
                {truncate(result.output, 2000)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
