import React, { useState, useEffect, useCallback, useRef } from 'react';
import MenuPage from './pages/menu/MenuPage';
import ProjectPage from './pages/project/ProjectPage';
import ReviewProjectPage from './pages/review/ReviewProjectPage'; // New implementation ready for Phase 2
import SettingsPage from './pages/SettingsPage.jsx';
import BranchContextModal from './components/BranchContextModal';
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
