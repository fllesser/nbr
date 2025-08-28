//! Init command handler for nbr
//!
//! This module handles initializing NoneBot projects in the current directory,
//! creating necessary files and directory structure.
#![allow(dead_code)]

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

impl InitHandler {}
