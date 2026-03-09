import React, { createContext, useContext, useReducer, useEffect, useMemo, useCallback } from 'react';
import {BRANCH_COMPARISON_MODE, COMMIT_REVIEW_MODE, isInteractiveReviewMode} from "../../constants";
import {hasIssues} from "../../utils/reviewUtils";

// Create the context
const ReviewContext = createContext(null);

// Initial state structure
const initialState = {
  // Core State (from getReviewFileSystemData)
  mode: COMMIT_REVIEW_MODE, // 'commit' | 'branch' | 'merge_review'
  projectId: null,
  projectPath: '',
  commit: '',
  issueId: null,
  branchContextId: null,

  // File system state
  touchedFiles: new Map(), // Map(path => { name, path, diffMode, state })

  // Issues from backend
  issues: new Map(), // Map(id => { id, comment })

  // Session state (commit mode only, in-memory)
  session: {
    // New issues created during this session
    newIssues: [], // Array of { tempId, comment, reviews }

    // Issue IDs marked as resolved
    resolvedIssueIds: new Set(),

    // Current review state per file
    fileReviews: new Map(), // Map(path => { defaultState, lineRanges })
  },

  // File content cache (from getReviewFileData)
  openFiles: new Map(), // Map(path => { content, diff, lineSummary, issues })

  // Diff change blocks (from Monaco diff editor)
  changeBlocks: new Map(), // Map(path => [{ startLine, endLine }])

  // Loading states
  isLoading: false,
  error: null,
};

// Action types
const ActionTypes = {
  INITIALIZE: 'INITIALIZE',
  SET_FILE_DATA: 'SET_FILE_DATA',
  SET_LINE_REVIEW: 'SET_LINE_REVIEW',
  SET_FILE_DEFAULT: 'SET_FILE_DEFAULT',
  SET_CHANGE_BLOCKS: 'SET_CHANGE_BLOCKS',
  CREATE_ISSUE: 'CREATE_ISSUE',
  UPDATE_ISSUE: 'UPDATE_ISSUE',
  DELETE_ISSUE: 'DELETE_ISSUE',
  RESOLVE_ISSUE: 'RESOLVE_ISSUE',
  UNRESOLVE_ISSUE: 'UNRESOLVE_ISSUE',
  CLEAR_SESSION: 'CLEAR_SESSION',
  SET_LOADING: 'SET_LOADING',
  SET_ERROR: 'SET_ERROR',
};

