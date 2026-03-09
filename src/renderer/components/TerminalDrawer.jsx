import React, { useState, useCallback, useRef, useEffect, useImperativeHandle, forwardRef } from 'react';
import TerminalPanel from './TerminalPanel';

const TerminalDrawer = forwardRef(({ onSessionsChange }, ref) => {
  const [sessions, setSessions] = useState([]);
  const [activeSessionId, setActiveSessionId] = useState(null);
  const [sessionRunningStates, setSessionRunningStates] = useState({});
  const nextSessionId = useRef(1);
  const [minimized, setMinimized] = useState(false);
  const [editingSessionId, setEditingSessionId] = useState(null);
  const [panelHeight, setPanelHeight] = useState(300);
  const isDragging = useRef(false);
  const dragStartY = useRef(0);
  const dragStartHeight = useRef(0);

  // Notify parent when sessions change (for sibling components that need the list)
  useEffect(() => { onSessionsChange?.(sessions); }, [sessions, onSessionsChange]);

  useImperativeHandle(ref, () => ({
    openNewSession(projectPath, prompt, issueId = null, command = null, args = null) {
      const id = nextSessionId.current++;
      const label = `Session ${id}`;
      setSessions(prev => [...prev, { id, projectPath, prompt, pendingPrompt: null, label, issueId, command, args }]);
      setActiveSessionId(id);
      setMinimized(false);
    },
    sendToExistingSession(sessionId, prompt) {
      setSessions(prev => prev.map(s =>
        s.id === sessionId ? { ...s, pendingPrompt: prompt } : s
      ));
      setActiveSessionId(sessionId);
      setMinimized(false);
    },
    resetAll() {
      setSessions([]);
      setActiveSessionId(null);
      setSessionRunningStates({});
    },
  }));

  const handleClearPendingPrompt = (sessionId) => {
    setSessions(prev => prev.map(s =>
      s.id === sessionId ? { ...s, pendingPrompt: null } : s
    ));
  };

  const handleCloseSession = (sessionId) => {
    setSessions(prev => {
      const updated = prev.filter(s => s.id !== sessionId);
      if (activeSessionId === sessionId && updated.length > 0) {
        setActiveSessionId(updated[updated.length - 1].id);
      } else if (updated.length === 0) {
        setActiveSessionId(null);
      }
      return updated;
    });
    setSessionRunningStates(prev => {
      const updated = { ...prev };
      delete updated[sessionId];
      return updated;
    });
  };

  const handleRunningChange = (sessionId, running) => {
    setSessionRunningStates(prev => ({ ...prev, [sessionId]: running }));
  };

  const handleRenameSession = (sessionId, newLabel) => {
    const trimmed = newLabel.trim();
    if (trimmed) {
      setSessions(prev => prev.map(s =>
        s.id === sessionId ? { ...s, label: trimmed } : s
      ));
    }
    setEditingSessionId(null);
  };

  const handleDragStart = useCallback((e) => {
    if (minimized) return;
    e.preventDefault();
    isDragging.current = true;
    dragStartY.current = e.clientY;
    dragStartHeight.current = panelHeight;

    const handleDragMove = (moveEvent) => {
      if (!isDragging.current) return;
      const delta = dragStartY.current - moveEvent.clientY;
      const newHeight = Math.max(100, Math.min(window.innerHeight - 80, dragStartHeight.current + delta));
      setPanelHeight(newHeight);
    };

    const handleDragEnd = () => {
      isDragging.current = false;
      document.removeEventListener('mousemove', handleDragMove);
      document.removeEventListener('mouseup', handleDragEnd);
    };

    document.addEventListener('mousemove', handleDragMove);
    document.addEventListener('mouseup', handleDragEnd);
  }, [minimized, panelHeight]);

  if (sessions.length === 0) return null;

  return (
    <div
      className={`terminal-drawer ${minimized ? 'terminal-drawer-minimized' : ''}`}
      style={minimized ? undefined : { height: panelHeight }}
    >
      {!minimized && (
        <div className="terminal-resize-handle" onMouseDown={handleDragStart} />
      )}
      <div className="terminal-panel-header">
        <div className="tab-bar" style={{ borderBottom: 'none', flex: 1 }}>
          {sessions.map(s => (
            <div
              key={s.id}
              className={`tab ${s.id === activeSessionId ? 'active' : ''}`}
              onClick={() => setActiveSessionId(s.id)}
            >
              {editingSessionId === s.id ? (
                <input
                  className="tab-name"
                  defaultValue={s.label}
                  autoFocus
                  onClick={e => e.stopPropagation()}
                  onBlur={e => handleRenameSession(s.id, e.target.value)}
                  onKeyDown={e => {
                    if (e.key === 'Enter') handleRenameSession(s.id, e.target.value);
                    if (e.key === 'Escape') setEditingSessionId(null);
                  }}
                  style={{ background: 'transparent', border: '1px solid #4ec9b0', color: 'inherit', font: 'inherit', padding: '0 2px', width: '80px', outline: 'none' }}
                />
              ) : (
                <span className="tab-name" onDoubleClick={e => { e.stopPropagation(); setEditingSessionId(s.id); }}>{s.label}</span>
              )}
              {sessionRunningStates[s.id] && <span className="terminal-running-indicator" />}
              <button
                className="tab-close-btn"
                onClick={e => { e.stopPropagation(); handleCloseSession(s.id); }}
              >
                ×
              </button>
            </div>
          ))}
        </div>
        <div className="terminal-panel-actions">
          <button
            className="terminal-header-btn"
            onClick={() => setMinimized(!minimized)}
            title={minimized ? 'Expand' : 'Minimize'}
          >
            {minimized ? '▴' : '▾'}
          </button>
        </div>
      </div>
      {sessions.map(session => (
        <div
          key={session.id}
          style={{
            display: !minimized && session.id === activeSessionId ? 'flex' : 'none',
            flex: 1,
            minHeight: 0,
          }}
        >
          <TerminalPanel
            projectPath={session.projectPath}
            prompt={session.prompt}
            pendingPrompt={session.pendingPrompt}
            onClearPendingPrompt={() => handleClearPendingPrompt(session.id)}
            onClose={() => handleCloseSession(session.id)}
            onRunningChange={(running) => handleRunningChange(session.id, running)}
            command={session.command}
            args={session.args}
          />
        </div>
      ))}
    </div>
  );
});

export default TerminalDrawer;
