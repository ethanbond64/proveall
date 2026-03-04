# Bulk Commit Review Plan

## Overview

Allow users to select a range of unreviewed commits from the project page and review them as a single bulk review. Instead of reviewing commits one-by-one, clicking on any unreviewed commit above the current `head_event_id` commit selects all commits between it and the oldest unreviewed commit, highlighting the range. The diff shown covers the entire range. On save, the backend creates an event row for every commit in the range (intermediate commits get events with no issues; the target commit's event holds all issues), propagating xrefs through each in sequence, and advances `head_event_id` to the target commit's event.

---

## Current Architecture (Summary)

- **ProjectPage** shows commits newest-first. The `oldestUnreviewedCommit` is the only clickable commit. Clicking it navigates to `ReviewProjectPage` with a single `commit` SHA.
- **ReviewContext** holds `commit` (a single SHA). `getReviewFileSystemData` diffs `commit^ vs commit`. `getReviewFileData` gets file content at `commit` and diff content at `commit^`.
- **Backend `create_event`** creates one event row per `commit` SHA, propagates xrefs from the previous event (found by looking up the parent commit's hash), and advances `branch_context.head_event_id`.
- The diff source in commit mode is always `commit^ vs commit` — a single commit's changes.

---

## Design

### Core Concept

A bulk commit review uses the diff between the **base commit** and the **target commit** (the most recent commit the user selected). The backend determines the base commit automatically:

- If `head_event_id` exists on the `BranchContext`, the base commit is the **commit hash stored on that event** (the last reviewed commit).
- If `head_event_id` is `None` (no commits reviewed yet), the base commit is the **base branch** (e.g., `main`).

This means the backend already has all the information it needs — no `baseCommit` parameter needs to be passed through the API. The frontend tracks `baseCommit` only for UI display purposes (showing the range in the diff source button).

For single-commit review, this is equivalent to the old behavior: the last reviewed commit is `commit^`, so diffing against `head_event_id`'s commit produces the same result as `commit^ vs commit`.

### What Changes

| Layer | File | Change |
|-------|------|--------|
| Frontend | `ProjectPage.jsx` | Allow clicking any unreviewed commit above `oldestUnreviewedCommit`. Track selected range. Highlight selected commits. Update diff source buttons to show range info. |
| Frontend | `App.jsx` | Pass `baseCommit` through `reviewContext` alongside `commit` (for UI display only). |
| Frontend | `ReviewContext.jsx` | Accept and store `baseCommit` (for UI display only). API call signatures unchanged. |
| Frontend | `ReviewProjectPage.jsx` | Pass `baseCommit` prop to provider. |
| Backend | `review_service.rs` | In commit mode, derive base from `head_event_id`'s commit hash (or base branch) instead of hardcoding `commit^`. |
| Backend | `event_service.rs` | In `find_previous_event`, use `head_event_id` to find the previous event instead of looking up `commit^`. Create intermediate event rows with xref propagation for bulk reviews. |

---

## Implementation Steps

### Step 1: ProjectPage — Commit Selection UI (DONE)

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
   - The button calls `onNavigateToReview(selectedTargetCommit.commit, COMMIT_REVIEW_MODE, null, baseCommitForBulk)` — passing the base commit for display purposes.
   - When `selectedTargetCommit` is null or equals `oldestUnreviewedCommit`, show the existing "Review Next Commit" button.

6. Clicking a commit that is already selected deselects it (resets to null).

### Step 2: App.jsx — Pass baseCommit Through (DONE)

**File: `src/renderer/App.jsx`**

1. Update `handleNavigateToReview` signature to accept an optional `baseCommit` parameter:
   ```js
   const handleNavigateToReview = (commit, mode, issueId = null, baseCommit = null) => {
     setReviewContext({ commit, issueId, branchContextId, baseCommit });
     ...
   };
   ```

2. Pass `reviewContext.baseCommit` to `ReviewProjectPage` as a prop.

### Step 3: ReviewContext — Store baseCommit (DONE)

**File: `src/renderer/pages/review/ReviewContext.jsx`**

1. Add `baseCommit` to `initialState` (default `null`).

2. Accept `baseCommit` as a prop in `ReviewContextProvider`.

3. In the `INITIALIZE` action, store `baseCommit`.

Note: `baseCommit` is stored for frontend display only. The backend API signatures are unchanged — the backend derives the base commit from `head_event_id` on the `BranchContext`.

### Step 4: Tauri API Bindings (NO CHANGES NEEDED)

**File: `src/renderer/tauriAPI.js`**

No changes. The backend determines the base commit automatically from `head_event_id` on the `BranchContext`. API signatures remain:
- `getReviewFileSystemData(projectId, commit, issueId, reviewType, branchContextId)`
- `getReviewFileData(projectId, commit, issueId, reviewType, filePath, branchContextId)`
- `createEvent(projectId, commit, eventType, newIssues, resolvedIssues, branchContextId)`

### Step 5: Backend Review Commands (NO CHANGES NEEDED)

**File: `tauri_src/src/commands/review_commands.rs`**

No changes. The command signatures stay the same since `base_commit` is not passed from the frontend.

### Step 6: Backend Review Service — Bulk Diff

**File: `tauri_src/src/services/review_service.rs`**

1. In `build_commit_touched_files`:
   - Look up `branch_context.head_event_id` → get the event → get its commit hash. This is the base commit.
   - If `head_event_id` exists, diff `base_commit..commit` instead of `commit^..commit`.
   - If `head_event_id` is `None` (first review ever), diff `base_branch..commit` using `branch_context.base_branch`.
   - Pass `base_commit` info through so `get_review_file_data` can use it too.

2. In `get_review_file_data` (commit mode):
   - Derive the base commit the same way (from `head_event_id` or `base_branch`).
   - The `diff` content should be `git show <base_commit>:<path>` instead of `git show <commit>^:<path>`.

3. Remove the "event already exists" error in `build_commit_touched_files` — when reviewing a range, the target commit won't have an event yet, but this check is overly restrictive.

### Step 7: Backend Event Service — find_previous_event (DONE)

**File: `tauri_src/src/services/event_service.rs`**

1. In `find_previous_event` (for commit events):
   - Instead of looking up `commit^`, look up the `head_event_id` from the `BranchContext` directly. This naturally handles both single and bulk reviews — the previous event is always the one pointed to by `head_event_id`.

### Step 7.5: Reorder create_event — Intermediates Before Target

**File: `tauri_src/src/services/event_service.rs`**

Restructure `create_event` so that **intermediate events are created before the target event**. The target event (with issues attached) should be the last event created.

#### New flow for `create_event`

1. **Validation** (unchanged).
2. **Find previous event** via `head_event_id` (unchanged from step 7).
3. **Enumerate intermediate commits** (if commit-type event and previous event exists):
   - Get the previous event's commit hash (`base_commit`).
   - Run `git log --format=%H <base_commit>..<target_commit>` to list all commits in the range (newest first).
   - Remove the target commit from the list, reverse to get chronological order (oldest first). These are the intermediates.
4. **Create intermediate events** in chronological order:
   - For each intermediate commit hash, create an event row (`type="commit"`, `hash=<hash>`, summary from git log).
   - Propagate xrefs from the previous event to this new event, translating line numbers via `git diff <prev_commit> <intermediate_commit>`.
   - Track the "current previous" event/commit — each intermediate becomes the previous for the next.
   - No issues are attached to intermediate events.
5. **Create the target event** (`hash=target_commit`), with summary from git log.
6. **Create issues with reviews** — attached to the target event.
7. **Mark resolved issues**.
8. **Propagate xrefs** from the last intermediate event (or `head_event_id` if no intermediates) to the target event.
9. **Create composites** for new issues' non-green reviews.
10. **Update `head_event_id`** to point to the target event.

For single-commit reviews (no intermediates), steps 3-4 are skipped and the flow is identical to the original.

### Step 8: No Changes Needed to project_service.rs

**No changes to `project_service.rs`** — every commit in the range now has an event row, so the existing `build_event_entries` logic works as-is.

---

## Detailed Data Flow (Bulk Review of 3 Commits)

```
Commit list (newest first):
  C5 (unreviewed)   <- user clicks here (selectedTargetCommit)
  C4 (unreviewed)   <- included in range
  C3 (unreviewed)   <- oldestUnreviewedCommit (base is C2)
  C2 (reviewed)     <- head_event_id points to C2's event
  C1 (reviewed)
```

1. User clicks C5. `selectedRange = [C5, C4, C3]`. Highlight applied.
2. Diff source button: "Review 3 Commits (C3..C5)". Click navigates with `commit=C5, baseCommit=C2` (baseCommit for display only).
3. `ReviewContext` calls `getReviewFileSystemData(projectId, C5, null, 'commit', bcId)`.
4. Backend looks up `head_event_id` -> finds C2's event -> diffs `C2..C5` to get all touched files.
5. User reviews files. Monaco shows `git show C2:<path>` as original, `git show C5:<path>` as modified.
6. Save calls `createEvent(projectId, C5, 'commit', newIssues, resolved, bcId)`.
7. Backend finds previous event via `head_event_id` (C2's event).
8. Backend enumerates intermediates: `git log --format=%H C2..C5` -> [C5, C4, C3], excludes C5 -> [C4, C3], reverses -> [C3, C4].
9. Creates event for C3 (no issues), propagates xrefs from C2's event, translating lines via `git diff C2 C3`.
10. Creates event for C4 (no issues), propagates xrefs from C3's event, translating lines via `git diff C3 C4`.
11. Creates the target event with `hash=C5`, attaches all new issues to it.
12. Propagates xrefs from C4's event to the C5 event, translating lines via `git diff C4 C5`.
13. Creates composites for C5's new issues, updates `head_event_id` to the C5 event.
14. Back on ProjectPage, `getProjectState` returns: C5, C4, C3 all have event rows -> all marked reviewed.

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

1. **Single commit selected** (clicking `oldestUnreviewedCommit`): Backend uses `head_event_id`'s commit as base, which is `commit^` — identical to old behavior.

2. **No previous event exists** (first review ever): `head_event_id` is `None`. Backend falls back to `base_branch` for the diff base (or `commit^` for single commit if appropriate).

3. **Deselection**: Clicking a selected commit deselects it. Clicking a different commit changes the selection.

4. **Only one unreviewed commit**: No change to current behavior — bulk selection UI only appears when there are 2+ unreviewed commits.

---

## Files Modified (Summary)

| File | Type of Change |
|------|---------------|
| `src/renderer/pages/project/ProjectPage.jsx` | Selection state, highlight, range button |
| `src/renderer/App.jsx` | Pass `baseCommit` in reviewContext (display only) |
| `src/renderer/pages/review/ReviewContext.jsx` | Store `baseCommit` (display only) |
| `src/renderer/pages/review/ReviewProjectPage.jsx` | Pass `baseCommit` prop to provider |
| `src/renderer/styles.css` | Selection highlight styles |
| `tauri_src/src/services/review_service.rs` | Derive base from `head_event_id`, use for diff range |
| `tauri_src/src/services/event_service.rs` | Use `head_event_id` for prev event lookup; create intermediate events with xref propagation for bulk reviews |
