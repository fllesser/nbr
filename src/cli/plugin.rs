use crate::config::get_cache_dir;
use crate::error::{NbrError, Result};
use crate::log::StyledText;
use crate::pyproject::NbTomlEditor;
use crate::utils::terminal_utils;
use crate::uv::{self, CmdBuilder, Package};
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
        #[clap(help = "Plugin name")]
        name: String,
        #[clap(short, long, help = "Specify the index url")]
        index: Option<String>,
        #[clap(short, long, help = "Upgrade the plugin")]
        upgrade: bool,
        #[clap(short, long, help = "Reinstall the plugin")]
        reinstall: bool,
        #[clap(short, long, help = "Fetch plugins from remote")]
        fetch_remote: bool,
    },
    #[clap(about = "Uninstall a plugin")]
    Uninstall {
        #[clap(help = "Plugin name")]
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
        #[clap(short, long, help = "Fetch plugins from remote")]
        fetch_remote: bool,
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
    #[clap(about = "Reset nonebot plugins, remove invalid plugins and add missing plugins")]
    Reset,
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
            reinstall,
            fetch_remote,
        } => {
            let options = InstallOptions::new(name, *upgrade, *reinstall, index.as_deref())?;
            manager.install(options, *fetch_remote).await
        }
        PluginCommands::Uninstall { name } => manager.uninstall(name).await,
        PluginCommands::List { outdated } => manager.list(*outdated).await,
        PluginCommands::Search {
            query,
            limit,
            fetch_remote,
        } => manager.search_plugins(query, *limit, *fetch_remote).await,
        PluginCommands::Update {
            name,
            all,
            reinstall,
        } => manager.update(name.as_deref(), *all, *reinstall).await,
        PluginCommands::Reset => manager.reset().await,
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
    registry_plugins: OnceLock<HashMap<String, RegistryPlugin>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new(None).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct InstallOptions<'a> {
    pub name: &'a str,
    pub module_name: Option<String>,
    pub git_url: Option<&'a str>,
    pub upgrade: bool,
    pub reinstall: bool,
    pub index_url: Option<&'a str>,
    pub extras: Option<Vec<&'a str>>,
    pub specifier: Option<&'a str>,
}

impl<'a> InstallOptions<'a> {
    pub fn new(
        name: &'a str,
        upgrade: bool,
        reinstall: bool,
        index_url: Option<&'a str>,
    ) -> Result<Self> {
        let options = Self {
            name,
            module_name: None,
            git_url: None,
            upgrade,
            reinstall,
            index_url,
            extras: None,
            specifier: None,
        };
        options.parse_name()
    }

    pub fn parse_name(mut self) -> Result<Self> {
        if self.name.starts_with("git+") {
            const GIT_URL_PATTERN: &str = r"nonebot-plugin-(?P<repo>[^/@]+)";
            let re = Regex::new(GIT_URL_PATTERN).unwrap();
            let captures = re
                .captures(self.name)
                .ok_or(NbrError::invalid_argument(format!(
                    "Invalid plugin name: {}",
                    self.name
                )))?;
            self.git_url = Some(self.name);
            self.name = captures.get(0).map(|m| m.as_str()).unwrap();
            self.module_name = Some(self.name.replace("-", "_"));
            return Ok(self);
        }
        const PATTERN: &str = r"^([a-zA-Z0-9_-]+)(?:\[([a-zA-Z0-9_,\s]*)\])?(?:\s*((?:==|>=|<=|>|<|~=)\s*[a-zA-Z0-9\.]+))?$";
        let re = Regex::new(PATTERN).unwrap();
        let captures = re
            .captures(self.name)
            .ok_or(NbrError::invalid_argument(format!(
                "Invalid plugin name: {}",
                self.name
            )))?;
        self.name = captures.get(1).map(|m| m.as_str()).unwrap();
        self.module_name = Some(self.name.replace("-", "_"));
        self.extras = captures
            .get(2)
            .map(|m| m.as_str().split(',').collect::<Vec<&str>>());
        self.specifier = captures.get(3).map(|m| m.as_str());
        Ok(self)
    }

