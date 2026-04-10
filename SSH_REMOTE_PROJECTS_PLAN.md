# SSH Remote Projects — Implementation Plan

## Overview

Add support for projects hosted on remote machines over SSH. Remote projects appear in the menu identically to local projects. All database interactions and settings remain on the host machine. Only filesystem browsing, git command execution, and PTY sessions run on the remote machine.

---

## Key Constraints

- **Database stays local**: All SQLite operations remain on the host (projects table, branch_context, events, issues, reviews, etc.)
- **Settings stay local**: `AppSettings` and `settings.json` are host-only
- **No passwords saved**: If key auth fails, prompt for a password once per session; never persist it
- **Remote operations**: Directory listing (SFTP), git subprocess execution, PTY spawning
- **Same UX**: Remote projects appear in the project list with a subtle SSH indicator; navigation and review flows are identical

---

## Architecture

### Connection Strategy: System SSH with ControlMaster

Use the system `ssh` binary rather than a Rust SSH crate. This gives us:
- Native support for all auth methods (keys, agent, keyboard-interactive)
- `~/.ssh/config` compatibility (aliases, jump hosts, custom keys, etc.)
- ControlMaster multiplexing — one authenticated connection shared across all git, SFTP, and PTY sessions for a host
- No need to reimplement SSH internals

**Connection lifecycle per host**:
1. App establishes a ControlMaster connection (`ssh -nNf -o ControlMaster=yes -o ControlPath=<socket> user@host`)
2. All subsequent operations use `-o ControlPath=<socket>` to reuse the authenticated channel instantly
3. Socket path is deterministic and derived from the connection parameters (see Socket Path below)
4. On app exit, ControlMaster is killed and socket cleaned up

**Socket path**:

Sockets live under `$TMPDIR/proveall-ssh/` (using `std::env::temp_dir()`, which on macOS resolves to a per-user session-scoped directory like `/var/folders/.../T/`). The socket filename is a truncated SHA-256 of the canonical connection string `user@host:port` — for example `a3f9c1d8e2b47f01.ctl`.

This approach:
- **Is deterministic**: the same host always maps to the same socket path, so if the app crashes and restarts it can check whether an old ControlMaster is still alive via `ssh -O check -o ControlPath=<socket> ...` before starting a new one
- **Is per-host**: each remote host gets its own independent socket and ControlMaster process
- **Stays short**: hashing avoids exceeding the 104-character Unix domain socket path limit on macOS, regardless of how long the hostname or username is
- **Is user-scoped**: `$TMPDIR` on macOS is already isolated per user session, so no cross-user collisions

**Authentication flow**:
1. Try key-based auth silently (respects `~/.ssh/config`, agent, and default key files)
2. If the connection fails within a short timeout, prompt the user for a password via a modal
3. Pass the password to `ssh` once via `sshpass` or `SSH_ASKPASS` trick — never write it to disk
4. If auth succeeds, ControlMaster is up; do not ask again during the session
5. If the connection drops mid-session, re-authenticate transparently using the same flow

---

## Data Model Changes

### `projects` table additions

Add two new nullable columns via a Diesel migration:

```sql
-- New migration: xxxx_add_ssh_to_projects/up.sql
ALTER TABLE projects ADD COLUMN connection_type TEXT NOT NULL DEFAULT 'local';
ALTER TABLE projects ADD COLUMN ssh_config     TEXT;           -- JSON, nullable
```

`ssh_config` JSON shape (stored only when `connection_type = 'ssh'`):

```json
{
  "host":        "myserver.example.com",
  "user":        "alice",
  "port":        22,
  "key_path":    "/Users/alice/.ssh/id_ed25519",  // optional, null → default key resolution
  "remote_path": "/home/alice/projects/myapp"
}
```

**Project ID**: Still derived by `hash_id::project_id(...)` — use the canonical string `user@host:remote_path` as the input, so IDs are stable and deterministic.