// Reducer function
function reviewReducer(state, action) {
  switch (action.type) {
    case ActionTypes.INITIALIZE: {
      const { mode, projectId, projectPath, commit, issueId, branchContextId, touchedFiles, issues } = action.payload;

      // Convert arrays to Maps for efficient lookups
      const touchedFilesMap = new Map();
      if (touchedFiles && Array.isArray(touchedFiles)) {
        touchedFiles.forEach(file => {
          touchedFilesMap.set(file.path, file);
        });
      }

      const issuesMap = new Map();
      if (issues && Array.isArray(issues)) {
        issues.forEach(issue => {
          issuesMap.set(issue.id, issue);
        });
      }

      // Auto-approve merge-only files in interactive modes
      const fileReviews = new Map(state.session.fileReviews);
      if (isInteractiveReviewMode(mode)) {
        touchedFilesMap.forEach((file, path) => {
          if (file.mergeOnly) {
            fileReviews.set(path, {
              ...(fileReviews.get(path) || { lineRanges: [] }),
              defaultState: 'green',
            });
          }
        });
      }

      return {
        ...state,
        mode,
        projectId,
        projectPath,
        commit,
        issueId,
        branchContextId,
        touchedFiles: touchedFilesMap,
        issues: issuesMap,
        session: {
          ...state.session,
          fileReviews,
        },
        isLoading: false,
      };
    }

    case ActionTypes.SET_FILE_DATA: {
      const { path, data } = action.payload;
      const newOpenFiles = new Map(state.openFiles);
      newOpenFiles.set(path, data);

      // Parse lineSummary and merge into session.fileReviews.lineRanges
      const newFileReviews = new Map(state.session.fileReviews);

      if (data.lineSummary && data.lineSummary.length > 0) {
        const existingFileReview = newFileReviews.get(path) || { defaultState: null, lineRanges: [] };

        // Convert lineSummary entries to lineRanges format
        const lineRangesFromBackend = data.lineSummary.map(entry => ({
          start: entry.start,
          end: entry.end,
          state: entry.state,
          issueRef: entry.issueId ? String(entry.issueId) : null,
        }));

        // In branch mode, just use backend data directly (read-only)
        // In commit mode, merge with existing session data
        let finalLineRanges;
        if (state.mode === BRANCH_COMPARISON_MODE) {
          // Read-only mode: use backend data directly
          finalLineRanges = lineRangesFromBackend;
        } else {
          // Commit mode: merge backend with session data
          const mergedLineRanges = [...lineRangesFromBackend];

          // Add any existing lineRanges that don't overlap with backend data
          existingFileReview.lineRanges.forEach(existingRange => {
            const overlaps = lineRangesFromBackend.some(backendRange =>
              !(existingRange.end < backendRange.start || existingRange.start > backendRange.end)
            );
            if (!overlaps) {
              mergedLineRanges.push(existingRange);
            }
          });

          finalLineRanges = mergedLineRanges;
        }

        // Sort by start line
        finalLineRanges.sort((a, b) => a.start - b.start);

        newFileReviews.set(path, {
          ...existingFileReview,
          lineRanges: finalLineRanges,
        });
      }

      return {
        ...state,
        openFiles: newOpenFiles,
        session: {
          ...state.session,
          fileReviews: newFileReviews,
        },
      };
    }

    case ActionTypes.SET_LINE_REVIEW: {
      const { path, start, end, state: reviewState, issueRef } = action.payload;

      const newFileReviews = new Map(state.session.fileReviews);
      const existingFileReview = newFileReviews.get(path) || { defaultState: null, lineRanges: [] };

      // Create a new fileReview object with new lineRanges array
      const fileReview = {
        defaultState: existingFileReview.defaultState,
        defaultIssueRef: existingFileReview.defaultIssueRef,
        lineRanges: [...existingFileReview.lineRanges] // Create new array
      };

      // Find existing range or add new one
      const rangeIndex = fileReview.lineRanges.findIndex(
        range => range.start === start && range.end === end
      );

      if (rangeIndex >= 0) {
        // Update existing range
        fileReview.lineRanges[rangeIndex] = {
          start,
          end,
          state: reviewState,
          issueRef: issueRef || null,
        };
      } else {
        // Add new range
        fileReview.lineRanges.push({
          start,
          end,
          state: reviewState,
          issueRef: issueRef || null,
        });
      }

      // Sort ranges by start line
      fileReview.lineRanges.sort((a, b) => a.start - b.start);

      newFileReviews.set(path, fileReview);

      return {
        ...state,
        session: {
          ...state.session,
          fileReviews: newFileReviews,
        },
      };
    }

    case ActionTypes.SET_FILE_DEFAULT: {
      const { path, state: defaultState, issueRef } = action.payload;

      const newFileReviews = new Map(state.session.fileReviews);
      const fileReview = newFileReviews.get(path) || { defaultState: null, lineRanges: [] };

      fileReview.defaultState = defaultState;
      fileReview.defaultIssueRef = issueRef || null;

      newFileReviews.set(path, fileReview);

      return {
        ...state,
        session: {
          ...state.session,
          fileReviews: newFileReviews,
        },
      };
    }

    case ActionTypes.SET_CHANGE_BLOCKS: {
      const { path, blocks } = action.payload;

      const newChangeBlocks = new Map(state.changeBlocks);
      newChangeBlocks.set(path, blocks);

      return {
        ...state,
        changeBlocks: newChangeBlocks,
      };
    }

    case ActionTypes.CREATE_ISSUE: {
      const { tempId, comment } = action.payload;

      const newIssue = {
        tempId,
        comment,
        reviews: [],
      };

      return {
        ...state,
        session: {
          ...state.session,
          newIssues: [...state.session.newIssues, newIssue],
        },
      };
    }

    case ActionTypes.UPDATE_ISSUE: {
      const { id, comment } = action.payload;

      const newIssues = state.session.newIssues.map(issue => {
        if (issue.tempId === id) {
          return { ...issue, comment };
        }
        return issue;
      });

      return {
        ...state,
        session: {
          ...state.session,
          newIssues,
        },
      };
    }

    case ActionTypes.DELETE_ISSUE: {
      const { id } = action.payload;

      const newIssues = state.session.newIssues.filter(issue => issue.tempId !== id);

      // Also remove references from line reviews
      const newFileReviews = new Map();
      state.session.fileReviews.forEach((fileReview, path) => {
        const updatedFileReview = {
          ...fileReview,
          lineRanges: fileReview.lineRanges.map(range => {
            if (range.issueRef === id) {
              return { ...range, issueRef: null };
            }
            return range;
          }),
        };
        if (fileReview.defaultIssueRef === id) {
          updatedFileReview.defaultIssueRef = null;
        }
        newFileReviews.set(path, updatedFileReview);
      });

      return {
        ...state,
        session: {
          ...state.session,
          newIssues,
          fileReviews: newFileReviews,
        },
      };
    }

    case ActionTypes.RESOLVE_ISSUE: {
      const { id } = action.payload;
      const newResolvedIds = new Set(state.session.resolvedIssueIds);
      newResolvedIds.add(id);

      return {
        ...state,
        session: {
          ...state.session,
          resolvedIssueIds: newResolvedIds,
        },
      };
    }

    case ActionTypes.UNRESOLVE_ISSUE: {
      const { id } = action.payload;
      const newResolvedIds = new Set(state.session.resolvedIssueIds);
      newResolvedIds.delete(id);

      return {
        ...state,
        session: {
          ...state.session,
          resolvedIssueIds: newResolvedIds,
        },
      };
    }

    case ActionTypes.CLEAR_SESSION: {
      return {
        ...state,
        session: {
          newIssues: [],
          resolvedIssueIds: new Set(),
          fileReviews: new Map(),
        },
      };
    }

    case ActionTypes.SET_LOADING: {
      return {
        ...state,
        isLoading: action.payload,
      };
    }

    case ActionTypes.SET_ERROR: {
      return {
        ...state,
        error: action.payload,
        isLoading: false,
      };
    }

    default:
      return state;
  }
}

