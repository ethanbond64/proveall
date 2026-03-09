# Merge-Only Files Separation in Bulk Commit Review

## Context

When bulk-reviewing a range of commits from ProjectPage, if the range includes a merge commit, ALL files from the merge appear as normal touched files. This is because `build_commit_touched_files` does a single `git diff --name-status base_ref commit` which aggregates everything.

In contrast, when reviewing commits one-by-one, merge commits are skipped (auto-reviewed) and each commit's review only shows its own files.

**Goal:** In bulk commit review, separate files that were ONLY touched via merge commits into a distinct "Merge Commit Files" section in the file tree (similar to the existing "Approved Files" separator in branch mode). Files touched in both a merge commit AND a non-merge commit should remain in the normal section.

For **conflicted merges**: files with conflicts should appear as normal touched files. Non-conflict files from a conflicted merge that were ONLY touched in that merge should appear in the separated section.

## Step 1: Write Failing Tests (TDD)

File: `tauri_src/src/services/review_service/tests.rs`

### Test helpers needed

Add a helper `git_merge_with_conflict(dir, branch)` to `test_utils.rs` that creates a merge with a conflict, resolves it, and commits. Returns the merge commit hash and the conflicted file path.

### Tests to add

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

1. Run `cargo test` in `tauri_src/` — all new tests should pass
2. Manual test: create a repo with a feature branch, make commits on both main and feature, merge main into feature, then bulk-review the range. Verify the file tree separates merge-only files.
3. Manual test: single commit review should be unchanged (no merge-only section appears).
4. Manual test: conflicted merge files should appear in the normal section.
