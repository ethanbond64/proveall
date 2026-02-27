import React, { useState, useCallback } from 'react';
import { useReviewContext } from '../ReviewContext';
import IssueResolveConfirmation from '../components/IssueResolveConfirmation';
import {BRANCH_COMPARISON_MODE, COMMIT_REVIEW_MODE} from "../../../constants";


// Issue card component - simplified and cleaner
function IssueCard({ issue, isNew, isResolved, isHighlighted, onEdit, onDelete, onResolve, onUnresolve, onClick, mode }) {
  const cardClass = `issue-card ${isNew ? 'issue-new' : ''} ${isResolved ? 'issue-resolved' : ''} ${isHighlighted ? 'issue-highlighted' : ''}`;

  // Handle card click (not button clicks)
  const handleCardClick = (e) => {
    // Only trigger if clicking on the card itself, not buttons
    if (e.target.closest('.issue-action-btn')) {
      return;
    }
    if (onClick && !isNew) {
      onClick(issue.id);
    }
  };

  return (
    <div
      className={cardClass}
      onClick={handleCardClick}
      style={{
        cursor: onClick && !isNew ? 'pointer' : 'default',
        opacity: isHighlighted === false ? 0.6 : 1
      }}
      title={onClick && !isNew ? 'Click to view issue details' : ''}
    >
      <div className="issue-header">
        <span className="issue-id">
          {isNew ? 'NEW' : `#${issue.id.substring(0, 8)}`}
        </span>
        <div className="issue-actions">
          {isNew && (
            <>
              <button className="issue-action-btn" onClick={onEdit} title="Edit issue">
                ✎
              </button>
              <button className="issue-action-btn" onClick={onDelete} title="Delete issue">
                ×
              </button>
            </>
          )}
          {!isNew && !isResolved && onResolve && (
            <button
              className="issue-action-btn"
              onClick={onResolve}
              title={mode === COMMIT_REVIEW_MODE ? "Stage for resolution" : "Mark as resolved"}
            >
              ✓
            </button>
          )}
          {!isNew && isResolved && onUnresolve && (
            <button
              className="issue-action-btn"
              onClick={onUnresolve}
              title={mode === COMMIT_REVIEW_MODE ? "Unstage resolution" : "Reopen issue"}
            >
              ↺
            </button>
          )}
        </div>
      </div>
      <div className="issue-comment">
        {issue.comment}
      </div>
    </div>
  );
}

