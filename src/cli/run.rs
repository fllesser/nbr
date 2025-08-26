//! Run command handler for nbr
//!
//! This module handles running NoneBot applications with various options
//! including auto-reload, custom host/port, and environment management.

use crate::cli::generate::generate_bot_content;
use crate::error::{NbrError, Result};
use crate::utils::process_utils;
use colored::Colorize;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::signal;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Bot process manager
pub struct BotRunner {
    /// Bot entry file path
    bot_file: PathBuf,
    /// Python executable path
    python_path: String,
    /// Enable auto-reload
    auto_reload: bool,
    /// Working directory
    work_dir: PathBuf,
    /// Current running process
    current_process: Arc<Mutex<Option<Child>>>,
    /// File watcher for auto-reload
    watcher: Option<RecommendedWatcher>,
    /// Watch event receiver
    watch_rx: Option<Receiver<Event>>,
}

impl BotRunner {
    /// Create a new bot runner
    pub fn new(
        bot_file: PathBuf,
        python_path: String,
        auto_reload: bool,
        work_dir: PathBuf,
    ) -> Result<Self> {
        let current_process = Arc::new(Mutex::new(None));
        let (watch_tx, watch_rx) = if auto_reload {
            let (tx, rx) = mpsc::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let mut runner = Self {
            bot_file,
            python_path,
            auto_reload,
            work_dir,
            current_process,
            watcher: None,
            watch_rx,
        };

        if auto_reload {
            runner.setup_file_watcher(watch_tx.unwrap())?;
        }

        Ok(runner)
    }

    /// Setup file watcher for auto-reload
    fn setup_file_watcher(&mut self, tx: Sender<Event>) -> Result<()> {
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res
                    && let Err(e) = tx.send(event)
                {
                    error!("Failed to send file watch event: {}", e);
                }
            },
            Config::default(),
        )
        .map_err(|e| NbrError::io(format!("Failed to create file watcher: {}", e)))?;

        // Watch the working directory recursively
        watcher
            .watch(&self.work_dir, RecursiveMode::Recursive)
            .map_err(|e| NbrError::io(format!("Failed to watch directory: {}", e)))?;

