//! Init command handler for nbr
//!
//! This module handles initializing NoneBot projects in the current directory,
//! creating necessary files and directory structure.
#![allow(unused)]

use crate::config::{ConfigManager, NbConfig};
use crate::error::{NbrError, Result};
use crate::utils::{git_utils, string_utils};

use colored::*;

use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Project initialization handler
pub struct InitHandler {
    /// Configuration manager
    config_manager: ConfigManager,
    /// Working directory
    work_dir: PathBuf,
    /// Project options
    options: InitOptions,
}

/// Project initialization options
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Project name
    pub name: String,
    /// Project description
    pub description: Option<String>,
    /// Author name
    pub author_name: Option<String>,
    /// Author email
    pub author_email: Option<String>,
    /// Python version requirement
    pub python_version: String,
    /// Initial adapters to install
    pub adapters: Vec<String>,
    /// Initial plugins to install
    pub plugins: Vec<String>,
    /// Initialize git repository
    pub init_git: bool,
    /// Create virtual environment
    pub create_venv: bool,
    /// Use poetry for dependency management
    pub use_poetry: bool,
    /// Force overwrite existing files
    pub force: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            name: "awesome-bot".to_string(),
            description: None,
            author_name: None,
            author_email: None,
            python_version: ">=3.10".to_string(),
            adapters: vec!["console".to_string()],
            plugins: vec![],
            init_git: true,
            create_venv: false,
            use_poetry: false,
            force: false,
        }
    }
}

impl InitHandler {
    /// Create a new init handler
    pub async fn new(_force: bool) -> Result<Self> {
        let config_manager = ConfigManager::new()?;
        let work_dir = config_manager.current_dir().to_path_buf();

        let mut options = InitOptions::default();
        // options.force = force;

        // Use directory name as default project name
        if let Some(dir_name) = work_dir.file_name().and_then(|n| n.to_str()) {
            options.name = string_utils::to_snake_case(dir_name);
        }

        // Use config author info if available
        let config = config_manager.config();
        if let Some(ref author) = config.user.author {
            options.author_name = Some(author.name.clone());
            options.author_email = author.email.clone();
        }

        Ok(Self {
            config_manager,
            work_dir,
            options,
        })
    }

    /// Initialize git repository
    fn init_git_repository(&self) -> Result<()> {
        info!("Initializing git repository...");

        if git_utils::is_git_repository(&self.work_dir) {
            warn!("Git repository already exists");
            return Ok(());
        }

        git_utils::init_repository(&self.work_dir, false)?;

        // Create initial commit
        let git_dir = self.work_dir.join(".git");
        if git_dir.exists() {
            info!("Git repository initialized successfully");
        }

        Ok(())
    }

    /// Create virtual environment
    async fn create_virtual_environment(&self) -> Result<()> {
        info!("Creating virtual environment...");

        let python_cmd = crate::utils::process_utils::find_python()
            .ok_or_else(|| NbrError::not_found("Python executable not found"))?;

        let venv_path = self.work_dir.join("venv");

        // Create virtual environment
        crate::utils::process_utils::execute_command_with_output(
            &python_cmd,
            &["-m", "venv", "venv"],
            Some(&self.work_dir),
            60,
        )
        .await?;

        info!("Virtual environment created at: {:?}", venv_path);

        // Install dependencies if requirements.txt exists
        let requirements_file = self.work_dir.join("requirements.txt");
        if requirements_file.exists() {
            debug!("Installing dependencies in virtual environment...");

            let uv_cmd = "uv";

            crate::utils::process_utils::execute_command_with_output(
                uv_cmd,
                &["pip", "install", "-r", "requirements.txt"],
                Some(&self.work_dir),
                300,
            )
            .await?;

            info!("Dependencies installed successfully");
        }

        Ok(())
    }

    /// Save project configuration
    async fn save_project_config(&mut self) -> Result<()> {
        self.config_manager
            .update_nb_config(|config| *config = NbConfig::default())?;

        self.config_manager.save()
    }

    /// Show completion message
    fn show_completion_message(&self) {
        println!();
        println!(
            "{}",
            "✓ Project initialized successfully!".bright_green().bold()
        );
        println!();
        println!("{}", "Next Steps:".bright_yellow().bold());

        if self.options.create_venv {
            if cfg!(windows) {
                println!(
                    "  1. Activate virtual environment: {}",
                    "venv\\Scripts\\activate".bright_cyan()
                );
            } else {
                println!(
                    "  1. Activate virtual environment: {}",
                    "source venv/bin/activate".bright_cyan()
                );
            }
        } else {
            println!(
                "  1. Install dependencies: {}",
                if self.options.use_poetry {
                    "poetry install".bright_cyan()
                } else {
                    "uv pip install -r requirements.txt".bright_cyan()
                }
            );
        }

        println!("  2. Configure your bot: {}", "edit .env".bright_cyan());
        println!("  3. Run your bot: {}", "python bot.py".bright_cyan());

        if self.options.init_git {
            println!(
                "  4. Make initial commit: {}",
                "git add . && git commit -m \"Initial commit\"".bright_cyan()
            );
        }

        println!();
        println!("For more help: {}", "nb --help".cyan());
        println!("Documentation: {}", "https://nonebot.dev/".cyan());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_handler_creation() {
        let handler = InitHandler::new(false).await;
        assert!(handler.is_ok());
    }

    #[test]
    fn test_init_options_default() {
        let options = InitOptions::default();
        assert_eq!(options.name, "nonebot-project");
        assert!(options.init_git);
        assert!(!options.create_venv);
        assert!(!options.use_poetry);
    }
}
