import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
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
  /** The tool call arguments containing old_text/new_text or content */
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
  /** Current session ID for loading backups */
  sessionId?: string;
}

/** Compute LCS-based line-level diff with prefix/suffix optimization */
function calculateDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split('\n');
  const newLines = newText.split('\n');

  // 1. Trim identical prefix lines
  let prefixLen = 0;
  while (
    prefixLen < oldLines.length &&
    prefixLen < newLines.length &&
    oldLines[prefixLen] === newLines[prefixLen]
  ) {
    prefixLen++;
  }

  // 2. Trim identical suffix lines
  let suffixLen = 0;
  while (
    suffixLen < oldLines.length - prefixLen &&
    suffixLen < newLines.length - prefixLen &&
    oldLines[oldLines.length - 1 - suffixLen] === newLines[newLines.length - 1 - suffixLen]
  ) {
    suffixLen++;
  }

  // 3. Middle lines to run LCS DP algorithm
  const midOld = oldLines.slice(prefixLen, oldLines.length - suffixLen);
  const midNew = newLines.slice(prefixLen, newLines.length - suffixLen);

  const m = midOld.length;
  const n = midNew.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (midOld[i - 1] === midNew[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }

  const midDiff: DiffLine[] = [];
  let i = m;
  let j = n;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && midOld[i - 1] === midNew[j - 1]) {
      midDiff.push({
        type: 'context',
        content: midOld[i - 1],
        lineNumberOld: prefixLen + i,
        lineNumberNew: prefixLen + j,
      });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      midDiff.push({
        type: 'added',
        content: midNew[j - 1],
        lineNumberNew: prefixLen + j,
      });
      j--;
    } else {
      midDiff.push({
        type: 'removed',
        content: midOld[i - 1],
        lineNumberOld: prefixLen + i,
      });
      i--;
    }
  }
  midDiff.reverse();

  // 4. Reconstruct full diff
  const fullDiff: DiffLine[] = [];

  // Prefix
  for (let k = 0; k < prefixLen; k++) {
    fullDiff.push({
      type: 'context',
      content: oldLines[k],
      lineNumberOld: k + 1,
      lineNumberNew: k + 1,
    });
  }

  // Mid
  fullDiff.push(...midDiff);

  // Suffix
  for (let k = 0; k < suffixLen; k++) {
    const oldIdx = oldLines.length - suffixLen + k;
    const newIdx = newLines.length - suffixLen + k;
    fullDiff.push({
      type: 'context',
      content: oldLines[oldIdx],
      lineNumberOld: oldIdx + 1,
      lineNumberNew: newIdx + 1,
    });
  }

  return fullDiff;
}

/** Compute new file content based on tools parameter */
function getNewContent(oldContent: string, args: Record<string, unknown>): string {
  if (args.content !== undefined) {
    return String(args.content);
  }

  let result = oldContent;
  if (args.old_text !== undefined) {
    const oldText = String(args.old_text);
    const newText = String(args.new_text ?? '');
    result = result.replace(oldText, newText);
  } else if (args.edits && Array.isArray(args.edits)) {
    for (const edit of args.edits) {
      if (edit.old_text !== undefined) {
        const oldText = String(edit.old_text);
        const newText = String(edit.new_text ?? '');
        result = result.replace(oldText, newText);
      }
    }
  }
  return result;
}

/** Reconstruct original file content by reversing edits from already-applied content */
function getOldContentFallback(currentContent: string, args: Record<string, unknown>): string {
  if (args.content !== undefined) {
    return ''; // Can't reconstruct from overwrite, default to empty (all added)
  }

  let result = currentContent;
  if (args.old_text !== undefined) {
    const oldText = String(args.old_text);
    const newText = String(args.new_text ?? '');
    result = result.replace(newText, oldText);
  } else if (args.edits && Array.isArray(args.edits)) {
    for (let k = args.edits.length - 1; k >= 0; k--) {
      const edit = args.edits[k];
      if (edit.old_text !== undefined) {
        const oldText = String(edit.old_text);
        const newText = String(edit.new_text ?? '');
        result = result.replace(newText, oldText);
      }
    }
  }
  return result;
}

interface CollapsedDiffGroup {
  type: 'visible' | 'collapsed';
  lines: DiffLine[];
  startIndex: number;
}

