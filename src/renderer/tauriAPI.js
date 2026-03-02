import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

// Quick Tauri wrapper to replace window.electronAPI
window.backendAPI = {
  projectsFetch: (limit) => invoke('fetch_projects', {limit}),
  projectsOpen: (path) => invoke('open_project', { path }).then(p => ({ id: p.id })),

  getCurrentBranch: (projectId) => invoke('get_current_branch', { projectId }),
  createBranchContext: (projectId, branch, baseBranch, settings = '{}') =>
    invoke('create_branch_context', { projectId, branch, baseBranch, settings }),

  getProjectState: (projectId, branchContextId) => invoke('get_project_state', { projectId, branchContextId }),
  getReviewFileSystemData: (projectId, commit, issueId, reviewType, branchContextId) =>
    invoke('get_review_file_system_data', { projectId, commit, issueId, reviewType, branchContextId }),
  getReviewFileData: (projectId, commit, issueId, reviewType, filePath, branchContextId) =>
    invoke('get_review_file_data', { projectId, commit, issueId, reviewType, filePath, branchContextId }),

  createEvent: (projectId, commit, eventType, newIssues, resolvedIssues, branchContextId) =>
    invoke('create_event', { projectId, commit, eventType, newIssues, resolvedIssues, branchContextId }),

  fixIssue: (projectId, issueId, branchContextId) => invoke('fix_issue', { projectId, issueId, branchContextId }),

  getDirectory: (projectId, directoryPath) => invoke('get_directory', { projectId, directoryPath }),
  readDirectory: (path) => invoke('get_directory', { projectId: 1, directoryPath: path }), // TODO Fallback with dummy projectId

  openDirectory: () => open({ directory: true, multiple: false }),
};