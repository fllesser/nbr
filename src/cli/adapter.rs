//! Adapter command handler for nbr
//!
//! This module handles adapter management including installation, removal,
//! and listing adapters for NoneBot applications.

use crate::error::{NbrError, Result};
use crate::pyproject::{Adapter, ToolNonebot};
use crate::uv::Uv;
use clap::ArgMatches;
use colored::*;
use dialoguer::Confirm;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::timeout;
use tracing::debug;

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

    /// Install an adapter
    pub async fn install_adapter(&mut self, package_name: &str) -> Result<()> {
        debug!("Installing adapter: {}", package_name);

        // Check if already installed
        if Uv::is_installed(package_name, Some(&self.work_dir)).await {
            return Err(NbrError::already_exists(format!(
                "Adapter '{}' is already installed",
                package_name
            )));
        }

        // Get registry adapter
        let registry_adapter = self.get_registry_adapter(package_name).await?;

        // Show adapter information if available
        self.display_adapter_info(&registry_adapter);

        if !Confirm::new()
            .with_prompt("Do you want to install this adapter?")
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            println!("Installation cancelled by user");
            return Ok(());
        }

        // Install the adapter
        Uv::add(
            &registry_adapter.project_link,
            false,
            None,
            Some(&self.work_dir),
        )
        .await?;

        // Add to configuration
        ToolNonebot::parse(None)?.add_adapters(vec![Adapter {
            name: registry_adapter.name.clone(),
            module_name: registry_adapter.module_name.clone(),
        }])?;

        println!(
            "{} Successfully installed adapter: {} ({} v{})",
            "✓".bright_green(),
            registry_adapter.name.bright_blue(),
            registry_adapter.project_link.bright_black(),
            registry_adapter.version.bright_white()
        );

        // Show configuration instructions
        // if let Some(ref adapter) = adapter_info {
        //     if let Some(ref config_template) = adapter.config_template {
        //         self.show_configuration_instructions(adapter, config_template);
        //     }
        // }

        Ok(())
    }

    /// Uninstall an adapter
    pub async fn uninstall_adapter(&mut self, package_name: &str) -> Result<()> {
        debug!("Uninstalling adapter: {}", package_name);

        // Check if installed
        if !Uv::is_installed(package_name, Some(&self.work_dir)).await {
            return Err(NbrError::not_found(format!(
                "Adapter '{}' is not installed",
                package_name
            )));
        }

        // Get registry adapter
        let registry_adapter = self.get_registry_adapter(package_name).await?;

        // Confirm uninstallation
        if !Confirm::new()
            .with_prompt(&format!(
                "Are you sure you want to uninstall '{}'?",
                registry_adapter.project_link
            ))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            println!("Uninstallation cancelled by user");
            return Ok(());
        }

        // Uninstall the package
        Uv::remove(&registry_adapter.project_link, Some(&self.work_dir)).await?;

        // Remove from configuration
        ToolNonebot::parse(None)?.remove_adapters(vec![registry_adapter.name.clone()])?;

        println!(
            "{} Successfully uninstalled adapter: {} ({} v{})",
            "✓".bright_green(),
            registry_adapter.name.bright_blue(),
            registry_adapter.project_link.bright_black(),
            registry_adapter.version.bright_white()
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

    /// Get adapter from registry
    async fn get_registry_adapter(&self, package_name: &str) -> Result<&RegistryAdapter> {
        let adapters_map = self.fetch_regsitry_adapters().await?;

        let adapter = adapters_map
            .values()
            .find(|a| a.project_link == package_name)
            .ok_or_else(|| {
                NbrError::not_found(format!("Adapter '{}' not found in registry", package_name))
            })?;
        Ok(adapter)
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
    let mut adapter_manager = AdapterManager::new()?;

    match matches.subcommand() {
        Some(("install", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            adapter_manager.install_adapter(name).await
        }
        Some(("uninstall", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            adapter_manager.uninstall_adapter(name).await
        }
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
