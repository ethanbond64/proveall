# Bulk Commit Review Plan

## Overview

Allow users to select a range of unreviewed commits from the project page and review them as a single bulk review. Instead of reviewing commits one-by-one, clicking on any unreviewed commit above the current `head_event_id` commit selects all commits between it and the oldest unreviewed commit, highlighting the range. The diff shown covers the entire range, and a single review event is created that advances `head_event_id` past all included commits.

---

## Current Architecture (Summary)

- **ProjectPage** shows commits newest-first. The `oldestUnreviewedCommit` is the only clickable commit. Clicking it navigates to `ReviewProjectPage` with a single `commit` SHA.
- **ReviewContext** holds `commit` (a single SHA). `getReviewFileSystemData` diffs `commit^ vs commit`. `getReviewFileData` gets file content at `commit` and diff content at `commit^`.
- **Backend `create_event`** creates one event row per `commit` SHA, propagates xrefs from the previous event (found by looking up the parent commit's hash), and advances `branch_context.head_event_id`.
- The diff source in commit mode is always `commit^ vs commit` — a single commit's changes.

---

## Design

### Core Concept

A bulk commit review uses the diff between the **base commit** (the commit at `head_event_id`, i.e. the last reviewed commit) and the **target commit** (the most recent commit the user selected). This is conceptually `git diff <base>...<target>` — the aggregate of all changes across multiple commits.

The event created will be associated with the **target commit** hash, but the backend will know the diff was computed against a specific base commit (not just `target^`).

### What Changes

| Layer | File | Change |
|-------|------|--------|
| Frontend | `ProjectPage.jsx` | Allow clicking any unreviewed commit above `oldestUnreviewedCommit`. Track selected range. Highlight selected commits. Update diff source buttons to show range info. |
| Frontend | `App.jsx` | Pass `baseCommit` through `reviewContext` alongside `commit`. |
| Frontend | `ReviewContext.jsx` | Accept and store `baseCommit`. Pass it to all backend API calls. |
| Frontend | `tauriAPI.js` | Add `baseCommit` parameter to `getReviewFileSystemData`, `getReviewFileData`, and `createEvent`. |
| Backend | `review_commands.rs` | Accept `base_commit` parameter in review commands. |
| Backend | `review_service.rs` | Use `base_commit` instead of `commit^` for commit-mode diffs. |
| Backend | `event_service.rs` | Accept `base_commit` for `create_event`. Use it to find the previous event and propagate xrefs. Create intermediate event rows for skipped commits. |

---

## Implementation Steps

### Step 1: ProjectPage — Commit Selection UI

**File: `src/renderer/pages/project/ProjectPage.jsx`**

1. Add state: `selectedTargetCommit` (the commit the user clicked, or null).

2. Change `handleCommitClick` logic:
   - Any **unreviewed commit** (where `event.id === null && event.event_type === 'commit'`) that is at or above `oldestUnreviewedCommit` in the list is clickable.
   - Clicking a commit sets `selectedTargetCommit` to that commit.
   - If the user clicks `oldestUnreviewedCommit` itself, it behaves as today (single commit review, `selectedTargetCommit = null` or same as oldest).

3. Compute `selectedRange`: all commits between `oldestUnreviewedCommit` and `selectedTargetCommit` (inclusive). Since commits are ordered newest-first, this is the slice from `selectedTargetCommit`'s index down to `oldestUnreviewedCommit`'s index.

4. Add CSS class `commit-item-selected` to commits in the selected range — a highlight/border that visually groups them.

5. Update the **Review Actions** panel:
   - When `selectedTargetCommit` is set and differs from `oldestUnreviewedCommit`, show a button: **"Review N Commits"** with sublabel showing the range (`abc1234..def5678`).
   - The button calls `onNavigateToReview(selectedTargetCommit.commit, COMMIT_REVIEW_MODE, null, oldestUnreviewedCommit.commit)` — passing the base commit (parent of oldest unreviewed, i.e., the last reviewed commit's hash).
   - When `selectedTargetCommit` is null or equals `oldestUnreviewedCommit`, show the existing "Review Next Commit" button.

6. Clicking a commit that is already selected deselects it (resets to null).

### Step 2: App.jsx — Pass baseCommit Through

**File: `src/renderer/App.jsx`**

1. Update `handleNavigateToReview` signature to accept an optional `baseCommit` parameter:
   ```js
   const handleNavigateToReview = (commit, mode, issueId = null, baseCommit = null) => {
     setReviewContext({ commit, issueId, branchContextId, baseCommit });
     ...
   };
   ```

2. Pass `reviewContext.baseCommit` to `ReviewProjectPage` as a prop.

### Step 3: ReviewContext — Store and Use baseCommit

**File: `src/renderer/pages/review/ReviewContext.jsx`**

1. Add `baseCommit` to `initialState` (default `null`).

2. Accept `baseCommit` as a prop in `ReviewContextProvider`.

3. In the `INITIALIZE` action, store `baseCommit`.

4. Pass `baseCommit` to all API calls:
   - `getReviewFileSystemData(projectId, commit, issueId, mode, branchContextId, baseCommit)`
   - `getReviewFileData(projectId, commit, issueId, mode, filePath, branchContextId, baseCommit)`
   - `createEvent(projectId, commit, eventType, newIssues, resolvedIssues, branchContextId, baseCommit)`

When `baseCommit` is `null`, the backend falls back to existing behavior (`commit^`).

### Step 4: Tauri API Bindings

**File: `src/renderer/tauriAPI.js`**

Add `baseCommit` parameter to:
- `getReviewFileSystemData` → `invoke('get_review_file_system_data', { ..., baseCommit })`
- `getReviewFileData` → `invoke('get_review_file_data', { ..., baseCommit })`
- `createEvent` → `invoke('create_event', { ..., baseCommit })`

### Step 5: Backend Review Commands

**File: `tauri_src/src/commands/review_commands.rs`**

Add `base_commit: Option<String>` parameter to:
- `get_review_file_system_data`
- `get_review_file_data`

Pass through to the service layer.

### Step 6: Backend Review Service — Bulk Diff

**File: `tauri_src/src/services/review_service.rs`**

1. In `build_commit_touched_files`:
   - If `base_commit` is `Some(bc)`, diff `bc..commit` instead of `commit^..commit`.
   - If `base_commit` is `None`, keep existing behavior (`commit^..commit`).

2. In `get_review_file_data` (commit mode):
   - If `base_commit` is `Some(bc)`, the `diff` content should be `git show <bc>:<path>` instead of `git show <commit>^:<path>`.
   - If `base_commit` is `None`, keep existing behavior.

3. Remove the "event already exists" error in `build_commit_touched_files` for the bulk case — when reviewing a range, the target commit won't have an event yet, but intermediate commits also won't. This check is still valid for single-commit mode.

### Step 7: Backend Event Service — Bulk Event Creation

**File: `tauri_src/src/services/event_service.rs`**

1. Add `base_commit: Option<String>` parameter to `create_event`.

2. In `find_previous_event` (for commit events):
   - If `base_commit` is `Some(bc)`, look up the event matching hash `bc` instead of `commit^`.
   - If `base_commit` is `None`, keep existing behavior (look up `commit^`).

3. The single event created will have `hash = target_commit`. The xref propagation uses the base commit's event to carry forward unresolved issues, translating line numbers across the full diff range (`base_commit..target_commit`).

4. After the event is created and `head_event_id` is updated, all the intermediate commits are effectively "skipped" — they don't get individual events. The project state page will see them as unreviewed in the `events` table, but since `head_event_id` has advanced past them, they're covered by the bulk review.

### Step 8: Project State — Handle Bulk-Reviewed Commits

**File: `tauri_src/src/services/project_service.rs`**

1. In `build_event_entries`, the current logic marks commits as reviewed only if they have a matching event row. After a bulk review, intermediate commits won't have event rows.

2. Change the logic: a commit is considered "reviewed" if:
   - It has an event row (existing behavior), OR
   - It is **older than** the commit associated with `head_event_id` (i.e., it was covered by a bulk review that advanced past it).

   Implementation: After fetching events, also fetch the `head_event_id` event to get its commit hash. Any commit that appears after (older than) that hash in the git log is implicitly reviewed.

3. Update `oldestUnreviewedCommit` derivation on the frontend: this should already work correctly since the backend will now mark covered commits as reviewed.

---

## Detailed Data Flow (Bulk Review of 3 Commits)

```
Commit list (newest first):
  C5 (unreviewed)   ← user clicks here (selectedTargetCommit)
  C4 (unreviewed)   ← included in range
  C3 (unreviewed)   ← oldestUnreviewedCommit (base is C2)
  C2 (reviewed)     ← head_event_id points to C2's event
  C1 (reviewed)
```

1. User clicks C5. `selectedRange = [C5, C4, C3]`. Highlight applied.
2. Diff source button: "Review 3 Commits (C3..C5)". Click navigates with `commit=C5, baseCommit=C2`.
3. `ReviewContext` calls `getReviewFileSystemData(projectId, C5, null, 'commit', bcId, C2)`.
4. Backend diffs `C2..C5` to get all touched files.
5. User reviews files. Monaco shows `git show C2:<path>` as original, `git show C5:<path>` as modified.
6. Save calls `createEvent(projectId, C5, 'commit', newIssues, resolved, bcId, C2)`.
7. Backend creates event with `hash=C5`, propagates xrefs from C2's event, translates lines using diff `C2..C5`, updates `head_event_id` to the new event.
8. Back on ProjectPage, `getProjectState` returns: C5 has event row (reviewed), C4/C3 are older than head's commit so marked reviewed.

---

## CSS Additions

```css
.commit-item-selected {
  border-left: 3px solid #569cd6;
  background-color: rgba(86, 156, 214, 0.08);
}

.commit-item-in-range {
  border-left: 3px solid rgba(86, 156, 214, 0.4);
  background-color: rgba(86, 156, 214, 0.04);
}
```

---

## Edge Cases

1. **Single commit selected** (clicking `oldestUnreviewedCommit`): `baseCommit` is `null`, existing single-commit flow is used unchanged.

2. **No previous event exists** (first review ever): `baseCommit` should be the merge-base with the base branch. The frontend computes this: if `oldestUnreviewedCommit` is the very first commit in the range, `baseCommit` is `null` and the backend uses `commit^` or the base branch as appropriate.

3. **Deselection**: Clicking a selected commit deselects it. Clicking a different commit changes the selection.

4. **Only one unreviewed commit**: No change to current behavior — bulk selection UI only appears when there are 2+ unreviewed commits.

---

## Files Modified (Summary)

| File | Type of Change |
|------|---------------|
| `src/renderer/pages/project/ProjectPage.jsx` | Selection state, highlight, range button |
| `src/renderer/App.jsx` | Pass `baseCommit` in reviewContext |
| `src/renderer/pages/review/ReviewContext.jsx` | Store `baseCommit`, pass to APIs |
| `src/renderer/pages/review/ReviewProjectPage.jsx` | Pass `baseCommit` prop to provider |
| `src/renderer/tauriAPI.js` | Add `baseCommit` to 3 API calls |
| `src/renderer/styles.css` | Selection highlight styles |
| `tauri_src/src/commands/review_commands.rs` | Add `base_commit` param |
| `tauri_src/src/commands/event_commands.rs` | Add `base_commit` param |
| `tauri_src/src/services/review_service.rs` | Use `base_commit` for diff range |
| `tauri_src/src/services/event_service.rs` | Use `base_commit` for prev event lookup |
| `tauri_src/src/services/project_service.rs` | Mark bulk-covered commits as reviewed |
