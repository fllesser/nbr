//! Utility functions module for nbr
//!
//! This module contains common utility functions used throughout the application.
#![allow(unused)]

use crate::error::Error;
use anyhow::{Context, Result};
use console::Term;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::Client;

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info};

/// File system utilities
pub mod fs_utils {
    use super::*;

    /// Find files matching a pattern
    pub fn find_files<P: AsRef<Path>>(
        dir: P,
        pattern: &str,
        recursive: bool,
    ) -> Result<Vec<PathBuf>> {
        let dir = dir.as_ref();
        let mut matches = Vec::new();
        let regex = Regex::new(pattern).context("Invalid regex pattern")?;

        find_files_recursive(dir, &regex, recursive, &mut matches)?;
        Ok(matches)
    }

    fn find_files_recursive(
        dir: &Path,
        regex: &Regex,
        recursive: bool,
        matches: &mut Vec<PathBuf>,
    ) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str())
                    && regex.is_match(filename)
                {
                    matches.push(path);
                }
            } else if path.is_dir() && recursive {
                find_files_recursive(&path, regex, recursive, matches)?;
            }
        }
        Ok(())
    }

    /// Get file size in a human-readable format
    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }
}

/// Process execution utilities
pub mod process_utils {
    use super::*;

    /// Execute a command with timeout and capture output
    pub async fn execute_command_with_output(
        program: &str,
        args: &[&str],
        working_dir: Option<&Path>,
        timeout_secs: u64,
    ) -> Result<Output> {
        debug!("Executing command: {} {}", program, args.join(" "));

        if program.is_empty() {
            anyhow::bail!("Program name cannot be empty");
        }

        let mut cmd = Command::new(program);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let output = timeout(Duration::from_secs(timeout_secs), async {
            cmd.output().await
        })
        .await
        .with_context(|| format!("Command '{}' timed out", program))?
        .with_context(|| format!("Failed to execute command: {}", program))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("{} {} failed: {}", program, args.join(" "), stderr);
        }

        debug!("Command executed successfully");
        Ok(output)
    }

    /// Execute a command interactively (inherit stdio)
    /// Execute a command interactively (inherit stdin/stdout/stderr)
    pub fn execute_interactive(
        program: &str,
        args: &[&str],
        working_dir: Option<&Path>,
    ) -> Result<()> {
        debug!("Executing interactively: {} {}", program, args.join(" "));

        let mut cmd = std::process::Command::new(program);
        cmd.args(args);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        let status = cmd
            .status()
            .with_context(|| format!("Failed to execute command: {}", program))?;

        if !status.success() {
            anyhow::bail!("Command '{} {}' failed", program, args.join(" "));
        }

        Ok(())
    }

    /// Check if a command is available in PATH
    pub fn command_exists(command: &str) -> bool {
        std::process::Command::new(command)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Find Python executable
    pub fn find_python() -> Option<String> {
        let candidates = vec!["python", "python3", "py"];

        for candidate in candidates {
            if command_exists(candidate) {
                // Verify it's actually Python 3
                if let Ok(output) = std::process::Command::new(candidate)
                    .arg("--version")
                    .output()
                {
                    let version = String::from_utf8_lossy(&output.stdout);
                    if version.contains("Python 3") {
                        return Some(candidate.to_string());
                    }
                }
            }
        }
        None
    }

    /// Get Python version
    pub async fn get_python_version(python_path: &str) -> Result<String> {
        let output = execute_command_with_output(python_path, &["--version"], None, 10).await?;
        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version.trim().to_string())
    }
}

/// Network utilities
pub mod net_utils {
    use super::*;

    /// Download file with progress bar
    pub async fn download_file(url: &str, destination: &Path, show_progress: bool) -> Result<()> {
        let client = Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download {}: HTTP {}", url, response.status());
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = if show_progress && total_size > 0 {
            let pb = ProgressBar::new(total_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                    .unwrap()
                    .progress_chars("█▉▊▋▌▍▎▏  "),
            );
            pb.set_message(format!("Downloading {}", url));
            Some(pb)
        } else {
            None
        };

        let mut file = fs::File::create(destination)
            .with_context(|| format!("Failed to create file: {:?}", destination))?;

        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Error::Network)?;
            file.write_all(&chunk)
                .with_context(|| format!("Failed to write to file: {:?}", destination))?;

