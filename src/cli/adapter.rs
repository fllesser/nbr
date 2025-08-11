//! Adapter command handler for nb-cli
//!
//! This module handles adapter management including installation, removal,
//! and listing adapters for NoneBot applications.
#![allow(dead_code)]

use crate::config::{AdapterInfo, ConfigManager};
use crate::error::{NbCliError, Result};
use crate::pyproject::PyProjectConfig;
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
            registry_adapters: OnceLock::new(),
        })
    }

    pub async fn get_regsitry_adapters(&self) -> Result<&HashMap<String, RegistryAdapter>> {
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

        PyProjectConfig::add_adapter(&registry_adapter.name, &registry_adapter.module_name).await?;

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
        PyProjectConfig::remove_adapter(&registry_adapter.name).await?;

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
    pub async fn list_adapters(&self) -> Result<()> {
        println!("{}", "Available Adapters:".bright_green().bold());
        println!();

        let config = self.config_manager.config();
        let installed_adapters = if let Some(ref project_config) = config.project {
            &project_config.adapters
        } else {
            &Vec::new()
        };

        // Show built-in adapters
        println!("{}", "Built-in Adapters:".bright_blue());
        for (name, package, description) in BUILTIN_ADAPTERS {
            let is_installed = installed_adapters.iter().any(|a| {
                a.name == *name || a.name.contains(&package.replace("nonebot-adapter-", ""))
            });

            let status = if is_installed {
                "installed".bright_green()
            } else {
                "available".bright_black()
            };

            println!(
                "  {} {} {} ({})",
                if is_installed { "✓" } else { "•" },
                name.bright_white(),
                format!("[{}]", status),
                description.bright_black()
            );
        }

        // Show installed adapters that are not built-in
        let non_builtin_installed: Vec<_> = installed_adapters
            .iter()
            .filter(|adapter| {
                !BUILTIN_ADAPTERS
                    .iter()
                    .any(|(name, _, _)| adapter.name == *name)
            })
            .collect();

        if !non_builtin_installed.is_empty() {
            println!();
            println!("{}", "Other Installed Adapters:".bright_blue());
            for adapter in &non_builtin_installed {
                println!(
                    "  {} {} {} (v{})",
                    "✓",
                    adapter.name.bright_white(),
                    "[installed]".bright_green(),
                    adapter.version.bright_black()
                );
            }
        }

        if installed_adapters.is_empty() {
            println!();
            println!("{}", "No adapters installed.".bright_yellow());
            println!("Use 'nb adapter install <name>' to install an adapter.");
        }

        Ok(())
    }

    /// Get adapter from registry
    async fn get_registry_adapter(&self, package_name: &str) -> Result<RegistryAdapter> {
        let adapters_map = self.get_regsitry_adapters().await?;

        let adapter = adapters_map
            .values()
            .find(|a| a.name == package_name)
            .ok_or_else(|| {
                NbCliError::not_found(format!("Adapter '{}' not found in registry", package_name))
            })?;
        Ok(adapter.clone())
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
    fn find_installed_adapter(&self, name: &str) -> Result<AdapterInfo> {
        let config = self.config_manager.config();

        if let Some(ref project_config) = config.project {
            for adapter in &project_config.adapters {
                if adapter.name == name
                    || adapter.name.to_lowercase().contains(&name.to_lowercase())
                    || name.to_lowercase().contains(&adapter.name.to_lowercase())
                {
                    return Ok(adapter.clone());
                }
            }
        }

        Err(NbCliError::not_found(format!(
            "Adapter '{}' is not installed",
            name
        )))
    }

    /// Add adapter to configuration
    async fn add_adapter_to_config(&mut self, adapter: AdapterInfo) -> Result<()> {
        self.config_manager
            .update_project_config(|project_config| {
                if let Some(config) = project_config {
                    // Remove existing adapter with same name
                    config.adapters.retain(|a| a.name != adapter.name);
                    // Add new adapter info
                    config.adapters.push(adapter);
                }
            })?;

        self.config_manager.save().await
    }

    /// Remove adapter from configuration
    async fn remove_adapter_from_config(&mut self, name: &str) -> Result<()> {
        self.config_manager
            .update_project_config(|project_config| {
                if let Some(config) = project_config {
                    config.adapters.retain(|a| a.name != name);
                }
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
    let config_manager = ConfigManager::new()?;
    let mut adapter_manager = AdapterManager::new(config_manager).await?;

    match matches.subcommand() {
        Some(("install", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            adapter_manager.install_adapter(name).await
        }
        Some(("uninstall", sub_matches)) => {
            let name = sub_matches.get_one::<String>("name").unwrap();
            adapter_manager.uninstall_adapter(name).await
        }
        Some(("list", _)) => adapter_manager.list_adapters().await,
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
    async fn test_get_regsitry_adapters_map() {
        let manager = AdapterManager::new(ConfigManager::new().unwrap())
            .await
            .unwrap();
        let adapters_map = manager.get_regsitry_adapters().await.unwrap();
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
