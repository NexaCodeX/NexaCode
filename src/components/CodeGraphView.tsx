import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { LucideIcon } from './LucideIcon';

interface CodeGraphViewProps {
  currentFolder: string | null;
  onClose: () => void;
}

interface NodeItem {
  id: string;
  file_path: string;
  name: string;
  kind: string; // file, class, function, struct, interface, method
  start_line: number;
  end_line: number;
}

interface SymbolDetails {
  node: NodeItem;
  callers: any[];
  callees: any[];
  imports?: string[];
  importedBy?: string[];
}

export const CodeGraphView: React.FC<CodeGraphViewProps> = ({
  currentFolder,
  onClose,
}) => {
  const [nodes, setNodes] = useState<NodeItem[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isIndexing, setIsIndexing] = useState(false);
  const [indexingMsg, setIndexingMsg] = useState('');
  const [selectedNode, setSelectedNode] = useState<NodeItem | null>(null);
  const [details, setDetails] = useState<SymbolDetails | null>(null);
  const [isLoadingDetails, setIsLoadingDetails] = useState(false);
  const [expandedFiles, setExpandedFiles] = useState<Record<string, boolean>>({});
  const [height, setHeight] = useState(350);

  useEffect(() => {
    fetchNodes();
  }, [currentFolder]);

  // Drag-to-resize the panel height (mirrors the Terminal panel behavior)
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

  const fetchNodes = async () => {
    try {
      const res: any = await invoke('tool_execute', {
        name: 'CodeGraph',
        args: { action: 'list_nodes' }
      });
      if (!res.is_error) {
        const list = JSON.parse(res.output) as NodeItem[];
        setNodes(list);
      }
    } catch (err) {
      console.error('Failed to load CodeGraph nodes:', err);
    }
  };

  const handleIndexProject = async () => {
    setIsIndexing(true);
    setIndexingMsg('Indexing workspace...');
    try {
      const res: any = await invoke('tool_execute', {
        name: 'CodeGraph',
        args: { action: 'index' }
      });
      if (res.is_error) {
        setIndexingMsg(`Error: ${res.output}`);
      } else {
        setIndexingMsg('Indexing complete!');
        await fetchNodes();
      }
    } catch (err: any) {
      setIndexingMsg(`Failed: ${err.message || err}`);
    } finally {
      setTimeout(() => {
        setIsIndexing(false);
        setIndexingMsg('');
      }, 3000);
    }
  };

  const handleSelectNode = async (node: NodeItem) => {
    setSelectedNode(node);
    setIsLoadingDetails(true);
    try {
      let callers: any[] = [];
      let callees: any[] = [];
      let imports: string[] = [];
      let importedBy: string[] = [];

      if (node.kind !== 'file') {
        const resCallers: any = await invoke('tool_execute', {
          name: 'CodeGraph',
          args: { action: 'get_call_hierarchy', symbol_id: node.id, direction: 'callers' }
        });
        if (!resCallers.is_error) callers = JSON.parse(resCallers.output);

        const resCallees: any = await invoke('tool_execute', {
          name: 'CodeGraph',
          args: { action: 'get_call_hierarchy', symbol_id: node.id, direction: 'callees' }
        });
        if (!resCallees.is_error) callees = JSON.parse(resCallees.output);
      } else {
        const resDeps: any = await invoke('tool_execute', {
          name: 'CodeGraph',
          args: { action: 'get_file_dependencies', path: node.file_path }
        });
        if (!resDeps.is_error) {
          const deps = JSON.parse(resDeps.output);
          imports = deps.imports;
          importedBy = deps.imported_by;
        }
      }

      setDetails({
        node,
        callers,
        callees,
        imports,
        importedBy
      });
    } catch (err) {
      console.error('Failed to load node details:', err);
    } finally {
      setIsLoadingDetails(false);
    }
  };

  const toggleFileExpand = (filePath: string) => {
    setExpandedFiles(prev => ({
      ...prev,
      [filePath]: !prev[filePath]
    }));
  };

  // Group nodes by file_path
  const fileGroups: Record<string, NodeItem[]> = {};
  const fileNodes = nodes.filter(n => n.kind === 'file');

  nodes.forEach(node => {
    if (node.kind !== 'file') {
      if (!fileGroups[node.file_path]) {
        fileGroups[node.file_path] = [];
      }
      fileGroups[node.file_path].push(node);
    }
  });

  // O(1) lookups for relation targets (avoids O(n) nodes.find() per rendered item)
  const nodeById = useMemo(() => {
    const m = new Map<string, NodeItem>();
    nodes.forEach(n => m.set(n.id, n));
    return m;
  }, [nodes]);
  const fileNodeByPath = useMemo(() => {
    const m = new Map<string, NodeItem>();
    nodes.forEach(n => { if (n.kind === 'file') m.set(n.file_path, n); });
    return m;
  }, [nodes]);

  const getFolderBasename = (path: string | null | undefined) => {
    if (!path) return 'NexaCode';
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  };

  // Split a file path into a dimmed directory prefix + emphasized basename
  const splitPath = (path: string) => {
    const parts = path.split(/[/\\]/);
    const base = parts.pop() || path;
    const dir = parts.length ? parts.join('/') + '/' : '';
    return { dir, base };
  };

  // Kind colors aligned with the app's warm accent palette (no neon),
  // softened for legibility on the dark utility panel.
  const getKindColor = (kind: string) => {
    switch (kind) {
      case 'class': return '#D4A64A'; // amber  ($accent-warning)
      case 'interface': return '#5BA577'; // green  ($accent-primary, lifted)
      case 'struct': return '#B58AD1'; // muted purple
      case 'function': return '#6FA8C7'; // muted blue
      case 'method': return '#D89575'; // coral  ($accent-coral)
      default: return '#9C9B99';
    }
  };

  const kindInitial = (kind: string) => (kind ? kind[0].toUpperCase() : '?');

  // Filter nodes based on search query
  const filteredFiles = fileNodes.filter(file => {
    const symbols = fileGroups[file.file_path] || [];
    const matchFile = file.file_path.toLowerCase().includes(searchQuery.toLowerCase());
    const matchSymbols = symbols.some(sym => sym.name.toLowerCase().includes(searchQuery.toLowerCase()));
    return matchFile || matchSymbols;
  });

  return (
    <div className="terminal-panel codegraph-panel" style={{ height: `${height}px` }}>
      {/* Resize handle */}
      <div className="terminal-resize-handle" onMouseDown={handleResizeMouseDown}>
        <div className="resize-bar" />
      </div>

      {/* Titlebar */}
      <div className="terminal-titlebar">
        <div className="terminal-titlebar-left">
          <LucideIcon name="git-branch" size={14} color="var(--accent-primary)" />
          <span className="terminal-title">CodeGraph Explorer</span>
          <span className="terminal-path">({getFolderBasename(currentFolder)})</span>
        </div>

        <div className="terminal-titlebar-actions">
          {indexingMsg && (
            <span className="indexing-msg-text">{indexingMsg}</span>
          )}
          <button
            className={`terminal-action-btn ${isIndexing ? 'indexing-active' : ''}`}
            onClick={handleIndexProject}
            disabled={isIndexing}
            title="Index / Refresh Workspace"
          >
            <LucideIcon name={isIndexing ? 'loader' : 'refresh-cw'} size={14} color="var(--accent-primary)" />
          </button>
          <button className="terminal-close-btn" onClick={onClose} title="Close Explorer">
            <LucideIcon name="x" size={14} color="var(--text-secondary)" />
          </button>
        </div>
      </div>

      {/* Explorer Search Input */}
      <div className="codegraph-searchbar">
        <LucideIcon name="search" size={14} color="var(--text-secondary)" />
        <input 
          type="text" 
          placeholder="Filter files or search symbols..." 
          className="codegraph-search-input"
          value={searchQuery}
          onChange={e => setSearchQuery(e.target.value)}
        />
      </div>

      {/* Content Body */}
      <div className="codegraph-body">
        {/* Left Side: Symbol Tree Explorer */}
        <div className="codegraph-tree-container">
          {filteredFiles.length === 0 ? (
            <div className="codegraph-empty-state">
              No indexed files found. Click the index icon to scan workspace.
            </div>
          ) : (
            filteredFiles.map(file => {
              const isExpanded = !!expandedFiles[file.file_path] || searchQuery.length > 0;
              const symbols = (fileGroups[file.file_path] || []).filter(sym =>
                sym.name.toLowerCase().includes(searchQuery.toLowerCase())
              );

              return (
                <div key={file.id} className="codegraph-tree-file-node">
                  <div 
                    className={`codegraph-file-header ${selectedNode?.id === file.id ? 'active' : ''}`}
                    onClick={() => handleSelectNode(file)}
                  >
                    <button 
                      className="codegraph-expand-btn"
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFileExpand(file.file_path);
                      }}
                    >
                      <LucideIcon name={isExpanded ? 'chevron-down' : 'chevron-right'} size={12} color="var(--text-secondary)" />
                    </button>
                    <LucideIcon name="folder" size={14} color="var(--accent-primary)" />
                    <span className="codegraph-node-name truncate" title={file.file_path}>
                      {(() => {
                        const { dir, base } = splitPath(file.file_path);
                        return (
                          <>
                            {dir && <span className="codegraph-node-dir">{dir}</span>}
                            {base}
                          </>
                        );
                      })()}
                    </span>
                  </div>

                  {isExpanded && (
                    <div className="codegraph-file-children">
                      {symbols.map(sym => (
                        <div 
                          key={sym.id} 
                          className={`codegraph-symbol-node ${selectedNode?.id === sym.id ? 'active' : ''}`}
                          onClick={() => handleSelectNode(sym)}
                        >
                          <span 
                            className="codegraph-kind-badge" 
                            style={{ backgroundColor: `${getKindColor(sym.kind)}22`, color: getKindColor(sym.kind) }}
                          >
                            {kindInitial(sym.kind)}
                          </span>
                          <span className="codegraph-node-name truncate">
                            {sym.name}
                          </span>
                          <span className="codegraph-node-line">
                            L{sym.start_line}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>

        {/* Right Side: Symbol Info and Call/Import Graph */}
        <div className="codegraph-details-container">
          {isLoadingDetails ? (
            <div className="codegraph-empty-state codegraph-loading">
              <span className="codegraph-loading-spinner">
                <LucideIcon name="loader" size={18} color="var(--accent-primary)" />
              </span>
              <span>Loading relationships…</span>
            </div>
          ) : !details ? (
            <div className="codegraph-empty-state">
              Select a file or symbol to explore call relationships and imports.
            </div>
          ) : (
            <div className="codegraph-details">
              <div className="codegraph-details-header">
                <span 
                  className="codegraph-kind-tag" 
                  style={{ backgroundColor: `${getKindColor(details.node.kind)}22`, color: getKindColor(details.node.kind) }}
                >
                  {details.node.kind}
                </span>
                <h4 className="codegraph-details-title">{details.node.name}</h4>
                <p className="codegraph-details-subtitle truncate" title={details.node.file_path}>
                  {details.node.file_path} (Lines {details.node.start_line} - {details.node.end_line})
                </p>
              </div>

              {details.node.kind !== 'file' ? (
                <div className="codegraph-details-relations">
                  {/* Callers */}
                  <div className="codegraph-relation-section">
                    <h5 className="codegraph-relation-title">Called By (Callers)</h5>
                    {details.callers.length === 0 ? (
                      <p className="codegraph-relation-empty">No callers found.</p>
                    ) : (
                      details.callers.map(caller => {
                        const targetNode = nodeById.get(caller.id);
                        return (
                          <div 
                            key={caller.id} 
                            className="codegraph-relation-item link"
                            onClick={() => targetNode && handleSelectNode(targetNode)}
                          >
                            <LucideIcon name="chevron-right" size={10} color="var(--accent-primary)" />
                            <span className="relation-symbol-name">{caller.name}</span>
                            <span className="relation-symbol-path">({caller.file_path})</span>
                          </div>
                        );
                      })
                    )}
                  </div>

                  {/* Callees */}
                  <div className="codegraph-relation-section">
                    <h5 className="codegraph-relation-title">Calls (Callees)</h5>
                    {details.callees.length === 0 ? (
                      <p className="codegraph-relation-empty">No callees found.</p>
                    ) : (
                      details.callees.map(callee => {
                        const targetNode = nodeById.get(callee.id);
                        return (
                          <div 
                            key={callee.id} 
                            className="codegraph-relation-item link"
                            onClick={() => targetNode && handleSelectNode(targetNode)}
                          >
                            <LucideIcon name="chevron-right" size={10} color="var(--accent-primary)" />
                            <span className="relation-symbol-name">{callee.name}</span>
                            <span className="relation-symbol-path">({callee.file_path})</span>
                          </div>
                        );
                      })
                    )}
                  </div>
                </div>
              ) : (
                <div className="codegraph-details-relations">
                  {/* Imports */}
                  <div className="codegraph-relation-section">
                    <h5 className="codegraph-relation-title">Imports</h5>
                    {(!details.imports || details.imports.length === 0) ? (
                      <p className="codegraph-relation-empty">No imports detected.</p>
                    ) : (
                      details.imports.map(imp => {
                        const targetNode = fileNodeByPath.get(imp);
                        return (
                          <div 
                            key={imp} 
                            className={`codegraph-relation-item ${targetNode ? 'link' : ''}`}
                            onClick={() => targetNode && handleSelectNode(targetNode)}
                          >
                            <LucideIcon name="file-text" size={12} color="var(--text-secondary)" />
                            <span className="relation-symbol-name truncate" title={imp}>{imp}</span>
                          </div>
                        );
                      })
                    )}
                  </div>

                  {/* Imported By */}
                  <div className="codegraph-relation-section">
                    <h5 className="codegraph-relation-title">Imported By</h5>
                    {(!details.importedBy || details.importedBy.length === 0) ? (
                      <p className="codegraph-relation-empty">Not imported by any files.</p>
                    ) : (
                      details.importedBy.map(impBy => {
                        const targetNode = fileNodeByPath.get(impBy);
                        return (
                          <div 
                            key={impBy} 
                            className={`codegraph-relation-item ${targetNode ? 'link' : ''}`}
                            onClick={() => targetNode && handleSelectNode(targetNode)}
                          >
                            <LucideIcon name="file-text" size={12} color="var(--text-secondary)" />
                            <span className="relation-symbol-name truncate" title={impBy}>{impBy}</span>
                          </div>
                        );
                      })
                    )}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
