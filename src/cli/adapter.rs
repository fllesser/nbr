//! Adapter command handler for nbr
//!
//! This module handles adapter management including installation, removal,
//! and listing adapters for NoneBot applications.

use crate::error::{NbrError, Result};
use crate::pyproject::{Adapter, NbTomlEditor, PyProjectConfig};
use crate::utils::terminal_utils;
use crate::uv;
use clap::Subcommand;
use colored::*;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, MultiSelect};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{debug, error, info, warn};

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

// {
// "module_name": "nonebot.adapters.onebot.v11",
// "project_link": "nonebot-adapter-onebot",
// "name": "OneBot V11",
// "desc": "OneBot V11 协议",
// "author": "yanyongyu",
// "homepage": "https://onebot.adapters.nonebot.dev/",
// "tags": [],
// "is_official": true,
// "time": "2024-10-24T07:34:56.115315Z",
// "version": "2.4.6"
// },
/// Adapter registry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAdapter {
    /// Adapter name
    pub module_name: String,
    pub project_link: String,
    pub name: String,
    pub desc: String,
    pub author: String,
    pub homepage: Option<String>,
    pub tags: Vec<HashMap<String, String>>,
    pub is_official: bool,
    pub time: String,
    pub version: String,
}

/// Adapter manager
pub struct AdapterManager {
    /// HTTP client for registry requests
    client: Client,
    /// Working directory
    work_dir: PathBuf,
    /// Registry adapters
    registry_adapters: OnceLock<HashMap<String, RegistryAdapter>>,
    /// Installed adapters
    installed_adapters: OnceLock<Vec<Adapter>>,
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new(None).unwrap()
    }
}

impl AdapterManager {
    /// Create a new adapter manager
    pub fn new(work_dir: Option<PathBuf>) -> Result<Self> {
        let work_dir = work_dir.unwrap_or_else(|| Path::new(".").to_path_buf());

        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("nbr")
            .build()
            .map_err(NbrError::Network)?;

        Ok(Self {
            client,
            work_dir,
            registry_adapters: OnceLock::new(),
            installed_adapters: OnceLock::new(),
        })
    }

    /// Fetch registry adapters from registry.nonebot.dev
    pub async fn fetch_regsitry_adapters(&self) -> Result<&HashMap<String, RegistryAdapter>> {
        if let Some(adapters) = self.registry_adapters.get() {
            return Ok(adapters);
        }
        let spinner = terminal_utils::create_spinner("Fetching adapters from registry...");
        let adapters_json_url = "https://registry.nonebot.dev/adapters.json";
        let response = self
            .client
            .get(adapters_json_url)
            .send()
            .await
            .map_err(NbrError::Network)?;

        spinner.finish_and_clear();
        if !response.status().is_success() {
            return Err(NbrError::not_found("Adapter registry not found"));
        }

        let adapters: Vec<RegistryAdapter> = response
            .json()
            .await
            .map_err(|e| NbrError::plugin(format!("Failed to parse adapter info: {}", e)))?;

        let mut adapters_map = HashMap::new();
        for adapter in adapters {
            adapters_map.insert(adapter.name.to_owned(), adapter);
        }

        self.registry_adapters.set(adapters_map).unwrap();
        Ok(self.registry_adapters.get().unwrap())
    }

    /// Parse installed adapters from pyproject.toml
    pub fn parse_installed_adapters(&self) -> Option<&Vec<Adapter>> {
        if let Some(adapters) = self.installed_adapters.get() {
            return Some(adapters);
        }

        let config = PyProjectConfig::parse(Some(&self.work_dir)).ok()?;
        let adapters = config.nonebot()?.adapters.to_owned()?;
        self.installed_adapters.set(adapters).unwrap();
        self.installed_adapters.get()
    }

    /// Get installed adapters names from pyproject.toml
    pub fn get_installed_adapters_names(&self) -> Vec<&str> {
        let installed_adapters = self.parse_installed_adapters();
        if let Some(adapters) = installed_adapters {
            adapters
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<&str>>()
        } else {
            vec![]
        }
    }

    /// Select adapters from registry
    pub async fn select_adapters(&self, filter_installed: bool) -> Result<Vec<&RegistryAdapter>> {
        // 获取 registry 中的 adapters
        let registry_adapters = self.fetch_regsitry_adapters().await?;
        let mut adapter_names: Vec<String> = registry_adapters.keys().cloned().collect();

        // 过滤已安装的 adapters
        if filter_installed {
            let installed_adapters = self.get_installed_adapters_names();
            adapter_names.retain(|name| !installed_adapters.contains(&name.as_str()));
        }

        // 排序
        adapter_names.sort();

        let selected_adapters = if !adapter_names.is_empty() {
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Which adapter(s) would you like to use")
                .items(&adapter_names)
                //.defaults(&vec![true; adapter_names.len().min(1)]) // Select first adapter by default
                .interact()
                .map_err(|e| NbrError::io(e.to_string()))?;

            selections
                .into_iter()
                .map(|i| adapter_names[i].to_string())
                .collect()
        } else {
            vec!["OneBot V11".to_string()] // Default adapter
        };

        Ok(selected_adapters
            .iter()
            .map(|name| registry_adapters.get(name).unwrap())
            .collect())
    }

