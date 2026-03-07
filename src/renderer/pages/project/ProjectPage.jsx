import React, { useState, useEffect, useMemo } from 'react';
import { COMMIT_REVIEW_MODE, BRANCH_COMPARISON_MODE, MERGE_REVIEW_MODE } from '../../constants';
import '../../styles.css';
import logoImage from '../../Square310x310Logo.png';

function ProjectPage({ project, projectState, setProjectState, branchContextId, onNavigateToReview, onNavigateBack, fixingIssueId, setFixingIssueId, onShowSettings }) {
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
  // Non-conflicted base-branch merges are auto-reviewed and skipped.
  // Conflicted base-branch merges (has_conflict_changes) require manual review.
  const oldestUnreviewedCommit = useMemo(() => {
    for (let i = commits.length - 1; i >= 0; i--) {
      const event = commits[i];
      if (event.id === null && event.event_type === 'commit') {
        // Skip non-conflicted base merges — they are auto-reviewed
        if (event.is_base_merge && !event.has_conflict_changes) continue;
        return event;
      }
    }
    return null;
  }, [commits]);

  // Index of the oldest unreviewed commit in the commits array
  const oldestUnreviewedIndex = useMemo(() => {
    if (!oldestUnreviewedCommit) return -1;
    return commits.findIndex(e => e.commit === oldestUnreviewedCommit.commit && e.event_type === 'commit');
  }, [commits, oldestUnreviewedCommit]);

  // Compute the set of commits in the selected bulk range
  // Non-conflicted base merges are excluded (auto-reviewed).
  const selectedRange = useMemo(() => {
    if (!selectedTargetCommit || !oldestUnreviewedCommit) return new Set();
    const targetIndex = commits.findIndex(e => e.commit === selectedTargetCommit && e.event_type === 'commit');
    if (targetIndex < 0 || oldestUnreviewedIndex < 0) return new Set();
    const rangeCommits = new Set();
    for (let i = targetIndex; i <= oldestUnreviewedIndex; i++) {
      const c = commits[i];
      if (c.id === null && c.event_type === 'commit') {
        // Skip non-conflicted base merges
        if (c.is_base_merge && !c.has_conflict_changes) continue;
        rangeCommits.add(c.commit);
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

    const clickedIndex = commits.findIndex(e => e.commit === event.commit && e.event_type === 'commit');
    // Only allow clicking unreviewed commits at or above oldestUnreviewedCommit
    if (clickedIndex < 0 || clickedIndex > oldestUnreviewedIndex) return;
    if (event.id !== null || event.event_type !== 'commit') return;

    // Conflicted base merges navigate directly to merge review
    if (event.is_base_merge && event.has_conflict_changes) {
      onNavigateToReview(event.commit, MERGE_REVIEW_MODE);
      return;
    }

    // If this is the next-to-review commit, navigate directly to review
    if (event.commit === oldestUnreviewedCommit.commit) {
      onNavigateToReview(event.commit, COMMIT_REVIEW_MODE);
      return;
    }

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
        <div className="project-header-row">
          <button
            className="header-icon-btn"
            onClick={onNavigateBack}
            title="Back to Project Menu"
          >
            ‹
          </button>
          <img src={logoImage} alt="PR Tool" className="project-page-logo" />
          <div className="project-header-spacer" />
          <button
            className="header-icon-btn"
            onClick={handlePageRefresh}
            disabled={isRefreshingPage || isLoading}
            title="Refresh commits and check for changes"
          >
            ⟲
          </button>
          <button
            className="header-icon-btn"
            onClick={onShowSettings}
            title="Settings"
          >
            ⚙
          </button>
        </div>
        <div className="project-header-row project-header-details">
          <span className="project-header-label">Current project:</span>
          <span className="project-name">{project?.path?.split('/').pop() || 'Project'}</span>
          {branchData.branch && (
            <>
              <span className="project-header-label">Current branch:</span>
              <span className="project-branch-info">{branchData.branch}</span>
              {branchData.base_branch && branchData.branch !== branchData.base_branch && (
                <>
                  <span className="project-header-label">Base branch:</span>
                  <span className="project-branch-info">{branchData.base_branch}</span>
                </>
              )}
            </>
          )}
        </div>
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
                const isBaseMerge = event.is_base_merge;
                const isConflictedMerge = isBaseMerge && event.has_conflict_changes;
                // Non-conflicted base merges are auto-handled — not clickable, not reviewable
                const isAutoMerge = isBaseMerge && !event.has_conflict_changes;
                // Check if this is the next commit to review
                const isNextToReview = !isResolution && !isAutoMerge && oldestUnreviewedCommit && event.commit === oldestUnreviewedCommit.commit;
                // Can click any unreviewed commit at or above oldestUnreviewedCommit
                // Non-conflicted base merges are never clickable. Conflicted merges are clickable when unreviewed.
                const isUnreviewedInRange = !isResolution && !isReviewed && !isAutoMerge && oldestUnreviewedIndex >= 0 && index <= oldestUnreviewedIndex;
                const canReview = isUnreviewedInRange;
                // Is this commit in the selected bulk range?
                const isInSelectedRange = selectedRange.has(event.commit);
                // Is this the selected target (top of range)?
                const isSelectedTarget = selectedTargetCommit === event.commit;

                return (
                  <div
                    key={index}
                    className={`commit-item ${canReview ? 'commit-item-clickable' : ''} ${isAutoMerge ? 'commit-item-merge' : ''} ${isNextToReview && !isBulkSelection ? 'commit-item-next' : ''} ${isInSelectedRange ? 'commit-item-in-range' : ''} ${isSelectedTarget ? 'commit-item-selected' : ''}`}
                    onClick={() => canReview && handleCommitClick(event)}
                    title={isAutoMerge ? 'Base branch merge (auto-reviewed)' : (canReview ? 'Click to select for review' : (isReviewed ? 'Already reviewed' : (isResolution ? 'Resolution event' : '')))}
                  >
                    <div className="commit-item-header">
                      <div className="commit-hash">{event.commit?.substring(0, 7)}</div>
                      {isResolution && (
                        <div className="commit-next-badge" style={{ color: '#569cd6', backgroundColor: 'rgba(86, 156, 214, 0.15)', borderColor: 'rgba(86, 156, 214, 0.3)' }}>RESOLUTION</div>
                      )}
                      {isConflictedMerge && !isReviewed && (
                        <div className="commit-merge-badge" style={{ color: '#f48771', backgroundColor: 'rgba(244, 135, 113, 0.15)', borderColor: 'rgba(244, 135, 113, 0.3)' }}>REVIEW MERGE</div>
                      )}
                      {isAutoMerge && (
                        <div className="commit-merge-badge">MERGE</div>
                      )}
                      {isNextToReview && !isBulkSelection && (
                        <div className="commit-next-badge" title="Next commit to review">
                          NEXT
                        </div>
                      )}
                      {isSelectedTarget && (
                        <div className="commit-next-badge" style={{ color: '#569cd6', backgroundColor: 'rgba(86, 156, 214, 0.15)', borderColor: 'rgba(86, 156, 214, 0.3)' }}>
                          SELECTED
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
