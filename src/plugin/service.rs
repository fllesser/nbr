use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::{debug, error, info, warn};

use crate::context::GlobalContext;
use crate::log::StyledText;
use crate::pyproject::{NbTomlEditor, PyProjectConfig};
use crate::registry_store;
use crate::uv::{self, Package};

use super::options::InstallOptions;
use super::registry::{RegistryPlugin, filter_search};

pub struct PluginManager {
    work_dir: PathBuf,
    registry_plugins: OnceLock<HashMap<String, RegistryPlugin>>,
    global_context: GlobalContext,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new(None, GlobalContext::default()).unwrap()
    }
}

impl PluginManager {
    pub fn new(work_dir: Option<PathBuf>, ctx: GlobalContext) -> Result<Self> {
        let work_dir = work_dir.unwrap_or_else(|| Path::new(".").to_path_buf());
        Ok(Self {
            work_dir,
            registry_plugins: OnceLock::new(),
            global_context: ctx,
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
        let git_url = options
            .git_url
            .context("git_url should be present if install_from_github is called")?;
        debug!("Installing plugin from github: {}", git_url);

        let prompt = StyledText::new(" ")
            .text("Would you like to install")
            .cyan(options.name)
            .text("from github")
            .to_string();
        if self.global_context.confirm(prompt, true).await? {
            options.install()?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![&options.module_name])?;

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
            .to_string();

        if self.global_context.confirm(prompt, true).await? {
            options.install()?;
        } else {
            error!("{}", "Installation operation cancelled.");
            return Ok(());
        }

        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![&options.module_name])?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed plugin:")
            .cyan_bold(options.name)
            .println();
        Ok(())
    }

    pub async fn install_registry_plugin(
        &self,
        registry_plugin: &RegistryPlugin,
        options: InstallOptions<'_>,
    ) -> Result<()> {
        let package_name = &registry_plugin.project_link;
        self.display_plugin_info(registry_plugin);

        let prompt = StyledText::new(" ")
            .text("Would you like to install")
            .cyan(package_name)
            .to_string();
        if self.global_context.confirm(prompt, true).await? {
            options.install()?;
        } else {
            error!("Installation operation cancelled.");
            return Ok(());
        }

        // Keep existing behavior (even if redundant)
        options.install()?;

        NbTomlEditor::with_work_dir(Some(&self.work_dir))?
            .add_plugins(vec![&registry_plugin.module_name])?;

        StyledText::new(" ")
            .green_bold("✓ Successfully installed plugin:")
            .cyan_bold(package_name)
            .println();

        Ok(())
    }

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
            anyhow::bail!("Plugin '{}' is not installed.", package_name);
        }

        let prompt = format!("Would you like to uninstall '{package_name}'");
        if self.global_context.confirm(prompt, true).await? {
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
        if !uv::is_installed(&package_name).await {
            anyhow::bail!(
                "Plugin '{}' is not installed.",
                registry_plugin.project_link
            );
        }
        let prompt = format!("Would you like to uninstall '{package_name}'");
        if self.global_context.confirm(prompt, false).await? {
            error!("{}", "Uninstallation operation cancelled.");
            return Ok(());
        }

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
        Ok(installed_packages
            .into_iter()
            .filter(|p| Self::is_plugin(&p.name))
            .collect())
    }

