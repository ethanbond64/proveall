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