    /// Install an adapter
    pub async fn install_adapters(&self) -> Result<()> {
        let selected_adapters = self.select_adapters(true).await?;

        if selected_adapters.is_empty() {
            warn!("You haven't selected any adapters to install");
            return Ok(());
        }

        let prompt = format!(
            "Would you like to install [{}] ?",
            selected_adapters
                .iter()
                .map(|a| a.name.clone().cyan().bold().to_string())
                .collect::<Vec<String>>()
                .join(", ")
        );

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&prompt)
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        // Install the adapter
        let adapter_packages = selected_adapters
            .iter()
            .map(|a| a.project_link.as_str())
            .collect::<HashSet<&str>>() // 🐶 ob
            .into_iter()
            .collect::<Vec<&str>>();

        uv::add(adapter_packages)
            .working_dir(&self.work_dir)
            .run()?;
        // Add to configuration
        let adapters = selected_adapters
            .iter()
            .map(|a| Adapter {
                name: a.name.clone(),
                module_name: a.module_name.clone(),
            })
            .collect::<Vec<Adapter>>();

        // Add adapters to configuration
        NbTomlEditor::parse(Some(&self.work_dir))?.add_adapters(adapters)?;

        info!(
            "✓ Successfully installed adapters: {}",
            selected_adapters
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<String>>()
                .join(", ")
                .cyan()
                .bold()
        );

        // Show configuration instructions
        // if let Some(ref adapter) = adapter_info {
        //     if let Some(ref config_template) = adapter.config_template {
        //         self.show_configuration_instructions(adapter, config_template);
        //     }
        // }

