# Plan: Merge Commit Handling in ProveAll

## Context

Currently `project_service::build_event_entries()` uses `git log --no-merges` to exclude merge commits entirely from the UI. Meanwhile, `event_service::create_intermediate_events()` does NOT filter merges, creating an inconsistency. The goal is to display merge commits in the event list, auto-review them (so users don't have to), and handle edge cases around ordering and conflicts.

## Phase 1: Git Layer — Detect Merge Commits

**File:** `tauri_src/src/utils/git.rs`

1. Add `parent_count: usize` to `GitCommit` (>1 = merge commit)
2. Change log format from `%H%n%s%n%an%n%aI` to `%H%n%P%n%s%n%an%n%aI` — parse 5-line chunks instead of 4, count space-separated parent hashes
3. Add `diff_tree_cc(project_path, merge_hash)` helper — runs `git diff-tree --cc -p <hash>` for Phase 6 (conflict review)

## Phase 2: EventEntry — Add `is_merge` Field

**File:** `tauri_src/src/commands/project_commands.rs`

Add to `EventEntry`:
```rust
pub is_merge: bool,
pub has_conflict_changes: bool, // true if diff-tree --cc has output
```

## Phase 3: Project Service — Include Merge Commits, Lazy-Create Events

**File:** `tauri_src/src/services/project_service.rs`

### 3a. Remove `--no-merges`

Line 93: change `git::log(path, &["--no-merges", range])` to `git::log(path, &[range])`

### 3b. Populate `is_merge` and `has_conflict_changes` on EventEntry

Use `commit.parent_count > 1`. For `has_conflict_changes`, call `git::diff_tree_cc()` only for merge commits and check if output contains `diff ` lines.

### 3c. Lazy-create merge commit events

After building entries, iterate oldest-to-newest. For each merge commit with `id: None`:
- Check if ALL non-merge commits older than it already have events
- If yes → auto-create a commit event (type "commit", no reviews/issues) and update branch_context head_event_id
- If no → leave it as unreviewed with MERGE label only (Requirement 4)

> **TODO:** Should lazy-creation happen in `get_project_state` (a read path) or should we introduce a separate endpoint/trigger? Creating DB records in a "get" call is a side effect. The alternative is a dedicated `sync_merge_events` command called on page load. What's your preference?

> **TODO:** When lazy-creating a merge event, should we also propagate xrefs from the previous event to the merge event (with skip_file_translation=true)? This seems necessary to maintain the xref chain, otherwise the next real commit's xref propagation would break since it looks at the previous event's xrefs. I believe the answer is yes — confirm?

## Phase 4: Event Service — Merge-Aware Intermediate Events

**File:** `tauri_src/src/services/event_service.rs`

### 4a. Add `skip_file_translation: bool` to `PropagateXrefsParams`

When true, `propagate_previous_xrefs()` always takes the "file not touched" branch — reusing existing composites without line translation.

### 4b. Update `create_intermediate_events()`

For each intermediate commit where `commit.parent_count > 1`:
- Create the event as normal
- Call `propagate_previous_xrefs()` with `skip_file_translation: true`
- This ensures xref touched files only reflect files truly modified on the feature branch

For non-merge intermediates: existing behavior unchanged (`skip_file_translation: false`).

## Phase 5: Frontend — Display Merge Commits

**File:** `src/renderer/pages/project/ProjectPage.jsx`

### 5a. Render merge badge (CSS already exists)

```jsx
{event.is_merge && <div className="commit-merge-badge">MERGE</div>}
```

Apply `commit-item-merge` class (already in CSS: opacity 0.7, cursor default).

### 5b. Exclude merges from review logic

- `oldestUnreviewedCommit`: skip merge commits (`!event.is_merge`)
- `canReview`: add `&& !isMerge`
- `selectedRange`: skip merges when computing bulk count
- Merge commits with events show green dot + MERGE badge (auto-reviewed, non-clickable)
- Merge commits without events show MERGE badge only (waiting for prior commits)

### 5c. Conflicted merge commits (has_conflict_changes)

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

- **Performance:** `diff-tree --cc` per merge commit in `build_event_entries()` adds latency. Mitigation: only called for merge commits, and the check is lightweight.
- **Breaking change to git::log():** Changing from 4-line to 5-line chunks affects all callers (`project_service`, `event_service::create_intermediate_events`, `event_service::build_event_summary`). All must be verified.
- **Existing data:** Projects with events created under the old `--no-merges` will now show merge commits as unreviewed. Lazy-creation handles this gracefully.
- **Side effects in read path:** Lazy-creating events in `get_project_state` is a read-with-side-effects pattern. See TODO above.

## Verification

1. Create a test branch with: regular commits, a clean merge from base, and a conflicted merge
2. Verify merge commits appear in the event list with MERGE badge
3. Verify merge commits are not clickable for review
4. Verify that when all prior commits are reviewed, merge commit events are auto-created
5. Verify bulk review across merge commits copies xrefs without merge's touched files
6. Verify the head-before-merge edge case: merge shows MERGE label but no green dot until prior commits reviewed
7. Run existing tests to ensure no regressions from the git log format change
