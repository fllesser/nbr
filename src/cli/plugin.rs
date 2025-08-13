//! Plugin command handler for nbr
//!
//! This module handles plugin management including installation, removal,
//! listing, searching, and updating plugins from various sources.
#![allow(dead_code)]

use crate::error::{NbrError, Result};
use crate::pyproject::ToolNonebot;
use crate::utils::{process_utils, terminal_utils};
use crate::uv::Uv;
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
use tracing::{debug, info};

// "module_name": "nonebot_plugin_status",
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

pub struct PyPIPlugin {
    pub package_name: String,
    pub module_name: String,
    pub version: String,

    pub author: String,
    pub homepage: Option<String>,
    pub description: String,
}

pub struct GitRepoPlugin {
    pub repo_url: String,
    pub package_name: String,
    pub module_name: String,

    pub version: String,
    pub author: String,
    pub description: String,
}

/// Plugin manager
pub struct PluginManager {
    /// HTTP client for registry requests
    client: Client,
    /// Working directory
    work_dir: PathBuf,
    /// Registry plugins, key is package name
    registry_plugins: OnceLock<Vec<RegistryPlugin>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Result<Self> {
        let work_dir = std::env::current_dir().unwrap();

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("nbr")
            .build()
            .map_err(|e| NbrError::Network(e))?;

        Ok(Self {
            client,
            work_dir,
            registry_plugins: OnceLock::new(),
        })
    }

    pub async fn install_plugin_from_github(&mut self, repo_url: &str) -> Result<()> {
        debug!("Installing plugin from github: {}", repo_url);

        Uv::add_from_github(repo_url, Some(&self.work_dir)).await?;

        // 获取 module_name
        let repo_name = repo_url.split("/").last().unwrap();
        let module_name = repo_name.replace("-", "_");

        // Add to configuration
        ToolNonebot::parse(None)?.add_plugins(vec![module_name])?;

        println!(
            "{} Successfully installed plugin: {}",
            "✓".bright_green(),
            repo_name.bright_blue()
        );
        Ok(())
    }

