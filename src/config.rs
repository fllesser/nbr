//! Configuration management module for nbr
//!
//! This module handles loading, saving, and managing configuration files
//! for both global user settings and project-specific configurations.
#![allow(dead_code)]

use crate::error::{NbrError, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Main configuration structure
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Cache configuration
    pub cache: CacheConfig,
    /// Registry configuration
    pub registry: RegistryConfig,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,
    /// Cache directory path
    pub directory: PathBuf,
    /// Maximum cache size in MB
    pub max_size_mb: u64,
    /// Cache TTL for different types
    pub ttl: CacheTtlConfig,
    /// Cleanup policy
    pub cleanup_policy: CacheCleanupPolicy,
}

/// Cache TTL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheTtlConfig {
    /// Plugin registry cache TTL in seconds
    pub plugins: u64,
    /// Adapter registry cache TTL in seconds
    pub adapters: u64,
}

/// Cache cleanup policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheCleanupPolicy {
    /// Clean based on age
    Age,
    /// Clean based on size (LRU)
    Size,
    /// Clean based on both age and size
    Both,
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Plugin registry URL
    pub plugin_registry: String,
    /// Adapter registry URL
    pub adapter_registry: String,
    /// Registry cache settings
    pub cache_enabled: bool,
    /// Registry timeout in seconds
    pub timeout: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: get_cache_dir(),
            max_size_mb: 100,
            ttl: CacheTtlConfig {
                plugins: 1800,  // 30 minutes
                adapters: 1800, // 30 minutes
            },
            cleanup_policy: CacheCleanupPolicy::Both,
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            plugin_registry: "https://registry.nonebot.dev/plugins.json".to_string(),
            adapter_registry: "https://registry.nonebot.dev/adapters.json".to_string(),
            cache_enabled: true,
            timeout: 30,
        }
    }
}

/// Configuration manager
pub struct ConfigManager {
    current_dir: PathBuf,
    config_dir: PathBuf,
    cache_dir: PathBuf,
    current_config: Config,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let current_dir = std::env::current_dir()?;

        let config_dir = get_config_dir();
        let cache_dir = get_cache_dir();

        // Ensure directories exist
        fs::create_dir_all(&config_dir)
            .map_err(|e| NbrError::config(format!("Failed to create config directory: {}", e)))?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| NbrError::config(format!("Failed to create cache directory: {}", e)))?;

        let current_config = Config::default();

        Ok(Self {
            current_dir,
            config_dir,
            cache_dir,
            current_config,
        })
    }

    /// TODO: Save configuration to files
    pub fn save(&self) -> Result<()> {
        debug!("Configuration saved successfully");
        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &Config {
        &self.current_config
    }

    /// Get mutable reference to configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.current_config
    }

    /// Get configuration directories
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get current directory
    pub fn current_dir(&self) -> &Path {
        &self.current_dir
    }

    /// Validate current configuration
    pub fn validate(&self) -> Result<()> {
        // Validate user config
        // if let Some(ref python_path) = self.current_config.user.python_path {
        //     if !Path::new(python_path).exists() {
        //         warn!("Python path does not exist: {}", python_path);
        //     }
        // }

        debug!("Configuration validation completed");
        Ok(())
    }

    /// Reset configuration to defaults
    pub fn reset_to_defaults(&mut self) {
        self.current_config = Config::default();
    }
}

/// Get platform-specific configuration directory
pub(crate) fn get_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "nonebot", "nbr") {
        proj_dirs.config_dir().to_path_buf()
    } else {
        // Fallback for systems without proper directory support
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".config").join("nbr")
    }
}

/// Get platform-specific cache directory
pub(crate) fn get_cache_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "nonebot", "nbr") {
        proj_dirs.cache_dir().to_path_buf()
    } else {
        // Fallback for systems without proper directory support
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".cache").join("nbr")
    }
}