**Project `path` field**: For SSH projects, store the canonical string `user@host:remote_path` so existing path-keyed lookups continue to work as a unique identifier.

**Project `name` field**: Derived from the basename of `remote_path`, same as local.

### Diesel model additions

Add `connection_type: String` and `ssh_config: Option<String>` to the `Project` struct. Add a `SshConfig` struct that serializes/deserializes from the JSON column.

---

## New Rust Components

### `tauri_src/src/services/ssh_connection_manager.rs`

Manages ControlMaster connections, one per remote host.

```
SshConnectionManager
  ├── connections: HashMap<String, SshConnection>  // keyed by "user@host:port"
  └── socket_dir: PathBuf                          // temp dir for ControlMaster sockets

SshConnection
  ├── config: SshConfig
  ├── socket_path: PathBuf
  ├── state: ConnectionState  // Disconnected | Connecting | Connected | Failed
  └── master_process: Option<Child>
```

**Public API**:
- `ensure_connected(config: &SshConfig) -> Result<(), SshError>` — idempotent; starts ControlMaster if not already running; called before any remote operation
- `is_connected(config: &SshConfig) -> bool`
- `run_command(config: &SshConfig, args: &[&str]) -> Result<Output, SshError>` — runs `ssh -o ControlPath=<socket> user@host <args>`
- `disconnect_all()` — called on app shutdown

### `tauri_src/src/utils/remote_git.rs`

A thin wrapper that routes git operations through SSH when the project is remote.

```rust
pub fn run_git(project: &Project, git_args: &[&str]) -> Result<Output, GitError>
```

- If `project.connection_type == "local"` → current behavior (local `Command::new("git")`)
- If `project.connection_type == "ssh"` → `ssh_connection_manager.run_command(config, &["git", "-C", remote_path, ...git_args])`

Replaces direct calls to `Command::new("git")` in `utils/git.rs` — each function gains a `project: &Project` (or `SshConfig`) parameter.

### `tauri_src/src/utils/remote_fs.rs`

