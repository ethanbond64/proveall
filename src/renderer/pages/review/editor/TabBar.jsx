import React from 'react';

/**
 * TabBar - Displays horizontal tabs for open files
 */
function TabBar({ tabs, activeTabId, onTabClick, onTabClose }) {
  if (tabs.length === 0) {
    return null;
  }

  return (
    <div className="tab-bar">
      {tabs.map((tab) => (
        <div
          key={tab.id}
          className={`tab ${tab.id === activeTabId ? 'active' : ''}`}
          onClick={() => onTabClick(tab.id)}
        >
          <span className="tab-name">{tab.fileName}</span>
          {tab.viewMode === 'diff' && tab.diffData?.status && (
            <span className={`tab-status-indicator status-${tab.diffData.status}`}>
              {tab.diffData.status}
            </span>
          )}
          <button
            className="tab-close-btn"
            onClick={(e) => {
              e.stopPropagation();
              onTabClose(tab.id);
            }}
            title="Close"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}

export default TabBar;
