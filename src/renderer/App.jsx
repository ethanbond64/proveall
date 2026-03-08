import React, { useState, useEffect, useCallback, useRef } from 'react';
import MenuPage from './pages/menu/MenuPage';
import ProjectPage from './pages/project/ProjectPage';
import ReviewProjectPage from './pages/review/ReviewProjectPage'; // New implementation ready for Phase 2
import SettingsPage from './pages/SettingsPage.jsx';
import BranchContextModal from './components/BranchContextModal';
import TerminalPanel from './components/TerminalPanel';
import { COMMIT_REVIEW_MODE, BRANCH_COMPARISON_MODE } from './constants';
import { checkForUpdate } from './utils/updater';
import './styles.css';

function App() {
  // Essential state only - project, navigation, and review context
  const [currentProject, setCurrentProject] = useState(null); // Stores {id, path}
  const [projectState, setProjectState] = useState(null); // Project state (branch, commits, etc.)
  const [showReviewPage, setShowReviewPage] = useState(false);
  const [reviewMode, setReviewMode] = useState(COMMIT_REVIEW_MODE);
  const [reviewContext, setReviewContext] = useState(null); // Stores {commit, issueId, branchContextId} for review
  const [branchContextId, setBranchContextId] = useState(null); // Branch context for the project
  const [showBranchModal, setShowBranchModal] = useState(false);
  const [currentBranch, setCurrentBranch] = useState(null);
  const [pendingProjectOpen, setPendingProjectOpen] = useState(null);
  const [fixingIssueId, setFixingIssueId] = useState(null);
  const [showSettings, setShowSettings] = useState(false);
  const [updateInfo, setUpdateInfo] = useState(null);
  const [updateDismissed, setUpdateDismissed] = useState(false);
  const [updateInstalling, setUpdateInstalling] = useState(false);
  const lastUpdateCheck = useRef(0);

  const UPDATE_CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000; // 4 hours

  const runUpdateCheck = useCallback(async () => {
    const now = Date.now();
    if (now - lastUpdateCheck.current < UPDATE_CHECK_INTERVAL_MS) return;
    lastUpdateCheck.current = now;

    const result = await checkForUpdate();
    if (!result.available) return;

    setUpdateInfo(result);
    setUpdateDismissed(false);

    try {
      const settings = await window.backendAPI.getSettings();
      if (settings.auto_update) {
        setUpdateInstalling(true);
        await result.download();
      }
    } catch (e) {
      console.error('Auto-update failed:', e);
      setUpdateInstalling(false);
    }
  }, []);

  // Check on mount
  useEffect(() => { runUpdateCheck(); }, [runUpdateCheck]);

  // Re-check on page transitions (gated by interval)
  useEffect(() => { runUpdateCheck(); }, [currentProject, showReviewPage, showSettings, runUpdateCheck]);

  // Multi-session terminal state
  const [sessions, setSessions] = useState([]); // [{ id, projectPath, prompt, pendingPrompt, label }]
  const [activeSessionId, setActiveSessionId] = useState(null);
  const [sessionRunningStates, setSessionRunningStates] = useState({}); // { [id]: boolean }
  const nextSessionId = useRef(1);
  const [editingSessionId, setEditingSessionId] = useState(null);
  const [minimized, setMinimized] = useState(false);
  const [panelHeight, setPanelHeight] = useState(300);
  const isDragging = useRef(false);
  const dragStartY = useRef(0);
  const dragStartHeight = useRef(0);

  const handleOpenNewSession = (projectPath, prompt, issueId = null) => {
    const id = nextSessionId.current++;
    const label = `Session ${id}`;
    setSessions(prev => [...prev, { id, projectPath, prompt, pendingPrompt: null, label, issueId }]);
    setActiveSessionId(id);
    setMinimized(false);
  };

  const handleSendToExistingSession = (sessionId, prompt) => {
    setSessions(prev => prev.map(s =>
      s.id === sessionId ? { ...s, pendingPrompt: prompt } : s
    ));
    setActiveSessionId(sessionId);
    setMinimized(false);
  };

  const handleClearPendingPrompt = (sessionId) => {
    setSessions(prev => prev.map(s =>
      s.id === sessionId ? { ...s, pendingPrompt: null } : s
    ));
  };

  const handleCloseSession = (sessionId) => {
    setSessions(prev => {
      const updated = prev.filter(s => s.id !== sessionId);
      // If closing active tab, switch to last remaining
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

  // Simplified project opening - just set the project
  const openProject = async (project) => {
    // Store the project temporarily while we get branch context
    setPendingProjectOpen(project);

    try {
      // Get the current branch name
      const branch = await window.backendAPI.getCurrentBranch(project.id);
      setCurrentBranch(branch);

      // Show modal to get base branch
      setShowBranchModal(true);
    } catch (error) {
      console.error('Failed to get current branch:', error);
      // Fall back to opening without branch context
      setCurrentProject(project);
      setProjectState(null);
      setShowReviewPage(false);
      setReviewContext(null);
    }
  };

  // Handle branch context confirmation
  const handleBranchContextConfirm = (contextId) => {
    setBranchContextId(contextId);
    setShowBranchModal(false);

    // Now open the project with the branch context
    if (pendingProjectOpen) {
      setCurrentProject(pendingProjectOpen);
      setProjectState(null);
      setShowReviewPage(false);
      setReviewContext(null);
      setPendingProjectOpen(null);
    }
  };

  // Handle branch modal cancel
  const handleBranchModalCancel = () => {
    setShowBranchModal(false);
    setPendingProjectOpen(null);
    setCurrentBranch(null);
  };

  // Navigate to Review Page with review context
  const handleNavigateToReview = (commit, mode = COMMIT_REVIEW_MODE, issueId = null, baseCommit = null) => {
    setReviewContext({ commit, issueId, branchContextId, baseCommit });
    setReviewMode(mode);
    setShowReviewPage(true);
  };

  // Simple back navigation - always go back to project page
  const handleNavigateBack = () => {
    setShowReviewPage(false);
    setReviewContext(null);
  };

  // Navigate to issue-specific review
  const handleNavigateToIssue = (issueId) => {
    // Keep the current commit but switch to branch mode with issueId
    setReviewContext(prev => ({
      commit: prev?.commit,
      issueId,
      branchContextId
    }));
    setReviewMode(BRANCH_COMPARISON_MODE); // Set to branch mode for issue filtering
  };

  // Navigate back to menu (project selection)
  const handleNavigateToMenu = () => {
    setCurrentProject(null);
    setProjectState(null);
    setShowReviewPage(false);
    setReviewContext(null);
    // Close all terminal sessions
    setSessions([]);
    setActiveSessionId(null);
    setSessionRunningStates({});
  };

  const handleInstallUpdate = async () => {
    if (!updateInfo) return;
    setUpdateInstalling(true);
    try {
      await updateInfo.download();
    } catch (e) {
      console.error('Update install failed:', e);
      setUpdateInstalling(false);
    }
  };

  return (
    <div className="app">

      {/* Update Banner */}
      {updateInfo?.available && !updateDismissed && (
        <div className="update-banner">
          {updateInstalling ? (
            <span>Installing update v{updateInfo.version}...</span>
          ) : (
            <>
              <span>Update available — v{updateInfo.version}.</span>
              <button className="update-banner-btn" onClick={handleInstallUpdate}>Install Now</button>
              <button className="update-banner-btn dismiss" onClick={() => setUpdateDismissed(true)}>Dismiss</button>
            </>
          )}
        </div>
      )}

      {showSettings ? (
        <SettingsPage onBack={() => setShowSettings(false)} />
      ) : !currentProject ? (
        <MenuPage
          onProjectSelected={openProject}
        />
      ) : !showReviewPage ? (
        <ProjectPage
          project={currentProject}
          projectState={projectState}
          setProjectState={setProjectState}
          branchContextId={branchContextId}
          onNavigateToReview={handleNavigateToReview}
          onNavigateBack={handleNavigateToMenu}
          fixingIssueId={fixingIssueId}
          setFixingIssueId={setFixingIssueId}
          onShowSettings={() => setShowSettings(true)}
          onOpenNewSession={handleOpenNewSession}
          onSendToExistingSession={handleSendToExistingSession}
          sessions={sessions}
        />
      ) : (
        <ReviewProjectPage
          project={currentProject}
          projectState={projectState}
          commit={reviewContext?.commit}
          issueId={reviewContext?.issueId}
          baseCommit={reviewContext?.baseCommit}
          reviewMode={reviewMode}
          branchContextId={branchContextId}
          onNavigateBack={handleNavigateBack}
          onNavigateToIssue={handleNavigateToIssue}
        />
      )}

      {/* Terminal drawer - persists across page navigation */}
      {sessions.length > 0 && (
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
          {!minimized && sessions.map(session => (
            <div
              key={session.id}
              style={{
                display: session.id === activeSessionId ? 'flex' : 'none',
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
              />
            </div>
          ))}
        </div>
      )}

      {/* Branch Context Modal */}
      {showBranchModal && currentBranch && pendingProjectOpen && (
        <BranchContextModal
          projectId={pendingProjectOpen.id}
          currentBranch={currentBranch}
          onConfirm={handleBranchContextConfirm}
          onCancel={handleBranchModalCancel}
        />
      )}
    </div>
  );
}

export default App;
