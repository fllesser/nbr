//! Adapter command handler for nbr
//!
//! This module handles adapter management including installation, removal,
//! and listing adapters for NoneBot applications.

use crate::config::get_cache_dir;
use crate::error::{NbrError, Result};
use crate::log::StyledText;
use crate::pyproject::{Adapter, NbTomlEditor, PyProjectConfig};
use crate::utils::terminal_utils;
use crate::uv;
use clap::Subcommand;
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

    fn get_cache_file(&self) -> Result<PathBuf> {
        let cache_dir = get_cache_dir()?;
        Ok(cache_dir.join("adapters.json"))
    }

    /// Fetch registry adapters from registry.nonebot.dev
    pub async fn fetch_registry_adapters(
        &self,
        fetch_remote: bool,
    ) -> Result<&HashMap<String, RegistryAdapter>> {
        if let Some(adapters) = self.registry_adapters.get() {
            return Ok(adapters);
        }

        // 从缓存中获取
        let cache_file = self.get_cache_file()?;
        if !fetch_remote && cache_file.exists() {
            debug!("Loading adapters from cache: {}", cache_file.display());
            let adapters: HashMap<String, RegistryAdapter> =
                serde_json::from_slice(&std::fs::read(&cache_file)?)?;
            self.registry_adapters
                .set(adapters)
                .map_err(|_| NbrError::cache("Failed to parse adapter info"))?;
            return Ok(self.registry_adapters.get().unwrap());
        }

        // 从 registry 获取
        let spinner = terminal_utils::create_spinner("Fetching adapters from registry...");
        let adapters_json_url = "https://registry.nonebot.dev/adapters.json";
        let response = self
            .client
            .get(adapters_json_url)
            .send()
            .await
            .map_err(NbrError::Network)?;

        let adapters: Vec<RegistryAdapter> = response
            .json()
            .await
            .map_err(|e| NbrError::plugin(format!("Failed to parse adapter info: {}", e)))?;

        // 解析成功后，结束 spinner
        spinner.finish_and_clear();

        let adapters_map = adapters
            .iter()
            .map(|a| (a.name.to_owned(), a.clone()))
            .collect::<HashMap<String, RegistryAdapter>>();

        self.registry_adapters
            .set(adapters_map.clone())
            .map_err(|_| NbrError::cache("Failed to cache adapter info"))?;

        // 缓存到文件
        std::fs::write(cache_file, serde_json::to_string(&adapters_map)?)?;

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
    pub async fn select_adapters(
        &self,
        fetch_remote: bool,
        filter_installed: bool,
    ) -> Result<Vec<&RegistryAdapter>> {
        // 获取 registry 中的 adapters
        let registry_adapters = self.fetch_registry_adapters(fetch_remote).await?;
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
    pub async fn install_adapters(&self, fetch_remote: bool) -> Result<()> {
        let selected_adapters = self.select_adapters(fetch_remote, true).await?;

        if selected_adapters.is_empty() {
            warn!("You haven't selected any adapters to install");
            return Ok(());
        }
        let selected_adapters_names = selected_adapters
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<String>>()
            .join(", ");
        let prompt = StyledText::new(" ")
            .white_bold("Would you like to install")
            .cyan_bold(format!("[{}]", selected_adapters_names).as_str())
            .build();

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
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?.add_adapters(adapters)?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed adapters:")
            .cyan_bold(&selected_adapters_names)
            .println();

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
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .remove_adapters(selected_adapters.to_vec())?;

        // Uninstall the package
        let registry_adapters = self.fetch_registry_adapters(false).await?;

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

        StyledText::new(" ")
            .green_bold("✓ Successfully uninstalled adapters:")
            .cyan_bold(&selected_adapters.join(", "))
            .println();

        Ok(())
    }

    /// List available and installed adapters
    pub async fn list_adapters(&self, show_all: bool) -> Result<()> {
        let installed_adapters = self.get_installed_adapters_names();
        let adapters_map = self.fetch_registry_adapters(show_all).await?;

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
        StyledText::new(" ")
            .cyan_bold("  •")
            .cyan_bold(&adapter.name)
            .text(format!("({})", adapter.project_link).as_str())
            .green(format!("v{}", adapter.version).as_str())
            .println();
    }

    /// Display adapter information
    #[allow(dead_code)]
    fn display_adapter_info(&self, adapter: &RegistryAdapter) {
        StyledText::new("").cyan_bold(&adapter.name).println();
        StyledText::new(" ")
            .text("  Package:")
            .text(&adapter.project_link)
            .println();
        StyledText::new(" ")
            .text("  Module:")
            .text(&adapter.module_name)
            .println();
        StyledText::new(" ")
            .text("  Desc:")
            .text(&adapter.desc)
            .println();
        StyledText::new(" ")
            .text("  Version:")
            .text(&adapter.version)
            .println();
        StyledText::new(" ")
            .text("  Author:")
            .text(&adapter.author)
            .println();
        if let Some(ref homepage) = adapter.homepage {
            StyledText::new(" ")
                .text("  Homepage:")
                .text(homepage)
                .println();
        }
    }
}

#[derive(Subcommand)]
pub enum AdapterCommands {
    #[clap(about = "Install adapters")]
    Install {
        #[clap(short, long, help = "Fetch adapters from remote")]
        fetch_remote: bool,
    },
    #[clap(about = "Uninstall adapters")]
    Uninstall,
    #[clap(about = "List installed adapters, show all adapters if --all is set")]
    List {
        #[clap(short, long, help = "Show all adapters")]
        all: bool,
    },
}

/// Handle the adapter command
pub async fn handle_adapter(commands: &AdapterCommands) -> Result<()> {
    let adapter_manager = AdapterManager::new(None)?;

    match commands {
        AdapterCommands::Install { fetch_remote } => {
            adapter_manager.install_adapters(*fetch_remote).await
        }
        AdapterCommands::Uninstall => adapter_manager.uninstall_adapters().await,
        AdapterCommands::List { all } => adapter_manager.list_adapters(*all).await,
    }
}
