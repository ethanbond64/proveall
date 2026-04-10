use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use crate::models::project::SshConfig;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SshError {
    AuthFailed,
    ConnectionRefused,
    Timeout,
    CommandFailed(String),
    Io(std::io::Error),
}

impl std::fmt::Display for SshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SshError::AuthFailed => write!(f, "SSH authentication failed"),
            SshError::ConnectionRefused => write!(f, "SSH connection refused"),
            SshError::Timeout => write!(f, "SSH connection timed out"),
            SshError::CommandFailed(s) => write!(f, "SSH command failed: {}", s),
            SshError::Io(e) => write!(f, "SSH IO error: {}", e),
        }
    }
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected,
    Failed(String),
}

// ---------------------------------------------------------------------------
// SshExecutor trait
// ---------------------------------------------------------------------------

/// Abstraction over SSH execution. The production implementation uses the
/// system `ssh` binary with ControlMaster multiplexing. Tests inject a
/// `FakeSshExecutor` that returns canned responses without a real server.
pub trait SshExecutor: Send + Sync {
    /// Ensure a ControlMaster is up for the given config. Idempotent.
    fn ensure_connected(&self, config: &SshConfig) -> Result<(), SshError>;

    /// Run a remote command and return the full output.
    fn run_command(&self, config: &SshConfig, args: &[&str]) -> Result<Output, SshError>;

    /// Non-blocking check — is the ControlMaster socket live right now?
    fn is_connected(&self, config: &SshConfig) -> bool;

    /// Return the ControlMaster socket path for the given config (used by PTY spawn).
    fn socket_path(&self, config: &SshConfig) -> PathBuf;

    /// Disconnect the ControlMaster for the given config, if connected.
    fn disconnect(&self, config: &SshConfig);
}

// ---------------------------------------------------------------------------
// Production implementation
// ---------------------------------------------------------------------------

pub struct SshConnectionManager {
    socket_dir: PathBuf,
    states: Mutex<HashMap<String, ConnectionState>>,
}

impl SshConnectionManager {
    pub fn new() -> Self {
        let socket_dir = std::env::temp_dir().join("proveall-ssh");
        std::fs::create_dir_all(&socket_dir).ok();
        Self {
            socket_dir,
            states: Mutex::new(HashMap::new()),
        }
    }

    fn compute_socket_path(&self, config: &SshConfig) -> PathBuf {
        let key = config.connection_key();
        let digest = Sha256::digest(key.as_bytes());
        let short = &hex::encode(digest)[..16];
        self.socket_dir.join(format!("{}.ctl", short))
    }

    fn check_socket_alive(&self, config: &SshConfig) -> bool {
        let socket = self.compute_socket_path(config);
        let user_host = format!("{}@{}", config.user, config.host);
        Command::new("ssh")
            .args([
                "-O",
                "check",
                "-o",
                &format!("ControlPath={}", socket.display()),
                "-p",
                &config.port.to_string(),
                &user_host,
            ])
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn start_control_master(&self, config: &SshConfig) -> Result<(), SshError> {
        let socket = self.compute_socket_path(config);
        let user_host = format!("{}@{}", config.user, config.host);
        let port_str = config.port.to_string();
        let control_path = format!("ControlPath={}", socket.display());

        let mut args: Vec<&str> = vec![
            "-nNf",
            "-o",
            "ControlMaster=yes",
            "-o",
            &control_path,
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=5",
            "-p",
            &port_str,
        ];

        let key_path_owned;
        if let Some(key) = &config.key_path {
            key_path_owned = key.clone();
            args.extend(["-i", &key_path_owned]);
        }
        args.push(&user_host);

        let status = Command::new("ssh")
            .args(&args)
            .status()
            .map_err(SshError::Io)?;

        if status.success() {
            Ok(())
        } else {
            // Distinguish auth failure from connection refused by exit code
            match status.code() {
                Some(255) => Err(SshError::AuthFailed),
                _ => Err(SshError::ConnectionRefused),
            }
        }
    }
}

impl Default for SshConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SshExecutor for SshConnectionManager {
    fn ensure_connected(&self, config: &SshConfig) -> Result<(), SshError> {
        let key = config.connection_key();
        {
            let states = self.states.lock().unwrap();
            if let Some(ConnectionState::Connected) = states.get(&key) {
                if self.check_socket_alive(config) {
                    return Ok(());
                }
            }
        }

        // Check if an existing socket from a previous app run is still live.
        if self.check_socket_alive(config) {
            self.states
                .lock()
                .unwrap()
                .insert(key, ConnectionState::Connected);
            return Ok(());
        }

        // Start a fresh ControlMaster.
        match self.start_control_master(config) {
            Ok(()) => {
                self.states
                    .lock()
                    .unwrap()
                    .insert(key, ConnectionState::Connected);
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                self.states
                    .lock()
                    .unwrap()
                    .insert(key, ConnectionState::Failed(msg));
                Err(e)
            }
        }
    }

