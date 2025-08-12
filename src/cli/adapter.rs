//! Adapter command handler for nb-cli
//!
//! This module handles adapter management including installation, removal,
//! and listing adapters for NoneBot applications.
#![allow(dead_code)]

use crate::config::ConfigManager;
use crate::error::{NbCliError, Result};
use crate::pyproject::Adapter;
use crate::utils::{process_utils, string_utils, terminal_utils};
use clap::ArgMatches;
use colored::*;
use dialoguer::Confirm;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

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

/// Built-in adapter definitions
const BUILTIN_ADAPTERS: &[(&str, &str, &str)] = &[
    (
        "OneBot V11",
        "nonebot-adapter-onebot",
        "OneBot V11 协议适配器",
    ),
    (
        "OneBot V12",
        "nonebot-adapter-ob12",
        "OneBot V12 协议适配器",
    ),
    (
        "Telegram",
        "nonebot-adapter-telegram",
        "Telegram Bot API 适配器",
    ),
    ("钉钉", "nonebot-adapter-ding", "钉钉机器人适配器"),
    ("飞书", "nonebot-adapter-feishu", "飞书机器人适配器"),
    ("Console", "nonebot-adapter-console", "控制台适配器"),
    ("Discord", "nonebot-adapter-discord", "Discord Bot 适配器"),
    ("Kaiheila", "nonebot-adapter-kaiheila", "开黑啦适配器"),
    ("Mirai", "nonebot-adapter-mirai", "Mirai 适配器"),
    ("红豆Live", "nonebot-adapter-red", "红豆Live适配器"),
    ("Satori", "nonebot-adapter-satori", "Satori 协议适配器"),
];

/// Adapter manager
pub struct AdapterManager {
    /// Configuration manager
    config_manager: ConfigManager,
    /// HTTP client for registry requests
    client: Client,
    /// Python executable path
    python_path: String,
    /// Working directory
    work_dir: PathBuf,
    /// Registry adapters
    registry_adapters: OnceLock<HashMap<String, RegistryAdapter>>,
}

impl AdapterManager {
    /// Create a new adapter manager
    pub async fn new() -> Result<Self> {
        let mut config_manager = ConfigManager::new()?;
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
        .map_err(|_| NbCliError::unknown("Request timeout"))?
        .map_err(|e| NbCliError::Network(e))?;

        if !response.status().is_success() {
            return Err(NbCliError::not_found("Adapter registry not found"));
        }

        let adapters: Vec<RegistryAdapter> = response
            .json()
            .await
            .map_err(|e| NbCliError::plugin(format!("Failed to parse adapter info: {}", e)))?;

        let mut adapters_map = HashMap::new();
        for adapter in adapters {
            adapters_map.insert(adapter.name.clone(), adapter);
        }

        self.registry_adapters.set(adapters_map).unwrap();
        Ok(self.registry_adapters.get().unwrap())
    }

    /// Install an adapter
    pub async fn install_adapter(&mut self, name: &str) -> Result<()> {
        info!("Installing adapter: {}", name);

        // Check if it's a built-in adapter
        let registry_adapter = self.get_registry_adapter(name).await?;

        // Validate package name
        string_utils::validate_package_name(&registry_adapter.project_link)?;

        // Check if already installed
        if self
            .is_adapter_installed(&registry_adapter.project_link)
            .await?
        {
            return Err(NbCliError::already_exists(format!(
                "Adapter '{}' is already installed",
                registry_adapter.project_link
            )));
        }

        // Show adapter information if available

        self.display_adapter_info(&registry_adapter);

        if !Confirm::new()
            .with_prompt("Do you want to install this adapter?")
            .default(true)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Installation cancelled by user");
            return Ok(());
        }

        // Install the adapter
        self.uv_install(&registry_adapter.project_link).await?;

        // PyProjectConfig::add_adapter(&registry_adapter.name, &registry_adapter.module_name).await?;

        println!(
            "{} Successfully installed adapter: {} ({} v{})",
            "✓".bright_green(),
            registry_adapter.name.bright_blue(),
            registry_adapter.project_link.bright_black(),
            registry_adapter.version.bright_white()
        );

        self.add_adapter_to_config(Adapter {
            name: registry_adapter.name.clone(),
            module_name: registry_adapter.module_name.clone(),
        })
        .await?;

        // Show configuration instructions
        // if let Some(ref adapter) = adapter_info {
        //     if let Some(ref config_template) = adapter.config_template {
        //         self.show_configuration_instructions(adapter, config_template);
        //     }
        // }

