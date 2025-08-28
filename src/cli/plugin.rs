//! Plugin command handler for nbr
//!
//! This module handles plugin management including installation, removal,
//! listing, searching, and updating plugins from various sources.

use crate::error::{NbrError, Result};
use crate::log::StyledText;
use crate::pyproject::NbTomlEditor;
use crate::utils::terminal_utils;
use crate::uv::{self, Package};
use clap::Subcommand;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, error, info, warn};

#[derive(Subcommand)]
pub enum PluginCommands {
    #[clap(about = "Install a plugin")]
    Install {
        #[clap()]
        name: String,
        #[clap(short, long)]
        index: Option<String>,
        #[clap(short, long)]
        upgrade: bool,
    },
    #[clap(about = "Uninstall a plugin")]
    Uninstall {
        #[clap()]
        name: String,
    },
    #[clap(about = "List installed plugins, show outdated plugins if --outdated is set")]
    List {
        #[clap(short, long, help = "Show outdated plugins")]
        outdated: bool,
    },
    #[clap(about = "Search plugins in registry")]
    Search {
        #[clap(help = "Search keyword")]
        query: String,
        #[clap(
            short,
            long,
            default_value = "10",
            help = "Limit the number of search results"
        )]
        limit: usize,
    },
    #[clap(about = "Update plugin(s)")]
    Update {
        #[clap(help = "Plugin name")]
        name: Option<String>,
        #[clap(short, long, help = "Update all plugins")]
        all: bool,
        #[clap(short, long, help = "Reinstall the plugin")]
        reinstall: bool,
    },
    #[clap(about = "Create a new plugin")]
    Create,
}

