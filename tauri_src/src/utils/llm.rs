use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::AppError;

pub trait LlmProvider {
    fn run(&self, project_path: &str, prompt: &str) -> Result<String, AppError>;
}

pub struct ClaudeProvider;

impl LlmProvider for ClaudeProvider {
    fn run(&self, project_path: &str, prompt: &str) -> Result<String, AppError> {
        let mut child = Command::new("claude")
            .args([
                "--print",
                "--dangerously-skip-permissions",
                "--allowedTools",
                "Edit,Read,Write,Grep,Glob",
            ])
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if output.status.success() {
            Ok(stdout)
        } else {
            Err(AppError::Llm(stderr))
        }
    }
}
