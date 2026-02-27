import React, { useState, useCallback } from 'react';
import { useReviewContext } from '../ReviewContext';
import IssueResolveConfirmation from '../components/IssueResolveConfirmation';

function IssueReviewControls({
  issueId,
  onResolveComplete // Callback after resolution is complete
}) {
  const context = useReviewContext();
  const [isResolving, setIsResolving] = useState(false);
  const [showConfirmation, setShowConfirmation] = useState(false);

  const handleResolveClick = useCallback(() => {
    setShowConfirmation(true);
  }, []);

  const handleCancelConfirmation = useCallback(() => {
    setShowConfirmation(false);
  }, []);

  const handleConfirmResolve = useCallback(async () => {
    if (isResolving) return;

    setIsResolving(true);
    try {
      const eventId = await context.actions.saveResolution(issueId);
      console.log('Issue resolved with event ID:', eventId);

      // Call the completion callback if provided (returns to project page when in branch mode with issueId)
      if (onResolveComplete) {
        onResolveComplete(eventId);
      }
    } catch (error) {
      console.error('Failed to resolve issue:', error);
      alert('Failed to resolve issue. Please try again.');
    } finally {
      setIsResolving(false);
      setShowConfirmation(false);
    }
  }, [context, isResolving, onResolveComplete, issueId]);

  // Get the issue details from context
  const currentIssue = context.issues.get(issueId);

  return (
    <div className="commit-review issue-review-mode">
      <div className="commit-review-header">
        <h3>Issue Resolution</h3>

        {/* Issue ID display */}
        {currentIssue && (
          <>
            <span className="issue-badge" title={`Issue #${currentIssue.id}`}>
              #{currentIssue.id}
            </span>
            <span className="issue-comment-preview" title={currentIssue.comment}>
              {currentIssue.comment.length > 50
                ? currentIssue.comment.substring(0, 50) + '...'
                : currentIssue.comment}
            </span>
          </>
        )}

        {/* Resolve button - always enabled */}
        <button
          className="save-review-btn resolve-issue-btn"
          onClick={handleResolveClick}
          disabled={isResolving}
          title={isResolving ? 'Resolving issue...' : 'Mark issue as resolved'}
          style={{ marginLeft: 'auto' }}
        >
          {isResolving ? 'Resolving...' : 'Resolve Issue'}
        </button>
      </div>

      {/* Confirmation Popup */}
      <IssueResolveConfirmation
        isOpen={showConfirmation}
        issue={currentIssue}
        onConfirm={handleConfirmResolve}
        onCancel={handleCancelConfirmation}
        isResolving={isResolving}
      />
    </div>
  );
}

export default IssueReviewControls;