        Ok(())
    }

    /// Get installed adapters from virtual environment
    #[allow(dead_code)]
    pub async fn get_installed_adapters_from_venv(&self) -> Result<HashSet<String>> {
        let installed_adapters = uv::list(false).await?;
        let installed_adapters_set = installed_adapters
            .into_iter()
            .filter(|a| a.name.contains("nonebot-adapter-"))
            .map(|a| a.name)
            .collect::<HashSet<String>>();
        debug!("Installed adapters: {:?}", installed_adapters_set);
        Ok(installed_adapters_set)
    }

    /// Uninstall an adapter
    pub async fn uninstall_adapters(&self) -> Result<()> {
        // get installed adapters from configuration
        let mut installed_adapters = self.get_installed_adapters_names();
        if installed_adapters.is_empty() {
            warn!("You haven't installed any adapters");
            return Ok(());
        }

        // select adapters to uninstall
        let selected_adapters: Vec<&str> = {
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Select installed adapter(s) to uninstall")
                .items(&installed_adapters)
                //.defaults(&vec![true; adapter_names.len().min(1)]) // Select first adapter by default
                .interact()
                .map_err(|e| NbrError::io(e.to_string()))?;

            selections
                .into_iter()
                .map(|i| installed_adapters[i])
                .collect()
        };

        // Remove from configuration
        NbTomlEditor::parse(Some(&self.work_dir))?.remove_adapters(selected_adapters.to_vec())?;

        // Uninstall the package
        let registry_adapters = self.fetch_regsitry_adapters().await?;

        let mut adapter_packages = selected_adapters
            .iter()
            .map(|name| registry_adapters.get(*name).unwrap().project_link.as_str())
            .collect::<HashSet<&str>>() // 🐶 ob
            .into_iter()
            .collect::<Vec<&str>>();
        // 特判 ob，沟槽的 ob，Onebot V11 和 Onebot V12 是同一个包
        // 剩下的 installed_adapters 中，如果包含 ob，则不删除
        installed_adapters.retain(|name| !selected_adapters.contains(name));
        if installed_adapters
            .iter()
            .any(|name| name.starts_with("OneBot"))
        {
            adapter_packages.retain(|name| *name != "nonebot-adapter-onebot");
        }

        if !adapter_packages.is_empty() {
            uv::remove(adapter_packages)
                .working_dir(&self.work_dir)
                .run()?;
        }

        info!(
            "✓ Successfully uninstalled adapters: {}",
            selected_adapters.to_vec().join(", ").cyan().bold()
        );

        Ok(())
    }

    /// List available and installed adapters
    pub async fn list_adapters(&self, show_all: bool) -> Result<()> {
        let installed_adapters = self.get_installed_adapters_names();
        let adapters_map = self.fetch_regsitry_adapters().await?;

        if show_all {
            info!("All Adapters:");
            adapters_map.iter().for_each(|(_, adapter)| {
                self.display_adapter(adapter);
            });
        } else {
            if installed_adapters.is_empty() {
                warn!("No adapters installed.");
                return Ok(());
            }

            info!("Installed Adapters:");
            installed_adapters.iter().for_each(|name| {
                let adapter = adapters_map.get(*name).unwrap();
                self.display_adapter(adapter);
            });
        }

        Ok(())
    }

    pub fn display_adapter(&self, adapter: &RegistryAdapter) {
        println!(
            " {} {} ({} {})",
            "•".cyan(),
            adapter.name.cyan().bold(),
            adapter.project_link.bright_black(),
            format!("v{}", adapter.version).bright_green(),
        );
    }

    /// Get adapter configuration template
    #[allow(dead_code)]
    fn get_adapter_config_template(&self, package_name: &str) -> Option<HashMap<String, String>> {
        let mut template = HashMap::new();

        match package_name {
            "nonebot-adapter-onebot" => {
                template.insert("driver".to_string(), "~httpx+~websockets".to_string());
                template.insert(
                    "onebot_access_token".to_string(),
                    "your_access_token_here".to_string(),
                );
                template.insert("onebot_secret".to_string(), "your_secret_here".to_string());
                template.insert(
                    "onebot_ws_urls".to_string(),
                    "[\"ws://127.0.0.1:6700/\"]".to_string(),
                );
            }
            "nonebot-adapter-telegram" => {
                template.insert("driver".to_string(), "~httpx".to_string());
                template.insert(
                    "telegram_bot_token".to_string(),
                    "your_bot_token_here".to_string(),
                );
            }
            "nonebot-adapter-ding" => {
                template.insert("driver".to_string(), "~httpx".to_string());
                template.insert(
                    "ding_access_token".to_string(),
                    "your_access_token_here".to_string(),
                );
                template.insert("ding_secret".to_string(), "your_secret_here".to_string());
            }
            "nonebot-adapter-feishu" => {
                template.insert("driver".to_string(), "~httpx".to_string());
                template.insert("feishu_app_id".to_string(), "your_app_id_here".to_string());
                template.insert(
                    "feishu_app_secret".to_string(),
                    "your_app_secret_here".to_string(),
                );
            }
            "nonebot-adapter-console" => {
                // Console adapter doesn't need additional config
                return None;
            }
            _ => return None,
        }

        Some(template)
    }

    /// Display adapter information
    #[allow(dead_code)]
    fn display_adapter_info(&self, adapter: &RegistryAdapter) {
        println!("{}", adapter.name.cyan().bold());
        println!("  Package: {}", adapter.project_link);
        println!("  Module: {}", adapter.module_name);
        println!("  Desc: {}", adapter.desc);
        println!(
            "  {} {}",
            "Version:".bright_black(),
            adapter.version.bright_white()
        );
        println!(
            "  {} {}",
            "Author:".bright_black(),
            adapter.author.bright_white()
        );

        if let Some(ref homepage) = adapter.homepage {
            println!(
                "  {} {}",
                "Homepage:".bright_black(),
                homepage.bright_cyan()
            );
        }
    }

    /// Show configuration instructions
    #[allow(dead_code)]
    fn show_configuration_instructions(
        &self,
        _adapter: &RegistryAdapter,
        config_template: &HashMap<String, String>,
    ) {
        println!();
        println!("{}", "Configuration Instructions:".bright_yellow().bold());
        println!("Add the following to your .env file:");
        println!();

        for (key, value) in config_template {
            println!("{}={}", key.bright_cyan(), value.bright_white());
        }

        println!();
        println!(
            "{}",
            "Remember to replace placeholder values with actual configuration!".bright_red()
        );
    }
}

#[derive(Subcommand)]
pub enum AdapterCommands {
    Install,
    Uninstall,
    List {
        #[clap(short, long)]
        all: bool,
    },
}

/// Handle the adapter command
pub async fn handle_adapter(commands: &AdapterCommands) -> Result<()> {
    let adapter_manager = AdapterManager::new(None)?;

    match commands {
        AdapterCommands::Install => adapter_manager.install_adapters().await,
        AdapterCommands::Uninstall => adapter_manager.uninstall_adapters().await,
        AdapterCommands::List { all } => adapter_manager.list_adapters(*all).await,
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_fetch_regsitry_adapters() {
        let manager = AdapterManager::default();

        let adapters_map = manager.fetch_regsitry_adapters().await.unwrap();
        assert!(adapters_map.len() > 0);
        for adapter in adapters_map.values() {
            println!("{}", adapter.name);
        }
    }
}
