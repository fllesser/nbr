use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::warn;

use crate::config::get_cache_dir;

pub const REGISTRY_BASE_URL: &str = "https://registry.nonebot.dev";

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
    let map = serde_json::from_slice(
        &std::fs::read(path)
            .with_context(|| format!("Failed to read cached file: {}", path.display()))?,
    )
    .with_context(|| format!("Failed to parse cached file: {}", path.display()))?;
    Ok(Some(map))
}

pub fn save_cached_map<T>(path: &Path, map: &HashMap<String, T>) -> Result<()>
where
    T: Serialize,
{
    std::fs::write(path, serde_json::to_string(map)?)?;
    Ok(())
}

async fn fetch_vec<T>(client: &Client, url: &str) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Network error while fetching registry data from: {}", url))?;
    let items = response
        .json::<Vec<T>>()
        .await
        .with_context(|| format!("Failed to parse registry data from: {}", url))?;
    Ok(items)
}

async fn fetch_vec_by_route<T>(route: &str) -> Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("nbr")
        .build()
        .context("Failed to build HTTP client")?;
    let url = registry_url(route);
    fetch_vec(&client, &url).await
}

pub async fn fetch_map_with_cache<T, F>(
    file_name: &str,
    fetch_remote: bool,
    key_fn: F,
) -> Result<HashMap<String, T>>
where
    T: DeserializeOwned + Serialize + Clone,
    F: Fn(&T) -> String,
{
    // file_name 同时用于缓存文件名和 registry 路由
    let cache_path = cache_file(file_name)?;

    if !fetch_remote && let Some(cached) = load_cached_map(&cache_path)? {
        return Ok(cached);
    }

    let items: Vec<T> = fetch_vec_by_route(file_name).await?;

    let map: HashMap<String, T> = items
        .iter()
        .cloned()
        .map(|item| {
            let key = key_fn(&item);
            (key, item)
        })
        .collect();

    // 忽略缓存写入错误以避免影响主流程
    if let Err(e) = save_cached_map(&cache_path, &map) {
        warn!("Failed to save cached map: {}", e);
    }

    Ok(map)
}
