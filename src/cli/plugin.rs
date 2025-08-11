//! Plugin command handler for nb-cli
//!
//! This module handles plugin management including installation, removal,
//! listing, searching, and updating plugins from various sources.
#![allow(dead_code)]

use crate::config::{ConfigManager, PluginInfo};
use crate::error::{NbCliError, Result};
use crate::pyproject::PyProjectConfig;
use crate::utils::{process_utils, terminal_utils};
use clap::ArgMatches;
use colored::*;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::env;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

//  "module_name": "nonebot_plugin_status",
// "project_link": "nonebot-plugin-status",
// "name": "服务器状态查看",
// "desc": "通过戳一戳获取服务器状态",
// "author": "yanyongyu",
// "homepage": "https://github.com/nonebot/plugin-status",
// "tags": [
//     {
//         "label": "server",
//         "color": "#aeeaa8"
//     }
// ],
// "is_official": true,
// "type": "application",
// "supported_adapters": null,
// "valid": true,
// "time": "2024-09-03T09:20:59.379554Z",
// "version": "0.9.0",
// "skip_test": false
// },
/// Plugin registry information

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPlugin {
    pub module_name: String,
    pub project_link: String,
    pub name: String,
    pub desc: String,
    pub author: String,
    pub homepage: Option<String>,
    pub tags: Vec<HashMap<String, String>>,
    pub is_official: bool,
    #[serde(rename = "type")]
    pub plugin_type: Option<String>,
    pub supported_adapters: Option<Vec<String>>,
    pub valid: bool,
    pub time: String,
    pub version: String,
    pub skip_test: bool,
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
    /// Registry plugins
    registry_plugins: OnceLock<HashMap<String, RegistryPlugin>>,
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
            registry_plugins: OnceLock::new(),
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
        let registry_plugin = self.get_registry_plugin(name).await?;
        let package_name = registry_plugin.project_link.clone();
        // Check if already installed
        if !upgrade && self.is_plugin_installed(&package_name).await? {
            return Err(NbCliError::already_exists(format!(
                "Plugin '{}' is already installed. Use --upgrade to update it.",
                registry_plugin.project_link
            )));
        }

        // Show plugin information if available
        self.display_plugin_info(&registry_plugin);

        if !Confirm::new()
            .with_prompt("Do you want to install this plugin?")
            .default(true)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Installation cancelled by user");
            return Ok(());
        }
        // Install the plugin
        self.uv_install(&package_name, index_url, upgrade).await?;

        PyProjectConfig::add_plugin(&registry_plugin.module_name).await?;

        println!(
            "{} Successfully installed plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );

        Ok(())
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&mut self, name: &str) -> Result<()> {
        info!("Uninstalling plugin: {}", name);

        let registry_plugin = self.get_registry_plugin(name).await?;
        let package_name = registry_plugin.project_link.clone();
        // Check if already installed
        if !self.is_plugin_installed(&package_name).await? {
            return Err(NbCliError::not_found(format!(
                "Plugin '{}' is not installed.",
                registry_plugin.project_link
            )));
        }
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
        self.uv_uninstall(&package_name).await?;

        // Remove from configuration
        //self.remove_plugin_from_config(&plugin_info.name).await?;
        PyProjectConfig::remove_plugin(&registry_plugin.module_name).await?;
        println!(
            "{} Successfully uninstalled plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );

        Ok(())
    }

    /// List installed plugins
    pub async fn list_plugins(&self, _show_outdated: bool) -> Result<()> {
        // let config = self.config_manager.config();
        let pyproject = PyProjectConfig::load().await?;
        if pyproject.is_none() {
            println!(
                "{}",
                "can't found pyproject.toml configuration found.".bright_yellow()
            );
            return Ok(());
        }
        let pyproject = pyproject.unwrap();
        let plugins = pyproject
            .tool
            .nonebot
            .plugins
            .iter()
            .map(|p| p.replace("_", "-"))
            .collect::<Vec<String>>();

        for package_name in plugins {
            let plugin = self.get_registry_plugin(&package_name).await?;
            let installed_version = self.get_installed_package_version(&package_name).await?;
            let mut plugin_display = format!(
                "  {} {} v{}",
                "•".bright_blue(),
                package_name.bright_white(),
                installed_version.bright_green(),
            );
            if installed_version != plugin.version {
                plugin_display += format!(" (available: {})", plugin.version)
                    .bright_yellow()
                    .to_string()
                    .as_str();
            }
            println!("{}", plugin_display);
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
        let registry_plugin = self.get_registry_plugin(name).await?;
        let package_name = registry_plugin.project_link.clone();
        let installed_version = self.get_installed_package_version(&package_name).await?;
        // Check if already installed
        if !self.is_plugin_installed(&package_name).await? {
            return Err(NbCliError::not_found(format!(
                "Plugin '{}' is not installed.",
                registry_plugin.project_link
            )));
        }

        if registry_plugin.version == installed_version {
            println!(
                "{} Plugin '{}' is already up to date (v{})",
                "✓".bright_green(),
                registry_plugin.project_link.bright_blue(),
                installed_version
            );
            return Ok(());
        }

        println!(
            "Updating {} {} → {}",
            registry_plugin.project_link.bright_blue(),
            installed_version.red(),
            registry_plugin.version.bright_green()
        );
        let plugin_deps_str = format!(
            "{} >= {}",
            registry_plugin.project_link, registry_plugin.version
        );
        self.uv_install(&plugin_deps_str, None, true).await?;

        // Add plugin to tool.nonnebot.plugins
        PyProjectConfig::add_plugin(&registry_plugin.module_name).await?;

        self.refresh_plugin_info().await?;

        println!(
            "{} Successfully updated plugin: {}",
            "✓".bright_green(),
            registry_plugin.project_link.bright_blue()
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

    async fn get_regsitry_plugins_map(&self) -> Result<&HashMap<String, RegistryPlugin>> {
        if let Some(plugins) = self.registry_plugins.get() {
            return Ok(plugins);
        }

        let plugins_json_url = "https://registry.nonebot.dev/plugins.json";
        let response = timeout(
            Duration::from_secs(10),
            self.client.get(plugins_json_url).send(),
        )
        .await
        .map_err(|_| NbCliError::unknown("Request timeout"))?
        .map_err(|e| NbCliError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbCliError::not_found("Plugin registry not found"));
        }

        let plugins: Vec<RegistryPlugin> = response
            .json()
            .await
            .map_err(|e| NbCliError::plugin(format!("Failed to parse plugin info: {}", e)))?;

        let mut plugins_map = HashMap::new();
        for plugin in plugins {
            plugins_map.insert(plugin.project_link.clone(), plugin);
        }

        self.registry_plugins.set(plugins_map).unwrap();

        Ok(self.registry_plugins.get().unwrap())
    }

    /// Get plugin from registry
    async fn get_registry_plugin(&self, package_name: &str) -> Result<RegistryPlugin> {
        let plugins = self.get_regsitry_plugins_map().await?;
        Ok(plugins.get(package_name).unwrap().clone())
    }

    /// Search plugins in registry
    async fn search_registry_plugins(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<PluginSearchResult>> {
        let plugins_map = self.get_regsitry_plugins_map().await?;

        let results: Vec<RegistryPlugin> = plugins_map
            .values()
            .filter(|plugin| {
                plugin.project_link.contains(query)
                    || plugin.name.contains(query)
                    || plugin.desc.contains(query)
                    || plugin.author.contains(query)
            })
            .take(limit)
            .cloned()
            .collect();

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
        println!("  {}", plugin.desc);
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
                plugin
                    .tags
                    .iter()
                    .map(|t| t.get("label").unwrap().bright_yellow().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    /// Display installed plugin
    fn display_installed_plugin(&self, plugin: &PluginInfo) {
        // • nonebot-plugin-abs v0.15.11 (available: v0.16.0)
        println!(
            "  {} {} {}",
            "•".bright_blue(),
            plugin.package_name.bright_white(),
            format!("v{}", plugin.version).bright_green(),
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

        println!("   {}", plugin.desc);

        if !plugin.tags.is_empty() {
            println!(
                "   {} {}",
                "Tags:".bright_black(),
                plugin
                    .tags
                    .iter()
                    .take(3)
                    .map(|t| t.get("label").unwrap().bright_yellow().to_string())
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
    if plugin.desc.to_lowercase().contains(&query_lower) {
        score += 20.0;
    }

    // Author match
    if plugin.author.to_lowercase().contains(&query_lower) {
        score += 5.0;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_regsitry_plugins_map() {
        let plugin_manager = PluginManager::new(ConfigManager::new().unwrap())
            .await
            .unwrap();
        let plugins = plugin_manager.get_regsitry_plugins_map().await.unwrap();
        for (_, plugin) in plugins {
            println!(
                "{} {} ({})",
                plugin.project_link.bright_green(),
                format!("v{}", plugin.version).bright_yellow(),
                plugin.name.bright_blue()
            );
        }
    }

    #[tokio::test]
    async fn test_get_registry_plugin() {
        let plugin_manager = PluginManager::new(ConfigManager::new().unwrap())
            .await
            .unwrap();
        let plugin = plugin_manager
            .get_registry_plugin("nonebot-plugin-status")
            .await
            .unwrap();
        println!("{}", plugin.project_link);
        println!("{}", plugin.name);
        println!("{}", plugin.desc);
        println!("{}", plugin.author);
        println!("{:?}", plugin.homepage);
        println!("{:?}", plugin.tags);
        println!("{:?}", plugin.plugin_type);
        println!("{:?}", plugin.supported_adapters);
    }
}