    fn run_command(&self, config: &SshConfig, args: &[&str]) -> Result<Output, SshError> {
        let socket = self.compute_socket_path(config);
        let user_host = format!("{}@{}", config.user, config.host);
        let port_str = config.port.to_string();
        let control_path = format!("ControlPath={}", socket.display());

        Command::new("ssh")
            .args(["-o", &control_path, "-p", &port_str, &user_host])
            .args(args)
            .output()
            .map_err(SshError::Io)
    }

    fn is_connected(&self, config: &SshConfig) -> bool {
        let key = config.connection_key();
        matches!(
            self.states.lock().unwrap().get(&key),
            Some(ConnectionState::Connected)
        )
    }

    fn socket_path(&self, config: &SshConfig) -> PathBuf {
        self.compute_socket_path(config)
    }

    fn disconnect(&self, config: &SshConfig) {
        let key = config.connection_key();
        let socket = self.compute_socket_path(config);
        let user_host = format!("{}@{}", config.user, config.host);
        let port_str = config.port.to_string();
        let control_path = format!("ControlPath={}", socket.display());
        // Send exit signal to ControlMaster; ignore errors.
        let _ = Command::new("ssh")
            .args([
                "-O",
                "exit",
                "-o",
                &control_path,
                "-p",
                &port_str,
                &user_host,
            ])
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status();
        self.states
            .lock()
            .unwrap()
            .insert(key, ConnectionState::Disconnected);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::FakeSshExecutor;

    fn test_config() -> SshConfig {
        SshConfig {
            host: "testhost.example.com".to_string(),
            user: "alice".to_string(),
            port: 22,
            key_path: None,
            remote_path: "/home/alice/project".to_string(),
        }
    }

    fn test_config2() -> SshConfig {
        SshConfig {
            host: "other.example.com".to_string(),
            user: "bob".to_string(),
            port: 2222,
            key_path: None,
            remote_path: "/home/bob/project".to_string(),
        }
    }

    #[test]
    fn test_socket_path_deterministic() {
        let mgr = SshConnectionManager::new();
        let config = test_config();
        let p1 = mgr.compute_socket_path(&config);
        let p2 = mgr.compute_socket_path(&config);
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_socket_path_unique_per_host() {
        let mgr = SshConnectionManager::new();
        let p1 = mgr.compute_socket_path(&test_config());
        let p2 = mgr.compute_socket_path(&test_config2());
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_socket_path_length() {
        let mgr = SshConnectionManager::new();
        let path = mgr.compute_socket_path(&test_config());
        let path_str = path.to_string_lossy();
        assert!(
            path_str.len() <= 104,
            "Socket path too long ({} chars): {}",
            path_str.len(),
            path_str
        );
    }

    #[test]
    fn test_fake_ensure_connected_success() {
        let fake = FakeSshExecutor::new();
        let config = test_config();
        assert!(fake.ensure_connected(&config).is_ok());
        assert!(fake.is_connected(&config));
    }

    #[test]
    fn test_fake_ensure_connected_failure() {
        let mut fake = FakeSshExecutor::new();
        fake.set_connect_result(Err(SshError::AuthFailed));
        let config = test_config();
        assert!(fake.ensure_connected(&config).is_err());
        assert!(!fake.is_connected(&config));
    }

    #[test]
    fn test_fake_run_command_with_expected_response() {
        let fake = FakeSshExecutor::new();
        fake.expect_command("git rev-parse", b"abc123\n", b"", 0);
        let config = test_config();
        fake.ensure_connected(&config).unwrap();
        let output = fake
            .run_command(&config, &["git", "rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap();
        assert_eq!(output.stdout, b"abc123\n");
        assert!(output.status.success());
        fake.assert_all_expectations_met();
    }

    #[test]
    fn test_fake_drop_connection() {
        let fake = FakeSshExecutor::new();
        let config = test_config();
        fake.ensure_connected(&config).unwrap();
        assert!(fake.is_connected(&config));
        fake.drop_connection(&config);
        assert!(!fake.is_connected(&config));
    }

    #[test]
    fn test_fake_is_disconnected_before_connect() {
        let fake = FakeSshExecutor::new();
        let config = test_config();
        assert!(!fake.is_connected(&config));
    }
}
