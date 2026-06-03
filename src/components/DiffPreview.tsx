import { useState } from 'react';
import { LucideIcon } from './LucideIcon';

interface DiffLine {
  type: 'context' | 'added' | 'removed';
  content: string;
  lineNumberOld?: number;
  lineNumberNew?: number;
}

interface DiffPreviewProps {
  /** The file path being edited */
  filePath: string;
  /** The tool call arguments containing old_text/new_text */
  arguments: Record<string, unknown>;
  /** The tool result output */
  result?: {
    output: string;
    is_error: boolean;
  };
  /** Whether to show accept/reject buttons */
  showActions?: boolean;
  /** Callback when user accepts the diff */
  onAccept?: () => void;
  /** Callback when user rejects the diff */
  onReject?: () => void;
}

/** Parse a simple diff from old_text/new_text */
function parseSimpleDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split('\n');
  const newLines = newText.split('\n');
  const lines: DiffLine[] = [];

  // Find common prefix
  let prefixLen = 0;
  while (prefixLen < oldLines.length && prefixLen < newLines.length && oldLines[prefixLen] === newLines[prefixLen]) {
    lines.push({
      type: 'context',
      content: oldLines[prefixLen],
      lineNumberOld: prefixLen + 1,
      lineNumberNew: prefixLen + 1,
    });
    prefixLen++;
  }

  // Removed lines
  for (let i = prefixLen; i < oldLines.length; i++) {
    // Check if this line appears in the new text (it's a context line)
    if (i - prefixLen < newLines.length - (oldLines.length - prefixLen) + (oldLines.length - prefixLen) && oldLines[i] !== newLines[i - prefixLen + prefixLen]) {
      lines.push({
        type: 'removed',
        content: oldLines[i],
        lineNumberOld: i + 1,
      });
    }
  }

  // Added lines
  for (let i = prefixLen; i < newLines.length; i++) {
    lines.push({
      type: 'added',
      content: newLines[i],
      lineNumberNew: i + 1,
    });
  }

  return lines;
}

/** Generate diff from Edit/MultiEdit tool arguments */
function generateDiff(args: Record<string, unknown>): DiffLine[] {
  const lines: DiffLine[] = [];

  // Single edit mode
  if (args.old_text && args.new_text) {
    const oldText = String(args.old_text);
    const newText = String(args.new_text);
    return parseSimpleDiff(oldText, newText);
  }

  // Multi-edit mode
  if (args.edits && Array.isArray(args.edits)) {
    for (const edit of args.edits as Array<Record<string, unknown>>) {
      if (edit.old_text && edit.new_text) {
        lines.push(...parseSimpleDiff(String(edit.old_text), String(edit.new_text)));
        lines.push({ type: 'context', content: '---' }); // separator between edits
      }
    }
  }

  return lines;
}

export function DiffPreview({
  filePath,
  arguments: args,
  result,
  showActions = true,
  onAccept,
  onReject,
}: DiffPreviewProps) {
  const [expanded, setExpanded] = useState(true);
  const diffLines = generateDiff(args);
  const addedCount = diffLines.filter((l) => l.type === 'added').length;
  const removedCount = diffLines.filter((l) => l.type === 'removed').length;
  const isApplied = result && !result.is_error;

  return (
    <div className="diff-preview">
      {/* Header */}
      <button
        className={`diff-preview-header ${expanded ? 'expanded' : ''}`}
        onClick={() => setExpanded(!expanded)}
      >
        <div className="diff-preview-header-left">
          <LucideIcon name="file-edit" size={14} color="#F59E0B" />
          <span className="diff-preview-filename">{filePath}</span>
          <span className="diff-preview-stats">
            <span className="diff-stat-added">+{addedCount}</span>
            <span className="diff-stat-removed">-{removedCount}</span>
          </span>
          {isApplied && (
            <span className="diff-applied-badge">Applied</span>
          )}
        </div>
        <LucideIcon
          name={expanded ? 'chevron-down' : 'chevron-right'}
          size={14}
          color="var(--text-tertiary)"
        />
      </button>

      {/* Diff content */}
      {expanded && (
        <div className="diff-preview-content">
          <table className="diff-table">
            <tbody>
              {diffLines.map((line, idx) => (
                <tr key={idx} className={`diff-line diff-line-${line.type}`}>
                  <td className="diff-line-number">
                    {line.type === 'removed' || line.type === 'context' ? line.lineNumberOld : ''}
                  </td>
                  <td className="diff-line-number">
                    {line.type === 'added' || line.type === 'context' ? line.lineNumberNew : ''}
                  </td>
                  <td className="diff-line-prefix">
                    {line.type === 'added' ? '+' : line.type === 'removed' ? '-' : ' '}
                  </td>
                  <td className="diff-line-content">{line.content}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Action buttons */}
      {showActions && !isApplied && (
        <div className="diff-preview-actions">
          <button className="diff-btn diff-btn-reject" onClick={onReject}>
            <LucideIcon name="x" size={14} color="#EF4444" />
            <span>Reject</span>
          </button>
          <button className="diff-btn diff-btn-accept" onClick={onAccept}>
            <LucideIcon name="check" size={14} color="#FFFFFF" />
            <span>Accept</span>
          </button>
        </div>
      )}
    </div>
  );
}
