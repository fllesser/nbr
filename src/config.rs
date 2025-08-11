//! Configuration management module for nb-cli
//!
//! This module handles loading, saving, and managing configuration files
//! for both global user settings and project-specific configurations.
#![allow(dead_code)]

use crate::error::{NbCliError, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global user configuration
    pub user: UserConfig,
    /// Project-specific configuration
    pub project: Option<ProjectConfig>,
    /// Template registry configuration
    pub templates: TemplateConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Registry configuration
    pub registry: RegistryConfig,
}

/// Global user configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// Default Python executable path
    pub python_path: Option<String>,
    /// Default template to use for new projects
    pub default_template: Option<String>,
    /// Preferred package index
    pub pypi_index: Option<String>,
    /// Extra PyPI indices
    pub extra_indices: Vec<String>,
    /// User's preferred editor
    pub editor: Option<String>,
    /// Auto-reload preference
    pub auto_reload: bool,
    /// Default host for running bots
    pub default_host: String,
    /// Default port for running bots
    pub default_port: u16,
    /// Enable colored output
    pub colored_output: bool,
    /// Logging level
    pub log_level: String,
    /// Check for updates automatically
    pub auto_update_check: bool,
    /// Telemetry opt-in
    pub telemetry_enabled: bool,
    /// User information for templates
    pub author: Option<AuthorInfo>,
}

/// Author information for code generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub name: String,
    pub email: Option<String>,
    pub github_username: Option<String>,
}

/// Project-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name
    pub name: String,
    /// Project version
    pub version: String,
    /// Project description
    pub description: Option<String>,
    /// Bot entry file
    pub bot_file: String,
    /// Environment file
    pub env_file: String,
    /// Installed adapters
    pub adapters: Vec<AdapterInfo>,
    /// Installed plugins
    pub plugins: Vec<PluginInfo>,
    /// Custom scripts
    pub scripts: HashMap<String, String>,
    /// Development dependencies
    pub dev_dependencies: Vec<String>,
    /// Project-specific Python path
    pub python_path: Option<String>,
    /// Virtual environment path
    pub venv_path: Option<String>,
    /// Project creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
}

/// Template configuration and registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    /// Official template registry URL
    pub registry_url: String,
    /// Custom template sources
    pub custom_sources: Vec<TemplateSource>,
    /// Cached template information
    pub cache: HashMap<String, TemplateInfo>,
    /// Template cache TTL in seconds
    pub cache_ttl: u64,
}

/// Template source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSource {
    /// Source name/identifier
    pub name: String,
    /// Source URL (Git repository, archive, etc.)
    pub url: String,
    /// Source type (git, archive, local)
    pub source_type: String,
    /// Authentication info if needed
    pub auth: Option<AuthConfig>,
}

/// Template information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Template name
    pub name: String,
    /// Template description
    pub description: String,
    /// Template version
    pub version: String,
    /// Template author
    pub author: String,
    /// Supported adapters
    pub adapters: Vec<String>,
    /// Included plugins
    pub plugins: Vec<String>,
    /// Template tags
    pub tags: Vec<String>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
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
    /// Template cache TTL in seconds
    pub templates: u64,
    /// Plugin registry cache TTL in seconds
    pub plugins: u64,
    /// Adapter registry cache TTL in seconds
    pub adapters: u64,
    /// Version info cache TTL in seconds
    pub versions: u64,
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
    /// Registry mirrors
    pub mirrors: Vec<String>,
    /// Registry cache settings
    pub cache_enabled: bool,
    /// Registry timeout in seconds
    pub timeout: u64,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication type (token, basic, ssh)
    pub auth_type: String,
    /// Token or password
    pub token: Option<String>,
    /// Username for basic auth
    pub username: Option<String>,
    /// SSH key path
    pub ssh_key: Option<PathBuf>,
}

/// Adapter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    /// Adapter name
    pub name: String,
    /// Installed version
    pub version: String,
    /// Installation method (uv, git, local)
    pub install_method: String,
    /// Installation source
    pub source: String,
    /// Installation timestamp
    pub installed_at: DateTime<Utc>,
}

/// Plugin information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Package name
    pub package_name: String,
    /// Module name
    pub module_name: String,
    /// Installed version
    pub version: String,
    /// Installation method (uv, git, local)
    pub install_method: String,
    /// Installation source
    pub source: String,
    /// Plugin type (builtin, external)
    pub plugin_type: String,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            python_path: None,
            default_template: Some("bootstrap".to_string()),
            pypi_index: Some("https://pypi.org/simple/".to_string()),
            extra_indices: vec![],
            editor: None,
            auto_reload: false,
            default_host: "127.0.0.1".to_string(),
            default_port: 8080,
            colored_output: true,
            log_level: "info".to_string(),
            auto_update_check: true,
            telemetry_enabled: false,
            author: None,
        }
    }
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            registry_url: "https://registry.nonebot.dev/templates".to_string(),
            custom_sources: vec![],
            cache: HashMap::new(),
            cache_ttl: 3600, // 1 hour
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: get_cache_dir(),
            max_size_mb: 100,
            ttl: CacheTtlConfig {
                templates: 3600, // 1 hour
                plugins: 1800,   // 30 minutes
                adapters: 1800,  // 30 minutes
                versions: 300,   // 5 minutes
            },
            cleanup_policy: CacheCleanupPolicy::Both,
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            plugin_registry: "https://registry.nonebot.dev/plugins".to_string(),
            adapter_registry: "https://registry.nonebot.dev/adapters".to_string(),
            mirrors: vec![],
            cache_enabled: true,
            timeout: 30,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            user: UserConfig::default(),
            project: None,
            templates: TemplateConfig::default(),
            cache: CacheConfig::default(),
            registry: RegistryConfig::default(),
        }
    }
}