pub async fn handle_plugin(commands: &PluginCommands) -> Result<()> {
    let mut manager = PluginManager::new(None)?;
    match commands {
        PluginCommands::Install {
            name,
            index,
            upgrade,
        } => manager.install(name, index.as_deref(), *upgrade).await,
        PluginCommands::Uninstall { name } => manager.uninstall(name).await,
        PluginCommands::List { outdated } => manager.list(*outdated).await,
        PluginCommands::Search { query, limit } => manager.search_plugins(query, *limit).await,
        PluginCommands::Update {
            name,
            all,
            reinstall,
        } => manager.update(name.as_deref(), *all, *reinstall).await,
        PluginCommands::Create => {
            unimplemented!()
        }
    }
}

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
        let work_dir = work_dir.unwrap_or_else(|| Path::new(".").to_path_buf());

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("nbr")
            .build()
            .map_err(NbrError::Network)?;

        let registry_plugins = OnceLock::new();

        Ok(Self {
            client,
            work_dir,
            registry_plugins,
        })
    }

    pub async fn install(
        &mut self,
        package: &str,
        index_url: Option<&str>,
        upgrade: bool,
    ) -> Result<()> {
        if package.starts_with("https") {
            return self.install_from_github(package).await;
        }

        // nonebot-plugin-orm[default] -> nonebot-plugin-orm, default
        // nonebot-plugin-orm -> nonebot-plugin-orm
        // nonebot-plugin-orm[default, sqlalchemy] -> nonebot-plugin-orm, default, sqlalchemy
        let package_and_extras = package.split('[').collect::<Vec<&str>>();
        let package_name = package_and_extras[0];
        let extras = if let Some(extras) = package_and_extras.get(1) {
            let extras = extras.trim_end_matches(']');
            extras.split(',').collect::<Vec<&str>>()
        } else {
            vec![]
        };

        if let Ok(registry_plugin) = self.get_registry_plugin(package_name).await {
            self.install_from_registry(registry_plugin, index_url, extras, upgrade)
                .await
        } else {
            self.install_unregistered_plugin(package_name, extras).await
        }
    }

    pub async fn install_from_github(&mut self, repo_url: &str) -> Result<()> {
        debug!("Installing plugin from github: {}", repo_url);

        // 确定是否安装 github 插件
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to install this plugin from github")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            uv::add_from_github(repo_url)?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        let regex = Regex::new(r"nonebot-plugin-(?P<repo>[^/@]+)").unwrap();
        let repo_name = regex.captures(repo_url).unwrap().get(0).unwrap().as_str();
        let module_name = repo_name.replace("-", "_");

        // Add to configuration
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?.add_plugins(vec![module_name])?;

        StyledText::new("")
            .green_bold("✓ Successfully installed plugin: ")
            .cyan_bold(repo_name)
            .println();
        Ok(())
    }

    pub async fn install_unregistered_plugin(
        &mut self,
        package_name: &str,
        extras: Vec<&str>,
    ) -> Result<()> {
        debug!("Installing unregistered plugin: {}", package_name);

        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to install this unregistered plugin from PyPI?")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            uv::add(vec![package_name])
                .extras(extras)
                .working_dir(&self.work_dir)
                .run()?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        let module_name = package_name.replace("-", "_");
        // Add to configuration
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?.add_plugins(vec![module_name])?;

        StyledText::new("")
            .green_bold("✓ Successfully installed plugin: ")
            .cyan_bold(package_name)
            .println();
        Ok(())
    }

    /// Install a plugin
    pub async fn install_from_registry(
        &self,
        registry_plugin: &RegistryPlugin,
        index_url: Option<&str>,
        extras: Vec<&str>,
        upgrade: bool,
    ) -> Result<()> {
        let package_name = &registry_plugin.project_link;
        // Show plugin information if available
        self.display_plugin_info(registry_plugin);

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to install this plugin")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }
        // Install the plugin

        uv::add(vec![package_name])
            .extras(extras)
            .upgrade(upgrade)
            .index_url_opt(index_url)
            .working_dir(&self.work_dir)
            .run()?;

        // Add to configuration
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![registry_plugin.module_name.clone()])?;

        StyledText::new("")
            .green_bold("✓ Successfully installed plugin: ")
            .cyan_bold(package_name)
            .println();

        Ok(())
    }

    /// Uninstall a plugin
    pub async fn uninstall(&self, name: &str) -> Result<()> {
        debug!("Uninstalling plugin: {}", name);

        if let Ok(registry_plugin) = self.get_registry_plugin(name).await {
            self.uninstall_registry_plugin(registry_plugin).await
        } else {
            self.uninstall_unregistered_plugin(name).await
        }
    }

    pub async fn uninstall_unregistered_plugin(&self, package_name: &str) -> Result<()> {
        debug!("Uninstalling unregistered plugin: {}", package_name);

        if !uv::is_installed(package_name).await {
            return Err(NbrError::not_found(format!(
                "Plugin '{}' is not installed.",
                package_name
            )));
        }

        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Would you like to uninstall '{package_name}'",))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            uv::remove(vec![&package_name])
                .working_dir(&self.work_dir)
                .run()?;
            NbTomlEditor::with_work_dir(Some(&self.work_dir))?
                .remove_plugins(vec![package_name.replace("-", "_")])?;

            StyledText::new("")
                .green_bold("✓ Successfully uninstalled plugin: ")
                .cyan_bold(package_name)
                .println();
        } else {
            error!("Uninstallation operation cancelled.");
            return Ok(());
        }

        Ok(())
    }

    pub async fn uninstall_registry_plugin(&self, registry_plugin: &RegistryPlugin) -> Result<()> {
        let package_name = registry_plugin.project_link.clone();
        // Check if already installed
        if !uv::is_installed(&package_name).await {
            return Err(NbrError::not_found(format!(
                "Plugin '{}' is not installed.",
                registry_plugin.project_link
            )));
        }
        // Confirm uninstallation
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Would you like to uninstall '{package_name}'"))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Uninstallation operation cancelled.");
            return Ok(());
        }

        // Uninstall the package
        uv::remove(vec![&package_name]).run()?;

        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .remove_plugins(vec![registry_plugin.module_name.clone()])?;

        StyledText::new("")
            .green_bold("✓ Successfully uninstalled plugin: ")
            .cyan_bold(&package_name)
            .println();

        Ok(())
    }

    pub async fn get_installed_plugins(&self, outdated: bool) -> Result<Vec<Package>> {
        let installed_packages = uv::list(outdated).await?;
        let installed_plugins = installed_packages
            .into_iter()
            .filter(|p| p.name.starts_with("nonebot") && p.name.contains("plugin"))
            .collect();
        Ok(installed_plugins)
    }

    pub async fn list(&self, show_outdated: bool) -> Result<()> {
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
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?.reset_plugins(
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

        let results = self.search_registry_plugins(query, limit).await?;

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
    pub async fn update(
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
                "Would you like to update these {} outdated plugins",
                outdated_plugins.len()
            ))
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Update operation cancelled.");
            return Ok(());
        }

        let package_names: Vec<&str> = outdated_plugins.iter().map(|p| p.name.as_str()).collect();
        uv::upgrade(package_names.clone())?;

        StyledText::new("")
            .green_bold("Successfully updated plugins: ")
            .cyan_bold(&package_names.join(", "))
            .println();

        Ok(())
    }

    /// Update a single plugin
    fn update_single_plugin(&self, package_name: &str, reinstall: bool) -> Result<()> {
        if reinstall {
            uv::reinstall(package_name)?;
        } else {
            uv::upgrade(vec![package_name])?;
        }
        info!("Successfully updated plugin: {}", package_name);
        Ok(())
    }

    pub async fn fetch_registry_plugins(&self) -> Result<&Vec<RegistryPlugin>> {
        if let Some(plugins) = self.registry_plugins.get() {
            return Ok(plugins);
        }
        let spinner = terminal_utils::create_spinner("Fetching plugins from registry...");
        let plugins_json_url = "https://registry.nonebot.dev/plugins.json";
        let response = self
            .client
            .get(plugins_json_url)
            .send()
            .await
            .map_err(NbrError::Network)?;

        spinner.finish_and_clear();
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
        StyledText::new("").cyan_bold(&plugin.name).println();
        StyledText::new("")
            .black("  ")
            .white(&plugin.desc)
            .println();
        StyledText::new("")
            .black("  Version:")
            .white(&plugin.version)
            .println();
        StyledText::new("")
            .black("  Author:")
            .white(&plugin.author)
            .println();

        if let Some(ref homepage) = plugin.homepage {
            StyledText::new("")
                .black("  Homepage:")
                .cyan(homepage)
                .println();
        }

        if !plugin.tags.is_empty() {
            StyledText::new("")
                .black("  Tags: ")
                .yellow(
                    &plugin
                        .tags
                        .iter()
                        .map(|t| t.get("label").unwrap().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .println();
        }
    }

    /// Display search result
    fn display_search_result(&self, plugin: &RegistryPlugin, index: usize) {
        StyledText::new("")
            .black(&format!("{}. ", index))
            .cyan_bold(&plugin.name)
            .println();

        StyledText::new("")
            .black("  Desc:")
            .white(&plugin.desc)
            .println();
        if let Some(ref homepage) = plugin.homepage {
            StyledText::new("")
                .black("  Homepage:")
                .cyan(homepage)
                .println();
        }

        StyledText::new("")
            .black("  Install Command:")
            .yellow(&format!("nbr plugin install {}", plugin.project_link))
            .println();
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
        let work_dir = Path::new("awesome-bot").to_path_buf();
        let plugin_manager = PluginManager::new(Some(work_dir)).unwrap();
        let installed_plugins = plugin_manager.get_installed_plugins(true).await.unwrap();
        dbg!(installed_plugins);
    }
}