/** Group diff lines into collapsed and visible blocks */
function getDiffGroups(diffLines: DiffLine[], contextLinesCount = 3): CollapsedDiffGroup[] {
  if (diffLines.length === 0) return [];

  const visible = new Array(diffLines.length).fill(false);

  for (let k = 0; k < diffLines.length; k++) {
    if (diffLines[k].type === 'added' || diffLines[k].type === 'removed') {
      const start = Math.max(0, k - contextLinesCount);
      const end = Math.min(diffLines.length - 1, k + contextLinesCount);
      for (let i = start; i <= end; i++) {
        visible[i] = true;
      }
    }
  }

  const groups: CollapsedDiffGroup[] = [];
  let currentGroup: DiffLine[] = [];
  let isCurrentVisible = visible[0];
  let groupStartIdx = 0;

  for (let k = 0; k < diffLines.length; k++) {
    if (visible[k] === isCurrentVisible) {
      currentGroup.push(diffLines[k]);
    } else {
      const actualType = !isCurrentVisible && currentGroup.length <= 4 ? 'visible' : (isCurrentVisible ? 'visible' : 'collapsed');
      groups.push({
        type: actualType,
        lines: currentGroup,
        startIndex: groupStartIdx,
      });
      currentGroup = [diffLines[k]];
      isCurrentVisible = visible[k];
      groupStartIdx = k;
    }
  }

  if (currentGroup.length > 0) {
    const actualType = !isCurrentVisible && currentGroup.length <= 4 ? 'visible' : (isCurrentVisible ? 'visible' : 'collapsed');
    groups.push({
      type: actualType,
      lines: currentGroup,
      startIndex: groupStartIdx,
    });
  }

  return groups;
}

export function DiffPreview({
  filePath,
  arguments: args,
  result,
  showActions = true,
  onAccept,
  onReject,
  sessionId,
}: DiffPreviewProps) {
  const [expanded, setExpanded] = useState(true);
  const [oldContent, setOldContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [expandedGroups, setExpandedGroups] = useState<Record<number, boolean>>({});

  useEffect(() => {
    let active = true;
    setLoading(true);

    const loadContent = async () => {
      try {
        const isApplied = result && !result.is_error;
        if (isApplied && sessionId) {
          try {
            // Load original content from backups
            const backup = await invoke<string>('read_file_backup', {
              sessionId,
              path: filePath,
            });
            if (active) {
              setOldContent(backup);
              setLoading(false);
              return;
            }
          } catch (e) {
            console.log('[DiffPreview] Backup file not found or failed, using fallback:', e);
          }
        }

        // Fallback to current file content on disk
        const raw = await invoke<string>('read_file_raw', { path: filePath });
        if (active) {
          setOldContent(raw);
          setLoading(false);
        }
      } catch (err) {
        console.error('[DiffPreview] Failed to read old file content:', err);
        if (active) {
          setOldContent('');
          setLoading(false);
        }
      }
    };

    loadContent();

    return () => {
      active = false;
    };
  }, [filePath, sessionId, result]);

  if (loading) {
    return (
      <div className="diff-preview loading">
        <div className="diff-preview-header">
          <div className="diff-preview-header-left">
            <span className="animate-spin" style={{ display: 'inline-flex', alignItems: 'center' }}>
              <LucideIcon name="loader" size={14} />
            </span>
            <span className="diff-preview-filename">Loading diff for {filePath}...</span>
          </div>
        </div>
      </div>
    );
  }

  const isApplied = result && !result.is_error;
  let oldText = oldContent || '';
  let newText = getNewContent(oldText, args);

  // If already applied on disk and we loaded the modified content as oldText,
  // reconstruct the original content by reversing edits to show the correct diff.
  if (isApplied && oldText === newText) {
    oldText = getOldContentFallback(oldContent || '', args);
    newText = oldContent || '';
  }

  const diffLines = calculateDiff(oldText, newText);
  const addedCount = diffLines.filter((l) => l.type === 'added').length;
  const removedCount = diffLines.filter((l) => l.type === 'removed').length;

  const groups = getDiffGroups(diffLines, 3);

  return (
    <div className="diff-preview">
      {/* Header */}
      <button
        type="button"
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
              {groups.map((group, groupIdx) => {
                if (group.type === 'visible' || expandedGroups[groupIdx]) {
                  return group.lines.map((line, lineIdx) => (
                    <tr key={`${groupIdx}-${lineIdx}`} className={`diff-line diff-line-${line.type}`}>
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
                  ));
                } else {
                  return (
                    <tr key={groupIdx} className="diff-collapsed-banner-row">
                      <td colSpan={4} className="diff-collapsed-banner-cell">
                        <button
                          type="button"
                          className="diff-collapsed-expand-btn"
                          onClick={() => setExpandedGroups(prev => ({ ...prev, [groupIdx]: true }))}
                        >
                          <LucideIcon name="chevrons-up-down" size={12} />
                          <span>Show {group.lines.length} unchanged lines</span>
                        </button>
                      </td>
                    </tr>
                  );
                }
              })}
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
