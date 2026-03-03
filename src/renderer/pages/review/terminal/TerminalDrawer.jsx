import React, { useRef, useEffect, useState, useCallback } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { invoke } from '@tauri-apps/api/core';

const MIN_DRAWER_HEIGHT = 120;
const DEFAULT_DRAWER_HEIGHT = 250;

function TerminalDrawer({ projectPath }) {
  const [isOpen, setIsOpen] = useState(false);
  const [drawerHeight, setDrawerHeight] = useState(DEFAULT_DRAWER_HEIGHT);
  const terminalRef = useRef(null);
  const termRef = useRef(null);
  const fitAddonRef = useRef(null);
  const shellCreatedRef = useRef(false);
  const readingRef = useRef(false);
  const isResizingRef = useRef(false);

  const fitTerminal = useCallback(() => {
    if (fitAddonRef.current && termRef.current) {
      fitAddonRef.current.fit();
      invoke('async_resize_pty', {
        rows: termRef.current.rows,
        cols: termRef.current.cols,
      }).catch(() => {});
    }
  }, []);

  // Initialize terminal when drawer opens
  useEffect(() => {
    if (!isOpen || termRef.current) return;

    const term = new Terminal({
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 13,
      theme: {
        background: '#1a1a1a',
        foreground: '#e0e0e0',
        cursor: '#e0e0e0',
      },
      cursorBlink: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminalRef.current);

    termRef.current = term;
    fitAddonRef.current = fitAddon;

    // Fit after a frame so the container has its dimensions
    requestAnimationFrame(() => {
      fitAddon.fit();

      // Create the shell
      if (!shellCreatedRef.current) {
        shellCreatedRef.current = true;
        invoke('async_create_shell', { projectPath: projectPath || null })
          .catch(err => console.error('Error creating shell:', err));
      }

      invoke('async_resize_pty', {
        rows: term.rows,
        cols: term.cols,
      }).catch(() => {});
    });

    // Forward keystrokes to PTY
    term.onData((data) => {
      invoke('async_write_to_pty', { data }).catch(() => {});
    });

    // Continuous read loop — the backend blocks (on a dedicated thread)
    // until PTY data is available, so each invoke resolves as soon as
    // there's output. No polling/RAF needed.
    readingRef.current = true;
    async function readLoop() {
      while (readingRef.current) {
        try {
          const data = await invoke('async_read_from_pty');
          if (data) {
            term.write(data);
          }
        } catch {
          // PTY not ready or closed — short pause then retry
          await new Promise(r => setTimeout(r, 100));
        }
      }
    }
    readLoop();

    return () => {
      readingRef.current = false;
    };
  }, [isOpen, projectPath, fitTerminal]);

  // Re-fit when drawer height changes (but not during active drag)
  useEffect(() => {
    if (isOpen && !isResizingRef.current) {
      requestAnimationFrame(fitTerminal);
    }
  }, [drawerHeight, isOpen, fitTerminal]);

  // Listen for window resize
  useEffect(() => {
    if (!isOpen) return;

    const handleResize = () => fitTerminal();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [isOpen, fitTerminal]);

  // Drag-to-resize handler
  const handleMouseDown = useCallback((e) => {
    e.preventDefault();
    isResizingRef.current = true;
    const startY = e.clientY;
    const startHeight = drawerHeight;

    const handleMouseMove = (e) => {
      const delta = startY - e.clientY;
      const newHeight = Math.max(MIN_DRAWER_HEIGHT, startHeight + delta);
      setDrawerHeight(newHeight);
    };

    const handleMouseUp = () => {
      isResizingRef.current = false;
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      // Fit after resize completes
      requestAnimationFrame(fitTerminal);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [drawerHeight, fitTerminal]);

  return (
    <div className="terminal-drawer-wrapper">
      <button
        className="terminal-drawer-toggle"
        onClick={() => setIsOpen(!isOpen)}
        title={isOpen ? 'Close terminal' : 'Open terminal'}
      >
        <span className="terminal-toggle-icon">{isOpen ? '▼' : '▲'}</span>
        <span className="terminal-toggle-label">Terminal</span>
      </button>

      {isOpen && (
        <div className="terminal-drawer" style={{ height: drawerHeight }}>
          <div className="terminal-drawer-resize-handle" onMouseDown={handleMouseDown} />
          <div className="terminal-container" ref={terminalRef} />
        </div>
      )}
    </div>
  );
}

export default TerminalDrawer;
