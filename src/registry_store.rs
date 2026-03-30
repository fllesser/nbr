use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::get_cache_dir;

pub const REGISTRY_BASE_URL: &str = "https://registry.nonebot.dev";

pub fn build_registry_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("nbr")
        .build()
        .context("Failed to build HTTP client")
}

pub fn cache_file(file_name: &str) -> Result<PathBuf> {
    let cache_dir = get_cache_dir()?;
    Ok(cache_dir.join(file_name))
}

pub fn registry_url(route: &str) -> String {
    format!(
        "{}/{}",
        REGISTRY_BASE_URL.trim_end_matches('/'),
        route.trim_start_matches('/')
    )
}

pub fn load_cached_map<T>(path: &Path) -> Result<Option<HashMap<String, T>>>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }
    let map: HashMap<String, T> = serde_json::from_slice(&std::fs::read(path)?)?;
    Ok(Some(map))
}

pub fn save_cached_map<T>(path: &Path, map: &HashMap<String, T>) -> Result<()>
where
    T: Serialize,
{
    std::fs::write(path, serde_json::to_string(map)?)?;
    Ok(())
}

pub async fn fetch_registry_vec<T>(client: &Client, url: &str) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let response = client
        .get(url)
        .send()
        .await
        .context("Network error while fetching registry data")?;
    let items = response
        .json::<Vec<T>>()
        .await
        .context("Failed to parse registry data")?;
    Ok(items)
}

pub async fn fetch_registry_vec_by_route<T>(route: &str) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let client = build_registry_client()?;
    let url = registry_url(route);
    fetch_registry_vec(&client, &url).await
}