impl TryFrom<&toml::Value> for ProjectConfig {
    type Error = NbCliError;

    fn try_from(value: &toml::Value) -> Result<Self> {
        let table = value
            .as_table()
            .ok_or_else(|| NbCliError::config("Expected table for project config"))?;

        let name = table
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed")
            .to_string();

        let version = table
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0")
            .to_string();

        let description = table
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let bot_file = table
            .get("bot_file")
            .or_else(|| table.get("bot-file"))
            .and_then(|v| v.as_str())
            .unwrap_or("bot.py")
            .to_string();

        let env_file = table
            .get("env_file")
            .or_else(|| table.get("env-file"))
            .and_then(|v| v.as_str())
            .unwrap_or(".env")
            .to_string();

        let adapters = table
            .get("adapters")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|name| AdapterInfo {
                        name: name.to_string(),
                        version: "latest".to_string(),
                        install_method: "pip".to_string(),
                        source: format!("nonebot-adapter-{}", name),
                        installed_at: Utc::now(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let plugins = table
            .get("plugins")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|name| PluginInfo {
                        name: name.to_string(),
                        package_name: name.to_string(),
                        module_name: name.replace("-", "_"),
                        version: "latest".to_string(),
                        install_method: "pip".to_string(),
                        source: name.to_string(),
                        plugin_type: "plugin".to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let scripts = table
            .get("scripts")
            .and_then(|v| v.as_table())
            .map(|scripts_table| {
                scripts_table
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        let dev_dependencies = table
            .get("dev_dependencies")
            .or_else(|| table.get("dev-dependencies"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let python_path = table
            .get("python_path")
            .or_else(|| table.get("python-path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let venv_path = table
            .get("venv_path")
            .or_else(|| table.get("venv-path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let now = Utc::now();
        let created_at = table
            .get("created_at")
            .or_else(|| table.get("created-at"))
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);

        let modified_at = table
            .get("modified_at")
            .or_else(|| table.get("modified-at"))
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);

        Ok(ProjectConfig {
            name,
            version,
            description,
            bot_file,
            env_file,
            adapters,
            plugins,
            scripts,
            dev_dependencies,
            python_path,
            venv_path,
            created_at,
            modified_at,
        })
    }
}

/// Configuration manager
pub struct ConfigManager {
    config_dir: PathBuf,
    cache_dir: PathBuf,
    current_config: Config,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let config_dir = get_config_dir();
        let cache_dir = get_cache_dir();

        // Ensure directories exist
        fs::create_dir_all(&config_dir)
            .map_err(|e| NbCliError::config(format!("Failed to create config directory: {}", e)))?;
        fs::create_dir_all(&cache_dir)
            .map_err(|e| NbCliError::config(format!("Failed to create cache directory: {}", e)))?;

        let current_config = Config::default();

        Ok(Self {
            config_dir,
            cache_dir,
            current_config,
        })
    }

    /// Load configuration from files
    pub async fn load(&mut self) -> Result<()> {
        debug!("Loading configuration from {:?}", self.config_dir);

        // Load global user config
        let user_config_path = self.config_dir.join("config.toml");
        if user_config_path.exists() {
            let content = fs::read_to_string(&user_config_path)
                .map_err(|e| NbCliError::config(format!("Failed to read user config: {}", e)))?;

            self.current_config.user = toml::from_str(&content)
                .map_err(|e| NbCliError::config(format!("Failed to parse user config: {}", e)))?;

            info!("Loaded user configuration");
        } else {
            debug!("No user config found, using defaults");
        }

        // Load project config if in a project directory
        if let Some(project_config) = self.load_project_config().await? {
            self.current_config.project = Some(project_config);
            info!("Loaded project configuration");
        }

        Ok(())
    }

    /// Save configuration to files
    pub async fn save(&self) -> Result<()> {
        debug!("Saving configuration to {:?}", self.config_dir);

        // Save user config
        let user_config_path = self.config_dir.join("config.toml");
        let user_config_content = toml::to_string_pretty(&self.current_config.user)
            .map_err(|e| NbCliError::config(format!("Failed to serialize user config: {}", e)))?;

        fs::write(&user_config_path, user_config_content)
            .map_err(|e| NbCliError::config(format!("Failed to write user config: {}", e)))?;

        // Save project config if it exists
        if let Some(ref project_config) = self.current_config.project {
            self.save_project_config(project_config).await?;
        }

        info!("Configuration saved successfully");
        Ok(())
    }

    /// Load project configuration from current directory
    async fn load_project_config(&self) -> Result<Option<ProjectConfig>> {
        let current_dir = std::env::current_dir()
            .map_err(|e| NbCliError::config(format!("Failed to get current directory: {}", e)))?;

        let config_path = current_dir.join("nb.toml");
        if config_path.exists() {
            self.parse_project_config(&config_path).await.map(Some)
        } else {
            Ok(None)
        }
    }

    /// Parse project configuration from file
    async fn parse_project_config(&self, config_path: &Path) -> Result<ProjectConfig> {
        let content = fs::read_to_string(config_path)
            .map_err(|e| NbCliError::config(format!("Failed to read project config: {}", e)))?;

        // Try to parse as different formats based on extension
        let config = if config_path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            // For pyproject.toml, look for [tool.nonebot] section
            if config_path.file_name().and_then(|name| name.to_str()) == Some("pyproject.toml") {
                let parsed: toml::Value = toml::from_str(&content)
                    .map_err(|e| NbCliError::config(format!("Failed to parse TOML: {}", e)))?;

                if let Some(nonebot_section) =
                    parsed.get("tool").and_then(|tool| tool.get("nonebot"))
                {
                    nonebot_section.try_into().map_err(|e| {
                        NbCliError::config(format!("Failed to parse nonebot section: {}", e))
                    })?
                } else {
                    return Err(NbCliError::config(
                        "No [tool.nonebot] section found in pyproject.toml",
                    ));
                }
            } else {
                toml::from_str(&content).map_err(|e| {
                    NbCliError::config(format!("Failed to parse TOML config: {}", e))
                })?
            }
        } else {
            return Err(NbCliError::config("Unsupported config file format"));
        };

        Ok(config)
    }

    /// Save project configuration
    async fn save_project_config(&self, project_config: &ProjectConfig) -> Result<()> {
        let current_dir = std::env::current_dir()
            .map_err(|e| NbCliError::config(format!("Failed to get current directory: {}", e)))?;

        let config_path = current_dir.join("nb.toml");
        let config_content = toml::to_string_pretty(project_config).map_err(|e| {
            NbCliError::config(format!("Failed to serialize project config: {}", e))
        })?;

        fs::write(&config_path, config_content)
            .map_err(|e| NbCliError::config(format!("Failed to write project config: {}", e)))?;

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

    /// Update user configuration
    pub fn update_user_config<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut UserConfig),
    {
        f(&mut self.current_config.user);
        Ok(())
    }

    /// Update project configuration
    pub fn update_project_config<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Option<ProjectConfig>),
    {
        f(&mut self.current_config.project);
        Ok(())
    }

    /// Get configuration directories
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Validate current configuration
    pub fn validate(&self) -> Result<()> {
        // Validate user config
        if let Some(ref python_path) = self.current_config.user.python_path {
            if !Path::new(python_path).exists() {
                warn!("Python path does not exist: {}", python_path);
            }
        }

        // Validate project config
        if let Some(ref project_config) = self.current_config.project {
            let current_dir = std::env::current_dir().map_err(|e| {
                NbCliError::config(format!("Failed to get current directory: {}", e))
            })?;

            let bot_file_path = current_dir.join(&project_config.bot_file);
            if !bot_file_path.exists() {
                warn!("Bot file does not exist: {}", project_config.bot_file);
            }

            let env_file_path = current_dir.join(&project_config.env_file);
            if !env_file_path.exists() {
                debug!(
                    "Environment file does not exist: {}",
                    project_config.env_file
                );
            }
        }

        info!("Configuration validation completed");
        Ok(())
    }

    /// Reset configuration to defaults
    pub fn reset_to_defaults(&mut self) {
        self.current_config = Config::default();
    }
}

/// Get platform-specific configuration directory
fn get_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "nonebot", "nb-cli") {
        proj_dirs.config_dir().to_path_buf()
    } else {
        // Fallback for systems without proper directory support
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".config").join("nb-cli")
    }
}

/// Get platform-specific cache directory
fn get_cache_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("dev", "nonebot", "nb-cli") {
        proj_dirs.cache_dir().to_path_buf()
    } else {
        // Fallback for systems without proper directory support
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".cache").join("nb-cli")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.user.default_host, "127.0.0.1");
        assert_eq!(config.user.default_port, 8080);
        assert!(config.user.colored_output);
        assert!(config.cache.enabled);
    }

    #[test]
    fn test_user_config_serialization() {
        let config = UserConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: UserConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.default_host, deserialized.default_host);
        assert_eq!(config.default_port, deserialized.default_port);
    }

    #[tokio::test]
    async fn test_config_manager_creation() {
        let manager = ConfigManager::new();
        assert!(manager.is_ok());
    }
}
