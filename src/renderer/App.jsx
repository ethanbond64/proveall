import React, { useState, useEffect } from 'react';
import MenuPage from './pages/menu/MenuPage';
import ProjectPage from './pages/project/ProjectPage';
import ReviewProjectPage from './pages/review/ReviewProjectPage'; // New implementation ready for Phase 2
import SettingsPage from './pages/SettingsPage';
import BranchContextModal from './components/BranchContextModal';
import { COMMIT_REVIEW_MODE, BRANCH_COMPARISON_MODE } from './constants';
import './styles.css';

function App() {
  // Essential state only - project, navigation, and review context
  const [currentProject, setCurrentProject] = useState(null); // Stores {id, path}
  const [projectState, setProjectState] = useState(null); // Project state (branch, commits, etc.)
  const [showReviewPage, setShowReviewPage] = useState(false);
  const [reviewMode, setReviewMode] = useState(COMMIT_REVIEW_MODE);
  const [reviewContext, setReviewContext] = useState(null); // Stores {commit, issueId, branchContextId} for review
  const [navigationHistory, setNavigationHistory] = useState([]); // Navigation history stack
  const [branchContextId, setBranchContextId] = useState(null); // Branch context for the project
  const [showBranchModal, setShowBranchModal] = useState(false);
  const [currentBranch, setCurrentBranch] = useState(null);
  const [pendingProjectOpen, setPendingProjectOpen] = useState(null);

  // Simplified project opening - just set the project
  const openProject = async (project) => {
    // Store the project temporarily while we get branch context
    setPendingProjectOpen(project);

    try {
      // Get the current branch name
      const branch = await window.electronAPI.getCurrentBranch(project.id);
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
      setNavigationHistory([]);
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
      setNavigationHistory([]);
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
  const handleNavigateToReview = (commit, mode = COMMIT_REVIEW_MODE, issueId = null, addToHistory = true) => {
    // Save current state to history before navigating
    if (addToHistory) {
      const historyEntry = {
        page: showReviewPage ? 'review' : 'project',
        mode: showReviewPage ? reviewMode : null,
        commit: showReviewPage ? reviewContext?.commit : null,
        issueId: showReviewPage ? reviewContext?.issueId : null,
        timestamp: Date.now()
      };

      // Limit history to 10 entries to prevent memory issues
      setNavigationHistory(prev => {
        const newHistory = [...prev, historyEntry];
        return newHistory.slice(-10);
      });
    }

    setReviewContext({ commit, issueId, branchContextId });
    setReviewMode(mode);
    setShowReviewPage(true);
  };

  // Smart back navigation using history
  const handleNavigateBack = () => {
    if (navigationHistory.length > 0) {
      const previousState = navigationHistory[navigationHistory.length - 1];
      setNavigationHistory(prev => prev.slice(0, -1)); // Pop from history

      if (previousState.page === 'project') {
        // Return to project page
        setShowReviewPage(false);
        setReviewContext(null);
      } else if (previousState.page === 'review') {
        // Restore previous review state without adding to history
        handleNavigateToReview(
          previousState.commit,
          previousState.mode,
          previousState.issueId,
          false // Don't add to history
        );
      }
    } else {
      // Default fallback to project page
      setShowReviewPage(false);
      setReviewContext(null);
    }
  };

  // Navigate to issue-specific review
  const handleNavigateToIssue = (issueId) => {
    // Save current state before switching to issue mode
    const historyEntry = {
      page: 'review',
      mode: reviewMode,
      commit: reviewContext?.commit,
      issueId: reviewContext?.issueId,
      timestamp: Date.now()
    };

    // Limit history to 10 entries
    setNavigationHistory(prev => {
      const newHistory = [...prev, historyEntry];
      return newHistory.slice(-10);
    });

    // Keep the current commit but switch to issue mode
    setReviewContext(prev => ({
      commit: prev?.commit,
      issueId
    }));
    setReviewMode('issue'); // Explicitly set to issue mode
  };

  // Navigate back to menu (project selection)
  const handleNavigateToMenu = () => {
    setCurrentProject(null);
    setProjectState(null);
    setShowReviewPage(false);
    setReviewContext(null);
    setNavigationHistory([]); // Clear history when leaving project
  };

  return (
    <div className="app">
      <header className="header">
        <div className="header-left">
        {/* TODO logo and name */}
        </div>
      </header>

      {!currentProject ? (
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
        />
      ) : (
        <ReviewProjectPage
          project={currentProject}
          projectState={projectState}
          commit={reviewContext?.commit}
          issueId={reviewContext?.issueId}
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