    pub async fn list(&self, show_outdated: bool) -> Result<()> {
        let mut installed_plugins = self.get_installed_plugins(false).await?;
        if show_outdated {
            let outdated_plugins = self.get_installed_plugins(true).await?;
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

        installed_plugins.retain(|p| !requires_plugins.contains(&p.name));

        let plugins = installed_plugins
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>();

        let installed_module_names = plugins
            .iter()
            .map(|p| p.replace("-", "_"))
            .collect::<Vec<String>>();

        let pyproject_cfg = PyProjectConfig::parse(Some(&self.work_dir))?;
        let configured_plugins = pyproject_cfg
            .tool
            .and_then(|t| t.nonebot)
            .and_then(|n| n.plugins)
            .unwrap_or_default();

        let invalid_plugins = configured_plugins
            .iter()
            .filter(|p| !installed_module_names.contains(p))
            .cloned()
            .collect::<Vec<String>>();
        let missing_plugins = installed_module_names
            .iter()
            .filter(|p| !configured_plugins.contains(p))
            .cloned()
            .collect::<Vec<String>>();

        let mut pyproject = NbTomlEditor::with_work_dir(Some(&self.work_dir))?;
        if !invalid_plugins.is_empty() {
            let invalid_refs = invalid_plugins
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            pyproject.remove_plugins(invalid_refs)?;
        }
        if !missing_plugins.is_empty() {
            let missing_refs = missing_plugins
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            pyproject.add_plugins(missing_refs)?;
        }

        if invalid_plugins.is_empty() && missing_plugins.is_empty() {
            info!("No plugins to reset.");
            return Ok(());
        }

        if !invalid_plugins.is_empty() {
            info!("Removed invalid plugins:");
            invalid_plugins.iter().for_each(|p| info!("  {}", p));
        }
        if !missing_plugins.is_empty() {
            info!("Added missing plugins:");
            missing_plugins.iter().for_each(|p| info!("  {}", p));
        }

        Ok(())
    }

    pub async fn search_plugins(
        &self,
        query: &str,
        limit: usize,
        fetch_remote: bool,
    ) -> Result<()> {
        let results = self
            .search_registry_plugins(query, limit, fetch_remote)
            .await?;

        if results.is_empty() {
            warn!("No plugins found for query: {}", query);
            return Ok(());
        }

        info!("Search Results:");
        results
            .iter()
            .enumerate()
            .for_each(|(index, result)| self.display_search_result(result, index + 1));
        Ok(())
    }

    pub async fn update(&self, name: Option<&str>, all: bool, reinstall: bool) -> Result<()> {
        if all {
            return self.update_all_plugins().await;
        }
        let package_name = name.context("Plugin name is required unless --all is specified")?;
        self.update_single_plugin(package_name, reinstall)?;
        Ok(())
    }

    async fn update_all_plugins(&self) -> Result<()> {
        let plugins = self.get_installed_plugins(false).await?;
        if plugins.is_empty() {
            warn!("No plugins installed.");
            return Ok(());
        }
        for plugin in plugins {
            self.update_single_plugin(plugin.name.as_str(), false)?;
        }
        Ok(())
    }

    fn update_single_plugin(&self, package_name: &str, reinstall: bool) -> Result<()> {
        if reinstall {
            uv::reinstall(package_name)?;
        } else {
            uv::upgrade(vec![package_name])?;
        }
        info!("Successfully updated plugin: {}", package_name);
        Ok(())
    }

    fn set_registry_plugins(&self, plugins: HashMap<String, RegistryPlugin>) -> Result<()> {
        self.registry_plugins
            .set(plugins)
            .map_err(|_| anyhow::anyhow!("Failed to parse cached plugins info"))
    }

    fn get_registry_plugins(&self) -> Result<&HashMap<String, RegistryPlugin>> {
        self.registry_plugins
            .get()
            .context("Registry plugins not initialized")
    }

    pub async fn fetch_registry_plugins(
        &self,
        fetch_remote: bool,
    ) -> Result<&HashMap<String, RegistryPlugin>> {
        if let Some(plugins) = self.registry_plugins.get() {
            return Ok(plugins);
        }

        let registry_plugins = registry_store::fetch_map_with_cache::<RegistryPlugin, _>(
            "plugins.json",
            fetch_remote,
            |p| p.project_link.clone(),
        )
        .await?;

        self.set_registry_plugins(registry_plugins)?;
        self.get_registry_plugins()
    }

    async fn get_registry_plugin(
        &self,
        package_name: &str,
        fetch_remote: bool,
    ) -> Result<&RegistryPlugin> {
        let plugins = self.fetch_registry_plugins(fetch_remote).await?;
        let plugin = plugins
            .get(package_name)
            .with_context(|| format!("Plugin '{package_name}' not found"))?;
        Ok(plugin)
    }

    async fn search_registry_plugins(
        &self,
        query: &str,
        limit: usize,
        fetch_remote: bool,
    ) -> Result<Vec<&RegistryPlugin>> {
        let plugins_map = self.fetch_registry_plugins(fetch_remote).await?;
        Ok(filter_search(plugins_map, query, limit))
    }

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
            let tags_str: String = plugin
                .tags
                .iter()
                .filter_map(|t| t.get("label"))
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            StyledText::new(" ")
                .text("  Tags:")
                .yellow(tags_str)
                .println();
        }
    }

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
            .yellow(format!("nbr plugin install {}", plugin.project_link))
            .println();
    }
}
