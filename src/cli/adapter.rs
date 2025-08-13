//! Adapter command handler for nbr
//!
//! This module handles adapter management including installation, removal,
//! and listing adapters for NoneBot applications.

use crate::error::{NbrError, Result};
use crate::pyproject::{Adapter, ToolNonebot};
use crate::utils::terminal_utils;
use crate::uv::Uv;
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, MultiSelect};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::debug;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::timeout;

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
}

impl AdapterManager {
    /// Create a new adapter manager
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
            registry_adapters: OnceLock::new(),
        })
    }

    pub async fn fetch_regsitry_adapters(&self) -> Result<&HashMap<String, RegistryAdapter>> {
        if let Some(adapters) = self.registry_adapters.get() {
            return Ok(adapters);
        }

        let adapters_json_url = "https://registry.nonebot.dev/adapters.json";
        let response = timeout(
            Duration::from_secs(10),
            self.client.get(adapters_json_url).send(),
        )
        .await
        .map_err(|_| NbrError::unknown("Request timeout"))?
        .map_err(|e| NbrError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbrError::not_found("Adapter registry not found"));
        }

        let adapters: Vec<RegistryAdapter> = response
            .json()
            .await
            .map_err(|e| NbrError::plugin(format!("Failed to parse adapter info: {}", e)))?;

        let mut adapters_map = HashMap::new();
        for adapter in adapters {
            adapters_map.insert(adapter.name.clone(), adapter);
        }

        self.registry_adapters.set(adapters_map).unwrap();
        Ok(self.registry_adapters.get().unwrap())
    }

    pub async fn select_adapter(&self) -> Result<Vec<RegistryAdapter>> {
        let spinner =
            terminal_utils::create_spinner(&format!("Fetching adapters from registry..."));
        let registry_adapters = self.fetch_regsitry_adapters().await?;
        spinner.finish_and_clear();

        let mut adapter_names: Vec<String> = registry_adapters.keys().cloned().collect();
        adapter_names.sort();

        let selected_adapters = if !adapter_names.is_empty() {
            println!("\n{}\n", "🔌 Select adapters to install:".bright_cyan());
            let selections = MultiSelect::new()
                .with_prompt("Adapters")
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
            .map(|name| registry_adapters.get(name).unwrap().clone())
            .collect())
    }

    /// Install an adapter
    pub async fn install_adapter(&self, registry_adapters: Vec<RegistryAdapter>) -> Result<()> {
        // get installed adapters
        let installed_adapters_set = self.get_installed_adapters().await?;

        // filter not installed adapters
        let registry_adapters: Vec<RegistryAdapter> = registry_adapters
            .into_iter()
            .filter(|a| !installed_adapters_set.contains(&a.project_link))
            .collect();

        let prompt = format!(
            "Do you want to install the selected uninstalled adapters: [{}]",
            registry_adapters
                .iter()
                .map(|a| a.name.clone().bright_blue().bold().to_string())
                .collect::<Vec<String>>()
                .join(", ")
        );

        if !Confirm::new()
            .with_prompt(&prompt)
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            println!("Installation cancelled by user");
            return Ok(());
        }

        // Install the adapter
        let adapter_packages = registry_adapters
            .iter()
            .map(|a| a.project_link.as_str())
            .collect::<Vec<&str>>();

        Uv::add(adapter_packages, false, None, Some(&self.work_dir)).await?;

        // Add to configuration
        let adapters = registry_adapters
            .iter()
            .map(|a| Adapter {
                name: a.name.clone(),
                module_name: a.module_name.clone(),
            })
            .collect::<Vec<Adapter>>();

        // Add adapters to configuration
        ToolNonebot::parse(None)?.add_adapters(adapters)?;

        println!(
            "{} Successfully installed adapters: {}",
            "✓".bright_green(),
            registry_adapters
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<String>>()
                .join(", ")
        );

        // Show configuration instructions
        // if let Some(ref adapter) = adapter_info {
        //     if let Some(ref config_template) = adapter.config_template {
        //         self.show_configuration_instructions(adapter, config_template);
        //     }
        // }

        Ok(())
    }

    pub async fn get_installed_adapters(&self) -> Result<HashSet<String>> {
        let installed_adapters = Uv::list(Some(&self.work_dir)).await?;
        let installed_adapters_set = installed_adapters
            .into_iter()
            .filter(|a| a.contains("nonebot-adapter-"))
            .map(|a| a.split(" ").next().unwrap().to_owned())
            .collect::<HashSet<String>>();
        debug!("Installed adapters: {:?}", installed_adapters_set);
        Ok(installed_adapters_set)
    }

    /// Uninstall an adapter
    pub async fn uninstall_adapter(&self) -> Result<()> {
        // get installed adapters from configuration
        let installed_adapters = ToolNonebot::parse(None)?.nonebot()?.adapters;
        let installed_adapters_names = installed_adapters
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<String>>();

        // select adapters to uninstall
        let selected_adapters: Vec<String> = if !installed_adapters_names.is_empty() {
            println!("\n{}\n", "🔌 Select adapters to uninstall:".bright_cyan());
            let selections = MultiSelect::new()
                .with_prompt("Adapters")
                .items(&installed_adapters_names)
                //.defaults(&vec![true; adapter_names.len().min(1)]) // Select first adapter by default
                .interact()
                .map_err(|e| NbrError::io(e.to_string()))?;

            selections
                .into_iter()
                .map(|i| installed_adapters_names[i].to_string())
                .collect()
        } else {
            return Err(NbrError::not_found("No adapters to uninstall"));
        };

        // Remove from configuration
        ToolNonebot::parse(None)?.remove_adapters(
            selected_adapters
                .iter()
                .map(|a| a.as_str())
                .collect::<Vec<&str>>(),
        )?;

        // Uninstall the package
        let registry_adapters = self.fetch_regsitry_adapters().await?;
        let adapter_packages = selected_adapters
            .iter()
            .map(|a| registry_adapters.get(a).unwrap().project_link.as_str())
            .collect::<Vec<&str>>();

        // filter not installed adapters
        let installed_adapters_package_set = self.get_installed_adapters().await?;
        let adapter_packages = adapter_packages
            .into_iter()
            .filter(|a| installed_adapters_package_set.contains(*a))
            .collect::<Vec<&str>>();

        Uv::remove(adapter_packages, Some(&self.work_dir)).await?;

        println!(
            "{} Successfully uninstalled adapters: {}",
            "✓".bright_green(),
            selected_adapters
                .iter()
                .map(|a| a.clone())
                .collect::<Vec<String>>()
                .join(", ")
        );

        Ok(())
    }

    /// List available and installed adapters
    pub async fn list_adapters(&self, show_all: bool) -> Result<()> {
        let nonebot = ToolNonebot::parse(None)?.nonebot()?;

        let adapters_map = self.fetch_regsitry_adapters().await?;
        if show_all {
            println!("{}", "All Adapters:".bright_green().bold());
            adapters_map.iter().for_each(|(_, adapter)| {
                self.display_adapter(adapter);
            });
        } else {
            if nonebot.adapters.is_empty() {
                println!("{}", "No adapters installed.".bright_yellow());
                return Ok(());
            }

            println!("{}", "Installed Adapters:".bright_green().bold());
            nonebot.adapters.iter().for_each(|ia| {
                let adapter = adapters_map.get(ia.name.as_str()).unwrap();
                self.display_adapter(adapter);
            });
        }

        Ok(())
    }

    pub fn display_adapter(&self, adapter: &RegistryAdapter) {
        println!(
            " {} {} ({} {})",
            "•".bright_blue(),
            adapter.name.bright_blue().bold(),
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

    /// Find installed adapter by name
    #[allow(dead_code)]
    fn find_installed_adapter(&self, name: &str) -> Result<Adapter> {
        let nonebot = ToolNonebot::parse(None)?.nonebot()?;

        for adapter in &nonebot.adapters {
            if adapter.name == name
                || adapter.name.to_lowercase().contains(&name.to_lowercase())
                || name.to_lowercase().contains(&adapter.name.to_lowercase())
            {
                return Ok(adapter.clone());
            }
        }

        Err(NbrError::not_found(format!(
            "Adapter '{}' is not installed",
            name
        )))
    }

    /// Display adapter information
    #[allow(dead_code)]
    fn display_adapter_info(&self, adapter: &RegistryAdapter) {
        println!("{}", adapter.name.bright_blue().bold());
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

/// Handle the adapter command
pub async fn handle_adapter(matches: &ArgMatches) -> Result<()> {
    let adapter_manager = AdapterManager::new()?;

    match matches.subcommand() {
        Some(("install", _)) => {
            let selected_adapters = adapter_manager.select_adapter().await?;
            adapter_manager.install_adapter(selected_adapters).await
        }
        Some(("uninstall", _)) => adapter_manager.uninstall_adapter().await,
        Some(("list", sub_matches)) => {
            let show_all = sub_matches.get_flag("all");
            adapter_manager.list_adapters(show_all).await
        }
        _ => Err(NbrError::invalid_argument("Invalid adapter subcommand")),
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_fetch_regsitry_adapters() {
        let manager = AdapterManager::new().unwrap();
        let adapters_map = manager.fetch_regsitry_adapters().await.unwrap();
        assert!(adapters_map.len() > 0);
        for adapter in adapters_map.values() {
            println!(
                "{} {} ({})",
                adapter.project_link.bright_green(),
                format!("v{}", adapter.version).bright_yellow(),
                adapter.name.bright_blue()
            );
        }
    }
}
