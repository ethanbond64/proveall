# Merge-Only Files Separation in Bulk Commit Review

## Context

When bulk-reviewing a range of commits from ProjectPage, if the range includes a merge commit, ALL files from the merge appear as normal touched files. This is because `build_commit_touched_files` does a single `git diff --name-status base_ref commit` which aggregates everything.

In contrast, when reviewing commits one-by-one, merge commits are skipped (auto-reviewed) and each commit's review only shows its own files.

**Goal:** In bulk commit review, separate files that were ONLY touched via merge commits into a distinct "Merge Commit Files" section in the file tree (similar to the existing "Approved Files" separator in branch mode). Files touched in both a merge commit AND a non-merge commit should remain in the normal section.

For **conflicted merges**: files with conflicts should appear as normal touched files. Non-conflict files from a conflicted merge that were ONLY touched in that merge should appear in the separated section.

There are also two prerequisite bugs to fix:

**Bug A — Conflicted merges are auto-reviewed:** `lazy_create_base_merge_events` in `project_service.rs:197` skips merges with `!entries[i].is_base_merge` but does NOT check `has_conflict_changes`. This means conflicted merge commits get auto-created events just like clean merges, preventing users from ever reviewing them in isolation.

**Bug B — Bulk review with conflicted merge at top sends wrong review type:** When a conflicted merge is the top commit in a selected range, `handleCommitClick` in `ProjectPage.jsx:130-132` sends `MERGE_REVIEW_MODE`, causing the backend to use `build_merge_review_touched_files` (only conflict files from that one commit) instead of `build_commit_touched_files` (full range diff). The bulk range should always use commit mode.

## Step 1: Write Failing Tests (TDD)

File: `tauri_src/src/services/review_service/tests.rs`

### Test helpers needed

Add a helper `git_merge_with_conflict(dir, branch)` to `test_utils.rs` that creates a merge with a conflict, resolves it, and commits. Returns the merge commit hash and the conflicted file path.

### Tests for merge-only file separation

**Test 1: `test_commit_bulk_merge_only_files_separated`**
- Setup: feature branch with commits c1 (touches `feature.txt`) and a base-branch merge (which brings in `main_only.txt` from a commit on main)
- Review c1's base commit, then call `get_review_file_system_data` for the merge commit in commit mode
- Assert: `feature.txt` has `merge_only: false`, `main_only.txt` has `merge_only: true`

**Test 2: `test_commit_bulk_file_in_both_merge_and_normal_not_merge_only`**
- Setup: feature branch with c1 (touches `shared.txt`), main also touches `shared.txt`, then merge
- Call `get_review_file_system_data` for the merge commit in commit mode
- Assert: `shared.txt` has `merge_only: false` (because it was touched in both c1 and the merge)

**Test 3: `test_commit_bulk_conflicted_merge_conflict_files_not_merge_only`**
- Setup: feature and main both modify the same file creating a conflict, merge with conflict resolution
- Assert: the conflicted file has `merge_only: false`, other non-conflict files from the merge have `merge_only: true`

**Test 4: `test_commit_single_no_merge_all_files_normal`**
- Setup: simple commit with no merges in range
- Assert: all files have `merge_only: false` (no regression)

**Test 5: `test_commit_non_base_merge_files_not_merge_only`**
- Setup: merge from another feature branch (not base branch) — should behave as normal commit
- Assert: all files have `merge_only: false`

### Tests for Bug A (conflicted merge lazy-creation)

File: `tauri_src/src/services/project_service/tests.rs`

**Test 6: `test_conflicted_base_merge_not_lazy_created`**
- Setup: feature branch with c1, main modifies same file as c1 creating a conflict, merge with conflict resolution, then review c1
- Call `get_project_state`
- Assert: the conflicted merge commit does NOT have an auto-created event (`id: None`) even though all prior commits have events

### Tests for Bug B (bulk review type)

File: `tauri_src/src/services/review_service/tests.rs`

**Test 7: `test_commit_mode_with_conflicted_merge_at_top_shows_full_range`**
- Setup: feature branch with c1 (touches `feature.txt`), then conflicted merge at top
- Review base commit, call `get_review_file_system_data` in **commit** mode for the merge commit hash
- Assert: response includes BOTH `feature.txt` (from c1) AND the conflict file — not just the conflict file
- This confirms that even when the top commit is a conflicted merge, commit mode shows the full range

