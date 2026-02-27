import React, { useState } from 'react';
import { useReviewContext } from '../ReviewContext';
import ReviewPopup from '../components/ReviewPopup';
import {BRANCH_COMPARISON_MODE, COMMIT_REVIEW_MODE} from "../../../constants";

// Component to render individual file with its own progress state
function FileTreeItem({ file, isSelected, onSelectFile, onShowPopup }) {
  const context = useReviewContext();

  // Get progress info from context's computed values
  const fileProgress = context.fileProgress?.get(file.path) || {
    progressState: 'untouched',
    defaultState: null,
    hasLineReviews: false,
    changeBlocksCount: 0,
    reviewedBlocksCount: 0,
  };

  const handleClick = () => {
    if (onSelectFile) {
      onSelectFile({
        path: file.path,
        name: file.name || file.path.split('/').pop(),
        isDirectory: false
      });
    }
  };

  const handleReviewToggle = (e) => {
    e.stopPropagation();
    e.preventDefault();

    if (context.mode !== COMMIT_REVIEW_MODE) return;

    onShowPopup(e, file.path, fileProgress.defaultState);
  };

  const fileName = file.name || file.path.split('/').pop();

  return (
    <div className="tree-node">
      <div
        className={`tree-node-row file ${isSelected ? 'selected' : ''}`}
        onClick={handleClick}
        title={file.path}
      >
        <span className="tree-chevron-placeholder"></span>
        <span className="tree-icon">📄</span>
        <span className={`tree-name ${file.diffMode ? `status-${file.diffMode}` : ''}`}>
          {fileName}
        </span>

        {context.mode === COMMIT_REVIEW_MODE && (
          <>
            {/* Diff mode indicator */}
            {file.diffMode && (
              <span
                className={`file-status file-status-${file.diffMode}`}
                title={`${file.diffMode === 'A' ? 'Added' : file.diffMode === 'M' ? 'Modified' : 'Deleted'}`}
              >
                {file.diffMode}
              </span>
            )}

            {/* Default review indicator */}
            <div className="file-review-toggles" onClick={(e) => e.stopPropagation()}>
              <button
                type="button"
                className={`file-review-dot ${fileProgress.defaultState || ''}`}
                onMouseDown={(e) => e.stopPropagation()}
                onClick={handleReviewToggle}
                title={fileProgress.defaultState
                  ? `File default: ${fileProgress.defaultState}. Click to change.`
                  : 'Click to set file default review'}
                style={{
                  opacity: fileProgress.defaultState ? 1 : 0.3,
                }}
              />
              {/* Progress indicator */}
              <span
                className={`file-progress-indicator ${fileProgress.progressState}`}
                title={
                  fileProgress.progressState === 'complete'
                    ? 'All lines reviewed'
                    : fileProgress.progressState === 'in-progress'
                    ? `Review in progress (${fileProgress.reviewedBlocksCount}/${fileProgress.changeBlocksCount} blocks)`
                    : 'Not reviewed'
                }
              />
            </div>
          </>
        )}

        {context.mode === BRANCH_COMPARISON_MODE && (
          <div className="file-review-toggles">
            <span
              className={`file-review-dot ${file.state || 'gray'}`}
              title={`Aggregated review: ${file.state || 'gray'}`}
              style={{ cursor: 'default' }}
            />
          </div>
        )}
      </div>
    </div>
  );
}

function FileTree({
  selectedPath,
  onSelectFile
}) {
  const context = useReviewContext();
  const [filePopupState, setFilePopupState] = useState(null);
  const [expandedSections, setExpandedSections] = useState({ issues: true, approved: true });

  // Extract touched files from context
  const touchedFiles = Array.from(context.touchedFiles.values());

  // In branch mode, categorize files by their review state
  let filesWithIssues = [];
  let approvedFiles = [];

  if (context.mode === BRANCH_COMPARISON_MODE) {
    touchedFiles.forEach(file => {
      if (file.state === 'green') {
        approvedFiles.push(file);
      } else {
        filesWithIssues.push(file);
      }
    });

    // Sort files alphabetically within each category
    filesWithIssues.sort((a, b) => (a.name || a.path).localeCompare(b.name || b.path));
    approvedFiles.sort((a, b) => (a.name || a.path).localeCompare(b.name || b.path));
  }

  const handleShowPopup = (e, path, currentState) => {
    setFilePopupState({
      position: {
        x: e.clientX,
        y: e.clientY
      },
      path,
      currentState
    });
  };

  const toggleSection = (section) => {
    setExpandedSections(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };

  return (
    <>
      <div className="file-tree">
        {touchedFiles.length === 0 ? (
          <div className="file-tree-empty">
            <p>No files to review</p>
          </div>
        ) : context.mode === BRANCH_COMPARISON_MODE ? (
          // Branch mode: Show categorized lists
          <>
            {/* Files with open issues */}
            {filesWithIssues.length > 0 && (
              <div className="file-tree-section">
                <div
                  className="file-tree-section-header"
                  onClick={() => toggleSection('issues')}
                  style={{ cursor: 'pointer' }}
                >
                  <span className="tree-chevron" style={{ transform: expandedSections.issues ? 'rotate(90deg)' : 'rotate(0deg)' }}>▶</span>
                  <span className="file-tree-section-title">
                    {context.issueId ? `Files with This Issue (${filesWithIssues.length})` : `Files with Open Issues (${filesWithIssues.length})`}
                  </span>
                </div>
                {expandedSections.issues && (
                  <div className="file-tree-section-content">
                    {filesWithIssues.map(file => (
                      <FileTreeItem
                        key={file.path}
                        file={file}
                        isSelected={selectedPath === file.path}
                        onSelectFile={onSelectFile}
                        onShowPopup={handleShowPopup}
                      />
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Approved files */}
            {approvedFiles.length > 0 && (
              <div className="file-tree-section">
                <div
                  className="file-tree-section-header"
                  onClick={() => toggleSection('approved')}
                  style={{ cursor: 'pointer' }}
                >
                  <span className="tree-chevron" style={{ transform: expandedSections.approved ? 'rotate(90deg)' : 'rotate(0deg)' }}>▶</span>
                  <span className="file-tree-section-title">Approved Files ({approvedFiles.length})</span>
                </div>
                {expandedSections.approved && (
                  <div className="file-tree-section-content">
                    {approvedFiles.map(file => (
                      <FileTreeItem
                        key={file.path}
                        file={file}
                        isSelected={selectedPath === file.path}
                        onSelectFile={onSelectFile}
                        onShowPopup={handleShowPopup}
                      />
                    ))}
                  </div>
                )}
              </div>
            )}
          </>
        ) : (
          // Commit mode: Show all files as before
          <>
            {touchedFiles.map(file => (
              <FileTreeItem
                key={file.path}
                file={file}
                isSelected={selectedPath === file.path}
                onSelectFile={onSelectFile}
                onShowPopup={handleShowPopup}
              />
            ))}
          </>
        )}
      </div>

      {/* Render file default review popup */}
      {filePopupState && (
        <ReviewPopup
          mode="file-default"
          position={filePopupState.position}
          path={filePopupState.path}
          currentState={filePopupState.currentState}
          onClose={() => setFilePopupState(null)}
        />
      )}
    </>
  );
}

export default FileTree;