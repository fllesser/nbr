//! Plugin command handler for nbr
//!
//! This module handles plugin management including installation, removal,
//! listing, searching, and updating plugins from various sources.

use crate::error::{NbrError, Result};
use crate::pyproject::ToolNonebot;
use crate::utils::terminal_utils;
use crate::uv::{self, Package};
use clap::ArgMatches;
use colored::*;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, error, info, warn};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPackageInfo {
    pub package_name: String,
    pub installed_version: String,
    pub latest_version: String,
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

impl Default for PluginManager {
    fn default() -> Self {
        Self::new(None).unwrap()
    }
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(work_dir: Option<PathBuf>) -> Result<Self> {
        let work_dir = work_dir.unwrap_or_else(|| std::env::current_dir().unwrap());

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("nbr")
            .build()
            .map_err(NbrError::Network)?;

        Ok(Self {
            client,
            work_dir,
            registry_plugins: OnceLock::new(),
        })
    }

    pub async fn install_plugin(
        &mut self,
        package: &str,
        index_url: Option<&str>,
        upgrade: bool,
    ) -> Result<()> {
        if package.starts_with("http") {
            self.install_plugin_from_github(package).await
        } else if let Ok(registry_plugin) = self.get_registry_plugin(package).await {
            self.install_plugin_from_registry(registry_plugin, index_url, upgrade)
                .await
        } else {
            self.install_unregistered_plugin(package).await
        }
    }

    pub async fn install_plugin_from_github(&mut self, repo_url: &str) -> Result<()> {
        debug!("Installing plugin from github: {}", repo_url);

        // 确定是否安装 github 插件
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Do you want to install this plugin from github")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            uv::add_from_github(repo_url, Some(&self.work_dir))?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        let regex = Regex::new(r"nonebot-plugin-(?P<repo>[^/@]+)").unwrap();
        let repo_name = regex.captures(repo_url).unwrap().get(0).unwrap().as_str();
        let module_name = repo_name.replace("-", "_");

        // Add to configuration
        ToolNonebot::parse(None)?.add_plugins(vec![module_name])?;

        info!(
            "✓ Successfully installed plugin: {}",
            repo_name.yellow().bold()
        );
        Ok(())
    }

    pub async fn install_unregistered_plugin(&mut self, package_name: &str) -> Result<()> {
        debug!("Installing unregistered plugin: {}", package_name);

        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Do you want to install this unregistered plugin from PyPI?")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            uv::add(vec![package_name], false, None, Some(&self.work_dir))?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        let module_name = package_name.replace("-", "_");
        // Add to configuration
        ToolNonebot::parse(None)?.add_plugins(vec![module_name])?;

        info!(
            "✓ Successfully installed plugin: {}",
            package_name.yellow().bold()
        );
        Ok(())
    }

    /// Install a plugin
    pub async fn install_plugin_from_registry(
        &self,
        registry_plugin: &RegistryPlugin,
        index_url: Option<&str>,
        upgrade: bool,
    ) -> Result<()> {
        let package_name = &registry_plugin.project_link;
        debug!("Installing plugin: {package_name}");
        // Show plugin information if available
        self.display_plugin_info(registry_plugin);

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Do you want to install this plugin")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }
        // Install the plugin
        uv::add(
            vec![&package_name],
            upgrade,
            index_url,
            Some(&self.work_dir),
        )?;

        // Add to configuration
        ToolNonebot::parse(None)?.add_plugins(vec![registry_plugin.module_name.clone()])?;

        info!(
            "✓ Successfully installed plugin: {}",
            package_name.yellow().bold()
        );

        Ok(())
    }

    /// Uninstall a plugin
    pub async fn uninstall_plugin(&self, name: &str) -> Result<()> {
        debug!("Uninstalling plugin: {}", name);

        if let Ok(registry_plugin) = self.get_registry_plugin(name).await {
            self.uninstall_plugin_from_registry(registry_plugin).await
        } else {
            self.uninstall_unregistered_plugin(name).await
        }
    }

    pub async fn uninstall_unregistered_plugin(&self, package_name: &str) -> Result<()> {
        debug!("Uninstalling unregistered plugin: {}", package_name);

        if !uv::is_installed(package_name, Some(&self.work_dir)).await {
            return Err(NbrError::not_found(format!(
                "Plugin '{}' is not installed.",
                package_name
            )));
        }

        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Are you sure you want to uninstall '{package_name}'",
            ))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            uv::remove(vec![&package_name], Some(&self.work_dir))?;
            ToolNonebot::parse(None)?.remove_plugins(vec![package_name.replace("-", "_")])?;

            info!(
                "✓ Successfully uninstalled plugin: {}",
                package_name.yellow().bold()
            );
        } else {
            error!("Uninstallation operation cancelled.");
            return Ok(());
        }

        Ok(())
    }

    pub async fn uninstall_plugin_from_registry(
        &self,
        registry_plugin: &RegistryPlugin,
    ) -> Result<()> {
        let package_name = registry_plugin.project_link.clone();
        // Check if already installed
        if !uv::is_installed(&package_name, Some(&self.work_dir)).await {
            return Err(NbrError::not_found(format!(
                "Plugin '{}' is not installed.",
                registry_plugin.project_link
            )));
        }
        // Confirm uninstallation
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Are you sure you want to uninstall '{package_name}'"
            ))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Uninstallation operation cancelled.");
            return Ok(());
        }

        // Uninstall the package
        uv::remove(vec![&package_name], Some(&self.work_dir))?;

        ToolNonebot::parse(None)?.remove_plugins(vec![registry_plugin.module_name.clone()])?;

        info!(
            "✓ Successfully uninstalled plugin: {}",
            package_name.yellow().bold()
        );

        Ok(())
    }

    pub async fn get_installed_plugins(&self, outdated: bool) -> Result<Vec<Package>> {
        let installed_packages = uv::list(Some(&self.work_dir), outdated).await?;
        let installed_plugins = installed_packages
            .into_iter()
            .filter(|p| p.name.starts_with("nonebot") && p.name.contains("plugin"))
            .collect();
        Ok(installed_plugins)
    }

    pub async fn list_plugins(&self, show_outdated: bool) -> Result<()> {
        // 获取所有插件
        let mut installed_plugins = self.get_installed_plugins(false).await?;
        // 获取需要更新的插件
        if show_outdated {
            let outdated_plugins = self.get_installed_plugins(true).await?;
            // 去重，保留 outdated 的包
            installed_plugins.retain(|p| !outdated_plugins.contains(p));
            installed_plugins.extend(outdated_plugins);
        }

        if installed_plugins.is_empty() {
            warn!("No plugins installed.");
            return Ok(());
        }

        info!("Installed Plugins:");
        installed_plugins.iter().for_each(|p| p.display_info());

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn fix_nonebot_plugins(&self) -> Result<()> {
        let installed_plugins = self.get_installed_plugins(false).await?;
        ToolNonebot::parse(None)?.reset_plugins(
            installed_plugins
                .iter()
                .map(|p| p.name.replace("-", "_"))
                .collect::<Vec<String>>(),
        )?;
        Ok(())
    }

    /// Search plugins in registry
    pub async fn search_plugins(&self, query: &str, limit: usize) -> Result<()> {
        debug!("Searching plugins for: {}", query);

        let spinner = terminal_utils::create_spinner(format!("Searching for '{}'...", query));

        let results = self.search_registry_plugins(query, limit).await?;
        spinner.finish_and_clear();

        if results.is_empty() {
            warn!("No plugins found for '{}'.", query);
            return Ok(());
        }

        info!("Found {} plugin(s):", results.len());

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
        reinstall: bool,
    ) -> Result<()> {
        if update_all {
            self.update_all_plugins().await
        } else if let Some(name) = plugin_name {
            self.update_single_plugin(name, reinstall)
        } else {
            Err(NbrError::invalid_argument(
                "Either specify a plugin name or use --all( -a) flag",
            ))
        }
    }

    /// Update all plugins
    async fn update_all_plugins(&self) -> Result<()> {
        let outdated_plugins = self.get_installed_plugins(true).await?;

        if outdated_plugins.is_empty() {
            info!("No plugins need to update.");
            return Ok(());
        }
        info!("Fount {} outdated plugins:", outdated_plugins.len());
        outdated_plugins
            .iter()
            .for_each(|plugin| plugin.display_info());

        // 确认更新
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Do you want to update these {} outdated plugins",
                outdated_plugins.len()
            ))
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Update operation cancelled.");
            return Ok(());
        }

        uv::add(
            outdated_plugins
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<&str>>(),
            true,
            None,
            Some(&self.work_dir),
        )
    }

    /// Update a single plugin
    fn update_single_plugin(&self, package_name: &str, reinstall: bool) -> Result<()> {
        if reinstall {
            uv::reinstall(package_name, Some(&self.work_dir))
        } else {
            uv::add(vec![package_name], true, None, Some(&self.work_dir))
        }
    }

    /// Get latest package version from PyPI
    #[allow(dead_code)]
    async fn get_latest_package_version(&self, package: &str) -> Result<String> {
        let url = format!("https://pypi.org/pypi/{}/json", package);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(NbrError::Network)?;

        if !response.status().is_success() {
            return Err(NbrError::not_found(format!(
                "Package '{}' not found on PyPI",
                package
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(NbrError::Network)?;

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
        let response = self
            .client
            .get(plugins_json_url)
            .send()
            .await
            .map_err(NbrError::Network)?;

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

    pub async fn get_plugins_map(&self) -> Result<HashMap<&str, &RegistryPlugin>> {
        let plugins = self.fetch_registry_plugins().await?;
        let plugins_map = plugins
            .iter()
            .map(|p| (p.project_link.as_str(), p))
            .collect::<HashMap<&str, &RegistryPlugin>>();
        Ok(plugins_map)
    }

    /// Get plugin from registry
    async fn get_registry_plugin(&self, package_name: &str) -> Result<&RegistryPlugin> {
        let plugins = self.get_plugins_map().await?;
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
        let plugins_map = self.get_plugins_map().await?;

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
    let mut plugin_manager = PluginManager::new(None)?;

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
            let reinstall = sub_matches.get_flag("reinstall");

            plugin_manager
                .update_plugins(plugin_name.map(|s| s.as_str()), update_all, reinstall)
                .await
        }
        _ => Err(NbrError::invalid_argument("Invalid plugin subcommand")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_regsitry_plugins_map() {
        let plugin_manager = PluginManager::default();
        let plugins = plugin_manager.get_plugins_map().await.unwrap();
        for (_, plugin) in plugins {
            dbg!(plugin);
        }
    }

    #[tokio::test]
    async fn test_get_registry_plugin() {
        let plugin_manager = PluginManager::default();
        let plugin = plugin_manager
            .get_registry_plugin("nonebot-plugin-status")
            .await
            .unwrap();
        dbg!(plugin);
    }

    #[tokio::test]
    async fn test_get_installed_plugins() {
        let work_dir = std::env::current_dir().unwrap().join("awesome-bot");
        let plugin_manager = PluginManager::new(Some(work_dir)).unwrap();
        let installed_plugins = plugin_manager.get_installed_plugins(true).await.unwrap();
        dbg!(installed_plugins);
    }
}