## Step 1.1: Fix Bug A — Skip lazy-creation for conflicted merges

File: `tauri_src/src/services/project_service.rs`

In `lazy_create_base_merge_events` (line ~197), change the skip condition from:
```rust
if !entries[i].is_base_merge || entries[i].id.is_some() {
    continue;
}
```
to:
```rust
if !entries[i].is_base_merge || entries[i].has_conflict_changes || entries[i].id.is_some() {
    continue;
}
```

This is a one-line change. Conflicted merges will no longer get auto-created events, allowing users to click them and enter merge review mode.

## Step 1.2: Fix Bug B — Bulk range with conflicted merge at top should use commit mode

File: `src/renderer/pages/project/ProjectPage.jsx`

In `handleCommitClick` (line ~130), the conflicted merge check fires before the bulk-range logic. When a conflicted merge is clicked and it would be the top of a bulk range, it should navigate with `COMMIT_REVIEW_MODE` instead of `MERGE_REVIEW_MODE`.

Change the logic so that:
- If the conflicted merge IS the oldest unreviewed commit (single commit, no bulk range), navigate with `MERGE_REVIEW_MODE` (existing behavior — review just the conflicts)
- Otherwise, set it as `selectedTargetCommit` like any other commit (it becomes part of a bulk range reviewed in commit mode)

This way `handleStartReview` will send `COMMIT_REVIEW_MODE` for the bulk range, and the backend's `build_commit_touched_files` will produce the full range diff.

## Step 2: Backend — Add `merge_only` field to `TouchedFile`

File: `tauri_src/src/commands/review_commands.rs`

```rust
pub struct TouchedFile {
    pub name: String,
    pub path: String,
    pub diff_mode: Option<String>,
    pub state: String,
    pub merge_only: bool,  // NEW
}
```

Update all existing `TouchedFile` construction sites to set `merge_only: false`.

## Step 3: Backend — Merge-aware `build_commit_touched_files`

File: `tauri_src/src/services/review_service.rs`

Modify `build_commit_touched_files` to accept `base_branch: &str` and:

1. Get the overall diff files (existing behavior): `diff_changed_files(path, &[base_ref, commit])`
2. Get all commits in range: `git::log(path, &[&format!("{}..{}", base_ref, commit)])`
3. Collect files touched by non-merge commits:
   - For each commit in range where `parents.len() == 1`: get its files via `diff_changed_files(path, &[&format!("{}^", hash), &hash])`
   - For base-branch merges with conflicts: get conflict files via `git::diff_tree_cc_name_only` and add those to the non-merge set too
4. For each file in the overall diff:
   - `merge_only = true` if file is NOT in the non-merge file set
   - `merge_only = false` otherwise

Update the call site in `get_review_file_system_data` to pass `base_branch`.

## Step 4: Frontend — Render Merge-Only Section in FileTree

File: `src/renderer/pages/review/filetree/FileTree.jsx`

In commit mode (currently renders a flat list), add section logic similar to branch mode:

1. Split `touchedFiles` into `normalFiles` and `mergeOnlyFiles` based on `file.mergeOnly`
2. If `mergeOnlyFiles` is non-empty, render two sections:
   - Normal files (no section header, or "Changed Files" header)
   - "Merge Commit Files (N)" — collapsible section with the existing `.file-tree-section` styling
3. If no merge-only files exist, render the flat list as before (no visual change)

Also update `ReviewContext.jsx` to pass through the `mergeOnly` field when building the touchedFiles Map from backend data.

## Verification

1. Run `cargo test` in `tauri_src/` — all new tests pass (including the bug fix tests)
2. Manual test: conflicted merge commit should NOT be auto-reviewed — it should remain clickable with the "REVIEW MERGE" badge
3. Manual test: clicking a conflicted merge as the top of a bulk range should review the full range, not just conflicts
4. Manual test: bulk review with merge → merge-only files separated in file tree
5. Manual test: single commit review → unchanged (no merge-only section appears)
6. Manual test: conflicted merge files → appear in the normal section
