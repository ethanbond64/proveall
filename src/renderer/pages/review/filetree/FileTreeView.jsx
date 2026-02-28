import React, { useEffect, useCallback } from 'react';
import FileTreeNode from './FileTreeNode';

/**
 * FileTreeView - Displays files as a tree structure with lazy loading
 * Used in "All Files" mode
 */
function FileTreeView({
  projectPath,
  projectId,
  touchedFiles,
  selectedPath,
  onSelectFile,
  fileReviews,
  onFileReviewToggle,
  expandedPaths,
  setExpandedPaths,
  childrenMap,
  setChildrenMap
}) {
  // Create maps for quick lookup of touched files status and reviews
  const touchedFilesMap = React.useMemo(() => {
    const map = new Map();
    if (touchedFiles) {
      touchedFiles.forEach(file => {
        map.set(file.path, file);
      });
    }
    return map;
  }, [touchedFiles]);

  // Convert absolute path to relative path
  const getRelativePath = useCallback((absolutePath) => {
    if (!projectPath) return absolutePath;
    if (absolutePath === projectPath) return '';
    return absolutePath.startsWith(projectPath)
      ? absolutePath.substring(projectPath.length + 1)
      : absolutePath;
  }, [projectPath]);

  // Load children for a directory
  const loadChildren = useCallback(async (dirPath) => {
    if (childrenMap.has(dirPath)) {
      return; // Already loaded
    }

    try {
      const children = await window.backendAPI.readDirectory(dirPath);
      setChildrenMap((prev) => {
        const next = new Map(prev);
        next.set(dirPath, children);
        return next;
      });
    } catch (error) {
      console.error('Failed to load directory:', dirPath, error);
      setChildrenMap((prev) => {
        const next = new Map(prev);
        next.set(dirPath, []);
        return next;
      });
    }
  }, [childrenMap, setChildrenMap]);

  // Toggle directory expansion
  const handleToggle = useCallback(async (path) => {
    const isCurrentlyExpanded = expandedPaths.has(path);

    if (!isCurrentlyExpanded) {
      // Load children before expanding
      await loadChildren(path);
    }

    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, [expandedPaths, loadChildren, setExpandedPaths]);

  // Initialize: load and expand project root
  useEffect(() => {
    if (projectPath && !childrenMap.has(projectPath)) {
      loadChildren(projectPath);
      setExpandedPaths(new Set([projectPath]));
    }
  }, [projectPath, childrenMap, loadChildren, setExpandedPaths]);

  // Recursive function to render tree nodes using FileTreeNode
  const renderNode = (node, depth = 0) => {
    const isExpanded = expandedPaths.has(node.path);
    const children = childrenMap.get(node.path);
    const hasChildren = children && children.length > 0;

    // Get overlay status and review from touchedFiles
    const relativePath = getRelativePath(node.path);
    const touchedFile = touchedFilesMap.get(relativePath);
    const overlayDiffMode = touchedFile?.diffMode;
    const overlayReview = fileReviews?.[relativePath];

    return (
      <FileTreeNode
        key={node.path}
        node={node}
        depth={depth}
        projectPath={projectPath}
        selectedPath={selectedPath}
        onSelectFile={onSelectFile}
        fileReviews={fileReviews}
        onFileReviewToggle={onFileReviewToggle}
        isExpanded={isExpanded}
        hasChildren={hasChildren}
        onToggleExpand={handleToggle}
        overlayStatus={overlayDiffMode}
        overlayReview={overlayReview}
      >
        {node.isDirectory && isExpanded && hasChildren && children.map(child => renderNode(child, depth + 1))}
      </FileTreeNode>
    );
  };

  if (!projectPath) {
    return (
      <div className="file-tree-view-empty">
        <p>No project selected</p>
      </div>
    );
  }

  // Create virtual root node
  const rootNode = {
    name: projectPath.split('/').pop() || projectPath,
    path: projectPath,
    isDirectory: true
  };

  return (
    <div className="file-tree-view">
      {renderNode(rootNode)}
    </div>
  );
}

export default FileTreeView;
