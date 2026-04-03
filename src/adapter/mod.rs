use crate::context::GlobalContext;
use crate::project::SelectedAdapter;
use crate::pyproject::{Adapter, PyProjectConfig};
use crate::{registry_store, uv};
use anyhow::{Context, Result};
use dialoguer::MultiSelect;
use dialoguer::theme::ColorfulTheme;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::debug;

mod registry;
mod service;

pub use registry::RegistryAdapter;

pub struct AdapterManager {
    work_dir: PathBuf,
    registry_adapters: OnceLock<HashMap<String, RegistryAdapter>>,
    installed_adapters: OnceLock<Vec<Adapter>>,
    global_context: GlobalContext,
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new(None, GlobalContext::default()).unwrap()
    }
}

impl AdapterManager {
    pub fn new(work_dir: Option<PathBuf>, ctx: GlobalContext) -> Result<Self> {
        let work_dir = work_dir.unwrap_or_else(|| Path::new(".").to_path_buf());

        Ok(Self {
            work_dir,
            registry_adapters: OnceLock::new(),
            installed_adapters: OnceLock::new(),
            global_context: ctx,
        })
    }

    fn set_registry_adapters(&self, adapters: HashMap<String, RegistryAdapter>) -> Result<()> {
        self.registry_adapters
            .set(adapters)
            .map_err(|_| anyhow::anyhow!("Failed to set cached adapters"))
    }

    pub fn get_registry_adapters(&self) -> Result<&HashMap<String, RegistryAdapter>> {
        self.registry_adapters
            .get()
            .context("Registry adapters not initialized")
    }

    pub async fn fetch_registry_adapters(
        &self,
        fetch_remote: bool,
    ) -> Result<&HashMap<String, RegistryAdapter>> {
        if let Some(adapters) = self.registry_adapters.get() {
            return Ok(adapters);
        }

        let registry_adapters = registry_store::fetch_map_with_cache::<RegistryAdapter, _>(
            "adapters.json",
            fetch_remote,
            |a| a.name.clone(),
        )
        .await?;

        self.set_registry_adapters(registry_adapters)?;
        self.get_registry_adapters()
    }

    pub fn parse_installed_adapters(&self) -> Option<&Vec<Adapter>> {
        if let Some(adapters) = self.installed_adapters.get() {
            return Some(adapters);
        }
        let config = PyProjectConfig::parse(Some(&self.work_dir)).ok()?;
        let adapters = config.nonebot()?.adapters.to_owned()?;
        self.installed_adapters.set(adapters).ok()?;
        self.installed_adapters.get()
    }

    pub fn get_installed_adapters_names(&self) -> Vec<&str> {
        self.parse_installed_adapters()
            .map(|adapters| adapters.iter().map(|a| a.name.as_str()).collect())
            .unwrap_or_default()
    }

    pub async fn select_adapters(
        &self,
        fetch_remote: bool,
        filter_installed: bool,
    ) -> Result<Vec<&RegistryAdapter>> {
        let registry_adapters = self.fetch_registry_adapters(fetch_remote).await?;
        let mut adapter_names: Vec<String> = registry_adapters.keys().cloned().collect();

        if filter_installed {
            let installed_adapters = self.get_installed_adapters_names();
            adapter_names.retain(|name| !installed_adapters.contains(&name.as_str()));
        }
        adapter_names.sort();

        let selected_adapters = if !adapter_names.is_empty() {
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Which adapter(s) would you like to use")
                .items(&adapter_names)
                .interact()?;
            selections
                .into_iter()
                .map(|i| adapter_names[i].to_string())
                .collect()
        } else {
            vec!["OneBot V11".to_string()]
        };

        Ok(selected_adapters
            .iter()
            .filter_map(|name| registry_adapters.get(name))
            .collect())
    }

    pub async fn resolve_selected_adapters(
        &self,
        adapter_names: Option<Vec<String>>,
    ) -> Result<Vec<SelectedAdapter>> {
        match adapter_names {
            Some(adapters) => {
                let registry_adapter_map = self.fetch_registry_adapters(false).await?;
                Ok(adapters
                    .into_iter()
                    .filter_map(|name| registry_adapter_map.get(&name))
                    .map(SelectedAdapter::from)
                    .collect())
            }
            None => Ok(self
                .select_adapters(false, false)
                .await?
                .into_iter()
                .map(SelectedAdapter::from)
                .collect()),
        }
    }

    #[allow(dead_code)]
    pub async fn get_installed_adapters_from_venv(&self) -> Result<HashSet<String>> {
        let installed_adapters_set = uv::list(false)
            .await?
            .into_iter()
            .filter(|a| a.name.contains("nonebot-adapter-"))
            .map(|a| a.name)
            .collect::<HashSet<String>>();
        debug!("Installed adapters: {:?}", installed_adapters_set);
        Ok(installed_adapters_set)
    }

    pub fn display_adapter(&self, adapter: &RegistryAdapter) {
        adapter.display();
    }
}
