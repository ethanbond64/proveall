# Plan: Merge Commit Handling in ProveAll

## Context

Currently `project_service::build_event_entries()` uses `git log --no-merges` to exclude merge commits entirely from the UI. Meanwhile, `event_service::create_intermediate_events()` does NOT filter merges, creating an inconsistency. The goal is to display **base-branch merge commits** in the event list, auto-review them (so users don't have to), and handle edge cases around ordering and conflicts.

**Critical distinction:** Only merge commits where the merged-in branch is the **base branch** (from the branch context) get special treatment. Merge commits from other branches (e.g., merging a sub-feature branch into the feature branch) should be treated as normal commits — fully reviewable, clickable, no auto-review.

## Phase 1: Git Layer — Detect and Classify Merge Commits

**File:** `tauri_src/src/utils/git.rs`

### 1a. Extend `GitCommit` with parent hashes

Add `parents: Vec<String>` to `GitCommit`. A commit is a merge when `parents.len() > 1`.

```rust
pub struct GitCommit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub parents: Vec<String>,  // NEW: empty for root, 1 for normal, 2+ for merge
}
```

### 1b. Update `log()` format string

Change from `%H%n%s%n%an%n%aI` to `%H%n%P%n%s%n%an%n%aI` — parse 5-line chunks instead of 4. Parse the `%P` line (space-separated parent hashes) into `parents`.

### 1c. Add `is_ancestor(project_path, commit, ancestor_candidate)` helper

Wraps `git merge-base --is-ancestor <ancestor_candidate> <commit>`. Returns `bool`. This is needed to determine if a merge commit's non-feature-branch parent is on the base branch.

```rust
pub fn is_ancestor(project_path: &str, ancestor: &str, descendant: &str) -> bool {
    run_git(project_path, &["merge-base", "--is-ancestor", ancestor, descendant]).is_ok()
}
```

### 1d. Add `is_base_branch_merge()` classification function

Given a merge commit's parents and the base branch ref, determine if this is a base-branch merge:
- Use `rev_parse` (already exists) to resolve the base branch to a hash
- For each parent of the merge commit, check if it's an ancestor of the base branch tip using `is_ancestor`
- If any parent IS the base branch tip or an ancestor relationship confirms the parent came from the base branch → **base-branch merge**
- Otherwise → **non-base merge** (treat as normal commit)

```rust
pub fn is_base_branch_merge(project_path: &str, parents: &[String], base_branch: &str) -> bool {
    let base_hash = match rev_parse(project_path, base_branch) {
        Ok(h) => h,
        Err(_) => return false,
    };
    // In a "merge base into feature" scenario, one parent is on the feature branch
    // and the other is the base branch tip (or an ancestor of it).
    // Check if any parent is an ancestor of the base branch HEAD.
    parents.iter().any(|p| is_ancestor(project_path, p, &base_hash))
}
```

> **TODO:** `is_ancestor` + `rev_parse` per merge commit adds git subprocess calls. For branches with many merges this could be slow. An alternative is to batch-check using `git log --ancestry-path` or `git branch --contains`. Is performance a concern here, or are merge commits rare enough that per-commit checks are fine?

### 1e. Add `diff_tree_cc()` helper for conflicted merge diffs

Runs `git diff-tree --cc -p <hash>` — used in Phase 6 for conflict review.

## Phase 2: EventEntry — Add Classification Fields

**File:** `tauri_src/src/commands/project_commands.rs`

Add to `EventEntry`:
```rust
pub is_base_merge: bool,         // true = base-branch merge (special handling)
pub has_conflict_changes: bool,  // true if diff-tree --cc has output (only checked when is_base_merge)
```

Note: non-base merge commits have `is_base_merge: false` and are treated identically to regular commits by the frontend.

## Phase 3: Project Service — Include Merge Commits, Lazy-Create Events

**File:** `tauri_src/src/services/project_service.rs`

### 3a. Remove `--no-merges`

Line 93: change `git::log(path, &["--no-merges", range])` to `git::log(path, &[range])`

### 3b. Classify each commit in `build_event_entries()`

The function now needs the `base_branch` name (pass it as a parameter). For each commit:
- If `commit.parents.len() > 1` → call `git::is_base_branch_merge(path, &commit.parents, &base_branch)`
- If true → `is_base_merge: true`, check `has_conflict_changes` via `diff_tree_cc`
- If false → `is_base_merge: false` (normal commit that happens to be a merge from another feature branch)

### 3c. Lazy-create events for base-branch merge commits

After building entries, iterate oldest-to-newest. For each **base-branch merge** commit with `id: None`:
- Check if ALL commits (including non-base merges) older than it already have events
- If yes → auto-create a commit event (type "commit", no reviews/issues), propagate xrefs with `skip_file_translation: true`, and update branch_context head_event_id
- If no → leave it as unreviewed with MERGE label only (Requirement 4: head before merge)

> **TODO:** Should lazy-creation happen in `get_project_state` (a read path) or should we introduce a separate endpoint/trigger? Creating DB records in a "get" call is a side effect. The alternative is a dedicated `sync_merge_events` command called on page load. What's your preference?

> **TODO:** When lazy-creating a merge event, should we also propagate xrefs from the previous event to the merge event (with skip_file_translation=true)? This seems necessary to maintain the xref chain, otherwise the next real commit's xref propagation would break since it looks at the previous event's xrefs. I believe the answer is yes — confirm?

## Phase 4: Event Service — Merge-Aware Intermediate Events

**File:** `tauri_src/src/services/event_service.rs`

### 4a. Add `skip_file_translation: bool` to `PropagateXrefsParams`

When true, `propagate_previous_xrefs()` always takes the "file not touched" branch — reusing existing composites without line translation.

### 4b. Update `create_intermediate_events()`

The function now has access to `commit.parents` for each intermediate commit. It also needs the `base_branch` name (add as parameter).

For each intermediate commit:
- If `commit.parents.len() > 1` AND `is_base_branch_merge(path, &commit.parents, &base_branch)`:
  - Create the event as normal
  - Call `propagate_previous_xrefs()` with `skip_file_translation: true`
  - This ensures xref touched files only reflect files truly modified on the feature branch, not files brought in via the base-branch merge
- If `commit.parents.len() > 1` but NOT a base-branch merge (merged from another feature branch):
  - Treat as a **normal commit** — `skip_file_translation: false`
  - The files changed in this merge ARE relevant branch work
- If `commit.parents.len() <= 1` (normal commit):
  - Existing behavior unchanged (`skip_file_translation: false`)

## Phase 5: Frontend — Display Merge Commits

**File:** `src/renderer/pages/project/ProjectPage.jsx`

### 5a. Render base-branch merge badge (CSS already exists)

```jsx
const isBaseMerge = event.is_base_merge;
```

```jsx
{isBaseMerge && <div className="commit-merge-badge">MERGE</div>}
```

Apply `commit-item-merge` class (already in CSS: opacity 0.7, cursor default). Non-base merges get no badge and are fully interactive.

### 5b. Exclude base-branch merges from review logic

- `oldestUnreviewedCommit`: skip base-branch merge commits (`!event.is_base_merge`)
- `canReview`: add `&& !isBaseMerge`
- `selectedRange`: skip base-branch merges when computing bulk count
- Base-branch merge commits with events → green dot + MERGE badge (auto-reviewed, non-clickable)
- Base-branch merge commits without events → MERGE badge only (waiting for prior commits)
- Non-base merge commits → fully interactive, same as regular commits

### 5c. Conflicted base-branch merge commits (has_conflict_changes)

- Show "REVIEW MERGE" badge instead of plain MERGE
- Make clickable — on click, navigate with `MERGE_REVIEW_MODE`
- Add `MERGE_REVIEW_MODE = 'merge_review'` to constants

> **TODO:** For conflicted merge commits — should the user be REQUIRED to review them before proceeding to the next commit? Or can they be skipped like normal merges? If required, they'd need to be treated more like regular commits in the ordering logic.

> **TODO:** What should the review UI look like for conflicted merges? The `diff-tree --cc` output is a combined diff format. Should we render it in the existing diff viewer, or does it need special handling? The existing review flow expects a two-commit diff — the combined diff format is different.

## Phase 6: Backend — Merge Conflict Review Support

**Files:** `tauri_src/src/services/review_service.rs`, `tauri_src/src/utils/git.rs`

For `MERGE_REVIEW_MODE`:
- File list: parse `git diff-tree --cc --name-status <hash>` (files with conflict changes only)
- File diff: `git diff-tree --cc -p <hash> -- <file>` (combined diff for that file)

> **TODO:** This phase is the most complex and could be deferred. Should we ship Phases 1-5 first and add conflict review later? The non-conflicted merge path covers the vast majority of cases.

## Implementation Order

1. Phase 1 (git.rs) — foundation, everything depends on this
2. Phase 2 (EventEntry struct) — needed by Phases 3 and 5
3. Phase 3 (project_service) + Phase 4 (event_service) — can be done in parallel
4. Phase 5 (frontend) — depends on Phases 2-3
5. Phase 6 (conflict review) — independent, can be deferred

## Risks

- **Performance:** `is_ancestor` + `rev_parse` per merge commit adds git subprocess calls. For branches with many base-branch merges this could add up. Mitigation: only called for merge commits (parent_count > 1), and base-branch merges are typically infrequent.
- **Performance:** `diff-tree --cc` per base-branch merge commit in `build_event_entries()` adds latency. Mitigation: only called for base-branch merge commits, and the check is lightweight.
- **Breaking change to git::log():** Changing from 4-line to 5-line chunks affects all callers (`project_service`, `event_service::create_intermediate_events`, `event_service::build_event_summary`). All must be verified.
- **Existing data:** Projects with events created under the old `--no-merges` will now show merge commits as unreviewed. Lazy-creation handles this gracefully.
- **Side effects in read path:** Lazy-creating events in `get_project_state` is a read-with-side-effects pattern. See TODO above.
- **Ancestor detection edge cases:** If the base branch has been force-pushed or rebased, `is_ancestor` checks may give unexpected results. This should be rare in normal workflows.

## Verification

1. Create a test branch with: regular commits, a clean merge from base, a merge from another feature branch, and a conflicted merge from base
2. Verify base-branch merge commits appear with MERGE badge, non-clickable
3. Verify non-base-branch merge commits appear as normal clickable commits (no badge)
4. Verify that when all prior commits are reviewed, base-branch merge events are auto-created
5. Verify bulk review across base-branch merge commits copies xrefs without merge's touched files
6. Verify bulk review across non-base merge commits propagates xrefs WITH touched file translation (normal behavior)
7. Verify the head-before-merge edge case: base-branch merge shows MERGE label but no green dot until prior commits reviewed
8. Run existing tests to ensure no regressions from the git log format change
