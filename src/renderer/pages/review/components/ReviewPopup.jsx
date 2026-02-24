import React, { useState, useEffect, useRef } from 'react';
import { useReviewContext } from '../ReviewContext';

/**
 * Consolidated Review Popup Component
 *
 * @param {Object} props
 * @param {string} props.mode - Either 'line' or 'file-default'
 * @param {Object} props.position - {x, y} coordinates for popup placement
 * @param {string} props.path - File path being reviewed
 * @param {string} props.currentState - Current review state ('green', 'yellow', 'red', or null)
 * @param {Object} props.range - Line range {start, end} - only for line mode
 * @param {Function} props.onClose - Callback when popup closes
 */
function ReviewPopup({ mode, position, path, currentState, range, onClose }) {
  const context = useReviewContext();
  const popupRef = useRef(null);
  const [isExpanded, setIsExpanded] = useState(false);
  const [selectedColor, setSelectedColor] = useState(currentState || '');
  const [selectedIssue, setSelectedIssue] = useState('');
  const [newIssueComment, setNewIssueComment] = useState('');

  const isLineMode = mode === 'line';
  const isFileDefaultMode = mode === 'file-default';

  // Handle clicks outside the popup when it's not expanded
  useEffect(() => {
    if (!isExpanded) {
      const handleClickOutside = (event) => {
        if (popupRef.current && !popupRef.current.contains(event.target)) {
          onClose();
        }
      };

      // Add event listener with a small delay to avoid immediate closure
      const timer = setTimeout(() => {
        document.addEventListener('mousedown', handleClickOutside);
      }, 0);

      return () => {
        clearTimeout(timer);
        document.removeEventListener('mousedown', handleClickOutside);
      };
    }
  }, [isExpanded, onClose]);

  const handleGreenClick = () => {
    if (isLineMode) {
      // Line mode: Immediately approve and close
      context.actions.updateLineReview(
        path,
        range.start,
        range.end,
        'green',
        null
      );
      onClose();
    } else {
      // File default mode: Immediately approve all lines and close
      context.actions.setFileDefault(path, 'green', null);
      onClose();
    }
  };

  const handleNonGreenClick = (color) => {
    setSelectedColor(color);
    setIsExpanded(true);
  };

  const handleSave = () => {
    let issueRef = null;

    // Create new issue if needed
    if (selectedColor !== 'green' && selectedIssue === 'new' && newIssueComment) {
      issueRef = context.actions.createIssue(newIssueComment);
    } else if (selectedColor !== 'green' && selectedIssue && selectedIssue !== 'new') {
      issueRef = selectedIssue;
    }

    // Call appropriate action based on mode
    if (isLineMode) {
      context.actions.updateLineReview(
        path,
        range.start,
        range.end,
        selectedColor,
        issueRef
      );
    } else {
      context.actions.setFileDefault(path, selectedColor, issueRef);
    }

    onClose();
  };

  const isValidSave = selectedColor === 'green' ||
    (selectedIssue && (selectedIssue !== 'new' || newIssueComment));

  const shouldShowIssueSection = selectedColor !== 'green' && isExpanded;

  return (
    <div
      ref={popupRef}
      className={`line-review-popup ${!isExpanded ? 'compact' : ''} ${isFileDefaultMode ? 'file-default-popup' : ''}`}
      style={{ left: position.x, top: position.y }}
    >
      {/* Compact view - same for both modes */}
      {!isExpanded ? (
        <div className="review-dot-buttons">
          <button
            className="line-review-dot-green review-dot-button"
            onClick={handleGreenClick}
            title={isFileDefaultMode ? "Approve all lines" : "Approve"}
            aria-label={isFileDefaultMode ? "Approve all lines" : "Approve"}
          />
          <button
            className="line-review-dot-yellow review-dot-button"
            onClick={() => handleNonGreenClick('yellow')}
            title="Warning"
            aria-label="Warning"
          />
          <button
            className="line-review-dot-red review-dot-button"
            onClick={() => handleNonGreenClick('red')}
            title="Issue"
            aria-label="Issue"
          />
        </div>
      ) : (
        /* Expanded view */
        <>
          {/* Header with mode indicator and selected color */}
          <div className="popup-header">
            {isFileDefaultMode && (
              <div className="popup-title">Set file default review</div>
            )}
            <div className="selected-indicator">
              <span className={`color-dot ${selectedColor}`} />
              <span className="color-label">
                {selectedColor === 'green' ? 'Approved' : selectedColor === 'yellow' ? 'Warning' : 'Issue'}
              </span>
            </div>
          </div>

          {/* Issue section - only show for non-green colors */}
          {shouldShowIssueSection && (
            <div className="issue-section">
              <select
                value={selectedIssue}
                onChange={e => {
                  setSelectedIssue(e.target.value);
                  setNewIssueComment('');
                }}
                autoFocus
              >
                <option value="">Select issue...</option>
                <option value="new">Create new issue</option>

                {/* New issues from session */}
                {context.session.newIssues.map(issue => (
                  <option key={issue.tempId} value={issue.tempId}>
                    New: {issue.comment.substring(0, 40)}...
                  </option>
                ))}

                {/* NOTE: Currently only showing net-new issues created during this commit review session.
                    Existing issues from the backend are not shown in the dropdown.
                    This behavior may change in the future to allow linking to existing issues. */}
              </select>

              {selectedIssue === 'new' && (
                <textarea
                  className="new-issue-comment"
                  placeholder="Enter issue description..."
                  value={newIssueComment}
                  onChange={e => setNewIssueComment(e.target.value)}
                  rows={3}
                />
              )}
            </div>
          )}

          {/* Action buttons */}
          <div className="popup-actions">
            <button
              className="save-btn"
              onClick={handleSave}
              disabled={!isValidSave}
            >
              {isFileDefaultMode ? 'Apply to All Lines' : 'Save'}
            </button>
            <button className="cancel-btn" onClick={onClose}>
              Cancel
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export default ReviewPopup;