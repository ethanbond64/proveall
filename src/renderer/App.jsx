import React, { useState } from 'react';
import MenuPage from './pages/menu/MenuPage';
import ProjectPage from './pages/project/ProjectPage';
import ReviewProjectPage from './pages/review/ReviewProjectPage'; // New implementation ready for Phase 2
import SettingsPage from './pages/SettingsPage.jsx';
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
  const [branchContextId, setBranchContextId] = useState(null); // Branch context for the project
  const [showBranchModal, setShowBranchModal] = useState(false);
  const [currentBranch, setCurrentBranch] = useState(null);
  const [pendingProjectOpen, setPendingProjectOpen] = useState(null);
  const [fixingIssueId, setFixingIssueId] = useState(null);
  const [showSettings, setShowSettings] = useState(false);

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

  return (
    <div className="app">
      <header className="header">
        <div className="header-left">
        {/* TODO logo and name */}
        </div>
        <div className="header-right">
          <button className="gear-btn" onClick={() => setShowSettings(true)} title="Settings">&#9881;</button>
        </div>
      </header>

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
