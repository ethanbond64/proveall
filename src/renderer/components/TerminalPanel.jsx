import React, { useEffect, useRef, useCallback, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';

function TerminalPanel({ projectPath, prompt, pendingPrompt, onClearPendingPrompt, onClose, onRunningChange, command, args }) {
  const termRef = useRef(null);
  const terminalInstance = useRef(null);
  const fitAddon = useRef(null);
  const sessionIdRef = useRef(null);
  const unlistenOutput = useRef(null);
  const unlistenExit = useRef(null);
  const promptInjected = useRef(false);
  const promptTimeout = useRef(null);
  const [isRunning, setIsRunning] = useState(false);

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
        await window.backendAPI.ptyKill(sessionIdRef.current);
      } catch (_) {}
      sessionIdRef.current = null;
    }
    setIsRunning(false);
  }, []);

  // Report running state changes to parent
  useEffect(() => {
    if (onRunningChange) {
      onRunningChange(isRunning);
    }
  }, [isRunning, onRunningChange]);

  // Handle pendingPrompt injection
  useEffect(() => {
    if (pendingPrompt && sessionIdRef.current !== null) {
      invoke('pty_write', { sessionId: sessionIdRef.current, data: pendingPrompt })
        .catch(console.error);
      if (onClearPendingPrompt) {
        onClearPendingPrompt();
      }
    }
  }, [pendingPrompt, onClearPendingPrompt]);

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
        window.backendAPI.ptyWrite(sessionIdRef.current, data).catch(console.error);
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
        window.backendAPI.ptyResize(sessionIdRef.current, cols, rows).catch(console.error);
      }
    });

    const startSession = async () => {
      await new Promise((r) => requestAnimationFrame(r));

      try {
        const { cols, rows } = term;
        const sessionId = await window.backendAPI.ptySpawn(projectPath, cols, rows, command, args);
        sessionIdRef.current = sessionId;
        setIsRunning(true);

        unlistenOutput.current = await listen(`pty-output-${sessionId}`, (event) => {
          const bytes = Uint8Array.from(atob(event.payload), c => c.charCodeAt(0));
          term.write(bytes);

          if (!promptInjected.current && prompt) {
            promptInjected.current = true;
            setTimeout(() => {
              if (sessionIdRef.current !== null) {
                window.backendAPI.ptyWrite(sessionIdRef.current, prompt).catch(console.error);
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

  return (
    <div
      className="terminal-container"
      ref={termRef}
      style={{ flex: 1 }}
    />
  );
}

export default TerminalPanel;