// Main panel component
function IssueDataPanel({ selectedFile, onNavigateBack, onNavigateToIssue, highlightedIssueId }) {
  const context = useReviewContext();
  const [editingIssue, setEditingIssue] = useState(null);
  const [editComment, setEditComment] = useState('');
  const [isCreatingIssue, setIsCreatingIssue] = useState(false);
  const [newIssueComment, setNewIssueComment] = useState('');
  const [confirmingIssue, setConfirmingIssue] = useState(null);
  const [isResolving, setIsResolving] = useState(false);

  // Handle issue creation
  const handleCreateIssue = useCallback(() => {
    if (!newIssueComment.trim()) return;

    context.actions.createIssue(newIssueComment.trim());
    setNewIssueComment('');
    setIsCreatingIssue(false);
  }, [newIssueComment, context.actions]);

  // Handle issue editing
  const handleStartEdit = useCallback((issue) => {
    setEditingIssue(issue.tempId);
    setEditComment(issue.comment);
  }, []);

  const handleSaveEdit = useCallback(() => {
    if (!editComment.trim()) return;

    context.actions.updateIssue(editingIssue, editComment.trim());
    setEditingIssue(null);
    setEditComment('');
  }, [editingIssue, editComment, context.actions]);

  const handleCancelEdit = useCallback(() => {
    setEditingIssue(null);
    setEditComment('');
  }, []);

  // Handle resolve button click - show confirmation only in branch mode
  const handleResolveClick = useCallback((issue) => {
    // In commit mode, just stage the resolution locally without confirmation
    if (context.mode === COMMIT_REVIEW_MODE) {
      context.actions.resolveIssue(issue.id);
    } else {
      // In branch mode, show confirmation popup
      setConfirmingIssue(issue);
    }
  }, [context.mode, context.actions]);

  // Handle cancel confirmation
  const handleCancelConfirmation = useCallback(() => {
    setConfirmingIssue(null);
  }, []);

  // Handle confirmed issue resolution with saving and navigation
  const handleConfirmResolve = useCallback(async () => {
    if (!confirmingIssue || isResolving) return;

    const issueId = confirmingIssue.id;

    // This should only be called in branch mode (not commit mode)
    setIsResolving(true);
    try {
      // Update local state first
      context.actions.resolveIssue(issueId);

      // Save the resolution to backend
      const eventId = await context.actions.saveResolution(issueId);
      console.log('Issue resolved with event ID:', eventId);

      // In branch mode: refresh the context data to show updated state
      if (context.mode === BRANCH_COMPARISON_MODE) {
        await context.actions.refreshData();
        // If we have an issueId, navigate back after refresh
        if (context.issueId && onNavigateBack) {
          onNavigateBack();
        }
      }
    } catch (error) {
      console.error('Failed to resolve issue:', error);
      // Revert the local state change on error
      context.actions.unresolveIssue(issueId);
      alert('Failed to resolve issue. Please try again.');
    } finally {
      setIsResolving(false);
      setConfirmingIssue(null);
    }
  }, [confirmingIssue, isResolving, context.actions, context.mode, onNavigateBack]);

  // Determine which issues to display based on mode
  let displayIssues = [];

  // In all modes, show issues based on file selection
  if (selectedFile && selectedFile.path) {
    // Show issues for the selected file
    const fileData = context.openFiles.get(selectedFile.path);
    if (fileData && fileData.issues) {
      displayIssues = Array.isArray(fileData.issues) ? fileData.issues : [];
    }
  } else {
    // No file selected - show all issues from file system data
    displayIssues = Array.from(context.issues.values());
  }

  // Filter unresolved and resolved issues
  const unresolvedExistingIssues = displayIssues
    .filter(issue => !context.session.resolvedIssueIds.has(issue.id));

  const resolvedIssues = Array.from(context.session.resolvedIssueIds)
    .map(id => {
      // Find the issue in displayIssues
      return displayIssues.find(issue => issue.id === id);
    })
    .filter(Boolean);


  const hasIssues = context.session.newIssues.length > 0 ||
                    resolvedIssues.length > 0 ||
                    unresolvedExistingIssues.length > 0;

  // In branch mode, can't create/edit issues, but can resolve them
  const canCreateEdit = context.mode === COMMIT_REVIEW_MODE;
  const canResolve = true; // All modes can resolve issues

  return (
    <div className="issue-data-content">
      {/* Mode-specific header */}
      <div className="mode-header">
        <p className="mode-info">
          {context.mode === COMMIT_REVIEW_MODE
            ? (selectedFile
                ? `Issues in ${selectedFile.path.split('/').pop()}`
                : 'All Issues - Mark resolutions to include in commit')
            : context.mode === BRANCH_COMPARISON_MODE
              ? (selectedFile
                  ? `Issues in ${selectedFile.path.split('/').pop()}`
                  : context.issueId ? 'Reviewing Specific Issue' : 'All Issues in Branch')
              : ''
          }
        </p>
      </div>

      {/* New Issues Section - Only show in commit mode */}
      {canCreateEdit && (context.session.newIssues.length > 0 || isCreatingIssue) && (
        <div className="issue-section new-issues">
          <h3 className="issue-section-title">
            New Issues ({context.session.newIssues.length})
          </h3>

          {context.session.newIssues.map(issue => (
            editingIssue === issue.tempId ? (
              <div key={issue.tempId} className="issue-form">
                <textarea
                  className="issue-textarea"
                  value={editComment}
                  onChange={(e) => setEditComment(e.target.value)}
                  autoFocus
                />
                <div className="issue-form-actions">
                  <button className="issue-form-btn primary" onClick={handleSaveEdit}>
                    Save
                  </button>
                  <button className="issue-form-btn" onClick={handleCancelEdit}>
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <IssueCard
                key={issue.tempId}
                issue={issue}
                isNew={true}
                isHighlighted={null}
                onEdit={() => handleStartEdit(issue)}
                onDelete={() => context.actions.deleteIssue(issue.tempId)}
                mode={context.mode}
              />
            )
          ))}

          {isCreatingIssue && (
            <div className="issue-form">
              <textarea
                className="issue-textarea"
                value={newIssueComment}
                onChange={(e) => setNewIssueComment(e.target.value)}
                placeholder="Describe the issue..."
                autoFocus
              />
              <div className="issue-form-actions">
                <button
                  className="issue-form-btn primary"
                  onClick={handleCreateIssue}
                  disabled={!newIssueComment.trim()}
                >
                  Create
                </button>
                <button
                  className="issue-form-btn"
                  onClick={() => {
                    setIsCreatingIssue(false);
                    setNewIssueComment('');
                  }}
                >
                  Cancel
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Resolved Issues Section */}
      {resolvedIssues.length > 0 && (
        <div className="issue-section resolved-issues">
          <h3 className="issue-section-title">
            {context.mode === COMMIT_REVIEW_MODE ? 'Staged Resolutions' : 'Resolved Issues'} ({resolvedIssues.length})
          </h3>
          {resolvedIssues.map(issue => (
            <IssueCard
              key={issue.id}
              issue={issue}
              isResolved={true}
              isHighlighted={highlightedIssueId ? issue.id === highlightedIssueId : null}
              onUnresolve={canResolve ? () => context.actions.unresolveIssue(issue.id) : undefined}
              onClick={onNavigateToIssue}
              mode={context.mode}
            />
          ))}
        </div>
      )}

      {/* Existing Issues Section */}
      {unresolvedExistingIssues.length > 0 && (
        <div className="issue-section existing-issues">
          <h3 className="issue-section-title">
            {context.mode === BRANCH_COMPARISON_MODE ? 'Issues' : 'Existing Issues'} ({unresolvedExistingIssues.length})
          </h3>
          {unresolvedExistingIssues.map(issue => (
            <IssueCard
              key={issue.id}
              issue={issue}
              isHighlighted={highlightedIssueId ? issue.id === highlightedIssueId : null}
              onResolve={canResolve ? () => handleResolveClick(issue) : undefined}
              onClick={onNavigateToIssue}
              mode={context.mode}
            />
          ))}
        </div>
      )}

      {/* Create issue button - only show in commit mode */}
      {canCreateEdit && !isCreatingIssue && (
        <button
          className="create-issue-btn"
          onClick={() => setIsCreatingIssue(true)}
        >
          + Create New Issue
        </button>
      )}

      {/* Empty state */}
      {!hasIssues && !isCreatingIssue && (
        <div className="issue-empty-state">
          <p>No issues {selectedFile && context.mode === BRANCH_COMPARISON_MODE ? 'in this file' : 'yet'}.</p>
          {canCreateEdit && (
            <p className="issue-empty-hint">
              Create issues for problems found during review.
            </p>
          )}
        </div>
      )}

      {/* Confirmation Popup */}
      <IssueResolveConfirmation
        isOpen={!!confirmingIssue}
        issue={confirmingIssue}
        onConfirm={handleConfirmResolve}
        onCancel={handleCancelConfirmation}
        isResolving={isResolving}
      />
    </div>
  );
}

export default IssueDataPanel;