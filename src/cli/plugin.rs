//! Plugin command handler for nb-cli
//!
//! This module handles plugin management including installation, removal,
//! listing, searching, and updating plugins from various sources.
#![allow(dead_code)]

use crate::config::{ConfigManager, PluginInfo};
use crate::error::{NbCliError, Result};
use crate::utils::{process_utils, terminal_utils};
use chrono::Utc;
use clap::ArgMatches;
use colored::*;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use std::env;

use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

/// Plugin registry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPlugin {
    /// Plugin name
    pub name: String,
    /// Plugin description
    pub description: String,
    /// Plugin version
    pub version: String,
    /// Plugin author
    pub author: String,
    /// PyPI package name
    pub pypi_name: String,
    /// Plugin homepage
    pub homepage: Option<String>,
    /// Plugin tags
    pub tags: Vec<String>,
    /// Plugin type (adapter, plugin, etc.)
    pub plugin_type: String,
    /// Supported Python versions
    pub python_requires: String,
    /// Plugin dependencies
    pub dependencies: Vec<String>,
    /// Download count
    pub downloads: u64,
    /// Last updated
    pub updated_at: String,
    /// Plugin rating
    pub rating: Option<f32>,
}

/// Plugin search result
#[derive(Debug, Clone)]
pub struct PluginSearchResult {
    /// Registry plugin info
    pub plugin: RegistryPlugin,
    /// Search relevance score
    pub score: f32,
}

/// Plugin manager
pub struct PluginManager {
    /// Configuration manager
    config_manager: ConfigManager,
    /// HTTP client for registry requests
    client: Client,
    /// Python executable path
    python_path: String,
    /// Working directory
    work_dir: PathBuf,
}

impl PluginManager {
    /// Create a new plugin manager
    pub async fn new(mut config_manager: ConfigManager) -> Result<Self> {
        config_manager.load().await?;
        let config = config_manager.config();

        let python_path = find_python_executable(config)?;
        let work_dir = env::current_dir()
            .map_err(|e| NbCliError::io(format!("Failed to get current directory: {}", e)))?;

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("nb-cli-rust")
            .build()
            .map_err(|e| NbCliError::Network(e))?;

        Ok(Self {
            config_manager,
            client,
            python_path,
            work_dir,
        })
    }

    /// Install a plugin
    pub async fn install_plugin(
        &mut self,
        name: &str,
        index_url: Option<&str>,
        upgrade: bool,
    ) -> Result<()> {
        info!("Installing plugin: {}", name);

        // Check if it's a registry plugin or PyPI package
        let plugin_info = if let Ok(registry_plugin) = self.get_registry_plugin(name).await {
            Some(registry_plugin)
        } else {
            None
        };

        let package_name = plugin_info
            .as_ref()
            .map(|p| p.pypi_name.clone())
            .unwrap_or_else(|| name.to_string());

        // Basic validation - ensure package name is not empty
        if package_name.trim().is_empty() {
            return Err(NbCliError::invalid_argument("Package name cannot be empty"));
        }

        // Check if already installed
        if !upgrade && self.is_plugin_installed(&package_name).await? {
            return Err(NbCliError::already_exists(format!(
                "Plugin '{}' is already installed. Use --upgrade to update it.",
                package_name
            )));
        }

        // Show plugin information if available
        if let Some(ref plugin) = plugin_info {
            self.display_plugin_info(plugin);

            if !Confirm::new()
                .with_prompt("Do you want to install this plugin?")
                .default(true)
                .interact()
                .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
            {
                info!("Installation cancelled by user");
                return Ok(());
            }
        }

        // Install the plugin
        self.uv_install(&package_name, index_url, upgrade).await?;

        // Update plugin registry
        let installed_plugin = PluginInfo {
            name: plugin_info
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| package_name.clone()),
            module_name: plugin_info
                .as_ref()
                .map(|p| p.name.replace("-", "_"))
                .unwrap_or_else(|| package_name.replace("-", "_")),
            version: self
                .get_installed_package_version(&package_name)
                .await
                .unwrap_or_else(|_| "unknown".to_string()),
            install_method: "uv".to_string(),
            source: index_url.unwrap_or("PyPI").to_string(),
            plugin_type: plugin_info
                .as_ref()
                .map(|p| p.plugin_type.clone())
                .unwrap_or_else(|| "external".to_string()),
            installed_at: Utc::now(),
        };

