// Review Mode Constants
export const REVIEW_MODES = {
  COMMIT_REVIEW: 'commit',
  BRANCH_COMPARISON: 'branch',
  MERGE_REVIEW: 'merge_review'
};

// Export individual constants for convenience
export const COMMIT_REVIEW_MODE = REVIEW_MODES.COMMIT_REVIEW;
export const BRANCH_COMPARISON_MODE = REVIEW_MODES.BRANCH_COMPARISON;
export const MERGE_REVIEW_MODE = REVIEW_MODES.MERGE_REVIEW;

/// Returns true for review modes where the user can interactively review
/// files, create issues, and save events (commit review & merge review).
/// Returns false for read-only modes (branch comparison).
export function isInteractiveReviewMode(mode) {
  return mode === COMMIT_REVIEW_MODE || mode === MERGE_REVIEW_MODE;
}