    pub fn install(&self) -> Result<()> {
        let mut args = vec!["add"];

        if let Some(git_url) = self.git_url {
            args.push(git_url);
        } else {
            args.push(self.name);
        }

        if self.upgrade {
            args.push("--upgrade");
        }
        if self.reinstall {
            args.push("--reinstall");
        }
        if let Some(index_url) = self.index_url {
            args.push("--index-url");
            args.push(index_url);
        }
        if let Some(ref extras) = self.extras {
            let extras = extras.iter().flat_map(|e| ["--extra", e]);
            args.extend(extras);
        }
        CmdBuilder::uv(args).run()
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

    pub async fn install(&mut self, options: InstallOptions<'_>, fetch_remote: bool) -> Result<()> {
        if options.git_url.is_some() {
            return self.install_from_github(options).await;
        }
        if let Ok(registry_plugin) = self.get_registry_plugin(options.name, fetch_remote).await {
            return self.install_registry_plugin(registry_plugin, options).await;
        }

        self.install_unregistered_plugin(options).await
    }

    pub async fn install_from_github(&mut self, options: InstallOptions<'_>) -> Result<()> {
        let git_url = options.git_url.unwrap();
        debug!("Installing plugin from github: {}", git_url);

        let prompt = StyledText::new(" ")
            .text("Would you like to install")
            .cyan(options.name)
            .text("from github")
            .build();
        // 确定是否安装 github 插件
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            options.install()?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        // Add to configuration
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![&options.module_name.unwrap()])?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed plugin:")
            .cyan_bold(options.name)
            .println();
        Ok(())
    }

