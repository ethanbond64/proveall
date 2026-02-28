import React, { useState } from 'react';
import '../styles.css';

function BranchContextModal({ projectId, currentBranch, onConfirm, onCancel }) {
  const [baseBranch, setBaseBranch] = useState('main');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState(null);

  const handleSubmit = async (e) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    try {
      // Create the branch context
      const response = await window.backendAPI.createBranchContext(
        projectId,
        currentBranch,
        baseBranch,
        '{}'
      );

      // Call the onConfirm callback with the branch context ID
      onConfirm(response.id);
    } catch (err) {
      console.error('Failed to create branch context:', err);
      setError(err.message || 'Failed to create branch context');
      setIsLoading(false);
    }
  };

  return (
    <div className="modal-overlay">
      <div className="modal-container">
        <div className="modal-header">
          <h2>Branch Context Setup</h2>
        </div>

        <form onSubmit={handleSubmit} className="modal-body">
          <div className="form-group">
            <label>Current Branch:</label>
            <div className="current-branch-display">
              <strong>{currentBranch}</strong>
            </div>
          </div>

          <div className="form-group">
            <label htmlFor="baseBranch">Base Branch:</label>
            <input
              id="baseBranch"
              type="text"
              value={baseBranch}
              onChange={(e) => setBaseBranch(e.target.value)}
              placeholder="Enter base branch name (e.g., main, master)"
              required
              disabled={isLoading}
              autoFocus
            />
            <small className="form-help">
              The branch that {currentBranch} was branched from or should be compared against.
            </small>
          </div>

          {error && (
            <div className="error-message">
              {error}
            </div>
          )}

          <div className="modal-footer">
            <button
              type="button"
              onClick={onCancel}
              disabled={isLoading}
              className="btn-secondary"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isLoading || !baseBranch}
              className="btn-primary"
            >
              {isLoading ? 'Creating...' : 'Continue'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export default BranchContextModal;