        self.add_plugin_to_config(installed_plugin.clone()).await?;

        self.add_plugin_to_pyproject(installed_plugin.clone())
            .await?;

        println!(
            "{} Successfully installed plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );

        Ok(())
    }

    pub async fn add_plugin_to_pyproject(&mut self, plugin: PluginInfo) -> Result<()> {
        let pyproject = self.config_manager.config_mut().pyproject.as_mut().unwrap();
        pyproject.tool.nonebot.plugins.push(plugin.module_name);
        self.config_manager.save().await
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&mut self, name: &str) -> Result<()> {
        info!("Uninstalling plugin: {}", name);

        // Find the plugin in configuration
        let plugin_info = self.find_installed_plugin(name)?;
        let package_name = &plugin_info.name;

        // Confirm uninstallation
        if !Confirm::new()
            .with_prompt(&format!(
                "Are you sure you want to uninstall '{}'?",
                package_name
            ))
            .default(false)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Uninstallation cancelled by user");
            return Ok(());
        }

        // Uninstall the package
        self.uv_uninstall(package_name).await?;

        // Remove from configuration
        self.remove_plugin_from_config(&plugin_info.name).await?;

        println!(
            "{} Successfully uninstalled plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );

        Ok(())
    }

    /// List installed plugins
    pub async fn list_plugins(&self, show_outdated: bool) -> Result<()> {
        let config = self.config_manager.config();

        if let Some(ref project_config) = config.project {
            let plugins = &project_config.plugins;

            if plugins.is_empty() {
                println!("{}", "No plugins installed.".bright_yellow());
                return Ok(());
            }

            println!("{}", "Installed plugins:".bright_green().bold());
            println!();

            let mut outdated_plugins = Vec::new();

            for plugin in plugins {
                if show_outdated {
                    // Check if plugin is outdated
                    if let Ok(latest_version) = self.get_latest_package_version(&plugin.name).await
                    {
                        if plugin.version != latest_version {
                            outdated_plugins.push((plugin, latest_version));
                        }
                    }
                } else {
                    // Display all plugins
                    self.display_installed_plugin(plugin);
                }
            }

            if show_outdated {
                if outdated_plugins.is_empty() {
                    println!("{}", "All plugins are up to date.".bright_green());
                } else {
                    println!("{}", "Outdated plugins:".bright_yellow().bold());
                    println!();
                    for (plugin, latest_version) in &outdated_plugins {
                        println!(
                            "  {} {} → {} {}",
                            "•".bright_blue(),
                            plugin.name.bright_white(),
                            plugin.version.red(),
                            latest_version.bright_green()
                        );
                    }
                }
            }
        } else {
            println!("{}", "No project configuration found.".bright_yellow());
        }

        Ok(())
    }

    /// Search plugins in registry
    pub async fn search_plugins(&self, query: &str, limit: usize) -> Result<()> {
        info!("Searching plugins for: {}", query);

        let spinner = terminal_utils::create_spinner(&format!("Searching for '{}'...", query));

        let results = self.search_registry_plugins(query, limit).await?;
        spinner.finish_and_clear();

        if results.is_empty() {
            println!(
                "{}",
                format!("No plugins found for '{}'.", query).bright_yellow()
            );
            return Ok(());
        }

        println!(
            "{} {}",
            "Found".bright_green(),
            format!("{} plugin(s):", results.len()).bright_white()
        );
        println!();

        for (index, result) in results.iter().enumerate() {
            if index >= limit {
                break;
            }

            self.display_search_result(result, index + 1);

            if index < results.len() - 1 && index < limit - 1 {
                println!();
            }
        }

        Ok(())
    }