Remote directory listing via SFTP (using `ssh`'s built-in sftp subsystem or a simple `ls` over SSH).

```rust
pub fn list_directory(project: &Project, path: &str) -> Result<Vec<DirectoryEntry>, FsError>
```

- Local → current behavior
- Remote → `ssh -o ControlPath=<socket> user@host ls -1ap <path>` parsed into `DirectoryEntry` values

Since file content is already fetched through `git show` (not raw filesystem reads), SFTP is only needed for the directory-tree browser.

---

## PTY Session Changes (`utils/pty.rs`)

### Local PTY (unchanged)

Current flow is unchanged for local projects.

### Remote PTY

When spawning a PTY for an SSH project:

1. Instead of launching `bash` locally, launch `ssh` locally as the PTY child process:
   ```
   ssh -tt -o ControlPath=<socket> -p <port> user@host
   ```
   The `-tt` flag allocates a remote PTY. The existing ControlMaster is reused, so no re-authentication.

2. Immediately write an initial command to the PTY stdin to `cd` into the project directory:
   ```
   cd /home/alice/projects/myapp\n
   ```
   This is the same "pending prompt injection" mechanism already used for AI commands.

3. All PTY reads, writes, resize, and kill operations work identically — the Rust side only sees a local child process wrapping the SSH connection.

4. On PTY exit, the ControlMaster is unaffected (other sessions or operations can continue).

---

## Tauri Command Changes

### New commands

| Command | Description |
|---|---|
| `add_ssh_project(host, user, port, key_path, remote_path)` | Validates SSH config, attempts connection, opens project |
| `test_ssh_connection(host, user, port, key_path)` | Returns `{success, needs_password}` for the "Add Remote Project" flow |
| `get_ssh_connection_state(project_id)` | Returns current connection state for a project |

### Modified commands

| Command | Change |
|---|---|
| `open_project(path)` | Accept either a local path or a `user@host:path` string; route accordingly |
| `get_directory(project_id, dir_path)` | Use `remote_fs::list_directory` for SSH projects |
| `pty_spawn(project_path, ...)` | Detect SSH project and use remote PTY path |
| `get_current_branch(project_id)` | Use `remote_git::run_git` |
| `get_project_state(...)` | All git calls routed through `remote_git` |
| All other git-touching commands | Routed through `remote_git` |

---

## Frontend Changes

### `MenuPage.jsx`

- Add an **"Add Remote Project"** button alongside the existing "Open" button
- Remote projects in the list show a small SSH chip/badge next to the name (e.g., `myapp  [ssh]`)
- Show the host as a subtitle line: `alice@myserver.example.com`
- Otherwise identical to local project entries (same open/delete actions)

### New `AddSshProjectModal.jsx`

A modal (or drawer) with fields:
- **Host** (required) — e.g., `myserver.example.com`
- **User** (required, defaults to current local user)
- **Port** (optional, defaults to 22)
- **SSH Key** (optional file picker, defaults to standard key resolution)
- **Remote Path** (required) — absolute path on the remote machine

**Flow on submit**:
1. Call `test_ssh_connection(...)` → if `needs_password`, show password input (not stored, passed once)
2. If connection succeeds, call `add_ssh_project(...)`
3. Project appears in the list; navigate to the project normally

### Password Prompt Modal

A minimal modal shown only when key auth fails:
- Single password field (type=password, no autocomplete/save)
- "Connect" and "Cancel" actions
- On success, modal dismissed and never shown again for this session (ControlMaster stays alive)
- Password string is passed as a Tauri command argument, used once, then dropped

### Connection State Indicator

For SSH projects currently open in the app, show a small status dot in the sidebar or header:
- Green: ControlMaster connected
- Yellow: Reconnecting
- Red: Connection lost (with retry button)

---

## Authentication Details

### Happy path (key auth)

```
ssh -nNf
    -o ControlMaster=yes
    -o ControlPath=$TMPDIR/proveall-ssh/<sha256_of_user@host:port>.ctl
    -o BatchMode=yes          ← fail immediately if interactive auth needed
    -o ConnectTimeout=5
    -p <port>
    user@host
```

Before spawning, check if a socket already exists and is live:
```
ssh -O check -o ControlPath=<socket> user@host 2>/dev/null
```
Exit 0 → reuse existing ControlMaster. Non-zero → start a new one.

If this exits 0 → connected, no user interaction needed.

### Password fallback

If the above fails (exit code non-zero or timeout), the frontend shows the password modal. The backend then attempts connection using `SSH_ASKPASS` + `DISPLAY=dummy setsid ssh ...` trick, or by piping via `sshpass -e` with the password in `SSH_PASSWORD` env var (never written to disk, lives only in the process env of the short-lived sshpass invocation).

Avoid saving `sshpass` as a hard dependency — check if it is available; if not, fall back to a temporary named pipe approach.

### `~/.ssh/config` compatibility

Because we use the system `ssh` binary, all entries in `~/.ssh/config` (aliases, `ProxyJump`, `IdentityFile`, per-host settings) work automatically. Users with complex SSH setups don't need to reconfigure anything.

---

## Error Handling

| Scenario | Behavior |
|---|---|
| Remote host unreachable | Show error in modal during project open; project remains in list for when connectivity is restored |
| Auth failure after password prompt | Show error, let user retry or cancel |
| ControlMaster dies mid-session | Detect via failed command; attempt reconnect once; show status indicator |
| Git command fails on remote | Propagate existing error handling (same as local git failures) |
| Remote path doesn't exist | Validation error during `add_ssh_project` |
| Remote has no git repo | Validation error during `add_ssh_project` |

---

## Testing Strategy

Tests must never require a real SSH server. All SSH execution is hidden behind a trait so tests can inject a fake that returns canned responses.

### Trait Abstraction (prerequisite for all testing)

Before any tests are written, introduce a `SshExecutor` trait in `ssh_connection_manager.rs`:

```rust
pub trait SshExecutor: Send + Sync {
    /// Ensure a ControlMaster is up for the given config.
    fn ensure_connected(&self, config: &SshConfig) -> Result<(), SshError>;
    /// Run a remote command and return stdout/stderr/status.
    fn run_command(&self, config: &SshConfig, args: &[&str]) -> Result<std::process::Output, SshError>;
    /// Non-blocking check — is the ControlMaster socket live right now?
    fn is_connected(&self, config: &SshConfig) -> bool;
    /// Spawn a PTY-bearing SSH child process (for terminal sessions).
    fn spawn_pty_process(&self, config: &SshConfig) -> Result<std::process::Child, SshError>;
}
```

`SshConnectionManager` implements this trait for production. All other components (`remote_git`, `remote_fs`, `pty.rs`) take `Arc<dyn SshExecutor>` rather than a concrete type. Tauri state holds the production implementation; tests inject a fake.

### `FakeSshExecutor`

Lives in `tauri_src/src/services/fake_ssh_executor.rs` (compiled only under `#[cfg(test)]` or a `test-utils` feature flag).

```rust
pub struct FakeSshExecutor {
    // Queued responses for run_command calls, matched by args prefix
    responses: Mutex<HashMap<String, VecDeque<FakeResponse>>>,
    // Whether ensure_connected should succeed or fail
    connect_result: Result<(), SshError>,
    // Simulated connection state
    connected: AtomicBool,
}

pub struct FakeResponse {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: i32,   // exit code
}
```

**Key methods on `FakeSshExecutor`**:
- `expect_command(prefix: &str, response: FakeResponse)` — enqueue a response for the next call whose args start with `prefix`
- `set_connect_result(result)` — make `ensure_connected` succeed or fail
- `drop_connection()` — simulate a mid-session ControlMaster death (subsequent `is_connected` returns false)
- `assert_all_expectations_met()` — panic if any enqueued response was never consumed

### Unit Tests: `ssh_connection_manager.rs`

File: `tauri_src/src/services/ssh_connection_manager_tests.rs`

| Test | What it verifies |
|---|---|
| `test_ensure_connected_success` | `ensure_connected` sets state to `Connected` when the check command exits 0 |
| `test_ensure_connected_idempotent` | Calling `ensure_connected` twice does not spawn a second ControlMaster |
| `test_ensure_connected_key_auth_failure` | When the fake returns a non-zero exit, `ensure_connected` returns `Err(SshError::AuthFailed)` |
| `test_is_connected_reflects_state` | Returns false before connect, true after, false after `drop_connection` |
| `test_socket_path_deterministic` | Same `user@host:port` always produces the same socket path string |
| `test_socket_path_unique_per_host` | Two different hosts produce different socket paths |
| `test_socket_path_length` | Socket path is always ≤ 104 characters |

### Unit Tests: `remote_git.rs`

File: `tauri_src/src/utils/remote_git_tests.rs`

Each test builds a `Project` with `connection_type = "ssh"` and injects a `FakeSshExecutor`.

| Test | Fake setup | What it verifies |
|---|---|---|
| `test_current_branch_remote` | Respond to `git -C <path> rev-parse --abbrev-ref HEAD` with `main\n` | Returns `"main"` |
| `test_git_log_remote` | Respond with multi-line commit log output | Parses into correct `Vec<GitCommit>` |
| `test_diff_file_remote` | Respond with unified diff text | Returns diff string unchanged |
| `test_git_command_failure` | Respond with exit code 128, stderr `"not a git repository"` | Propagates as `GitError` |
| `test_local_project_bypasses_executor` | `FakeSshExecutor` with no expectations | Local project runs git locally; fake is never called |
| `test_git_args_forwarded_correctly` | Capture args passed to `run_command` | Verifies `-C <remote_path>` is always prepended |

### Unit Tests: `remote_fs.rs`

File: `tauri_src/src/utils/remote_fs_tests.rs`

| Test | Fake setup | What it verifies |
|---|---|---|
| `test_list_directory_remote` | Respond to `ls -1ap <path>` with `file.txt\nsubdir/\n` | Returns two `DirectoryEntry` values with correct types |
| `test_list_directory_empty` | Respond with empty stdout | Returns empty `Vec` |
| `test_list_directory_path_not_found` | Respond with exit code 1, stderr `"No such file"` | Returns `FsError::NotFound` |
| `test_list_directory_local_uses_std_fs` | No fake involved | Local path uses existing `std::fs` path, not SSH |

### Unit Tests: PTY (remote path)

File: `tauri_src/src/utils/pty_tests.rs`

PTY testing is the most involved because it needs a fake child process. The approach: `spawn_pty_process` on the fake returns a child process running a local script that echoes configurable output and exits on demand.

| Test | Setup | What it verifies |
|---|---|---|
| `test_remote_pty_spawn_calls_ssh` | Fake records the args passed to `spawn_pty_process` | Args include `-tt`, `-o ControlPath=<socket>`, `user@host` |
| `test_remote_pty_cd_injected` | Fake child process echoes stdin back | Initial write contains `cd <remote_path>\n` |
| `test_remote_pty_output_forwarded` | Fake child writes bytes to stdout | `pty-output-{id}` event is emitted with correct base64 content |
| `test_remote_pty_exit_event` | Fake child exits immediately | `pty-exit-{id}` event is emitted |
| `test_remote_pty_write` | Fake child echoes stdin | `pty_write` sends bytes to child stdin |
| `test_remote_pty_resize` | Ensure no panic | `pty_resize` is a no-op on SSH PTYs (resize is handled by the remote shell via SIGWINCH) |

### Unit Tests: `ssh_commands.rs` (Tauri command handlers)

File: `tauri_src/src/commands/ssh_command_tests.rs`

These test the Tauri command layer using an in-memory SQLite database and a `FakeSshExecutor`.

| Test | What it verifies |
|---|---|
| `test_add_ssh_project_success` | Project row is inserted with correct `connection_type` and `ssh_config` JSON; returns project ID |
| `test_add_ssh_project_duplicate` | Adding the same `user@host:path` twice updates `last_opened` rather than creating a duplicate |
| `test_add_ssh_project_invalid_path` | Fake returns error for `git rev-parse` check; command returns validation error |
| `test_test_ssh_connection_key_auth` | Fake `ensure_connected` succeeds immediately; returns `{success: true, needs_password: false}` |
| `test_test_ssh_connection_needs_password` | Fake `ensure_connected` returns `AuthFailed`; returns `{success: false, needs_password: true}` |
| `test_test_ssh_connection_unreachable` | Fake returns `ConnectionRefused`; returns `{success: false, needs_password: false, error: "..."}` |
| `test_get_ssh_connection_state` | Checks state is `"connected"` / `"disconnected"` depending on fake's `is_connected` |

### Integration Tests: End-to-End Command Flow

File: `tauri_src/src/tests/ssh_integration_tests.rs`

These wire together the full backend stack (real in-memory DB, real project service, real git/fs routing) with only the SSH executor faked out. They test that the layers compose correctly.

| Test | Scenario |
|---|---|
| `test_open_ssh_project_and_get_state` | Add SSH project → `get_project_state` → verifies git log parsed correctly from fake output |
| `test_reconnect_after_drop` | Open project, drop connection mid-way, verify next command triggers reconnect attempt |
| `test_local_and_remote_projects_coexist` | One local project + one SSH project in DB; `fetch_projects` returns both; operations on each go to correct executor |
| `test_ssh_project_delete` | Delete an SSH project → connection manager disconnects that host; project removed from DB |

### Frontend Tests

File: `src/renderer/pages/menu/__tests__/MenuPage.test.jsx` and `AddSshProjectModal.test.jsx`

Mock `window.backendAPI` at the test boundary (the same boundary where `tauriAPI.js` lives). No Tauri runtime is needed.

**`MenuPage` tests**:

| Test | Mock setup | What it verifies |
|---|---|---|
| `renders ssh project with badge` | `projectsFetch` returns one project with `connection_type: "ssh"` and `ssh_config` | SSH badge and host subtitle are rendered |
| `renders local and ssh projects together` | Mix of local and SSH projects | Both render without errors; local has no badge |
| `opens ssh project on click` | `projectsOpen` spy | Calls `projectsOpen` with the canonical `user@host:path` string |

**`AddSshProjectModal` tests**:

| Test | Mock setup | What it verifies |
|---|---|---|
| `submits with correct fields` | `testSshConnection` returns `{success: true, needs_password: false}` | `addSshProject` called with correct host/user/port/path |
| `shows password prompt when needed` | `testSshConnection` returns `{needs_password: true}` | Password field appears; `addSshProject` not called until password entered |
| `shows error on unreachable host` | `testSshConnection` returns `{success: false, needs_password: false}` | Error message displayed; form not submitted |
| `disables submit while connecting` | `testSshConnection` hangs (unresolved promise) | Submit button is disabled during the async call |
| `port defaults to 22` | — | Port field has value `22` when empty |
| `user defaults to current user` | `get_current_user` returns `"alice"` | User field pre-populated |

### Test File Locations

| Test file | Tests |
|---|---|
| `tauri_src/src/services/fake_ssh_executor.rs` | The fake itself (test-only) |
| `tauri_src/src/services/ssh_connection_manager_tests.rs` | Connection manager unit tests |
| `tauri_src/src/utils/remote_git_tests.rs` | Remote git wrapper unit tests |
| `tauri_src/src/utils/remote_fs_tests.rs` | Remote FS wrapper unit tests |
| `tauri_src/src/utils/pty_tests.rs` | PTY remote path unit tests |
| `tauri_src/src/commands/ssh_command_tests.rs` | Tauri command handler tests |
| `tauri_src/src/tests/ssh_integration_tests.rs` | Full backend integration tests |
| `src/renderer/pages/menu/__tests__/MenuPage.test.jsx` | Frontend menu rendering tests |
| `src/renderer/pages/menu/__tests__/AddSshProjectModal.test.jsx` | Add SSH project form tests |

---

## Migration & Backwards Compatibility

- Existing local projects are unaffected — new columns default to `'local'` / `NULL`
- No breaking changes to any existing Tauri command signatures for local projects
- `remote_git::run_git` returns the same types as current git functions

---

## File Locations for Implementation

| Component | File |
|---|---|
| DB migration | `tauri_src/migrations/<timestamp>_add_ssh_to_projects/` |
| SSH connection manager | `tauri_src/src/services/ssh_connection_manager.rs` |
| Remote git wrapper | `tauri_src/src/utils/remote_git.rs` |
| Remote FS wrapper | `tauri_src/src/utils/remote_fs.rs` |
| New Tauri commands | `tauri_src/src/commands/ssh_commands.rs` |
| PTY changes | `tauri_src/src/utils/pty.rs` |
| Project model/repo | `tauri_src/src/models/project.rs`, `tauri_src/src/repositories/project_repository.rs` |
| Frontend menu | `src/renderer/pages/menu/MenuPage.jsx` |
| Add SSH modal | `src/renderer/pages/menu/AddSshProjectModal.jsx` |
| Password prompt | `src/renderer/components/SshPasswordModal.jsx` |
| Tauri API wrapper | `src/renderer/tauriAPI.js` |

---

## Out of Scope

- Saving passwords (explicitly excluded)
- SSH tunneling / port forwarding beyond what ControlMaster provides
- Windows SSH support (macOS only for now, matching current platform target)
- SFTP-based file editing (files remain read-only via git; terminal is the write path)
- Key generation or management within the app
