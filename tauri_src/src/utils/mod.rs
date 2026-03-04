pub mod git;
pub mod hash_id;
pub mod llm;

/// macOS GUI apps get a minimal PATH that excludes user-installed tools.
/// Spawn a login shell to resolve the user's full PATH and set it on this process.
pub fn fix_path_env() {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    if let Ok(output) = std::process::Command::new(&shell)
        .args(["-li", "-c", "printf '%s' \"$PATH\""])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                std::env::set_var("PATH", &path);
            }
        }
    }
}