    /// Update plugins
    pub async fn update_plugins(
        &mut self,
        plugin_name: Option<&str>,
        update_all: bool,
    ) -> Result<()> {
        if update_all {
            self.update_all_plugins().await
        } else if let Some(name) = plugin_name {
            self.update_single_plugin(name).await
        } else {
            Err(NbCliError::invalid_argument(
                "Either specify a plugin name or use --all flag",
            ))
        }
    }

    /// Update all plugins
    async fn update_all_plugins(&mut self) -> Result<()> {
        let config = self.config_manager.config();

        let plugins = if let Some(ref project_config) = config.project {
            project_config.plugins.clone()
        } else {
            println!("{}", "No project configuration found.".bright_yellow());
            return Ok(());
        };

        if plugins.is_empty() {
            println!("{}", "No plugins installed.".bright_yellow());
            return Ok(());
        }

        println!("{}", "Checking for plugin updates...".bright_blue());

        let mut outdated_plugins = Vec::new();
        let pb = ProgressBar::new(plugins.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] Checking {pos}/{len} plugins...")
                .unwrap(),
        );

        for plugin in &plugins {
            pb.set_message(format!("Checking {}", plugin.name));

            if let Ok(latest_version) = self.get_latest_package_version(&plugin.name).await {
                if plugin.version != latest_version {
                    outdated_plugins.push((plugin.clone(), latest_version));
                }
            }

            pb.inc(1);
        }

        pb.finish_and_clear();

        if outdated_plugins.is_empty() {
            println!("{}", "All plugins are up to date.".bright_green());
            return Ok(());
        }

        println!(
            "Found {} outdated plugin(s):",
            outdated_plugins.len().to_string().bright_yellow()
        );

        for (plugin, latest_version) in &outdated_plugins {
            println!(
                "  {} {} {} → {}",
                "•".bright_blue(),
                plugin.name.bright_white(),
                plugin.version.red(),
                latest_version.bright_green()
            );
        }