        Ok(())
    }

    /// Uninstall an adapter
    pub async fn uninstall_adapter(&mut self, name: &str) -> Result<()> {
        info!("Uninstalling adapter: {}", name);

        // Find the adapter in configuration
        let registry_adapter = self.get_registry_adapter(name).await?;

        // Confirm uninstallation
        if !Confirm::new()
            .with_prompt(&format!(
                "Are you sure you want to uninstall '{}'?",
                registry_adapter.project_link
            ))
            .default(false)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Uninstallation cancelled by user");
            return Ok(());
        }

        // Uninstall the package
        self.uv_uninstall(&registry_adapter.project_link).await?;

        // Remove from configuration
        // PyProjectConfig::remove_adapter(&registry_adapter.name).await?;

        println!(
            "{} Successfully uninstalled adapter: {} ({} v{})",
            "✓".bright_green(),
            registry_adapter.name.bright_blue(),
            registry_adapter.project_link.bright_black(),
            registry_adapter.version.bright_white()
        );

        self.remove_adapter_from_config(registry_adapter.name.to_string())
            .await?;

        Ok(())
    }

    /// List available and installed adapters
    pub async fn list_adapters(&self, show_all: bool) -> Result<()> {
        if show_all {
            println!("{}", "All Adapters:".bright_green().bold());
        } else {
            println!("{}", "Installed Adapters:".bright_green().bold());
        }

        println!();
        let config = self.config_manager.config();
        let installed_adapters = &config.nb_config.tool.nonebot.adapters;

        let adapters_map = self.fetch_regsitry_adapters().await?;
        if show_all {
            adapters_map.iter().for_each(|(_, adapter)| {
                self.display_adapter(adapter);
            });
        } else {
            installed_adapters.iter().for_each(|ia| {
                let adapter = adapters_map.get(ia.name.as_str()).unwrap();
                self.display_adapter(adapter);
            });
        }

        Ok(())
    }

    pub fn display_adapter(&self, adapter: &RegistryAdapter) {
        println!(" {}", adapter.name.bright_blue().bold());
        println!("   Package: {}", adapter.project_link);
        println!("   Module: {}", adapter.module_name);
        println!("   Desc: {}", adapter.desc);
    }

    /// Get adapter from registry
    async fn get_registry_adapter(&self, package_name: &str) -> Result<&RegistryAdapter> {
        let adapters_map = self.fetch_regsitry_adapters().await?;

        let adapter = adapters_map
            .values()
            .find(|a| a.name == package_name)
            .ok_or_else(|| {
                NbCliError::not_found(format!("Adapter '{}' not found in registry", package_name))
            })?;
        Ok(adapter)
    }

    /// Get adapter configuration template
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

    /// Install package via uv
    async fn uv_install(&self, package: &str) -> Result<()> {
        let args = vec!["add", package];

        let spinner = terminal_utils::create_spinner(&format!("Installing {}...", package));

        let result = process_utils::execute_command_with_output(
            "uv",
            &args,
            Some(&self.work_dir),
            300, // 5 minutes timeout
        )
        .await;

        spinner.finish_and_clear();

        result.map(|_| ())
    }

    /// Uninstall package via uv
    async fn uv_uninstall(&self, package: &str) -> Result<()> {
        let args = vec!["remove", package];

        let spinner = terminal_utils::create_spinner(&format!("Uninstalling {}...", package));

        let result = process_utils::execute_command_with_output(
            "uv",
            &args,
            Some(&self.work_dir),
            60, // 1 minute timeout
        )
        .await;

        spinner.finish_and_clear();

        result.map(|_| ())
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

    /// Check if adapter is installed
    async fn is_adapter_installed(&self, package: &str) -> Result<bool> {
        match self.get_installed_package_version(package).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Find installed adapter by name
    fn find_installed_adapter(&self, name: &str) -> Result<Adapter> {
        let config = self.config_manager.config();

        for adapter in &config.nb_config.tool.nonebot.adapters {
            if adapter.name == name
                || adapter.name.to_lowercase().contains(&name.to_lowercase())
                || name.to_lowercase().contains(&adapter.name.to_lowercase())
            {
                return Ok(adapter.clone());
            }
        }

        Err(NbCliError::not_found(format!(
            "Adapter '{}' is not installed",
            name
        )))
    }

    /// Add adapter to configuration
    async fn add_adapter_to_config(&mut self, adapter: Adapter) -> Result<()> {
        self.config_manager.update_nb_config(|nb_config| {
            // Remove existing adapter with same name
            nb_config
                .tool
                .nonebot
                .adapters
                .retain(|a| a.name != adapter.name);
            // Add new adapter info
            nb_config.tool.nonebot.adapters.push(adapter);
        })?;

        self.config_manager.save().await
    }

    /// Remove adapter from configuration
    async fn remove_adapter_from_config(&mut self, name: String) -> Result<()> {
        self.config_manager.update_nb_config(|nb_config| {
            nb_config.tool.nonebot.adapters.retain(|a| a.name != name);
        })?;

        self.config_manager.save().await
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
    let mut adapter_manager = AdapterManager::new().await?;

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
        _ => Err(NbCliError::invalid_argument("Invalid adapter subcommand")),
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

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_fetch_regsitry_adapters() {
        let manager = AdapterManager::new().await.unwrap();
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
