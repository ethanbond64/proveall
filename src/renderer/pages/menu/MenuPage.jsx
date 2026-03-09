import React, { useState, useEffect } from 'react';
import '../../styles.css';
import logoImage from '../../Square310x310Logo.png';

function MenuPage({ onProjectSelected }) {
  const [recentProjects, setRecentProjects] = useState([]);
  const [openMenuProjectId, setOpenMenuProjectId] = useState(null);

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = () => setOpenMenuProjectId(null);
    document.addEventListener('click', handleClickOutside);
    return () => document.removeEventListener('click', handleClickOutside);
  }, []);

  // Load recent projects on mount
  useEffect(() => {
    const loadRecentProjects = async () => {
      const projects = await window.backendAPI.projectsFetch(5);
      setRecentProjects(projects);
    };
    loadRecentProjects();
  }, []);

  const projectSelect = async (projectPath) => {
    // Call projectsOpen to update lastOpenedAt and get the project ID
    const projectResult = await window.backendAPI.projectsOpen(projectPath);
    const project = { id: projectResult.id, path: projectPath };

    // Notify parent component
    onProjectSelected(project);
  };

  const handleOpenProject = async () => {
    // Launches file picker window
    window.backendAPI.openDirectory().then(path => {
      if (path) {
        projectSelect(path)
      }
    });
  };

  return (
    <div className="menu-page">
      {recentProjects && recentProjects.length > 0 && (
        <div className="recent-projects">
          <h3 className="recent-projects-header">Recent Projects</h3>
          <div className="recent-projects-list">
            {recentProjects.map((project) => (
              <div
                key={project.id}
                className="recent-project-item"
                onClick={() => projectSelect(project.path)}
              >
                <div className="recent-project-info">
                  <div className="recent-project-name">{project.name}</div>
                  <div className="recent-project-path">{project.path}</div>
                </div>
                <div className="recent-project-menu-container">
                  <button
                    className="recent-project-menu-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      setOpenMenuProjectId(openMenuProjectId === project.id ? null : project.id);
                    }}
                  >
                    ...
                  </button>
                  {openMenuProjectId === project.id && (
                    <div className="recent-project-menu-popup">
                      <button
                        className="recent-project-menu-popup-item delete"
                        onClick={(e) => {
                          e.stopPropagation();
                          // TODO: implement delete project
                        }}
                      >
                        Delete Project
                      </button>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      <div className="menu-page-content">
        <img src={logoImage} alt="PR Tool Logo" className="menu-page-logo" />
        <h2 className="menu-page-title">Welcome to ProveAll</h2>
        <p className="menu-page-subtitle">Open a project to get started</p>
        <button onClick={handleOpenProject} className="menu-page-open-btn">
          Open Project
        </button>
      </div>
    </div>
  );
}

export default MenuPage;