        if !Confirm::new()
            .with_prompt("Do you want to update these plugins?")
            .default(true)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Update cancelled by user");
            return Ok(());
        }

        // Update plugins
        for (plugin, _) in &outdated_plugins {
            match self.uv_install(&plugin.name, None, true).await {
                Ok(_) => {
                    println!(
                        "{} Updated {}",
                        "✓".bright_green(),
                        plugin.name.bright_blue()
                    );
                }
                Err(e) => {
                    println!(
                        "{} Failed to update {}: {}",
                        "✗".bright_red(),
                        plugin.name,
                        e
                    );
                }
            }
        }

        // Refresh plugin information
        self.refresh_plugin_info().await?;

        Ok(())
    }

    /// Update a single plugin
    async fn update_single_plugin(&mut self, name: &str) -> Result<()> {
        let plugin_info = self.find_installed_plugin(name)?;

        let latest_version = self.get_latest_package_version(&plugin_info.name).await?;

        if plugin_info.version == latest_version {
            println!(
                "{} Plugin '{}' is already up to date (v{})",
                "✓".bright_green(),
                plugin_info.name.bright_blue(),
                latest_version
            );
            return Ok(());
        }

        println!(
            "Updating {} {} → {}",
            plugin_info.name.bright_white(),
            plugin_info.version.red(),
            latest_version.bright_green()
        );

        self.uv_install(&plugin_info.name, None, true).await?;
        self.refresh_plugin_info().await?;

        println!(
            "{} Successfully updated plugin: {}",
            "✓".bright_green(),
            plugin_info.name.bright_blue()
        );

        Ok(())
    }

    /// Install package via uv
    async fn uv_install(
        &self,
        package: &str,
        index_url: Option<&str>,
        upgrade: bool,
    ) -> Result<()> {
        let mut args = vec!["add"];

        if upgrade {
            args.push("--upgrade");
        }

        if let Some(index) = index_url {
            args.push("--index-url");
            args.push(index);
        }

        args.push(package);

        let spinner = terminal_utils::create_spinner(&format!("Installing {}...", package));

        let output = process_utils::execute_command_with_output(
            "uv",
            &args,
            Some(&self.work_dir),
            300, // 5 minutes timeout
        )
        .await;

        spinner.finish_and_clear();

        // 重新读取 pyproject.toml
        // if let Some(pyproject_config) = PyProjectConfig::load().await? {
        //     self.config_manager.config_mut().pyproject = Some(pyproject_config);
        // }

        output.map(|_| ())
    }

    /// Uninstall package via uv
    async fn uv_uninstall(&self, package: &str) -> Result<()> {
        let args = vec!["remove", package];

        let spinner = terminal_utils::create_spinner(&format!("Uninstalling {}...", package));

        let output = process_utils::execute_command_with_output(
            "uv",
            &args,
            Some(&self.work_dir),
            60, // 1 minute timeout
        )
        .await;

        spinner.finish_and_clear();

        output.map(|_| ())
    }

    /// Get installed package version
    async fn get_installed_package_version(&self, package: &str) -> Result<String> {
        let output = process_utils::execute_command_with_output(
            "uv",
            &["pip", "show", package],
            Some(&self.work_dir),
            30,
        )
        .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.starts_with("Version:") {
                return Ok(line.replace("Version:", "").trim().to_string());
            }
        }

        Err(NbCliError::not_found(format!(
            "Version not found for package: {}",
            package
        )))
    }

    /// Get latest package version from PyPI
    async fn get_latest_package_version(&self, package: &str) -> Result<String> {
        let url = format!("https://pypi.org/pypi/{}/json", package);

        let response = timeout(Duration::from_secs(10), self.client.get(&url).send())
            .await
            .map_err(|_| NbCliError::unknown("Request timeout"))?
            .map_err(|e| NbCliError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbCliError::not_found(format!(
                "Package '{}' not found on PyPI",
                package
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| NbCliError::Network(e))?;

        json.get("info")
            .and_then(|info| info.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| NbCliError::not_found("Version field not found in PyPI response"))
    }

    /// Check if plugin is installed
    async fn is_plugin_installed(&self, package: &str) -> Result<bool> {
        match self.get_installed_package_version(package).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get plugin from registry
    async fn get_registry_plugin(&self, name: &str) -> Result<RegistryPlugin> {
        let config = self.config_manager.config();
        let registry_url = &config.registry.plugin_registry;

        let url = format!("{}/plugins/{}", registry_url, name);

        let response = timeout(Duration::from_secs(10), self.client.get(&url).send())
            .await
            .map_err(|_| NbCliError::unknown("Request timeout"))?
            .map_err(|e| NbCliError::Network(e))?;
        if !response.status().is_success() {
            return Err(NbCliError::not_found(format!(
                "Plugin '{}' not found in registry",
                name
            )));
        }

        let plugin_info = response
            .json::<RegistryPlugin>()
            .await
            .map_err(|e| NbCliError::plugin(format!("Failed to parse plugin info: {}", e)))?;

        Ok(plugin_info)
    }

    /// Search plugins in registry
    async fn search_registry_plugins(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PluginSearchResult>> {
        let config = self.config_manager.config();
        let registry_url = &config.registry.plugin_registry;

        let url = format!(
            "{}/search?q={}&limit={}",
            registry_url,
            urlencoding::encode(query),
            limit
        );

        let response = timeout(Duration::from_secs(10), self.client.get(&url).send())
            .await
            .map_err(|_| NbCliError::unknown("Request timeout"))?
            .map_err(|e| NbCliError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbCliError::unknown("Plugin registry search failed"));
        }

        let results: Vec<RegistryPlugin> = response
            .json()
            .await
            .map_err(|e| NbCliError::plugin(format!("Failed to parse search results: {}", e)))?;

        // Convert to search results with relevance scoring
        let search_results = results
            .into_iter()
            .map(|plugin| {
                let score = calculate_search_relevance(&plugin, query);
                PluginSearchResult { plugin, score }
            })
            .collect::<Vec<_>>();

        Ok(search_results)
    }

    /// Find installed plugin by name
    fn find_installed_plugin(&self, name: &str) -> Result<PluginInfo> {
        let config = self.config_manager.config();

        if let Some(ref project_config) = config.project {
            for plugin in &project_config.plugins {
                if plugin.name == name || plugin.name.contains(name) {
                    return Ok(plugin.clone());
                }
            }
        }

        Err(NbCliError::not_found(format!(
            "Plugin '{}' is not installed",
            name
        )))
    }

    /// Add plugin to configuration
    async fn add_plugin_to_config(&mut self, plugin: PluginInfo) -> Result<()> {
        self.config_manager
            .update_project_config(|project_config| {
                if let Some(config) = project_config {
                    // Remove existing plugin with same name
                    config.plugins.retain(|p| p.name != plugin.name);
                    // Add new plugin info
                    config.plugins.push(plugin);
                }
            })?;

        self.config_manager.save().await
    }

    /// Remove plugin from configuration
    async fn remove_plugin_from_config(&mut self, name: &str) -> Result<()> {
        self.config_manager
            .update_project_config(|project_config| {
                if let Some(config) = project_config {
                    config.plugins.retain(|p| p.name != name);
                }
            })?;

        self.config_manager.save().await
    }

    /// Refresh plugin information
    async fn refresh_plugin_info(&mut self) -> Result<()> {
        let config = self.config_manager.config();

        if let Some(ref project_config) = config.project {
            let mut updated_plugins = Vec::new();

            for plugin in &project_config.plugins {
                if let Ok(version) = self.get_installed_package_version(&plugin.name).await {
                    let mut updated_plugin = plugin.clone();
                    updated_plugin.version = version;
                    updated_plugins.push(updated_plugin);
                } else {
                    updated_plugins.push(plugin.clone());
                }
            }

            self.config_manager
                .update_project_config(|project_config| {
                    if let Some(config) = project_config {
                        config.plugins = updated_plugins;
                    }
                })?;

            self.config_manager.save().await?;
        }

        Ok(())
    }

    /// Display plugin information
    fn display_plugin_info(&self, plugin: &RegistryPlugin) {
        println!("{}", plugin.name.bright_blue().bold());
        println!("  {}", plugin.description);
        println!(
            "  {} {}",
            "Version:".bright_black(),
            plugin.version.bright_white()
        );
        println!(
            "  {} {}",
            "Author:".bright_black(),
            plugin.author.bright_white()
        );

        if let Some(ref homepage) = plugin.homepage {
            println!(
                "  {} {}",
                "Homepage:".bright_black(),
                homepage.bright_cyan()
            );
        }

        if !plugin.tags.is_empty() {
            println!(
                "  {} {}",
                "Tags:".bright_black(),
                plugin.tags.join(", ").bright_yellow()
            );
        }

        println!(
            "  {} {}",
            "Downloads:".bright_black(),
            plugin.downloads.to_string().bright_white()
        );
    }

    /// Display installed plugin
    fn display_installed_plugin(&self, plugin: &PluginInfo) {
        println!(
            "  {} {} {} ({})",
            "•".bright_blue(),
            plugin.name.bright_white(),
            format!("v{}", plugin.version).bright_green(),
            plugin.plugin_type.bright_black()
        );
    }

    /// Display search result
    fn display_search_result(&self, result: &PluginSearchResult, index: usize) {
        let plugin = &result.plugin;

        println!(
            "{}. {} {}",
            index.to_string().bright_black(),
            plugin.name.bright_blue().bold(),
            format!("v{}", plugin.version).bright_green()
        );

        println!("   {}", plugin.description);

        if !plugin.tags.is_empty() {
            println!(
                "   {} {}",
                "Tags:".bright_black(),
                plugin
                    .tags
                    .iter()
                    .take(3)
                    .map(|t| t.bright_yellow().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
}

/// Handle the plugin command
pub async fn handle_plugin(matches: &ArgMatches) -> Result<()> {
    let config_manager = ConfigManager::new()?;
    let mut plugin_manager = PluginManager::new(config_manager).await?;

    match matches.subcommand() {
        Some(("install", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            let index_url = sub_matches.get_one::<String>("index");
            let upgrade = sub_matches.get_flag("upgrade");

            plugin_manager
                .install_plugin(name, index_url.map(|s| s.as_str()), upgrade)
                .await
        }
        Some(("uninstall", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            plugin_manager.uninstall_plugin(name).await
        }
        Some(("list", sub_matches)) => {
            let outdated = sub_matches.get_flag("outdated");
            plugin_manager.list_plugins(outdated).await
        }
        Some(("search", sub_matches)) => {
            let query = sub_matches.get_one::<String>("query").unwrap();
            let limit: usize = sub_matches
                .get_one::<String>("limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(10);

            plugin_manager.search_plugins(query, limit).await
        }
        Some(("update", sub_matches)) => {
            let plugin_name = sub_matches.get_one::<String>("name");
            let update_all = sub_matches.get_flag("all");

            plugin_manager
                .update_plugins(plugin_name.map(|s| s.as_str()), update_all)
                .await
        }
        _ => Err(NbCliError::invalid_argument("Invalid plugin subcommand")),
    }
}

/// Find Python executable
fn find_python_executable(config: &crate::config::Config) -> Result<String> {
    // Use configured Python path if available
    if let Some(ref python_path) = config.user.python_path {
        if std::path::Path::new(python_path).exists() {
            return Ok(python_path.clone());
        }
    }

    // Try to find Python in project virtual environment
    let current_dir = env::current_dir()
        .map_err(|e| NbCliError::io(format!("Failed to get current directory: {}", e)))?;

    let venv_paths = [
        current_dir.join("venv").join("bin").join("python"),
        current_dir.join("venv").join("Scripts").join("python.exe"),
        current_dir.join(".venv").join("bin").join("python"),
        current_dir.join(".venv").join("Scripts").join("python.exe"),
    ];

    for venv_path in &venv_paths {
        if venv_path.exists() {
            return Ok(venv_path.to_string_lossy().to_string());
        }
    }

    // Fall back to system Python
    process_utils::find_python().ok_or_else(|| {
        NbCliError::not_found(
            "Python executable not found. Please install Python 3.8+ or configure python_path",
        )
    })
}

/// Calculate search relevance score
fn calculate_search_relevance(plugin: &RegistryPlugin, query: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let mut score = 0.0;

    // Exact name match gets highest score
    if plugin.name.to_lowercase() == query_lower {
        score += 100.0;
    } else if plugin.name.to_lowercase().contains(&query_lower) {
        score += 50.0;
    }

    // Description match
    if plugin.description.to_lowercase().contains(&query_lower) {
        score += 20.0;
    }

    // Tags match
    for tag in &plugin.tags {
        if tag.to_lowercase().contains(&query_lower) {
            score += 10.0;
        }
    }

    // Author match
    if plugin.author.to_lowercase().contains(&query_lower) {
        score += 5.0;
    }

    // Boost score based on downloads (popularity)
    score += (plugin.downloads as f32).log10() * 2.0;

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_relevance() {
        let plugin = RegistryPlugin {
            name: "nonebot-plugin-test".to_string(),
            description: "A test plugin for NoneBot".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            pypi_name: "nonebot-plugin-test".to_string(),
            homepage: None,
            tags: vec!["test".to_string(), "demo".to_string()],
            plugin_type: "plugin".to_string(),
            python_requires: ">=3.8".to_string(),
            dependencies: vec![],
            downloads: 1000,
            updated_at: "2023-01-01T00:00:00Z".to_string(),
            rating: None,
        };

        let score = calculate_search_relevance(&plugin, "test");
        assert!(score > 0.0);

        let exact_score = calculate_search_relevance(&plugin, "nonebot-plugin-test");
        assert!(exact_score > score);
    }
}