    pub async fn install_unregistered_plugin(&mut self, options: InstallOptions<'_>) -> Result<()> {
        debug!("Installing unregistered plugin: {}", options.name);

        let prompt = StyledText::new(" ")
            .text("Would you like to install")
            .cyan(options.name)
            .text("from PyPI?")
            .build();
        if Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            options.install()?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        // Add to configuration
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![&options.module_name.unwrap()])?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed plugin:")
            .cyan_bold(options.name)
            .println();
        Ok(())
    }

    /// Install a plugin
    pub async fn install_registry_plugin(
        &self,
        registry_plugin: &RegistryPlugin,
        options: InstallOptions<'_>,
    ) -> Result<()> {
        let package_name = &registry_plugin.project_link;
        // Show plugin information if available
        self.display_plugin_info(registry_plugin);

        let prompt = StyledText::new(" ")
            .text("Would you like to install")
            .cyan(package_name)
            .build();
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(true)
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
        {
            error!("Installation operation cancelled.");
            return Ok(());
        }
        // Install the plugin
        options.install()?;

        // Add to configuration
        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![&registry_plugin.module_name])?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed plugin:")
            .cyan_bold(package_name)
            .println();

        Ok(())
    }

    /// Uninstall a plugin
    pub async fn uninstall(&self, name: &str) -> Result<()> {
        debug!("Uninstalling plugin: {}", name);

        if let Ok(registry_plugin) = self.get_registry_plugin(name, false).await {
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
                .remove_plugins(vec![&package_name.replace("-", "_")])?;

            StyledText::new(" ")
                .green_bold("✓ Successfully uninstalled plugin:")
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
            .remove_plugins(vec![&registry_plugin.module_name])?;

        StyledText::new(" ")
            .green_bold("✓ Successfully uninstalled plugin:")
            .cyan_bold(&package_name)
            .println();

        Ok(())
    }

    pub async fn get_installed_plugins(&self, outdated: bool) -> Result<Vec<Package>> {
        let installed_packages = uv::list(outdated).await?;
        let installed_plugins = installed_packages
            .into_iter()
            .filter(|p| Self::is_plugin(&p.name))
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

    pub fn is_plugin(package_name: &str) -> bool {
        package_name.starts_with("nonebot") && package_name.contains("plugin")
    }

    pub async fn reset(&self) -> Result<()> {
        let mut installed_plugins = self.get_installed_plugins(false).await?;

        let mut requires_plugins: Vec<String> = Vec::new();
        for plugin in &installed_plugins {
            let requires = uv::show_package_info(plugin.name.as_str(), Some(&self.work_dir))
                .await?
                .requires
                .unwrap_or_default();
            for require in requires {
                if Self::is_plugin(&require) && !requires_plugins.contains(&require) {
                    requires_plugins.push(require);
                }
            }
        }

        // 去除 requires 的插件
        installed_plugins.retain(|p| !requires_plugins.contains(&p.name));

        let plugins = installed_plugins
            .iter()
            .map(|p| p.name.replace("-", "_"))
            .collect::<Vec<String>>();

        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .reset_plugins(plugins.iter().map(|p| p.as_str()).collect())?;
        StyledText::new(" ")
            .green_bold("✓ Successfully reset nonebot plugins:")
            .cyan_bold(&plugins.join(", "))
            .println();
        Ok(())
    }

    /// Search plugins in registry
    pub async fn search_plugins(
        &self,
        query: &str,
        limit: usize,
        fetch_remote: bool,
    ) -> Result<()> {
        debug!("Searching plugins for: {}", query);

        let results = self
            .search_registry_plugins(query, limit, fetch_remote)
            .await?;

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

        StyledText::new(" ")
            .green_bold("Successfully updated plugin(s):")
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

    pub fn get_cache_file(&self) -> Result<PathBuf> {
        let cache_dir = get_cache_dir()?;
        Ok(cache_dir.join("plugins.json"))
    }

    pub async fn fetch_registry_plugins(
        &self,
        fetch_remote: bool,
    ) -> Result<&HashMap<String, RegistryPlugin>> {
        if let Some(plugins) = self.registry_plugins.get() {
            return Ok(plugins);
        }

        let cache_file = self.get_cache_file()?;
        if !fetch_remote && cache_file.exists() {
            debug!("Loading plugins from cache: {}", cache_file.display());
            let plugins: HashMap<String, RegistryPlugin> =
                serde_json::from_slice(&std::fs::read(&cache_file)?)?;
            self.registry_plugins
                .set(plugins)
                .map_err(|_| NbrError::cache("Failed to parse plugin info"))?;
            return Ok(self.registry_plugins.get().unwrap());
        }

        let spinner = terminal_utils::create_spinner("Fetching plugins from registry...");
        let plugins_json_url = "https://registry.nonebot.dev/plugins.json";
        let response = self
            .client
            .get(plugins_json_url)
            .send()
            .await
            .map_err(NbrError::Network)?;

        let plugins: Vec<RegistryPlugin> = response
            .json()
            .await
            .map_err(|e| NbrError::plugin(format!("Failed to parse plugin info: {}", e)))?;

        spinner.finish_and_clear();
        let plugins_map = plugins
            .iter()
            .map(|p| (p.project_link.clone(), p.clone()))
            .collect::<HashMap<String, RegistryPlugin>>();
        self.registry_plugins
            .set(plugins_map.clone())
            .map_err(|_| NbrError::cache("Failed to cache plugin info"))?;

        // 缓存到文件
        std::fs::write(cache_file, serde_json::to_string(&plugins_map)?)?;
        Ok(self.registry_plugins.get().unwrap())
    }

    /// Get plugin from registry
    async fn get_registry_plugin(
        &self,
        package_name: &str,
        fetch_remote: bool,
    ) -> Result<&RegistryPlugin> {
        let plugins = self.fetch_registry_plugins(fetch_remote).await?;
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
        fetch_remote: bool,
    ) -> Result<Vec<&RegistryPlugin>> {
        let plugins_map = self.fetch_registry_plugins(fetch_remote).await?;

        let results: Vec<&RegistryPlugin> = plugins_map
            .values()
            .filter(|plugin| {
                plugin.project_link.contains(query)
                    || plugin.name.contains(query)
                    || plugin.desc.contains(query)
                    || plugin.author.contains(query)
            })
            .take(limit)
            .collect();

        Ok(results)
    }

    /// Display plugin information
    fn display_plugin_info(&self, plugin: &RegistryPlugin) {
        StyledText::new("").cyan_bold(&plugin.name).println();
        StyledText::new(" ")
            .text("  Desc:")
            .white(&plugin.desc)
            .println();
        StyledText::new(" ")
            .text("  Version:")
            .white(&plugin.version)
            .println();
        StyledText::new(" ")
            .text("  Author:")
            .white(&plugin.author)
            .println();

        if let Some(ref homepage) = plugin.homepage {
            StyledText::new(" ")
                .text("  Homepage:")
                .cyan(homepage)
                .println();
        }

        if !plugin.tags.is_empty() {
            StyledText::new(" ")
                .text("  Tags:")
                .yellow(
                    plugin
                        .tags
                        .iter()
                        .map(|t| t.get("label").unwrap().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .as_str(),
                )
                .println();
        }
    }

    /// Display search result
    fn display_search_result(&self, plugin: &RegistryPlugin, index: usize) {
        StyledText::new("")
            .cyan_bold(format!("{}.{}", index, plugin.name).as_str())
            .println();

        StyledText::new(" ")
            .text("  Desc:")
            .white(&plugin.desc)
            .println();
        if let Some(ref homepage) = plugin.homepage {
            StyledText::new(" ")
                .text("  Homepage:")
                .cyan(homepage)
                .println();
        }

        StyledText::new(" ")
            .text("  Install Command:")
            .yellow(&format!("nbr plugin install {}", plugin.project_link))
            .println();
    }
}
