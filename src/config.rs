//! Configuration management module for nbr
//!
//! This module handles loading, saving, and managing configuration files
//! for both global user settings and project-specific configurations.

use crate::error::Result;
use directories::ProjectDirs;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Get platform-specific configuration directory
#[allow(unused)]
pub(crate) fn get_config_dir() -> Result<PathBuf> {
    let config_dir = if let Some(proj_dirs) = ProjectDirs::from("dev", "nonebot", "nbr") {
        proj_dirs.config_dir().to_path_buf()
    } else {
        // Fallback for systems without proper directory support
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".config").join("nbr")
    };
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
        debug!("Created config directory: {}", config_dir.display());
    }
    Ok(config_dir)
}

/// Get platform-specific cache directory
pub(crate) fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = if let Some(proj_dirs) = ProjectDirs::from("dev", "nonebot", "nbr") {
        proj_dirs.cache_dir().to_path_buf()
    } else {
        // Fallback for systems without proper directory support
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".cache").join("nbr")
    };
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)?;
        debug!("Created cache directory: {}", cache_dir.display());
    }
    Ok(cache_dir)
}
