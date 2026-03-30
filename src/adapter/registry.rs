use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::project::SelectedAdapter;
use crate::pyproject::Adapter;
use crate::registry_store;
use crate::utils::terminal_utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAdapter {
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

impl From<&RegistryAdapter> for Adapter {
    fn from(adapter: &RegistryAdapter) -> Self {
        Self {
            name: adapter.name.clone(),
            module_name: adapter.module_name.clone(),
        }
    }
}

impl From<&RegistryAdapter> for SelectedAdapter {
    fn from(adapter: &RegistryAdapter) -> Self {
        Self {
            module_name: adapter.module_name.clone(),
            name: adapter.name.clone(),
            project_link: adapter.project_link.clone(),
            version: adapter.version.clone(),
        }
    }
}

pub fn cache_file() -> Result<PathBuf> {
    registry_store::cache_file("adapters.json")
}

pub async fn fetch_remote_registry_adapters() -> Result<Vec<RegistryAdapter>> {
    let spinner = terminal_utils::create_spinner("Fetching adapters from registry...");
    let adapters: Vec<RegistryAdapter> =
        registry_store::fetch_registry_vec_by_route("adapters.json")
            .await
            .context("Failed to parse adapter info")?;
    spinner.finish_and_clear();
    Ok(adapters)
}
