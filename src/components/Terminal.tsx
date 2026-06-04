import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { LucideIcon } from './LucideIcon';

interface TerminalProps {
  currentFolder: string | null;
  onFolderChange: (path: string) => void;
  onClose: () => void;
}

interface TerminalLine {
  type: 'input' | 'output' | 'error' | 'system';
  text: string;
  dir?: string;
}

export const Terminal: React.FC<TerminalProps> = ({
  currentFolder,
  onFolderChange,
  onClose,
}) => {
  const [input, setInput] = useState('');
  const [lines, setLines] = useState<TerminalLine[]>([
    { type: 'system', text: 'Welcome to NexaCode Interactive Terminal 🦀' },
    { type: 'system', text: 'Type commands to run them in the current project environment.' },
  ]);
  const [isRunning, setIsRunning] = useState(false);
  
  // Resizing height state
  const [height, setHeight] = useState(280);
  
  // History states
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [autoScroll, setAutoScroll] = useState(true);

  const bufferEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input on mount and whenever loading finishes
  useEffect(() => {
    if (!isRunning && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isRunning]);

  // Set up event listeners for streaming terminal process outputs
  useEffect(() => {
    let unlistenStdout: (() => void) | null = null;
    let unlistenStderr: (() => void) | null = null;
    let unlistenExit: (() => void) | null = null;

    const setupListeners = async () => {
      unlistenStdout = await listen<string>('terminal-stdout', (event) => {
        setLines(prev => [...prev, { type: 'output', text: event.payload }]);
      });

      unlistenStderr = await listen<string>('terminal-stderr', (event) => {
        setLines(prev => [...prev, { type: 'error', text: event.payload }]);
      });

      unlistenExit = await listen<number>('terminal-exit', (event) => {
        const code = event.payload;
        if (code !== 0) {
          setLines(prev => [...prev, { type: 'error', text: `Process exited with code ${code}.` }]);
        } else {
          setLines(prev => [...prev, { type: 'system', text: `Process exited successfully (code 0).` }]);
        }
        setIsRunning(false);
      });
    };

    setupListeners();

    // Cleanup listeners and terminate process on unmount
    return () => {
      if (unlistenStdout) unlistenStdout();
      if (unlistenStderr) unlistenStderr();
      if (unlistenExit) unlistenExit();
      invoke('terminal_kill').catch(console.error);
    };
  }, []);

  // Auto-scroll logic
  useEffect(() => {
    if (autoScroll && bufferEndRef.current) {
      bufferEndRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [lines, autoScroll]);

  // Focus input when clicking anywhere on the terminal body
  const handleBodyClick = () => {
    if (inputRef.current) {
      inputRef.current.focus();
    }
  };

  // Helper function to resolve relative path strings in JS
  const resolvePath = (current: string, target: string): string => {
    const isWindowsAbsolute = /^[a-zA-Z]:[/\\]/.test(target);
    const isUnixAbsolute = target.startsWith('/');
    if (isWindowsAbsolute || isUnixAbsolute) {
      return target;
    }

    const isWindowsSep = current.includes('\\') || target.includes('\\');
    const currentParts = current.split(/[/\\]/).filter(Boolean);
    const drive = (isWindowsSep && current.match(/^[a-zA-Z]:/)) ? current.match(/^[a-zA-Z]:/)![0] : '';
    const targetParts = target.split(/[/\\]/).filter(Boolean);

    const resultParts = [...currentParts];
    for (const part of targetParts) {
      if (part === '.') {
        continue;
      } else if (part === '..') {
        if (resultParts.length > 0) {
          resultParts.pop();
        }
      } else {
        resultParts.push(part);
      }
    }

    if (isWindowsSep) {
      let path = resultParts.join('\\');
      if (drive && !path.startsWith(drive)) {
        path = drive + '\\' + path;
      }
      return path;
    } else {
      return '/' + resultParts.join('/');
    }
  };

  const executeCommand = async (cmd: string) => {
    const trimmed = cmd.trim();
    if (!trimmed) return;

    // Add command echo line
    setLines(prev => [...prev, { type: 'input', text: trimmed, dir: currentFolder || '' }]);

    // Update history
    setHistory(prev => {
      const updated = [...prev, trimmed];
      if (updated.length > 100) updated.shift();
      return updated;
    });
    setHistoryIndex(-1);
    setIsRunning(true);
    setInput('');

    // Handle 'clear' command directly
    if (trimmed === 'clear') {
      setLines([]);
      setIsRunning(false);
      return;
    }

    // Handle 'cd' navigation
    const cdMatch = trimmed.match(/^cd\s*(.*)$/);
    if (cdMatch) {
      const targetDir = cdMatch[1].trim();
      if (!targetDir || targetDir === '~') {
        setLines(prev => [...prev, { type: 'system', text: 'cd home directory not supported. Specify a relative or absolute path.' }]);
        setIsRunning(false);
        return;
      }

      const resolved = resolvePath(currentFolder || '', targetDir);
      try {
        await invoke('tool_set_working_dir', { path: resolved });
        onFolderChange(resolved);
      } catch (err: any) {
        setLines(prev => [...prev, { type: 'error', text: `cd: ${err.message || err}` }]);
      }
      setIsRunning(false);
      return;
    }

    // Run other commands in Tauri backend via streaming terminal_spawn
    try {
      await invoke('terminal_spawn', { command: trimmed });
    } catch (err: any) {
      setLines(prev => [...prev, { type: 'error', text: `Failed to execute: ${err.message || err}` }]);
      setIsRunning(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (isRunning) {
      if (e.ctrlKey && e.key.toLowerCase() === 'c') {
        e.preventDefault();
        setLines(prev => [...prev, { type: 'system', text: '^C' }]);
        handleStopProcess();
      } else {
        e.preventDefault();
      }
      return;
    }

    if (e.key === 'Enter') {
      executeCommand(input);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (history.length === 0) return;
      
      const nextIndex = historyIndex === -1 ? history.length - 1 : Math.max(0, historyIndex - 1);
      setHistoryIndex(nextIndex);
      setInput(history[nextIndex]);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (historyIndex === -1) return;
      
      if (historyIndex === history.length - 1) {
        setHistoryIndex(-1);
        setInput('');
      } else {
        const nextIndex = historyIndex + 1;
        setHistoryIndex(nextIndex);
        setInput(history[nextIndex]);
      }
    }
  };

  // Drag resize handler
  const handleResizeMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startHeight = height;

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const deltaY = startY - moveEvent.clientY;
      const newHeight = Math.max(150, Math.min(window.innerHeight - 150, startHeight + deltaY));
      setHeight(newHeight);
    };

    const handleMouseUp = () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  const getFolderBasename = (path: string | null | undefined) => {
    if (!path) return 'NexaCode';
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  };

  const handleStopProcess = async () => {
    try {
      await invoke('terminal_kill');
    } catch (err) {
      console.error('Failed to kill terminal process:', err);
    }
  };

  return (
    <div className="terminal-panel" style={{ height: `${height}px` }}>
      {/* Resize handle */}
      <div className="terminal-resize-handle" onMouseDown={handleResizeMouseDown}>
        <div className="resize-bar" />
      </div>

      {/* Terminal Titlebar */}
      <div className="terminal-titlebar">
        <div className="terminal-titlebar-left">
          <LucideIcon name="terminal" size={14} color="var(--accent-primary)" />
          <span className="terminal-title">Terminal</span>
          <span className="terminal-path" title={currentFolder || ''}>
            ({getFolderBasename(currentFolder)})
          </span>
        </div>

        <div className="terminal-titlebar-actions">
          {isRunning && (
            <button 
              className="terminal-action-btn stop-process-btn" 
              onClick={handleStopProcess} 
              title="Terminate Process (Ctrl+C)"
            >
              <LucideIcon name="square" size={12} color="var(--accent-negative)" />
            </button>
          )}

          <button 
            className="terminal-action-btn" 
            onClick={() => setLines([])} 
            title="Clear Buffer"
          >
            <LucideIcon name="trash-2" size={14} color="var(--text-secondary)" />
          </button>
          
          <button 
            className={`terminal-action-btn ${autoScroll ? 'active' : ''}`}
            onClick={() => setAutoScroll(!autoScroll)}
            title={autoScroll ? "Disable Auto Scroll" : "Enable Auto Scroll"}
          >
            <LucideIcon name="chevron-down" size={14} color={autoScroll ? "var(--accent-primary)" : "var(--text-secondary)"} />
          </button>

          <button className="terminal-close-btn" onClick={onClose} title="Close Terminal">
            <LucideIcon name="x" size={14} color="var(--text-secondary)" />
          </button>
        </div>
      </div>

      {/* Terminal Output Stream */}
      <div className="terminal-body" onClick={handleBodyClick}>
        <div className="terminal-output-container">
          {lines.map((line, idx) => (
            <div key={idx} className={`terminal-line ${line.type}`}>
              {line.type === 'input' && (
                <span className="terminal-prompt">
                  [{getFolderBasename(line.dir)}] $&nbsp;
                </span>
              )}
              <span className="terminal-text">{line.text}</span>
            </div>
          ))}
          {isRunning && (
            <div className="terminal-line system">
              <span className="terminal-loader">
                <LucideIcon name="loader" size={12} color="var(--accent-primary)" />
              </span>
              <span className="terminal-text italic">Running command...</span>
            </div>
          )}
          <div ref={bufferEndRef} />
        </div>

        {/* Input Prompt */}
        <div className="terminal-input-row">
          <span className="terminal-prompt">
            [{getFolderBasename(currentFolder)}] $&nbsp;
          </span>
          <input
            ref={inputRef}
            type="text"
            className="terminal-input"
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            autoFocus
            spellCheck={false}
            autoComplete="off"
            autoCapitalize="off"
          />
        </div>
      </div>
    </div>
  );
};
