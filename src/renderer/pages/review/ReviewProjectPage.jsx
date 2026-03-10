import React, { useState } from 'react';
import { ReviewContextProvider, useReviewContext } from './ReviewContext';
import FileTree from './filetree/FileTree';
import EditorPanel from './editor/EditorPanel';
import ReviewControls from './controls/ReviewControls';
import IssueDataPanel from './issuedata/IssueDataPanel';
import { COMMIT_REVIEW_MODE, BRANCH_COMPARISON_MODE, MERGE_REVIEW_MODE, isInteractiveReviewMode } from '../../constants';
import { hasIssues } from '../../utils/reviewUtils';
import '../../styles.css';

// Inner component that uses the context
function ReviewProjectPageInner({
  project,
  projectState,
  onNavigateBack,
  onNavigateToIssue
}) {
  const context = useReviewContext();
  const [isFilePanelCollapsed, setIsFilePanelCollapsed] = useState(false);
  const [isIssuePanelCollapsed, setIsIssuePanelCollapsed] = useState(false);
  const [selectedFile, setSelectedFile] = useState(null);

  // Extract VCS info from projectState
  const vcsInfo = projectState?.branchData ? {
    isVCS: true,
    type: 'git',
    instanceData: {
      branch: projectState.branchData.branch,
      baseBranch: projectState.branchData.baseBranch
    },
    hiddenFiles: new Set(['.git', 'node_modules', '.idea', '.vscode'])
  } : null;

  // Extract diff stats from context
  const diffStats = context.touchedFiles.size > 0 ? {
    additions: Array.from(context.touchedFiles.values())
      .reduce((sum, file) => sum + (file.additions || 0), 0),
    deletions: Array.from(context.touchedFiles.values())
      .reduce((sum, file) => sum + (file.deletions || 0), 0)
  } : null;

  // Auto-select file when loading with issueId in branch mode (prefer files with issues)
  React.useEffect(() => {
    if (context.mode === BRANCH_COMPARISON_MODE && context.issueId && context.touchedFiles.size > 0 && !selectedFile) {
      // Sort files alphabetically by path for consistent ordering
      const touchedFilesArray = Array.from(context.touchedFiles.values())
        .sort((a, b) => a.path.localeCompare(b.path));

      // First, try to find a file with non-green status (files tied to the issue)
      let fileToSelect = touchedFilesArray.find(file => hasIssues(file.state));

      // If no non-green files found, fall back to first file
      if (!fileToSelect) {
        fileToSelect = touchedFilesArray[0];
      }

      if (fileToSelect) {
        // Set the selected file to open it in the editor
        setSelectedFile({
          path: fileToSelect.path,
          name: fileToSelect.path.split('/').pop()
        });
      }
    }
  }, [context.mode, context.issueId, context.touchedFiles]); // Intentionally omit selectedFile to avoid re-triggering

  return (
    <div className="project-page">
      {/* Project Page Header - copied exact structure */}
      <div className="project-page-header">
        {onNavigateBack && (
          <button className="header-icon-btn" onClick={onNavigateBack} title="Back to Project Page">
            ‹
          </button>
        )}
        {vcsInfo?.isVCS && (
          <div className="vcs-info">
            <span className="vcs-badge" title={`Version control: ${vcsInfo.type}`}>
              {vcsInfo.type.toUpperCase()}
            </span>
            {vcsInfo.instanceData?.branch && (
              <span className="vcs-branch" title="Current branch">
                {vcsInfo.instanceData.branch}
              </span>
            )}
            {vcsInfo.instanceData?.baseBranch && (
              <span className="vcs-compare" title={`Comparing to ${vcsInfo.instanceData.baseBranch}`}>
                vs {vcsInfo.instanceData.baseBranch}
              </span>
            )}
            {diffStats && (diffStats.additions > 0 || diffStats.deletions > 0) && (
              <span className="diff-stats">
                {diffStats.additions > 0 && (
                  <span className="additions" title="Lines added">
                    +{diffStats.additions}
                  </span>
                )}
                {diffStats.deletions > 0 && (
                  <span className="deletions" title="Lines deleted">
                    -{diffStats.deletions}
                  </span>
                )}
              </span>
            )}
          </div>
        )}
      </div>

      {/* Commit Review Component - shows in commit-review and merge-review modes */}
      {isInteractiveReviewMode(context.mode) && (
        <ReviewControls
          commitHash={context.commit}
          onSaveComplete={(eventId) => {
            console.log('Review saved with event ID:', eventId);
            if (onNavigateBack) {
              onNavigateBack();
            }
          }}
        />
      )}

      {/* Merge Review Mode Indicator */}
      {context.mode === MERGE_REVIEW_MODE && (
        <div className="branch-comparison-banner" style={{ borderColor: 'rgba(244, 135, 113, 0.3)', backgroundColor: 'rgba(244, 135, 113, 0.05)' }}>
          <span className="mode-indicator" style={{ color: '#f48771' }}>Merge Conflict Review</span>
          <span className="mode-description">Reviewing conflict resolutions from base branch merge</span>
        </div>
      )}

      {/* Branch Comparison Mode Indicator */}
      {context.mode === BRANCH_COMPARISON_MODE && (
        <div className="branch-comparison-banner">
          <span className="mode-indicator">Branch Comparison Mode</span>
          <span className="mode-description">Viewing aggregated reviews (read-only)</span>
        </div>
      )}

      {/* Main Content Area */}
      <div className="main-content">
        {/* File Panel */}
        <div className={`file-panel ${isFilePanelCollapsed ? 'collapsed' : ''}`}>
          <div className="panel-header">
            <button
              className="panel-collapse-btn"
              onClick={() => setIsFilePanelCollapsed(!isFilePanelCollapsed)}
              title={isFilePanelCollapsed ? 'Expand file panel' : 'Collapse file panel'}
            >
              {isFilePanelCollapsed ? '▶' : '◀'}
            </button>
            {!isFilePanelCollapsed && <span className="panel-title">Files</span>}
          </div>
          {!isFilePanelCollapsed && (
            <FileTree
              selectedPath={selectedFile?.path}
              onSelectFile={setSelectedFile}
            />
          )}
        </div>

        {/* Editor Panel */}
        <EditorPanel
          project={project}
          selectedFile={selectedFile}
          onActiveFileChange={setSelectedFile}
        />

        {/* Issue Data Panel - always shown */}
        <div className={`issue-data-panel ${isIssuePanelCollapsed ? 'collapsed' : ''}`}>
          <div className="panel-header">
            {!isIssuePanelCollapsed && <span className="panel-title">Issues</span>}
            <button
              className="panel-collapse-btn"
              onClick={() => setIsIssuePanelCollapsed(!isIssuePanelCollapsed)}
              title={isIssuePanelCollapsed ? 'Expand issue panel' : 'Collapse issue panel'}
            >
              {isIssuePanelCollapsed ? '◀' : '▶'}
            </button>
          </div>
          {!isIssuePanelCollapsed && (
            <IssueDataPanel
              selectedFile={selectedFile}
              onNavigateBack={onNavigateBack}
              onNavigateToIssue={onNavigateToIssue}
              highlightedIssueId={context.issueId}
            />
          )}
        </div>
      </div>
    </div>
  );
}

// Main component that provides the context
function ReviewProjectPage({
  project,
  projectState,
  commit,
  issueId,
  reviewMode = COMMIT_REVIEW_MODE,
  branchContextId,
  onNavigateBack,
  onNavigateToIssue
}) {

  return (
    <ReviewContextProvider
      mode={reviewMode}
      projectId={project?.id}
      projectPath={project?.path}
      commit={commit}
      issueId={issueId}
      branchContextId={branchContextId}
    >
      <ReviewProjectPageInner
        project={project}
        projectState={projectState}
        onNavigateBack={onNavigateBack}
        onNavigateToIssue={onNavigateToIssue}
      />
    </ReviewContextProvider>
  );
}

export default ReviewProjectPage;