        self.watcher = Some(watcher);
        debug!("File watcher setup for auto-reload");
        Ok(())
    }

    /// Start the bot process
    pub async fn run(&mut self) -> Result<()> {
        // Setup signal handling for graceful shutdown
        let process_handle = Arc::clone(&self.current_process);
        tokio::spawn(async move {
            let _ = signal::ctrl_c().await;

            warn!("Received interrupt signal, shutting down...");
            if let Ok(mut process) = process_handle.lock()
                && let Some(mut child) = process.take()
            {
                let _ = child.kill();
                let _ = child.wait();
            }
            // sleep 2 second
            sleep(Duration::from_secs(2)).await;
            std::process::exit(0);
        });

        if self.auto_reload {
            self.run_with_reload().await
        } else {
            self.run_once().await
        }
    }

    /// Run bot once without reload
    async fn run_once(&mut self) -> Result<()> {
        let mut process = self.start_bot_process()?;

        let exit_status = process
            .wait()
            .map_err(|e| NbrError::io(format!("Failed to wait for process: {}", e)))?;

        if exit_status.success() {
            info!("Bot process exited successfully");
        } else {
            let exit_code = exit_status.code().unwrap_or(-1);
            error!("Bot process failed with exit code: {}", exit_code);
        }
        Ok(())
    }

    /// Run bot with auto-reload
    async fn run_with_reload(&mut self) -> Result<()> {
        let mut last_restart = std::time::Instant::now();
        const MAX_RAPID_RESTARTS: u32 = 5;
        const RAPID_RESTART_THRESHOLD: Duration = Duration::from_secs(10);

        loop {
            // Start the bot process
            match self.start_bot_process() {
                Ok(process) => {
                    {
                        let mut current = self.current_process.lock().unwrap();
                        *current = Some(process);
                    }

                    info!("Bot started successfully with auto-reload enabled");
                    let mut restart_count = 0;

                    // Wait for file changes or process exit
                    let reload_needed = self.wait_for_reload_trigger().await?;

                    // Kill current process
                    self.kill_current_process();

                    if !reload_needed {
                        break;
                    }

                    // Check for rapid restarts
                    let now = std::time::Instant::now();
                    if now.duration_since(last_restart) < RAPID_RESTART_THRESHOLD {
                        restart_count += 1;
                        if restart_count >= MAX_RAPID_RESTARTS {
                            warn!("Too many rapid restarts, adding delay 5s...");
                            sleep(Duration::from_secs(5)).await;
                        }
                    }
                    last_restart = now;

                    debug!("Starting bot process...");
                }
                Err(e) => {
                    error!("Failed to start bot process: {}", e);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Ok(())
    }

    /// Wait for reload trigger (file change or process exit)
    async fn wait_for_reload_trigger(&self) -> Result<bool> {
        if self.watch_rx.is_none() {
            return Ok(false);
        }
        let watch_rx = self.watch_rx.as_ref().unwrap();

        loop {
            // Check if process is still running
            {
                let mut process_guard = self.current_process.lock().unwrap();
                if let Some(process) = process_guard.as_mut() {
                    match process.try_wait() {
                        Ok(Some(status)) => {
                            info!("Bot process exited with status: {}", status);
                            return Ok(false); // Process exited, don't reload
                        }
                        Ok(None) => {} // Process still running
                        Err(e) => {
                            error!("Checking bot process status: {}", e);
                            return Ok(false);
                        }
                    }
                }
            }
            // Check for file changes
            match watch_rx.try_recv() {
                Ok(event) => {
                    if self.should_reload_for_event(&event) {
                        info!("File change detected, reloading bot...");
                        // 清空未处理的事件
                        while watch_rx.try_recv().is_ok() {}
                        return Ok(true);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No events, continue waiting
                    sleep(Duration::from_millis(1000)).await;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    error!("File watcher disconnected");
                    return Ok(false);
                }
            }
        }
    }

    /// Check if an event should trigger a reload
    fn should_reload_for_event(&self, event: &Event) -> bool {
        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {}
            _ => return false,
        }

        // 需要重载的文件名
        let file_names = ["pyproject.toml", ".env", ".env.dev", ".env.prod"];

        for path in &event.paths {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && file_names.contains(&name)
            {
                return true;
            }

            // Only reload for Python files
            if path.extension().and_then(|ext| ext.to_str()) == Some("py") {
                return true;
            }
        }
        false
    }

    #[allow(unused)]
    fn start_bot_process_with_uv(&self) -> Result<Child> {
        let mut cmd = Command::new("uv");
        cmd.arg("run").arg("--no-sync");
        if self.bot_file.exists() {
            cmd.arg(&self.bot_file);
        } else {
            cmd.arg("python")
                .arg("-c")
                .arg(generate_bot_content(&self.work_dir)?);
        }
        cmd.current_dir(&self.work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let process = cmd
            .spawn()
            .map_err(|e| NbrError::io(format!("Failed to start bot process: {}", e)))?;

        debug!("Bot process started with PID: {}", process.id());
        Ok(process)
    }

    fn start_bot_process(&self) -> Result<Child> {
        let mut cmd = Command::new(self.python_path.clone());
        if self.bot_file.exists() {
            cmd.arg(&self.bot_file);
        } else {
            let bot_content = generate_bot_content(&self.work_dir)?;
            cmd.arg("-c").arg(bot_content);
        }
        cmd.current_dir(&self.work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let process = cmd
            .spawn()
            .map_err(|e| NbrError::io(format!("Failed to start bot process: {}", e)))?;

        debug!("Bot process started with PID: {}", process.id());
        Ok(process)
    }

    /// Kill current process
    fn kill_current_process(&self) {
        let mut process_guard = self.current_process.lock().unwrap();
        if let Some(mut process) = process_guard.take() {
            debug!("Stopping bot process...");

            // Try graceful shutdown first
            if let Err(e) = process.kill() {
                warn!("Failed to kill process gracefully: {}", e);
            }

            // Wait for process to exit
            match process.wait() {
                Ok(status) => {
                    debug!("Process exited with status: {}", status);
                }
                Err(e) => {
                    warn!("Error waiting for process to exit: {}", e);
                }
            }
        }
    }
}

impl Drop for BotRunner {
    fn drop(&mut self) {
        self.kill_current_process();
    }
}

/// Handle the run command
pub async fn handle_run(file: Option<String>, reload: bool) -> Result<()> {
    let bot_file = file.unwrap_or("bot.py".to_string());
    // Load configuration
    let work_dir = std::env::current_dir().unwrap();
    // Find bot file
    let bot_file_path = work_dir.join(bot_file);
    // Find Python executable
    let python_path = find_python_executable(&work_dir)?;
    // Create and run bot
    let mut runner = BotRunner::new(bot_file_path, python_path, reload, work_dir)?;

    info!("Using Python: {}", runner.python_path.cyan().bold());

    runner.run().await
}

/// Find Python executabled
fn find_python_executable(work_dir: &Path) -> Result<String> {
    #[cfg(target_os = "windows")]
    let venv_path = work_dir.join(".venv").join("Scripts").join("python.exe");

    #[cfg(not(target_os = "windows"))]
    let venv_path = work_dir.join(".venv").join("bin").join("python");

    if venv_path.exists() {
        return Ok(venv_path.to_string_lossy().to_string());
    }
    // Fall back to system Python
    process_utils::find_python().ok_or_else(|| {
        NbrError::not_found(
            "Python executable not found. Please use `uv sync -p {version}` to install Python",
        )
    })
}

/// Verify Python environment
#[allow(unused)]
async fn verify_python_environment(python_path: &str) -> Result<()> {
    debug!("Verifying Python environment...");

    // Check Python version
    let version = process_utils::get_python_version(python_path).await?;
    debug!("Python version: {}", version);

    // Verify it's Python 3.10+
    if !version.contains("Python 3.1") {
        return Err(NbrError::environment(format!(
            "Python 3.10+ required, found: {}",
            version
        )));
    }

    // Check if NoneBot is installed
    match process_utils::execute_command_with_output(
        python_path,
        &["-c", "import nonebot"],
        None,
        60,
    )
    .await
    {
        Ok(_) => {
            debug!("NoneBot is installed");
        }
        Err(_) => {
            warn!("NoneBot doesn't seem to be installed. The bot may fail to start.");
        }
    }

    Ok(())
}

/// Load environment variables from .env files
#[allow(unused)]
fn load_environment_variables(work_dir: &Path) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();

    let env_files = [".env", ".env.dev", ".env.prod"];

    for env_file in &env_files {
        let env_path = work_dir.join(env_file);
        if env_path.exists() {
            debug!("Loading environment variables from {}", env_path.display());

            let content = fs::read_to_string(&env_path)
                .map_err(|e| NbrError::io(format!("Failed to read {}: {}", env_file, e)))?;

            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                if let Some(eq_pos) = line.find('=') {
                    let key = line[..eq_pos].trim().to_string();
                    let value = line[eq_pos + 1..].trim();

                    // Remove quotes if present
                    let value = if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        &value[1..value.len() - 1]
                    } else {
                        value
                    };

                    env_vars.insert(key, value.to_string());
                }
            }
        }
    }

    debug!("Loaded {} environment variables", env_vars.len());
    Ok(env_vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    #[test]
    fn test_load_environment_variables() {
        let temp_dir = tempdir().unwrap();
        let env_path = temp_dir.path().join(".env");

        std::fs::write(
            &env_path,
            "TEST_VAR=test_value\nANOTHER_VAR=\"quoted value\"",
        )
        .unwrap();

        let result = load_environment_variables(temp_dir.path());
        assert!(result.is_ok());

        let env_vars = result.unwrap();
        assert_eq!(env_vars.len(), 2);
        assert!(env_vars.contains_key("TEST_VAR"));
        assert!(env_vars.contains_key("ANOTHER_VAR"));
    }
}
