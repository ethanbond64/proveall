/// Remote filesystem operations for SSH projects.
///
/// For local projects the existing `std::fs` path is used unchanged.
/// For SSH projects, directory listing is done via `ls -1ap` over SSH.
use std::path::Path;

use crate::commands::fs_commands::DirectoryEntry;
use crate::error::AppError;
use crate::models::project::Project;
use crate::services::ssh_connection_manager::{SshError, SshExecutor};

fn ssh_error_to_app(e: SshError) -> AppError {
    AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

/// List the entries of a directory.
///
/// - `directory_path`: for local projects, the absolute path on disk; for SSH
///   projects, the absolute path on the remote machine.
pub fn list_directory(
    project: &Project,
    executor: &dyn SshExecutor,
    directory_path: &str,
) -> Result<Vec<DirectoryEntry>, AppError> {
    if !project.is_ssh() {
        return list_directory_local(directory_path);
    }
    list_directory_remote(project, executor, directory_path)
}

fn list_directory_local(directory_path: &str) -> Result<Vec<DirectoryEntry>, AppError> {
    let full_path = Path::new(directory_path);
    let mut entries: Vec<DirectoryEntry> = std::fs::read_dir(full_path)
        .map_err(AppError::from)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            Some(DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
                entry_type: if metadata.is_dir() {
                    "dir".to_string()
                } else {
                    "file".to_string()
                },
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        (b.entry_type == "dir")
            .cmp(&(a.entry_type == "dir"))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

fn list_directory_remote(
    project: &Project,
    executor: &dyn SshExecutor,
    directory_path: &str,
) -> Result<Vec<DirectoryEntry>, AppError> {
    let config = project
        .parse_ssh_config()
        .ok_or_else(|| AppError::Git("SSH project has no ssh_config".to_string()))?;

    let output = executor
        .run_command(&config, &["ls", "-1ap", directory_path])
        .map_err(ssh_error_to_app)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if stderr.contains("No such file") || stderr.contains("cannot access") {
            return Err(AppError::NotFound(format!(
                "Remote path not found: {}",
                directory_path
            )));
        }
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            stderr,
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut entries: Vec<DirectoryEntry> = stdout
        .lines()
        .filter(|line| !line.is_empty() && *line != "./" && *line != "../")
        .map(|line| {
            let is_dir = line.ends_with('/');
            let name = line.trim_end_matches('/').to_string();
            let path = format!(
                "{}/{}",
                directory_path.trim_end_matches('/'),
                name
            );
            DirectoryEntry {
                name,
                path,
                entry_type: if is_dir { "dir".to_string() } else { "file".to_string() },
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        (b.entry_type == "dir")
            .cmp(&(a.entry_type == "dir"))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{NewProject, SshConfig};
    use crate::test_utils::FakeSshExecutor;

    fn ssh_project() -> Project {
        let config = SshConfig {
            host: "testhost.example.com".to_string(),
            user: "alice".to_string(),
            port: 22,
            key_path: None,
            remote_path: "/home/alice/myapp".to_string(),
        };
        let new = NewProject::new_ssh(&config);
        Project {
            id: new.id,
            path: new.path,
            name: new.name,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            last_opened: None,
            connection_type: "ssh".to_string(),
            ssh_config: new.ssh_config,
        }
    }

    #[test]
    fn test_list_directory_remote() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        // ls -1ap output: files and dirs (dirs end with /)
        let ls_output = b"./\n../\nfile.txt\nsubdir/\nREADME.md\n";
        fake.expect_command("ls -1ap /home/alice/myapp", ls_output, b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let entries = list_directory(&project, &fake, "/home/alice/myapp").unwrap();
        // ./ and ../ are filtered; 3 entries remain; dirs sorted first
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].entry_type, "dir");
        assert_eq!(entries[0].name, "subdir");
        assert!(entries.iter().any(|e| e.name == "file.txt" && e.entry_type == "file"));
        assert!(entries.iter().any(|e| e.name == "README.md"));
    }

    #[test]
    fn test_list_directory_remote_empty() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        fake.expect_command("ls -1ap /home/alice/myapp", b"./\n../\n", b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let entries = list_directory(&project, &fake, "/home/alice/myapp").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_list_directory_remote_not_found() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        fake.expect_command(
            "ls -1ap /home/alice/missing",
            b"",
            b"ls: cannot access '/home/alice/missing': No such file or directory\n",
            2,
        );
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let result = list_directory(&project, &fake, "/home/alice/missing");
        let err = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        };
        assert!(err.contains("not found") || err.contains("No such"), "err: {}", err);
    }

    #[test]
    fn test_list_directory_local_uses_std_fs() {
        // Local project path: use the actual temp dir (always exists).
        let tmp = std::env::temp_dir();
        let local_project = Project {
            id: "local-test".to_string(),
            path: tmp.to_string_lossy().into_owned(),
            name: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            last_opened: None,
            connection_type: "local".to_string(),
            ssh_config: None,
        };
        let fake = FakeSshExecutor::new();
        // If the fake is called, the test should still work (default success response).
        let result = list_directory(&local_project, &fake, &tmp.to_string_lossy());
        // Temp dir exists so we should get a list (possibly empty).
        assert!(result.is_ok());
        // Fake should NOT have been called for local projects.
        fake.assert_all_expectations_met();
    }
}
