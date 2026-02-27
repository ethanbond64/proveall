import React, { useState, useEffect } from 'react';
import CodeEditor from './CodeEditor';
import DiffEditor from './DiffEditor';
import TabBar from './TabBar';
import { useReviewContext } from '../ReviewContext';

function EditorPanel({ selectedFile, onActiveFileChange = () => {} }) {
  const context = useReviewContext();
  const [tabs, setTabs] = useState([]);
  const [activeTabId, setActiveTabId] = useState(null);

  // Load file data when a file is selected
  useEffect(() => {
    if (!selectedFile || selectedFile.isDirectory || !context.projectId) {
      return;
    }

    const existingTab = tabs.find(tab => tab.filePath === selectedFile.path);

    if (existingTab) {
      setActiveTabId(existingTab.id);
    } else {
      openFileInNewTab(selectedFile);
    }
  }, [selectedFile]);

  // Reload file data when issueId changes (for branch mode with issue filtering)
  useEffect(() => {
    if (context.mode === 'branch' && tabs.length > 0) {
      // Reload all open tabs to get updated line summaries for the new issue
      tabs.forEach(tab => {
        if (tab.viewMode === 'ready') {
          context.actions.loadFileData(tab.relativePath).catch(error => {
            console.error(`Failed to reload file ${tab.fileName} after issue change:`, error);
          });
        }
      });
    }
  }, [context.issueId]); // Re-run when issueId changes

  const openFileInNewTab = async (node) => {
    const tabId = `tab-${Date.now()}`;

    const newTab = {
      id: tabId,
      fileName: node.name || node.path.split('/').pop(),
      filePath: node.path,
      relativePath: node.path.startsWith(context.projectPath)
        ? node.path.substring(context.projectPath.length + 1)
        : node.path,
      viewMode: 'loading',
      showDiff: true,
    };

    setTabs(prev => [newTab, ...prev]);
    setActiveTabId(tabId);

    try {
      await context.actions.loadFileData(newTab.relativePath);
      setTabs(prev => prev.map(tab =>
        tab.id === tabId ? { ...tab, viewMode: 'ready' } : tab
      ));
    } catch (error) {
      console.error('Failed to load file:', error);
      setTabs(prev => prev.map(tab =>
        tab.id === tabId ? { ...tab, viewMode: 'error' } : tab
      ));
    }
  };

  const handleTabClick = (tabId) => {
    setActiveTabId(tabId);

    const clickedTab = tabs.find(tab => tab.id === tabId);
    if (clickedTab) {
      onActiveFileChange({
        name: clickedTab.fileName,
        path: clickedTab.filePath,
        isDirectory: false
      });
    }
  };

  const closeTab = (tabId) => {
    setTabs(prev => {
      const newTabs = prev.filter(tab => tab.id !== tabId);

      if (newTabs.length === 0) {
        setActiveTabId(null);
        onActiveFileChange(null);
      } else if (tabId === activeTabId) {
        const closedIndex = prev.findIndex(tab => tab.id === tabId);
        const newActiveIndex = closedIndex > 0 ? closedIndex - 1 : 0;
        const newActiveTab = newTabs[newActiveIndex];
        setActiveTabId(newActiveTab.id);

        onActiveFileChange({
          name: newActiveTab.fileName,
          path: newActiveTab.filePath,
          isDirectory: false
        });
      }

      return newTabs;
    });
  };

  const toggleViewMode = () => {
    if (!activeTab || activeTab.viewMode !== 'ready') return;

    setTabs(prev => prev.map(tab =>
      tab.id === activeTabId
        ? { ...tab, showDiff: !tab.showDiff }
        : tab
    ));
  };

  const activeTab = tabs.find(tab => tab.id === activeTabId);
  const fileData = activeTab ? context.openFiles.get(activeTab.relativePath) : null;
  const fileReviews = activeTab ? context.session.fileReviews.get(activeTab.relativePath) : null;

  return (
    <div className="editor-panel">
      <TabBar
        tabs={tabs}
        activeTabId={activeTabId}
        onTabClick={handleTabClick}
        onTabClose={closeTab}
      />

      {activeTab ? (
        <>
          <div className="editor-header">
            <span className="editor-filename">{activeTab.fileName}</span>
            {activeTab.viewMode === 'ready' && fileData?.diff && (
              <button onClick={toggleViewMode} className="editor-view-toggle-btn">
                {activeTab.showDiff ? 'Show Code' : 'Show Diff'}
              </button>
            )}
          </div>

          <div className="editor-container">
            {activeTab.viewMode === 'loading' ? (
              <div className="editor-placeholder">
                <p>Loading...</p>
              </div>
            ) : activeTab.viewMode === 'error' ? (
              <div className="editor-placeholder">
                <p className="error-message">Failed to load file</p>
              </div>
            ) : activeTab.viewMode === 'ready' && fileData ? (
              fileData.diff && activeTab.showDiff ? (
                <DiffEditor
                  originalContent={fileData.diff}
                  modifiedContent={fileData.content}
                  filename={activeTab.fileName}
                  path={activeTab.relativePath}
                  lineReviews={fileReviews}
                  readOnly={context.mode !== 'commit'}
                />
              ) : (
                <CodeEditor
                  filename={activeTab.fileName}
                  content={fileData.content}
                />
              )
            ) : null}
          </div>
        </>
      ) : (
        <div className="editor-placeholder">
          <p>Select a file to view its contents</p>
        </div>
      )}
    </div>
  );
}

export default React.memo(EditorPanel);