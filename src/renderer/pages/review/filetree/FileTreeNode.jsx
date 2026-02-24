import React from 'react';

// Helper function to get status description
function getStatusTitle(status) {
  const titles = {
    'M': 'Modified',
    'A': 'Added',
    'D': 'Deleted',
    'R': 'Renamed',
    'C': 'Copied'
  };
  return titles[status] || 'Changed';
}

/**
 * FileTreeNode component - Renders a single node in the file tree
 * Works for both flat lists (depth=0, no children) and tree views (with children)
 */
function FileTreeNode({
  node,
  depth = 0,
  projectPath,
  selectedPath,
  onSelectFile,
  fileReviews,
  onFileReviewToggle,
  // Optional: for tree mode with lazy loading
  isExpanded,
  hasChildren,
  children,
  onToggleExpand,
  // Optional: overlay data from touchedFiles
  overlayStatus,
  overlayReview
}) {
  const isSelected = selectedPath === node.path;

  // Get relative path for review lookup
  const getRelativePath = (absolutePath) => {
    if (!projectPath) return absolutePath;
    if (absolutePath === projectPath) return '';
    return absolutePath.startsWith(projectPath)
      ? absolutePath.substring(projectPath.length + 1)
      : absolutePath;
  };

  const relativePath = getRelativePath(node.path);

  // Use overlay data if provided, otherwise look up from node
  const nodeStatus = overlayStatus || node.status || node.diffMode;
  const review = overlayReview || fileReviews?.[relativePath];
  const showReviewDot = onFileReviewToggle != null && review;

  const handleClick = () => {
    if (node.isDirectory && onToggleExpand) {
      onToggleExpand(node.path);
    } else if (!node.isDirectory) {
      onSelectFile(node);
    }
  };

  const handleReviewToggle = (e) => {
    e.stopPropagation();
    e.preventDefault();
    if (onFileReviewToggle) {
      onFileReviewToggle(relativePath);
    }
  };

  const paddingLeft = 12 + depth * 16;

  return (
    <div className="tree-node">
      <div
        className={`tree-node-row ${isSelected ? 'selected' : ''} ${node.isDirectory ? 'directory' : 'file'}`}
        style={{ paddingLeft }}
        onClick={handleClick}
      >
        {node.isDirectory ? (
          <span className="tree-chevron">
            {isExpanded ? '\u25BC' : '\u25B6'}
          </span>
        ) : (
          <span className="tree-chevron-placeholder"></span>
        )}
        <span className="tree-icon">
          {node.isDirectory ? '\uD83D\uDCC1' : '\uD83D\uDCC4'}
        </span>
        <span className={`tree-name ${nodeStatus ? `status-${nodeStatus}` : ''}`}>
          {node.name}
        </span>
        {nodeStatus && (
          <span className={`file-status file-status-${nodeStatus}`} title={getStatusTitle(nodeStatus)}>
            {nodeStatus}
          </span>
        )}
        {showReviewDot && (
          <div className="file-review-toggles" onClick={(e) => e.stopPropagation()}>
            <button
              type="button"
              className={`file-review-dot ${review.read}`}
              onMouseDown={(e) => e.stopPropagation()}
              onClick={handleReviewToggle}
              title={`Read: ${review.read}`}
            />
          </div>
        )}
      </div>

      {node.isDirectory && isExpanded && hasChildren && children && (
        <div className="tree-children">
          {children}
        </div>
      )}
    </div>
  );
}

export default FileTreeNode;
