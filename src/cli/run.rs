//! Run command handler for nbr
//!
//! This module handles running NoneBot applications with various options
//! including auto-reload, custom host/port, and environment management.

use crate::error::{NbrError, Result};
use crate::utils::process_utils;
use clap::ArgMatches;
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::env;
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
        println!("{}", "File watcher setup for auto-reload".bright_green());
        Ok(())
    }

    /// Start the bot process
    pub async fn run(&mut self) -> Result<()> {
        // Validate bot file exists
        if !self.bot_file.exists() {
            return Err(NbrError::not_found(format!(
                "Bot file not found: {}",
                self.bot_file.display()
            )));
        }

        // Setup signal handling for graceful shutdown
        let process_handle = Arc::clone(&self.current_process);
        tokio::spawn(async move {
            let _ = signal::ctrl_c().await;

            println!(
                " {}",
                "Received interrupt signal, shutting down...".bright_yellow()
            );
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
            println!("{}", "Bot process exited successfully".green().bold());
        } else {
            let exit_code = exit_status.code().unwrap_or(-1);
            println!(
                "{}",
                format!("❌ Bot process failed with exit code: {}", exit_code).bright_red()
            );
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

                    println!(
                        "{}",
                        "Bot started successfully with auto-reload enabled".green()
                    );
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
                            warn!("Too many rapid restarts, adding delay...");
                            sleep(Duration::from_secs(5)).await;
                        }
                    }
                    last_restart = now;

                    println!("{}", "Restarting bot...".bright_yellow());
                    sleep(Duration::from_millis(500)).await;
                }
                Err(e) => {
                    println!(
                        "{}",
                        format!("Failed to start bot process: {}", e).bright_red()
                    );
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Ok(())
    }

    /// Wait for reload trigger (file change or process exit)
    async fn wait_for_reload_trigger(&self) -> Result<bool> {
        if let Some(ref watch_rx) = self.watch_rx {
            let mut ignored_extensions = HashSet::new();
            ignored_extensions.extend([
                "pyc",
                "pyo",
                "__pycache__",
                ".git",
                ".pytest_cache",
                "node_modules",
                ".venv",
                "venv",
                ".env",
            ]);

            loop {
                // Check if process is still running
                {
                    let mut process_guard = self.current_process.lock().unwrap();
                    if let Some(process) = process_guard.as_mut() {
                        match process.try_wait() {
                            Ok(Some(status)) => {
                                info!("Bot process exited with status: {:?}", status);
                                return Ok(false); // Process exited, don't reload
                            }
                            Ok(None) => {
                                // Process still running
                            }
                            Err(e) => {
                                error!("Error checking process status: {}", e);
                                return Ok(false);
                            }
                        }
                    }
                }

                // Check for file changes
                match watch_rx.try_recv() {
                    Ok(event) => {
                        if self.should_reload_for_event(&event, &ignored_extensions) {
                            info!("File change detected, reloading bot...");
                            return Ok(true);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // No events, continue waiting
                        sleep(Duration::from_millis(100)).await;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        error!("File watcher disconnected");
                        return Ok(false);
                    }
                }
            }
        }

        // If no watch receiver, just wait for process to exit
        loop {
            let should_sleep = {
                let mut process_guard = self.current_process.lock().unwrap();
                match process_guard.as_mut() {
                    Some(process) => match process.try_wait() {
                        Ok(Some(_)) => return Ok(false),
                        Ok(None) => true, // 需要 sleep
                        Err(e) => {
                            error!("Error checking process status: {}", e);
                            return Ok(false);
                        }
                    },
                    None => return Ok(false),
                }
            };

            if should_sleep {
                sleep(Duration::from_millis(100)).await;
            }
        }
    }

    /// Check if an event should trigger a reload
    fn should_reload_for_event(&self, event: &Event, ignored_extensions: &HashSet<&str>) -> bool {
        match event.kind {
            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                for path in &event.paths {
                    // Skip hidden files and directories
                    if let Some(name) = path.file_name().and_then(|n| n.to_str())
                        && name.starts_with('.')
                    {
                        continue;
                    }

                    // Skip ignored extensions
                    if let Some(extension) = path.extension().and_then(|ext| ext.to_str())
                        && ignored_extensions.contains(extension)
                    {
                        continue;
                    }

                    // Skip ignored directories
                    if let Some(path_str) = path.to_str()
                        && ignored_extensions.iter().any(|&ext| path_str.contains(ext))
                    {
                        continue;
                    }

                    // Only reload for Python files or config files
                    if let Some(extension) = path.extension().and_then(|ext| ext.to_str())
                        && matches!(extension, "py" | "toml" | "yaml" | "yml" | "json" | "env")
                    {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Start the bot process
    fn start_bot_process(&self) -> Result<Child> {
        let mut cmd = Command::new(self.python_path.clone());
        cmd.arg(&self.bot_file)
            .current_dir(&self.work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let process = cmd
            .spawn()
            .map_err(|e| NbrError::io(format!("Failed to start bot process: {}", e)))?;

        debug!("Bot process started with PID: {:?}", process.id());
        Ok(process)
    }

    /// Kill current process
    fn kill_current_process(&self) {
        let mut process_guard = self.current_process.lock().unwrap();
        if let Some(mut process) = process_guard.take() {
            info!("Stopping bot process...");

            // Try graceful shutdown first
            if let Err(e) = process.kill() {
                warn!("Failed to kill process gracefully: {}", e);
            }

            // Wait for process to exit
            match process.wait() {
                Ok(status) => {
                    debug!("Process exited with status: {:?}", status);
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
pub async fn handle_run(matches: &ArgMatches) -> Result<()> {
    let bot_file = matches
        .get_one::<String>("file")
        .map(|s| s.as_str())
        .unwrap_or("bot.py");

    let reload = matches.get_flag("reload");
    // Load configuration
    let work_dir = std::env::current_dir().unwrap();

    // Find bot file
    let bot_file_path = find_bot_file(&work_dir, bot_file)?;

    // Find Python executable
    let python_path = find_python_executable()?;

    // Verify Python environment
    // verify_python_environment(&python_path).await?;

    // Create and run bot
    let mut runner = BotRunner::new(bot_file_path, python_path, reload, work_dir)?;

    println!("{}", "Starting NoneBot Application...".bright_green());
    println!(
        "{} {}",
        "Using Python:".bright_blue(),
        runner.python_path.bright_green()
    );

    if reload {
        println!(
            "{} {}",
            "Auto-reload:".bright_blue(),
            "enabled".bright_green()
        );
    }

    runner.run().await
}

/// Find bot entry file
fn find_bot_file(work_dir: &Path, bot_file: &str) -> Result<PathBuf> {
    let bot_path = work_dir.join(bot_file);

    if bot_path.exists() {
        return Ok(bot_path);
    }

    // Try common bot file names
    let common_names = ["bot.py", "app.py", "main.py", "run.py"];
    for name in &common_names {
        let path = work_dir.join(name);
        if path.exists() {
            info!("Found bot file: {}", path.display());
            return Ok(path);
        }
    }

    // 询问用户是否创建bot文件
    let need_create_bot_file = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Bot file '{bot_file}' not found. Do you want to create it"
        ))
        .default(false)
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;

    if !need_create_bot_file {
        return Err(NbrError::not_found(format!(
            "Bot file '{}' not found. Tried: {}",
            bot_file,
            common_names.join(", ")
        )));
    }

    // 创建bot文件
    let bot_file_content = include_str!("nbfile/bot.py");
    fs::write(&bot_path, bot_file_content)
        .map_err(|e| NbrError::io(format!("Failed to create bot file: {}", e)))?;

    Ok(bot_path)
}

/// Find Python executabled
fn find_python_executable() -> Result<String> {
    // Try to find Python in project virtual environment
    let current_dir = env::current_dir()
        .map_err(|e| NbrError::io(format!("Failed to get current directory: {}", e)))?;

    let venv_paths = [
        current_dir.join("venv").join("bin").join("python"),
        current_dir.join("venv").join("Scripts").join("python.exe"),
        current_dir.join(".venv").join("bin").join("python"),
        current_dir.join(".venv").join("Scripts").join("python.exe"),
    ];

    for venv_path in &venv_paths {
        if venv_path.exists() {
            debug!("Using virtual environment Python: {}", venv_path.display());
            return Ok(venv_path.to_string_lossy().to_string());
        }
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
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_find_bot_file() {
        let temp_dir = tempdir().unwrap();
        let bot_path = temp_dir.path().join("bot.py");
        File::create(&bot_path).unwrap();

        let result = find_bot_file(temp_dir.path(), "bot.py");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), bot_path);
    }

    #[test]
    fn test_find_bot_file_fallback() {
        let temp_dir = tempdir().unwrap();
        let app_path = temp_dir.path().join("app.py");
        File::create(&app_path).unwrap();

        let result = find_bot_file(temp_dir.path(), "nonexistent.py");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), app_path);
    }

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
