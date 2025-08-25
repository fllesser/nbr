//! Utility functions module for nbr
//!
//! This module contains common utility functions used throughout the application.
#![allow(unused)]

use crate::error::{NbrError, Result};
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

    /// Check if a directory exists and is writable
    pub fn is_dir_writable<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();
        if !path.exists() || !path.is_dir() {
            return false;
        }

        // Try to create a temporary file
        let temp_file = path.join(".nb_test_write");
        match std::fs::File::create(&temp_file) {
            Ok(_) => {
                let _ = std::fs::remove_file(&temp_file);
                true
            }
            Err(_) => false,
        }
    }

    /// Create directory recursively if it doesn't exist
    pub fn ensure_dir<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir_all(path).map_err(|e| {
                NbrError::io(format!("Failed to create directory {:?}: {}", path, e))
            })?;
            debug!("Created directory: {:?}", path);
        }
        Ok(())
    }

    /// Copy file with progress reporting
    pub fn copy_file_with_progress<P: AsRef<Path>, Q: AsRef<Path>>(
        from: P,
        to: Q,
        show_progress: bool,
    ) -> Result<()> {
        let from = from.as_ref();
        let to = to.as_ref();

        let file_size = from.metadata()?.len();
        let mut source = fs::File::open(from)?;
        let mut dest = fs::File::create(to)?;

        let pb = if show_progress {
            let pb = ProgressBar::new(file_size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                    .unwrap()
                    .progress_chars("█▉▊▋▌▍▎▏  "),
            );
            pb.set_message(format!("Copying {}", from.display()));
            Some(pb)
        } else {
            None
        };

        let mut buffer = [0; 8192];
        let mut total_bytes = 0;

        loop {
            let bytes_read = source.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            dest.write_all(&buffer[..bytes_read])?;
            total_bytes += bytes_read as u64;

            if let Some(ref pb) = pb {
                pb.set_position(total_bytes);
            }
        }

        if let Some(pb) = pb {
            pb.finish_with_message(format!("Copied {} bytes", total_bytes));
        }

        Ok(())
    }

    /// Find files matching a pattern
    pub fn find_files<P: AsRef<Path>>(
        dir: P,
        pattern: &str,
        recursive: bool,
    ) -> Result<Vec<PathBuf>> {
        let dir = dir.as_ref();
        let mut matches = Vec::new();
        let regex = Regex::new(pattern)
            .map_err(|e| NbrError::invalid_argument(format!("Invalid pattern: {}", e)))?;

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
            return Err(NbrError::invalid_argument("Program name cannot be empty"));
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
        .map_err(|_| NbrError::command_execution(format!("{} {}", program, args.join(" ")), -1))?
        .map_err(|e| NbrError::io(format!("Failed to execute command: {}", e)))?;

        if !output.status.success() {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NbrError::command_execution(
                format!("{} {} - {}", program, args.join(" "), stderr),
                exit_code,
            ));
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
            .map_err(|e| NbrError::io(format!("Failed to execute command: {}", e)))?;

        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);
            return Err(NbrError::command_execution(
                format!("{} {}", program, args.join(" ")),
                exit_code,
            ));
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
        let response = client.get(url).send().await.map_err(NbrError::Network)?;

        if !response.status().is_success() {
            return Err(NbrError::unknown(format!(
                "Failed to download {}: HTTP {}",
                url,
                response.status()
            )));
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
            .map_err(|e| NbrError::io(format!("Failed to create file: {}", e)))?;

        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(NbrError::Network)?;
            file.write_all(&chunk).map_err(|e| {
                NbrError::Io(std::io::Error::other(format!(
                    "Failed to write to file: {}",
                    e
                )))
            })?;

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

    /// Convert string to snake_case
    pub fn to_snake_case(s: &str) -> String {
        let re = Regex::new(r"([a-z0-9])([A-Z])").unwrap();
        re.replace_all(s, "${1}_${2}")
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    }

    /// Convert string to PascalCase
    pub fn to_pascal_case(s: &str) -> String {
        s.split('_')
            .filter(|s| !s.is_empty())
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.collect::<String>()
                    }
                    None => String::new(),
                }
            })
            .collect()
    }

    /// Validate project name
    pub fn validate_project_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(NbrError::validation("Project name cannot be empty"));
        }

        if name.len() > 100 {
            return Err(NbrError::validation(
                "Project name is too long (max 100 characters)",
            ));
        }

        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").unwrap();
        if !re.is_match(name) {
            return Err(NbrError::validation(
                "Project name must start with a letter and contain only letters, numbers, underscores, and hyphens",
            ));
        }

        Ok(())
    }

    /// Validate package name
    pub fn validate_package_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(NbrError::validation("Package name cannot be empty"));
        }

        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").unwrap();
        if !re.is_match(name) {
            return Err(NbrError::validation(
                "Package name must start with a letter and contain only letters, numbers, underscores, and hyphens",
            ));
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
            .map_err(|e| NbrError::io(format!("Failed to clear screen: {}", e)))?;
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
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
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
    fn test_snake_case() {
        assert_eq!(string_utils::to_snake_case("CamelCase"), "camel_case");
        assert_eq!(
            string_utils::to_snake_case("already_snake"),
            "already_snake"
        );
        assert_eq!(string_utils::to_snake_case("mixedCASE"), "mixed_case");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(string_utils::to_pascal_case("snake_case"), "SnakeCase");
        assert_eq!(
            string_utils::to_pascal_case("already_pascal"),
            "AlreadyPascal"
        );
    }

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
        let destination = Path::new("nbr.tar.gz");
        let show_progress = true;
        let result = net_utils::download_file(url, destination, show_progress).await;
        fs::remove_file(destination).unwrap();
        assert!(result.is_ok());
    }
}
