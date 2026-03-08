import React, { useEffect, useRef, useCallback, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

function TerminalPanel({ projectPath, prompt, onClose }) {
  const termRef = useRef(null);
  const terminalInstance = useRef(null);
  const fitAddon = useRef(null);
  const sessionIdRef = useRef(null);
  const unlistenOutput = useRef(null);
  const unlistenExit = useRef(null);
  const promptInjected = useRef(false);
  const [isRunning, setIsRunning] = useState(false);
  const [minimized, setMinimized] = useState(false);
  const [panelHeight, setPanelHeight] = useState(300);
  const isDragging = useRef(false);
  const dragStartY = useRef(0);
  const dragStartHeight = useRef(0);

  const cleanup = useCallback(async () => {
    if (unlistenOutput.current) {
      unlistenOutput.current();
      unlistenOutput.current = null;
    }
    if (unlistenExit.current) {
      unlistenExit.current();
      unlistenExit.current = null;
    }
    if (sessionIdRef.current !== null) {
      try {
        await invoke('pty_kill', { sessionId: sessionIdRef.current });
      } catch (_) {}
      sessionIdRef.current = null;
    }
    setIsRunning(false);
  }, []);

  // Drag-to-resize handlers
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

  // Re-fit terminal when minimized state changes
  useEffect(() => {
    if (!minimized && fitAddon.current) {
      requestAnimationFrame(() => {
        fitAddon.current.fit();
        if (terminalInstance.current) {
          terminalInstance.current.focus();
        }
      });
    }
  }, [minimized]);

  useEffect(() => {
    const term = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#d4d4d4',
      },
      scrollback: 10000,
      allowProposedApi: true,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(termRef.current);

    requestAnimationFrame(() => {
      fit.fit();
      term.focus();
    });

    terminalInstance.current = term;
    fitAddon.current = fit;

    // Handle user input -> PTY
    term.onData((data) => {
      if (sessionIdRef.current !== null) {
        invoke('pty_write', { sessionId: sessionIdRef.current, data }).catch(console.error);
      }
    });

    // Debounced resize
    let resizeTimer = null;
    const resizeObserver = new ResizeObserver(() => {
      if (resizeTimer) clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => {
        if (fitAddon.current) {
          fitAddon.current.fit();
        }
      }, 100);
    });
    resizeObserver.observe(termRef.current);

    term.onResize(({ cols, rows }) => {
      if (sessionIdRef.current !== null) {
        invoke('pty_resize', { sessionId: sessionIdRef.current, cols, rows }).catch(console.error);
      }
    });

    const startSession = async () => {
      await new Promise((r) => requestAnimationFrame(r));

      try {
        const { cols, rows } = term;
        const sessionId = await invoke('pty_spawn', {
          projectPath,
          cols,
          rows,
        });
        sessionIdRef.current = sessionId;
        setIsRunning(true);

        unlistenOutput.current = await listen(`pty-output-${sessionId}`, (event) => {
          const bytes = Uint8Array.from(atob(event.payload), c => c.charCodeAt(0));
          term.write(bytes);

          if (!promptInjected.current && prompt) {
            promptInjected.current = true;
            setTimeout(() => {
              if (sessionIdRef.current !== null) {
                invoke('pty_write', { sessionId: sessionIdRef.current, data: prompt })
                  .catch(console.error);
              }
            }, 500);
          }
        });

        unlistenExit.current = await listen(`pty-exit-${sessionId}`, () => {
          term.writeln('\r\n\x1b[90m--- Process exited ---\x1b[0m');
          setIsRunning(false);
          sessionIdRef.current = null;
        });
      } catch (error) {
        term.writeln(`\x1b[31mFailed to start session: ${error}\x1b[0m`);
      }
    };

    startSession();

    return () => {
      if (resizeTimer) clearTimeout(resizeTimer);
      resizeObserver.disconnect();
      cleanup();
      term.dispose();
    };
  }, [projectPath, prompt]);

  const handleClose = async () => {
    await cleanup();
    onClose();
  };

  return (
    <div
      className={`terminal-drawer ${minimized ? 'terminal-drawer-minimized' : ''}`}
      style={minimized ? undefined : { height: panelHeight }}
    >
      {!minimized && (
        <div className="terminal-resize-handle" onMouseDown={handleDragStart} />
      )}
      <div className="terminal-panel-header">
        <span className="terminal-panel-title">
          LLM Terminal
          {isRunning && <span className="terminal-running-indicator" />}
        </span>
        <div className="terminal-panel-actions">
          <button
            className="terminal-header-btn"
            onClick={() => setMinimized(!minimized)}
            title={minimized ? 'Expand' : 'Minimize'}
          >
            {minimized ? '▴' : '▾'}
          </button>
          <button className="terminal-header-btn" onClick={handleClose} title="Close terminal">
            ×
          </button>
        </div>
      </div>
      <div
        className="terminal-container"
        ref={termRef}
        style={minimized ? { display: 'none' } : undefined}
      />
    </div>
  );
}

export default TerminalPanel;
