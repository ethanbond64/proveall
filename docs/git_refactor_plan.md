# git.rs Refactor Plan

## Library Recommendation: `git2`

Before implementing the plan below, consider switching to the [`git2`](https://crates.io/crates/git2) crate (Rust bindings for libgit2) instead of shelling out to the git CLI.

**Why `git2` is worth considering:**
- No subprocess overhead or shell quoting risks
- No git CLI dependency at runtime
- Strongly-typed API with structured return values (no text parsing for hashes, refs, etc.)
- Better error messages
- Already used by cargo and many production Rust projects

**Why to stick with the CLI approach:**
- `git2` is a non-trivial migration — roughly 200 lines of call-site changes
- The text parsing done in `parse_name_status`, `parse_stat`, and the diff hunk parser would need to be rewritten against libgit2 data structures
- `git diff --shortstat` has no direct libgit2 equivalent; it must be reconstructed by iterating hunks
- The existing approach works and is easy to understand

**Recommendation:** If you want to move to `git2`, do it as a separate PR after this refactor. The named-method interface proposed below isolates the git layer cleanly, so the internal implementation can be swapped without touching any callers.

---

## Refactor Plan (CLI approach, no library change)

### Goal

- Make `run_git` a `fn` (private, not `pub(crate)`)
- Add one named public method per distinct git command used in the project
- Update all call sites to use the named methods
- Remove all direct `run_git` imports from services/commands

---

### Step 1 — Inventory of current `run_git` call sites

| Call site | Args | Proposed method |
|---|---|---|
| `project_service.rs:61` | `["rev-parse", "--abbrev-ref", "HEAD"]` | `current_branch` |
| `project_service.rs:73` | `["diff", "--shortstat", range]` | `diff_shortstat` |
| `project_service.rs:103` | `["log", "--no-merges", format, range]` | `log_commits` |
| `llm_service.rs:80` | `["add", "-A"]` | `add_all` |
| `llm_service.rs:82` | `["status", "--porcelain"]` | `status_porcelain` |
| `llm_service.rs:84` | `["commit", "-m", message]` | `commit` |
| `event_service.rs:172` | `["rev-parse", branch]` | `rev_parse` |
| `event_service.rs:184` | `["log", "--format=%H", range]` | `log_hashes` |
| `event_service.rs:206` | `["log", "-1", "--format=%s", hash]` | `log_subject` |
| `event_service.rs:259` | `["log", "-1", "--format=%s", commit]` | `log_subject` (same) |
| `event_service.rs:451` | `["diff", prev, curr, "--", file]` | `diff_file` |
| `review_service.rs:208` | `["show", "commit:path"]` | `show` |
| `review_service.rs:221` | `["show", "base:path"]` | `show` (same) |
| `review_service.rs:243` | `["show", "base_ref:path"]` | `show` (same) |
| `project_commands.rs:154` | `["rev-parse", "--abbrev-ref", "HEAD"]` | `current_branch` (same) |
| `git.rs:58` (inside `diff_changed_files`) | `["diff", "--name-status", ...range]` | stays internal |

---

### Step 2 — New public API for `git.rs`

```rust
// PRIVATE — internal executor
fn run_git(project_path: &str, args: &[&str]) -> Result<GitOutput, AppError> { ... }

// --- Named public methods ---

/// `git rev-parse --abbrev-ref HEAD` → current branch name
pub fn current_branch(project_path: &str) -> Result<String, AppError>

/// `git rev-parse <git_ref>` → resolved commit hash
pub fn rev_parse(project_path: &str, git_ref: &str) -> Result<String, AppError>

/// `git diff --shortstat <range>` → raw shortstat string
pub fn diff_shortstat(project_path: &str, range: &str) -> Result<String, AppError>

/// `git log --no-merges <format_arg> <range>` → raw log output
pub fn log_commits(project_path: &str, format_arg: &str, range: &str) -> Result<String, AppError>

/// `git log --format=%H <range>` → list of commit hashes
pub fn log_hashes(project_path: &str, range: &str) -> Result<Vec<String>, AppError>

/// `git log -1 --format=%s <hash>` → commit subject line
pub fn log_subject(project_path: &str, hash: &str) -> Result<String, AppError>

/// `git show <git_ref>` → raw file or object content
pub fn show(project_path: &str, git_ref: &str) -> Result<String, AppError>

/// `git diff <prev_commit> <curr_commit> -- <file_path>` → unified diff
pub fn diff_file(project_path: &str, prev: &str, curr: &str, file_path: &str) -> Result<String, AppError>

/// `git add -A`
pub fn add_all(project_path: &str) -> Result<(), AppError>

/// `git status --porcelain` → raw porcelain output
pub fn status_porcelain(project_path: &str) -> Result<String, AppError>

/// `git commit -m <message>`
pub fn commit(project_path: &str, message: &str) -> Result<(), AppError>

// --- Existing composite helpers (unchanged) ---
pub fn diff_changed_files(project_path: &str, range_args: &[&str]) -> Result<Vec<DiffFile>, AppError>
pub(crate) fn parse_name_status(raw: &str) -> Vec<DiffFile>
pub(crate) fn parse_stat(s: &str, keyword: &str) -> u64
```

---

### Step 3 — Caller changes

Update imports and call sites in each file. No business logic changes.

**`services/project_service.rs`**
```rust
// Before
use crate::utils::git::{diff_changed_files, parse_stat, run_git};
run_git(path, &["rev-parse", "--abbrev-ref", "HEAD"])?.stdout.trim().to_string()
run_git(path, &["diff", "--shortstat", range]).map(|o| o.stdout).unwrap_or_default()
run_git(path, &["log", "--no-merges", format_arg, range]).map(|o| o.stdout).unwrap_or_default()

// After
use crate::utils::git::{current_branch, diff_changed_files, diff_shortstat, log_commits, parse_stat};
current_branch(path)?
diff_shortstat(path, range).unwrap_or_default()
log_commits(path, format_arg, range).unwrap_or_default()
```

**`services/llm_service.rs`**
```rust
// Before
use crate::utils::git::run_git;
run_git(&ctx.project_path, &["add", "-A"])?;
let status = run_git(&ctx.project_path, &["status", "--porcelain"])?;
if !status.stdout.trim().is_empty() { run_git(&ctx.project_path, &["commit", "-m", ...])?; }

// After
use crate::utils::git::{add_all, commit, status_porcelain};
add_all(&ctx.project_path)?;
let status = status_porcelain(&ctx.project_path)?;
if !status.trim().is_empty() { commit(&ctx.project_path, &format!("fix: {}", ...))?; }
```

**`services/event_service.rs`**
```rust
// Before
use crate::utils::git::{diff_changed_files, run_git};

// After
use crate::utils::git::{diff_changed_files, diff_file, log_hashes, log_subject, rev_parse};
```

**`services/review_service.rs`**
```rust
// Before
use crate::utils::git::{diff_changed_files, run_git};

// After
use crate::utils::git::{diff_changed_files, show};
```

**`commands/project_commands.rs`**
```rust
// Before
crate::utils::git::run_git(&project.path, &["rev-parse", "--abbrev-ref", "HEAD"])

// After
crate::utils::git::current_branch(&project.path)
```

---

### Step 4 — Visibility change

Change `pub(crate) fn run_git` → `fn run_git` (fully private to the module).

Remove `pub(crate)` from `parse_name_status` if it is not used outside `git.rs` after the refactor (it is currently only called from `diff_changed_files`, so it can become private).

---

### Files to modify

1. `tauri_src/src/utils/git.rs` — add named methods, make `run_git` private
2. `tauri_src/src/services/project_service.rs` — update imports and 3 call sites
3. `tauri_src/src/services/llm_service.rs` — update imports and 3 call sites
4. `tauri_src/src/services/event_service.rs` — update imports and 5 call sites
5. `tauri_src/src/services/review_service.rs` — update imports and 3 call sites
6. `tauri_src/src/commands/project_commands.rs` — update 1 call site

No schema, model, or DB changes required.
