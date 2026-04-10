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
3. Socket lives in a temp directory (`/tmp/proveall-ssh-<hash>/`)
4. On app exit, ControlMaster is killed and socket cleaned up

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
    -o ControlPath=/tmp/proveall-ssh-abc123/ctl
    -o BatchMode=yes          ← fail immediately if interactive auth needed
    -o ConnectTimeout=5
    -p <port>
    user@host
```

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