// Provider component
export function ReviewContextProvider({ children, mode, projectId, projectPath, commit, issueId, branchContextId }) {
  const [state, dispatch] = useReducer(reviewReducer, initialState);

  // Initialize context with backend data
  useEffect(() => {
    const loadInitialData = async () => {
      if (!projectId || !branchContextId) return;

      dispatch({ type: ActionTypes.SET_LOADING, payload: true });

      try {
        // Load file system data from backend
        const data = await window.backendAPI.getReviewFileSystemData(
          projectId,
          commit,
          issueId,
          mode,
          branchContextId
        );

        dispatch({
          type: ActionTypes.INITIALIZE,
          payload: {
            mode,
            projectId,
            projectPath,
            commit,
            issueId,
            branchContextId,
            touchedFiles: data?.touchedFiles || [],
            issues: data?.issues || [],
          },
        });
      } catch (error) {
        console.error('Failed to initialize review context:', error);
        dispatch({ type: ActionTypes.SET_ERROR, payload: error.message });
      }
    };

    loadInitialData();
  }, [mode, projectId, projectPath, commit, issueId, branchContextId]);

  // Action methods
  const actions = useMemo(() => ({
    loadFileData: async (path) => {
      try {
        const data = await window.backendAPI.getReviewFileData(
          projectId,
          commit,
          issueId,
          state.mode,
          path,
          branchContextId
        );

        dispatch({
          type: ActionTypes.SET_FILE_DATA,
          payload: { path, data },
        });
      } catch (error) {
        console.error('Failed to load file data:', error);
        dispatch({ type: ActionTypes.SET_ERROR, payload: error.message });
      }
    },

    updateLineReview: (path, start, end, reviewState, issueRef) => {
      dispatch({
        type: ActionTypes.SET_LINE_REVIEW,
        payload: { path, start, end, state: reviewState, issueRef },
      });
    },

    setFileDefault: (path, defaultState, issueRef) => {
      dispatch({
        type: ActionTypes.SET_FILE_DEFAULT,
        payload: { path, state: defaultState, issueRef },
      });
    },

    setChangeBlocks: (path, blocks) => {
      dispatch({
        type: ActionTypes.SET_CHANGE_BLOCKS,
        payload: { path, blocks },
      });
    },

    createIssue: (comment) => {
      const tempId = `temp-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
      dispatch({
        type: ActionTypes.CREATE_ISSUE,
        payload: { tempId, comment },
      });
      return tempId;
    },

    updateIssue: (id, comment) => {
      dispatch({
        type: ActionTypes.UPDATE_ISSUE,
        payload: { id, comment },
      });
    },

    deleteIssue: (id) => {
      dispatch({
        type: ActionTypes.DELETE_ISSUE,
        payload: { id },
      });
    },

    resolveIssue: (id) => {
      dispatch({
        type: ActionTypes.RESOLVE_ISSUE,
        payload: { id },
      });
    },

    unresolveIssue: (id) => {
      dispatch({
        type: ActionTypes.UNRESOLVE_ISSUE,
        payload: { id },
      });
    },

    saveResolution: async (issueId) => {
      try {
        // Call the API to create a resolution event
        const eventId = await window.backendAPI.createEvent(
          state.projectId,
          state.commit,
          'resolution',  // eventType
          [],  // newIssues (empty for resolution)
          [issueId],  // resolvedIssues (single issue)
          state.branchContextId
        );

        console.log(`Issue ${issueId} resolved successfully. Event ID: ${eventId}`);
        return eventId;
      } catch (error) {
        console.error(`Failed to resolve issue ${issueId}:`, error);
        throw error;
      }
    },

    refreshData: async () => {
      dispatch({ type: ActionTypes.SET_LOADING, payload: true });

      try {

        // Reload file system data from backend
        const data = await window.backendAPI.getReviewFileSystemData(
          state.projectId,
          state.commit,
          state.issueId,
          state.mode,
          state.branchContextId
        );

        dispatch({
          type: ActionTypes.INITIALIZE,
          payload: {
            mode: state.mode,
            projectId: state.projectId,
            projectPath: state.projectPath,
            commit: state.commit,
            issueId: state.issueId,
            branchContextId: state.branchContextId,
            touchedFiles: data?.touchedFiles || [],
            issues: data?.issues || [],
          },
        });
      } catch (error) {
        console.error('Failed to refresh review data:', error);
        dispatch({ type: ActionTypes.SET_ERROR, payload: error.message });
        throw error;
      }
    },

    saveReview: async () => {
      // Build event payload from session state
      const newIssues = state.session.newIssues.map(issue => {
        // Collect reviews associated with this issue
        const reviews = [];

        // Check file reviews for references to this issue
        state.session.fileReviews.forEach((fileReview, path) => {
          // Check default review
          if (fileReview.defaultIssueRef === issue.tempId) {
            reviews.push({
              type: 'file',
              path,
              start: null,
              end: null,
              state: fileReview.defaultState,
            });
          }

          // Check line ranges
          fileReview.lineRanges.forEach(range => {
            if (range.issueRef === issue.tempId) {
              reviews.push({
                type: 'line',
                path,
                start: range.start,
                end: range.end,
                state: range.state,
              });
            }
          });
        });

        return {
          comment: issue.comment,
          reviews,
        };
      });

      const resolvedIssues = Array.from(state.session.resolvedIssueIds);

      try {
        // Validate based on mode
        if (state.mode === BRANCH_COMPARISON_MODE && state.issueId) {
          // In branch mode with issueId, we're primarily resolving/unresolving issues
          if (resolvedIssues.length === 0 && newIssues.length === 0) {
            throw new Error('No changes to save for this issue');
          }
        } else if (isInteractiveReviewMode(state.mode)) {
          // In commit mode, we don't enforce validation here
          // The save button should be disabled if not complete
          // This is just a safety check
          if (newIssues.length === 0 && resolvedIssues.length === 0 &&
              state.session.fileReviews.size === 0) {
            throw new Error('No review data to save');
          }
        }

        // Call the API to create the event
        const eventType = (state.mode === BRANCH_COMPARISON_MODE && state.issueId) ? 'resolution' : 'commit';
        const eventId = await window.backendAPI.createEvent(
          state.projectId,
          state.commit,
          eventType,
          newIssues,
          resolvedIssues,
          state.branchContextId
        );

        // Clear session state after successful save
        dispatch({ type: ActionTypes.CLEAR_SESSION });

        console.log(`Review saved successfully. Event ID: ${eventId}, Type: ${eventType}`);
        return eventId;
      } catch (error) {
        console.error('Failed to save review:', error);
        throw error;
      }
    },
  }), [projectId, commit, issueId, state.mode, state.session, state.projectPath, state.branchContextId, branchContextId]);


  // Computed values
  const computedValues = useMemo(() => {
    // Calculate progress state for each file
    const fileProgress = new Map();

    for (const [path, file] of state.touchedFiles) {
      const review = state.session.fileReviews.get(path);
      const changeBlocks = state.changeBlocks.get(path) || [];

      let progressState = 'untouched';

      // Deleted files are always complete
      if (file.diffMode === 'D') {
        progressState = 'complete';
      }
      // If file has default state, it's complete
      else if (review?.defaultState) {
        progressState = 'complete';
      }
      // Check if all change blocks are covered by line reviews
      else if (changeBlocks.length > 0 && review?.lineRanges?.length > 0) {
        const allBlocksCovered = changeBlocks.every(block => {
          // Check if this block is covered by any line review range
          for (const range of review.lineRanges) {
            // Block is covered if review range fully contains the block
            if (range.start <= block.startLine && range.end >= block.endLine) {
              return true;
            }
          }
          return false;
        });

        if (allBlocksCovered) {
          progressState = 'complete';
        } else {
          progressState = 'in-progress';
        }
      }
      // Has some line reviews but not all blocks covered
      else if (review?.lineRanges?.length > 0) {
        progressState = 'in-progress';
      }
      // No reviews at all
      else {
        progressState = 'untouched';
      }

      fileProgress.set(path, {
        progressState,
        defaultState: review?.defaultState || null,
        hasLineReviews: (review?.lineRanges?.length || 0) > 0,
        changeBlocksCount: changeBlocks.length,
        reviewedBlocksCount: changeBlocks.filter(block => {
          if (!review?.lineRanges) return false;
          return review.lineRanges.some(range =>
            range.start <= block.startLine && range.end >= block.endLine
          );
        }).length,
      });
    }

    // Check if all files are reviewed (for commit mode)
    const isComplete = (() => {
      if (!isInteractiveReviewMode(state.mode)) return false;

      // Check each file's progress
      for (const [path, progress] of fileProgress) {
        if (progress.progressState !== 'complete') {
          return false;
        }
      }

      // All non-green reviews must have issues
      for (const [path, review] of state.session.fileReviews) {
        // Check default review
        if (review.defaultState && hasIssues(review.defaultState) && !review.defaultIssueRef) {
          return false;
        }

        // Check line range reviews
        for (const range of review.lineRanges) {
          if (hasIssues(range.state) && !range.issueRef) {
            return false;
          }
        }
      }

      return true;
    })();

    // Calculate review summary
    const reviewSummary = (() => {
      const totalFiles = state.touchedFiles.size;
      let filesReviewed = 0;

      for (const [path, progress] of fileProgress) {
        if (progress.progressState === 'complete') {
          filesReviewed++;
        }
      }

      return {
        totalFiles,
        filesReviewed,
      };
    })();

    return {
      isComplete,
      reviewSummary,
      fileProgress,
    };
  }, [state.mode, state.touchedFiles, state.session.fileReviews, state.changeBlocks]);

  // Context value
  const value = {
    ...state,
    ...computedValues,
    actions,
  };

  return (
    <ReviewContext.Provider value={value}>
      {children}
    </ReviewContext.Provider>
  );
}

// Custom hook to use the context
export function useReviewContext() {
  const context = useContext(ReviewContext);
  if (!context) {
    throw new Error('useReviewContext must be used within a ReviewContextProvider');
  }
  return context;
}

export default ReviewContext;