//! Init command handler for nbr
//!
//! This module handles initializing NoneBot projects in the current directory,
//! creating necessary files and directory structure.
#![allow(dead_code)]

use crate::error::Result;
use crate::utils::string_utils;

use colored::Colorize;

use std::path::PathBuf;

/// Project initialization handler
pub struct InitHandler {
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
    /// Create virtual environment
    pub create_venv: bool,
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
            python_version: "3.10".to_string(),
            adapters: vec!["OneBot V11".to_string()],
            plugins: vec!["echo".to_string()],
            create_venv: true,
            force: false,
        }
    }
}

impl InitHandler {
    /// Create a new init handler
    pub fn new(force: bool) -> Result<Self> {
        let work_dir = std::env::current_dir()?;

        let mut options = InitOptions::default();
        options.force = force;

        // Use directory name as default project name
        if let Some(dir_name) = work_dir.file_name().and_then(|n| n.to_str()) {
            options.name = string_utils::to_snake_case(dir_name);
        }

        Ok(Self { work_dir, options })
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

        if !self.options.create_venv {
            println!("  • Install dependencies: {}", "uv sync".bright_cyan());
        }

        println!("  • Configure your bot: {}", "vim .env".bright_cyan());
        println!("  • Run your bot: {}", "nbr run".bright_cyan());

        println!();
        println!("For more help: {}", "nbr --help".cyan());
        println!(
            "Documentation: {}",
            "https://github.com/fllesser/nbr".cyan()
        );
    }
}