            downloaded += chunk.len() as u64;
            if let Some(ref pb) = pb {
                pb.set_position(downloaded);
            }
        }

        if let Some(pb) = pb {
            pb.finish_with_message("Download completed");
        }

        info!("Downloaded {} ({} bytes)", url, downloaded);
        Ok(())
    }

    /// Check if URL is accessible
    pub async fn check_url_accessible(url: &str) -> bool {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        match client.head(url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

/// String utilities
pub mod string_utils {
    use super::*;

    /// Validate project name
    pub fn validate_project_name(name: &str) -> Result<()> {
        if name.is_empty() {
            anyhow::bail!("Project name cannot be empty");
        }

        if name.len() > 100 {
            anyhow::bail!("Project name is too long (max 100 characters)");
        }

        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").unwrap();
        if !re.is_match(name) {
            anyhow::bail!(
                "Project name must start with a letter and contain only letters, numbers, underscores, and hyphens"
            );
        }

        Ok(())
    }

    /// Validate package name
    pub fn validate_package_name(name: &str) -> Result<()> {
        if name.is_empty() {
            anyhow::bail!("Package name cannot be empty");
        }

        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").unwrap();
        if !re.is_match(name) {
            anyhow::bail!(
                "Package name must start with a letter and contain only letters, numbers, underscores, and hyphens"
            );
        }

        Ok(())
    }

    /// Sanitize filename
    pub fn sanitize_filename(name: &str) -> String {
        let forbidden_chars: Vec<char> = if cfg!(windows) {
            vec!['<', '>', ':', '"', '|', '?', '*', '/', '\\']
        } else {
            vec!['/', '\0']
        };

        let mut result = String::new();
        for ch in name.chars() {
            if forbidden_chars.contains(&ch) || ch.is_control() {
                result.push('_');
            } else {
                result.push(ch);
            }
        }

        // Trim dots and spaces from the end (Windows restriction)
        result.trim_end_matches(&['.', ' '][..]).to_string()
    }

    /// Truncate string with ellipsis
    pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else if max_len <= 3 {
            "...".to_string()
        } else {
            format!("{}...", &s[..max_len - 3])
        }
    }
}

/// Terminal utilities
pub mod terminal_utils {
    use super::*;

    /// Get terminal size
    pub fn get_terminal_size() -> (usize, usize) {
        let term = Term::stdout();
        term.size_checked()
            .map(|(rows, cols)| (rows as usize, cols as usize))
            .unwrap_or((24, 80))
    }

    /// Check if running in TTY
    pub fn is_tty() -> bool {
        Term::stdout().is_term()
    }

    /// Clear terminal screen
    pub fn clear_screen() -> Result<()> {
        Term::stdout()
            .clear_screen()
            .context("Failed to clear screen")?;
        Ok(())
    }

    /// Create a progress bar with custom style
    pub fn create_progress_bar(len: u64, message: impl Into<String>) -> ProgressBar {
        let pb = ProgressBar::new(len);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        pb.set_message(message.into());
        pb
    }

    /// Create a spinner with custom message
    pub fn create_spinner(message: impl Into<String>) -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.green.bold} {msg:.green.bold}").unwrap(),
        );
        pb.set_message(message.into());
        pb.enable_steady_tick(Duration::from_millis(100));
        pb
    }

    pub fn spinner_with_message<T>(message: &str, f: impl FnOnce() -> T) -> T {
        let pb = create_spinner(message);
        let result = f();
        pb.finish_and_clear();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_name_validation() {
        assert!(string_utils::validate_project_name("valid_project").is_ok());
        assert!(string_utils::validate_project_name("ValidProject").is_ok());
        assert!(string_utils::validate_project_name("valid-project").is_ok());

        assert!(string_utils::validate_project_name("").is_err());
        assert!(string_utils::validate_project_name("1invalid").is_err());
        assert!(string_utils::validate_project_name("invalid@project").is_err());
    }

    #[tokio::test]
    async fn test_download_file() {
        let url =
            "https://github.com/fllesser/nbr/releases/latest/download/nbr-Linux-musl-x86_64.tar.gz";
        let temp_dir = tempfile::tempdir().unwrap();
        let destination = temp_dir.path().join("nbr.tar.gz");
        let show_progress = true;
        let result = net_utils::download_file(url, &destination, show_progress).await;
        temp_dir.close().unwrap();
        assert!(result.is_ok());
    }
}
