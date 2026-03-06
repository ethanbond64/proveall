use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub command: String,
    pub args: String,
}

pub fn call_llm(config: &LlmConfig, project_path: &str, prompt: &str) -> Result<String, AppError> {
    let args: Vec<&str> = config.args.split_whitespace().collect();

    let mut child = Command::new(&config.command)
        .args(&args)
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
