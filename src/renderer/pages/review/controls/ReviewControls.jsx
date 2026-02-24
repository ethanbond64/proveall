import React, { useState, useCallback } from 'react';
import { useReviewContext } from '../ReviewContext';

function ReviewControls({
  commitHash,
  onSaveComplete // Callback after save is complete
}) {
  const context = useReviewContext();
  const [isSaving, setIsSaving] = useState(false);

  const handleSaveReview = useCallback(async () => {
    if (isSaving || !context.isComplete) return;

    setIsSaving(true);
    try {
      const eventId = await context.actions.saveReview();
      console.log('Review saved with event ID:', eventId);

      // Call the completion callback if provided
      if (onSaveComplete) {
        onSaveComplete(eventId);
      }
    } catch (error) {
      console.error('Failed to save review:', error);
      alert('Failed to save review. Please try again.');
    } finally {
      setIsSaving(false);
    }
  }, [context, isSaving, onSaveComplete]);

  // Calculate review progress
  const getReviewProgress = () => {
    const { filesReviewed, totalFiles } = context.reviewSummary;
    if (totalFiles === 0) return 100; // No files = complete
    return Math.round((filesReviewed / totalFiles) * 100);
  };

  const progress = getReviewProgress();

  return (
    <div className="commit-review">
      <div className="commit-review-header">
        <h3>Review</h3>

        {/* Commit hash display */}
        {commitHash && (
          <span className="commit-hash" title={commitHash}>
            {commitHash.substring(0, 7)}
          </span>
        )}

        {/* Review summary with proper spacing */}
        <span className="review-summary">
          {context.reviewSummary.filesReviewed}/{context.reviewSummary.totalFiles} files reviewed
          {context.session.newIssues.length > 0 && (
            <> | {context.session.newIssues.length} new issue{context.session.newIssues.length !== 1 ? 's' : ''}</>
          )}
          {context.session.resolvedIssueIds.size > 0 && (
            <> | {context.session.resolvedIssueIds.size} resolved</>
          )}
        </span>

        {/* Visual progress indicator */}
        <div className="review-progress">
          <div className="review-progress-bar-container">
            <div
              className={`review-progress-bar-fill ${progress === 100 ? 'complete' : ''}`}
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>

        {/* Save button */}
        <button
          className="save-review-btn"
          onClick={handleSaveReview}
          disabled={!context.isComplete || isSaving}
          title={
            !context.isComplete
              ? 'Complete all file reviews before saving'
              : isSaving
              ? 'Saving review...'
              : 'Save review'
          }
        >
          {isSaving ? 'Saving...' : 'Save Review'}
        </button>

        {/* Completion indicator */}
        {context.isComplete && (
          <span className="review-complete-check" title="All files reviewed">
            ✓
          </span>
        )}
      </div>
    </div>
  );
}

export default ReviewControls;