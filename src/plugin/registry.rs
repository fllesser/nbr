use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::registry_store;

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

pub async fn fetch_all(client: &reqwest::Client) -> Result<Vec<RegistryPlugin>> {
    let plugins_json_url = registry_store::registry_url("plugins.json");
    registry_store::fetch_registry_vec(client, &plugins_json_url)
        .await
        .context("Failed to fetch plugin info")
}

pub fn filter_search<'a>(
    plugins: &'a HashMap<String, RegistryPlugin>,
    query: &str,
    limit: usize,
) -> Vec<&'a RegistryPlugin> {
    plugins
        .values()
        .filter(|plugin| {
            plugin.project_link.contains(query)
                || plugin.name.contains(query)
                || plugin.desc.contains(query)
                || plugin.author.contains(query)
        })
        .take(limit)
        .collect()
}