    pub async fn install_unofficial_plugin(&mut self, package_name: &str) -> Result<()> {
        debug!("Installing unofficial plugin: {}", package_name);

        // Install the plugin
        Uv::add(vec![package_name], false, None, Some(&self.work_dir)).await?;
        let module_name = package_name.replace("-", "_");

        // Add to configuration
        ToolNonebot::parse(None)?.add_plugins(vec![module_name])?;

        println!(
            "{} Successfully installed plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );
        Ok(())
    }
    /// Install a plugin
    pub async fn install_plugin(
        &self,
        name: &str,
        index_url: Option<&str>,
        upgrade: bool,
    ) -> Result<()> {
        debug!("Installing plugin: {}", name);

        // Check if it's a registry plugin or PyPI package
        let registry_plugin = self.get_registry_plugin(name).await?;
        let package_name = registry_plugin.project_link.clone();
        // Check if already installed
        if !upgrade && Uv::is_installed(&package_name, Some(&self.work_dir)).await {
            return Err(NbrError::already_exists(format!(
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
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            println!("Installation cancelled by user");
            return Ok(());
        }
        // Install the plugin
        Uv::add(
            vec![&package_name],
            upgrade,
            index_url,
            Some(&self.work_dir),
        )
        .await?;

        // Add to configuration
        ToolNonebot::parse(None)?.add_plugins(vec![registry_plugin.module_name.clone()])?;

        println!(
            "{} Successfully installed plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );

        Ok(())
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&self, name: &str) -> Result<()> {
        debug!("Uninstalling plugin: {}", name);

        let registry_plugin = self.get_registry_plugin(name).await?;
        let package_name = registry_plugin.project_link.clone();
        // Check if already installed
        if !Uv::is_installed(&package_name, Some(&self.work_dir)).await {
            return Err(NbrError::not_found(format!(
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
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            println!("Uninstallation cancelled by user");
            return Ok(());
        }

        // Uninstall the package
        Uv::remove(vec![&package_name], Some(&self.work_dir)).await?;

        ToolNonebot::parse(None)?.remove_plugins(vec![registry_plugin.module_name.clone()])?;
        // self.remove_plugin_in_config(&registry_plugin.module_name.to_string())
        //     .await?;

        println!(
            "{} Successfully uninstalled plugin: {}",
            "✓".bright_green(),
            package_name.bright_blue()
        );

        Ok(())
    }

    /// List installed plugins
    pub async fn list_plugins(&self, _show_outdated: bool) -> Result<()> {
        let nonebot = ToolNonebot::parse(None)?.nonebot()?;

        let registry_plugins = self.module_plugins_map().await?;

        let plugins: Vec<&RegistryPlugin> = nonebot
            .plugins
            .iter()
            .filter_map(|module| registry_plugins.get(module.as_str()).cloned())
            .collect();

        if plugins.is_empty() {
            println!("{}", "No plugins installed.".bright_yellow());
            return Ok(());
        }

        println!("{}", "Installed Plugins:".bright_green().bold());

        for plugin in plugins {
            let installed_version =
                Uv::get_installed_version(&plugin.project_link, Some(&self.work_dir)).await?;

            let mut plugin_display = format!(
                "  {} {} {}",
                "•".bright_blue(),
                plugin.project_link.bright_white(),
                format!("v{}", installed_version).bright_green(),
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
        debug!("Searching plugins for: {}", query);

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
            Err(NbrError::invalid_argument(
                "Either specify a plugin name or use --all flag",
            ))
        }
    }

    /// Update all plugins
    async fn update_all_plugins(&self) -> Result<()> {
        let nonebot = ToolNonebot::parse(None)?.nonebot()?;

        if nonebot.plugins.is_empty() {
            println!("{}", "No plugins installed.".bright_yellow());
            return Ok(());
        }

        println!("{}", "Checking for plugin updates...".bright_blue());

        // registry plugins, installed plugins
        let mut outdated_plugins = Vec::new();
        let pb = ProgressBar::new(nonebot.plugins.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} Checking {pos}/{len} plugins...")
                .unwrap(),
        );

        for plugin_module in nonebot.plugins {
            pb.set_message(format!("Checking {}", plugin_module));
            let module_plugins_map = self.module_plugins_map().await?;
            let plugin = module_plugins_map
                .get(plugin_module.as_str())
                .ok_or_else(|| {
                    NbrError::not_found(format!("Plugin '{}' not found", plugin_module))
                })?;
            let installed_version =
                Uv::get_installed_version(&plugin.project_link, Some(&self.work_dir)).await?;
            let latest_version = self
                .get_latest_package_version(&plugin.project_link)
                .await?;

            if installed_version != latest_version {
                outdated_plugins.push((*plugin, installed_version.clone()));
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

        for (plugin, installed_version) in &outdated_plugins {
            println!(
                "  {} {} {} → {}",
                "•".bright_blue(),
                plugin.project_link.bright_white(),
                installed_version.red(),
                plugin.version.bright_green()
            );
        }

        if !Confirm::new()
            .with_prompt("Do you want to update these plugins?")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Update cancelled by user");
            return Ok(());
        }

        // Update plugins
        for (plugin, _) in &outdated_plugins {
            match Uv::add(vec![&plugin.project_link], true, None, Some(&self.work_dir)).await {
                Ok(_) => {
                    println!(
                        "{} Updated {}",
                        "✓".bright_green(),
                        plugin.project_link.bright_blue()
                    );
                }
                Err(e) => {
                    println!(
                        "{} Failed to update {}: {}",
                        "✗".bright_red(),
                        plugin.project_link,
                        e
                    );
                }
            }
        }

        // Refresh plugin information
        // self.refresh_plugin_info().await?;

        Ok(())
    }

    async fn update_installed_github_plugin(&mut self) -> Result<()> {
        // uv sync
        Uv::sync(Some(&self.work_dir)).await?;
        Ok(())
    }

    /// Update a single plugin
    async fn update_single_plugin(&self, name: &str) -> Result<()> {
        let registry_plugin = self.get_registry_plugin(name).await?;
        let package_name = registry_plugin.project_link.clone();
        let installed_version =
            Uv::get_installed_version(&package_name, Some(&self.work_dir)).await?;
        // Check if already installed
        if !Uv::is_installed(&package_name, Some(&self.work_dir)).await {
            return Err(NbrError::not_found(format!(
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
        Uv::add(vec![&plugin_deps_str], true, None, Some(&self.work_dir)).await?;

        println!(
            "{} Successfully updated plugin: {}",
            "✓".bright_green(),
            registry_plugin.project_link.bright_blue()
        );
        // self.refresh_plugin_info().await?;

        Ok(())
    }

    /// Get latest package version from PyPI
    async fn get_latest_package_version(&self, package: &str) -> Result<String> {
        let url = format!("https://pypi.org/pypi/{}/json", package);

        let response = timeout(Duration::from_secs(10), self.client.get(&url).send())
            .await
            .map_err(|_| NbrError::unknown("Request timeout"))?
            .map_err(|e| NbrError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbrError::not_found(format!(
                "Package '{}' not found on PyPI",
                package
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| NbrError::Network(e))?;

        json.get("info")
            .and_then(|info| info.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| NbrError::not_found("Version field not found in PyPI response"))
    }

    pub async fn fetch_registry_plugins(&self) -> Result<&Vec<RegistryPlugin>> {
        if let Some(plugins) = self.registry_plugins.get() {
            return Ok(plugins);
        }

        let plugins_json_url = "https://registry.nonebot.dev/plugins.json";
        let response = timeout(
            Duration::from_secs(10),
            self.client.get(plugins_json_url).send(),
        )
        .await
        .map_err(|_| NbrError::unknown("Request timeout"))?
        .map_err(|e| NbrError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbrError::not_found("Plugin registry not found"));
        }

        let plugins: Vec<RegistryPlugin> = response
            .json()
            .await
            .map_err(|e| NbrError::plugin(format!("Failed to parse plugin info: {}", e)))?;

        self.registry_plugins.set(plugins).unwrap();

        Ok(self.registry_plugins.get().unwrap())
    }

    pub async fn package_plugins_map(&self) -> Result<HashMap<&str, &RegistryPlugin>> {
        let plugins = self.fetch_registry_plugins().await?;

        let mut plugins_map = HashMap::new();
        for plugin in plugins {
            plugins_map.insert(plugin.project_link.as_str(), plugin);
        }

        Ok(plugins_map)
    }

    pub async fn module_plugins_map(&self) -> Result<HashMap<&str, &RegistryPlugin>> {
        let plugins = self.fetch_registry_plugins().await?;
        let mut plugins_map = HashMap::new();
        for plugin in plugins {
            plugins_map.insert(plugin.module_name.as_str(), plugin);
        }

        Ok(plugins_map)
    }

    /// Get plugin from registry
    async fn get_registry_plugin(&self, package_name: &str) -> Result<&RegistryPlugin> {
        let plugins = self.package_plugins_map().await?;
        let plugin = plugins
            .get(package_name)
            .ok_or_else(|| NbrError::not_found(format!("Plugin '{}' not found", package_name)))?;
        Ok(plugin)
    }

    /// Search plugins in registry
    async fn search_registry_plugins(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<&RegistryPlugin>> {
        let plugins_map = self.package_plugins_map().await?;

        let results: Vec<&RegistryPlugin> = plugins_map
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

        Ok(results)
    }

    /// Find installed plugin by name
    fn find_installed_plugin(&self, name: &str) -> Result<String> {
        let nonebot = ToolNonebot::parse(None)?.nonebot()?;
        for plugin in &nonebot.plugins {
            if plugin == name || plugin.contains(name) {
                return Ok(plugin.clone());
            }
        }

        Err(NbrError::not_found(format!(
            "Plugin '{}' is not installed",
            name
        )))
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

    /// Display search result
    fn display_search_result(&self, plugin: &RegistryPlugin, index: usize) {
        println!(
            "{}. {} ({}) {}",
            index.to_string().bright_black(),
            plugin.name.bright_blue().bold(),
            plugin.project_link.bright_cyan(),
            format!("v{}", plugin.version).bright_green()
        );

        println!("   Desc: {}", plugin.desc);
        if let Some(ref homepage) = plugin.homepage {
            println!("   Homepage: {}", homepage.bright_cyan());
        }

        // if !plugin.tags.is_empty() {
        //     println!(
        //         "   {} {}",
        //         "Tags:".bright_black(),
        //         plugin
        //             .tags
        //             .iter()
        //             .take(3)
        //             .map(|t| t.get("label").unwrap().bright_yellow().to_string())
        //             .collect::<Vec<_>>()
        //             .join(", ")
        //     );
        // }

        println!(
            "   Install Command: {}",
            format!("nbr plugin install {}", plugin.project_link).bright_yellow()
        );
    }
}

/// Handle the plugin command
pub async fn handle_plugin(matches: &ArgMatches) -> Result<()> {
    let mut plugin_manager = PluginManager::new()?;

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
        _ => Err(NbrError::invalid_argument("Invalid plugin subcommand")),
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
        .map_err(|e| NbrError::io(format!("Failed to get current directory: {}", e)))?;

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
        NbrError::not_found(
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
        let plugin_manager = PluginManager::new().unwrap();
        let plugins = plugin_manager.package_plugins_map().await.unwrap();
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
        let plugin_manager = PluginManager::new().unwrap();
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
