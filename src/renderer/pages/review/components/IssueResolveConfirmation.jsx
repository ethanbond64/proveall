import React from 'react';

function IssueResolveConfirmation({
  isOpen,
  issue,
  onConfirm,
  onCancel,
  isResolving
}) {
  if (!isOpen || !issue) return null;

  return (
    <div className="confirmation-backdrop" onClick={onCancel}>
      <div className="confirmation-popup" onClick={(e) => e.stopPropagation()}>
        <h3>Confirm Issue Resolution</h3>

        <div className="confirmation-issue-details">
          <div className="confirmation-issue-id">
            Issue ID: #{issue.id.substring(0, 8)}
          </div>
          <div className="confirmation-issue-comment">
            {issue.comment}
          </div>
        </div>

        <div className="confirmation-prompt">
          Are you sure you want to mark this issue as resolved?
        </div>

        <div className="confirmation-actions">
          <button
            className="confirmation-btn cancel"
            onClick={onCancel}
            disabled={isResolving}
          >
            Cancel
          </button>
          <button
            className="confirmation-btn confirm"
            onClick={onConfirm}
            disabled={isResolving}
          >
            {isResolving ? 'Resolving...' : 'Resolve Issue'}
          </button>
        </div>
      </div>
    </div>
  );
}

export default IssueResolveConfirmation;