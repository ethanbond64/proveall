/**
 * Utility functions for review status checks
 */

/**
 * Check if a file or review status is approved
 * @param {string} state - The state/status to check
 * @returns {boolean} - True if the state represents an approved status
 */
export function isApproved(state) {
  return state === 'green';
}

/**
 * Check if a file or review status is not approved (has issues)
 * @param {string} state - The state/status to check
 * @returns {boolean} - True if the state represents a non-approved status
 */
export function hasIssues(state) {
  return state !== 'green';
}

/// Base-branch merge that is auto-reviewed (no conflict resolutions to inspect).
export function isAutoMerge(event) {
  return event.is_base_merge && !event.has_conflict_changes;
}

/// Base-branch merge with conflict resolutions that require manual review.
export function isConflictedMerge(event) {
  return event.is_base_merge && event.has_conflict_changes;
}