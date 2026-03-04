import React, { useState, useEffect, useMemo } from 'react';
import { COMMIT_REVIEW_MODE, BRANCH_COMPARISON_MODE } from '../../constants';
import '../../styles.css';
import logoImage from '../../Square310x310Logo.png';

function ProjectPage({ project, projectState, setProjectState, branchContextId, onNavigateToReview, onNavigateBack, fixingIssueId, setFixingIssueId }) {
  const [isRefreshingPage, setIsRefreshingPage] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [selectedTargetCommit, setSelectedTargetCommit] = useState(null);

  // Extract data from projectState
  const commits = projectState?.events || [];
  const issues = projectState?.issues || [];
  const hasUncommittedChanges = projectState?.diffSummary &&
    (projectState.diffSummary.filesAdded > 0 ||
     projectState.diffSummary.filesModified > 0 ||
     projectState.diffSummary.filesDeleted > 0);
  const branchData = projectState?.branch_data || {};


  // TODO mismatch of camelcase and underscores in payload and expectations!
  console.log({projectState});
  console.log({branchData});

  // Find the oldest unreviewed commit (commits are ordered newest to oldest)
  // Using useMemo to ensure this recalculates when commits or projectState changes
  const oldestUnreviewedCommit = useMemo(() => {
    // Start from the end (oldest) and work backwards to find the last unreviewed commit
    for (let i = commits.length - 1; i >= 0; i--) {
      const event = commits[i];
      // Events with id are reviewed, without id are unreviewed
      if (event.id === null && event.event_type === 'commit') {
        return event;
      }
    }
    return null;
  }, [commits]);

  // Index of the oldest unreviewed commit in the commits array
  const oldestUnreviewedIndex = useMemo(() => {
    if (!oldestUnreviewedCommit) return -1;
    return commits.findIndex(e => e.commit === oldestUnreviewedCommit.commit);
  }, [commits, oldestUnreviewedCommit]);

  // Compute the set of commits in the selected bulk range
  const selectedRange = useMemo(() => {
    if (!selectedTargetCommit || !oldestUnreviewedCommit) return new Set();
    const targetIndex = commits.findIndex(e => e.commit === selectedTargetCommit);
    if (targetIndex < 0 || oldestUnreviewedIndex < 0) return new Set();
    // commits are newest-first, so target is at a lower index than oldest
    const rangeCommits = new Set();
    for (let i = targetIndex; i <= oldestUnreviewedIndex; i++) {
      if (commits[i].id === null && commits[i].event_type === 'commit') {
        rangeCommits.add(commits[i].commit);
      }
    }
    return rangeCommits;
  }, [commits, selectedTargetCommit, oldestUnreviewedCommit, oldestUnreviewedIndex]);

  // The number of commits in the bulk selection
  const bulkCount = selectedRange.size;

  // Whether bulk mode is active (more than 1 commit selected)
  const isBulkSelection = bulkCount > 1;

  // Find the base commit for bulk review: the last reviewed commit's hash
  // (the commit just below oldestUnreviewedCommit in the list)
  const baseCommitForBulk = useMemo(() => {
    if (!isBulkSelection || oldestUnreviewedIndex < 0) return null;
    // Look for the first reviewed commit below the oldest unreviewed
    for (let i = oldestUnreviewedIndex + 1; i < commits.length; i++) {
      if (commits[i].id !== null && commits[i].event_type === 'commit') {
        return commits[i].commit;
      }
    }
    return null; // No reviewed commit found (first review ever)
  }, [commits, isBulkSelection, oldestUnreviewedIndex]);

  console.log({oldestUnreviewedCommit, commits})

  // Load project state using the new API
  const loadProjectState = async () => {
    if (!project?.id || !branchContextId) {
      setIsLoading(false);
      return;
    }

    try {
      const state = await window.backendAPI.getProjectState(project.id, branchContextId);
      setProjectState(state);
    } catch (error) {
      console.error('Failed to load project state:', error);
    } finally {
      setIsLoading(false);
    }
  };

  // Handle page refresh
  const handlePageRefresh = async () => {
    setIsRefreshingPage(true);
    try {
      await loadProjectState();
    } catch (error) {
      console.error('Error refreshing page:', error);
    } finally {
      setIsRefreshingPage(false);
    }
  };

  // Load project state on mount or when project changes
  useEffect(() => {
    loadProjectState();
  }, [project?.id, branchContextId]);

  const handleFixWithLLM = async (e, issue) => {
    e.stopPropagation();
    if (fixingIssueId) return;
    setFixingIssueId(issue.id);
    try {
      await window.backendAPI.fixIssue(project.id, issue.id, branchContextId);
      await loadProjectState();
    } catch (error) {
      console.error('Failed to fix issue:', error);
      alert('Failed to fix issue: ' + error);
    } finally {
      setFixingIssueId(null);
    }
  };

  // Handle commit click - select for bulk review or toggle selection
  const handleCommitClick = (event) => {
    if (!oldestUnreviewedCommit) return;

    const clickedIndex = commits.findIndex(e => e.commit === event.commit);
    // Only allow clicking unreviewed commits at or above oldestUnreviewedCommit
    if (clickedIndex < 0 || clickedIndex > oldestUnreviewedIndex) return;
    if (event.id !== null || event.event_type !== 'commit') return;

    // Toggle: if clicking the already-selected target, deselect
    if (selectedTargetCommit === event.commit) {
      setSelectedTargetCommit(null);
      return;
    }

    // Select this commit as the target (top of the range)
    setSelectedTargetCommit(event.commit);
  };

  // Handle the review action button click
  const handleStartReview = () => {
    if (!oldestUnreviewedCommit) return;

    if (isBulkSelection) {
      onNavigateToReview(selectedTargetCommit, COMMIT_REVIEW_MODE, null, baseCommitForBulk);
    } else {
      // Single commit review (either oldest selected or no selection)
      const commitToReview = selectedTargetCommit || oldestUnreviewedCommit.commit;
      onNavigateToReview(commitToReview, COMMIT_REVIEW_MODE);
    }
  };

  return (
    <div className="project-page-container">
      <div className="project-page-header">
        <button
          className="back-button"
          onClick={onNavigateBack}
          title="Back to Project Menu"
        >
          ‹ Back
        </button>
        <img src={logoImage} alt="PR Tool" className="project-page-logo" />
        <div className="project-info">
          <span className="project-name">{project?.path?.split('/').pop() || 'Project'}</span>
        </div>
        <button
          className="page-refresh-btn"
          onClick={handlePageRefresh}
          disabled={isRefreshingPage || isLoading}
          title="Refresh commits and check for changes"
        >
          {isRefreshingPage ? 'Refreshing...' : '⟲ Refresh'}
        </button>
      </div>
      <div className="project-page-layout">
        {/* Left half - Commit History */}
        <div className="commit-history-panel">
          <h2 className="panel-title">Commit History</h2>
          <div className="commit-list">
            {isLoading ? (
              <div className="empty-state">Loading...</div>
            ) : commits.length > 0 ? (
              commits.map((event, index) => {
                const isResolution = event.event_type === 'resolution';
                const isReviewed = event.id !== null;
                // Check if this is the next commit to review
                const isNextToReview = !isResolution && oldestUnreviewedCommit && event.commit === oldestUnreviewedCommit.commit;
                // Can click any unreviewed commit at or above oldestUnreviewedCommit
                const isUnreviewedInRange = !isResolution && !isReviewed && oldestUnreviewedIndex >= 0 && index <= oldestUnreviewedIndex;
                const canReview = isUnreviewedInRange;
                // Is this commit in the selected bulk range?
                const isInSelectedRange = selectedRange.has(event.commit);
                // Is this the selected target (top of range)?
                const isSelectedTarget = selectedTargetCommit === event.commit;

                return (
                  <div
                    key={index}
                    className={`commit-item ${canReview ? 'commit-item-clickable' : ''} ${isNextToReview && !isBulkSelection ? 'commit-item-next' : ''} ${isInSelectedRange ? 'commit-item-in-range' : ''} ${isSelectedTarget ? 'commit-item-selected' : ''}`}
                    onClick={() => canReview && handleCommitClick(event)}
                    title={canReview ? 'Click to select for review' : (isReviewed ? 'Already reviewed' : (isResolution ? 'Resolution event' : ''))}
                  >
                    <div className="commit-item-header">
                      <div className="commit-hash">{event.commit?.substring(0, 7)}</div>
                      {isResolution && (
                        <div className="commit-next-badge" style={{ color: '#569cd6', backgroundColor: 'rgba(86, 156, 214, 0.15)', borderColor: 'rgba(86, 156, 214, 0.3)' }}>RESOLUTION</div>
                      )}
                      {isNextToReview && !isBulkSelection && (
                        <div className="commit-next-badge" title="Next commit to review">
                          NEXT
                        </div>
                      )}
                      {isInSelectedRange && (
                        <div className="commit-next-badge" style={{ color: '#569cd6', backgroundColor: 'rgba(86, 156, 214, 0.15)', borderColor: 'rgba(86, 156, 214, 0.3)' }}>
                          {isSelectedTarget ? 'SELECTED' : 'INCLUDED'}
                        </div>
                      )}
                      {isReviewed && !isResolution && (
                        <div
                          className={`commit-review-status green`}
                          title="Reviewed"
                        />
                      )}
                    </div>
                    <div className="commit-message">{event.message}</div>
                    <div className="commit-meta">
                      <div className="commit-author">{event.author}</div>
                      <div className="commit-date">{new Date(event.created_at).toLocaleDateString()}</div>
                    </div>
                  </div>
                );
              })
            ) : (
              <div className="empty-state">No commits found</div>
            )}
          </div>
        </div>

        {/* Right half - Split into top and bottom */}
        <div className="right-panel">
          {/* Top right quadrant - Diff Source Buttons */}
          <div className="review-section">
            <div className="diff-source-buttons">
              <h3 className="diff-source-buttons-title">Review Actions</h3>

              {/* Warning for uncommitted changes */}
              {hasUncommittedChanges && (
                <div className="uncommitted-changes-warning">
                  <span className="warning-icon">⚠</span>
                  <span className="warning-text">
                    This tool is intended for reviewing commits. You have uncommitted changes in your repository.
                  </span>
                </div>
              )}

              <div className="diff-source-buttons-grid">
                {/* Show review button: bulk or single commit */}
                {oldestUnreviewedCommit && isBulkSelection ? (
                  <button
                    className="diff-source-card diff-source-card-primary"
                    onClick={handleStartReview}
                  >
                    <span className="diff-source-card-icon">→</span>
                    <span className="diff-source-card-label">
                      Review {bulkCount} Commits
                      <span className="diff-source-card-sublabel">
                        {oldestUnreviewedCommit.commit.substring(0, 7)}..{selectedTargetCommit.substring(0, 7)}
                      </span>
                    </span>
                  </button>
                ) : oldestUnreviewedCommit ? (
                  <button
                    className="diff-source-card diff-source-card-primary"
                    onClick={handleStartReview}
                  >
                    <span className="diff-source-card-icon">→</span>
                    <span className="diff-source-card-label">
                      Review Next Commit
                      <span className="diff-source-card-sublabel">
                        {oldestUnreviewedCommit.commit.substring(0, 7)}: {oldestUnreviewedCommit.message.substring(0, 50)}
                        {oldestUnreviewedCommit.message.length > 50 ? '...' : ''}
                      </span>
                    </span>
                  </button>
                ) : branchData.branch && branchData.base_branch && branchData.branch !== branchData.base_branch ? (
                  // Show branch comparison if all commits are reviewed and we're on a feature branch
                  <button
                    className="diff-source-card"
                      //  TODO index of 0 is unsafe
                    onClick={() => onNavigateToReview(commits[0].commit, BRANCH_COMPARISON_MODE)}
                  >
                    <span className="diff-source-card-icon">⌥</span>
                    <span className="diff-source-card-label">
                      {branchData.branch} vs {branchData.baseBranch}
                      <span className="diff-source-card-sublabel">
                        All commits reviewed - view aggregate
                      </span>
                    </span>
                  </button>
                ) : (
                  <div className="empty-state">All commits reviewed</div>
                )}
              </div>
            </div>
          </div>

          {/* Bottom right quadrant - Issues */}
          <div className="issues-section">
            <h2 className="panel-title">Issues</h2>
            <div className="issue-list">
              {isLoading ? (
                <div className="empty-state">Loading...</div>
              ) : issues.length > 0 ? (
                issues.map((issue) => (
                  //  TODO index of 0 is unsafe
                  <div key={issue.id} className="issue-item" onClick={() => onNavigateToReview(commits[0].commit, BRANCH_COMPARISON_MODE, issue.id)}>
                    <div className="issue-item-content">
                      <div className="issue-id">#{issue.id.substring(0, 8)}</div>
                      <div className="issue-comment">{issue.comment}</div>
                    </div>
                    <button
                      className="btn-primary btn-sm"
                      onClick={(e) => handleFixWithLLM(e, issue)}
                      disabled={fixingIssueId === issue.id}
                      title={fixingIssueId === issue.id ? "Fixing this issue..." : "Send to LLM"}
                    >
                      {fixingIssueId === issue.id ? 'Fixing...' : 'Send to LLM'}
                    </button>
                  </div>
                ))
              ) : (
                <div className="empty-state">No issues</div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default ProjectPage;
