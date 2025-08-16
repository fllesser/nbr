//! Cache command handler for nbr
//!
//! This module handles cache management including clearing caches,
//! showing cache information, and managing cache policies.
#![allow(dead_code)]

use crate::config::ConfigManager;
use crate::error::{NbrError, Result};
use crate::utils::{fs_utils, terminal_utils};
use clap::ArgMatches;
use colored::*;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Cache types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheType {
    Templates,
    Plugins,
    Adapters,
    Versions,
    Downloads,
    All,
}

impl CacheType {
    /// Get cache directory name
    pub fn dir_name(&self) -> &'static str {
        match self {
            CacheType::Templates => "templates",
            CacheType::Plugins => "plugins",
            CacheType::Adapters => "adapters",
            CacheType::Versions => "versions",
            CacheType::Downloads => "downloads",
            CacheType::All => "",
        }
    }

    /// Get cache description
    pub fn description(&self) -> &'static str {
        match self {
            CacheType::Templates => "Project templates and template registry",
            CacheType::Plugins => "Plugin registry and plugin information",
            CacheType::Adapters => "Adapter registry and adapter information",
            CacheType::Versions => "Package version information and checks",
            CacheType::Downloads => "Downloaded files and archives",
            CacheType::All => "All cached data",
        }
    }
}

/// Cache entry information
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Entry path
    pub path: PathBuf,
    /// Entry size in bytes
    pub size: u64,
    /// Last modified time
    pub modified: SystemTime,
    /// Cache type
    pub cache_type: CacheType,
    /// Entry name/identifier
    pub name: String,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total cache size in bytes
    pub total_size: u64,
    /// Number of cache entries
    pub entry_count: usize,
    /// Cache entries by type
    pub entries_by_type: HashMap<CacheType, Vec<CacheEntry>>,
    /// Oldest entry
    pub oldest_entry: Option<CacheEntry>,
    /// Largest entry
    pub largest_entry: Option<CacheEntry>,
}

/// Cache manager
pub struct CacheManager {
    /// Configuration manager
    config_manager: ConfigManager,
    /// Cache root directory
    cache_dir: PathBuf,
}

impl CacheManager {
    /// Create a new cache manager
    pub async fn new() -> Result<Self> {
        let config_manager = ConfigManager::new()?;

        let cache_dir = config_manager.cache_dir().to_path_buf();
        fs_utils::ensure_dir(&cache_dir)?;

        Ok(Self {
            config_manager,
            cache_dir,
        })
    }

    /// Show cache information
    pub async fn show_info(&self) -> Result<()> {
        println!("{}", "Cache Information".bright_cyan().bold());
        println!();

        let spinner = terminal_utils::create_spinner("Analyzing cache...");
        let stats = self.gather_cache_stats().await?;
        spinner.finish_and_clear();

        self.display_cache_info(&stats);
        Ok(())
    }

    /// Clear cache
    pub async fn clear_cache(&self, cache_types: Vec<CacheType>, force: bool) -> Result<()> {
        if cache_types.is_empty() {
            return Err(NbrError::invalid_argument("No cache types specified"));
        }

        let stats = self.gather_cache_stats().await?;

        // Calculate what will be cleared
        let mut total_size = 0u64;
        let mut total_entries = 0usize;

        for cache_type in &cache_types {
            if let Some(entries) = stats.entries_by_type.get(cache_type) {
                total_size += entries.iter().map(|e| e.size).sum::<u64>();
                total_entries += entries.len();
            }
        }

        if total_entries == 0 {
            println!("{}", "No cache entries found to clear.".bright_yellow());
            return Ok(());
        }

        // Show what will be cleared
        println!("{}", "Cache Clearing Summary:".bright_blue().bold());
        for cache_type in &cache_types {
            if let Some(entries) = stats.entries_by_type.get(cache_type)
                && !entries.is_empty()
            {
                let type_size: u64 = entries.iter().map(|e| e.size).sum();
                println!(
                    "  {} {} entries ({})",
                    "•".bright_blue(),
                    format!("{}: {}", cache_type.description(), entries.len()).bright_white(),
                    fs_utils::format_file_size(type_size).bright_yellow()
                );
            }
        }

        println!();
        println!(
            "{} {} entries, {}",
            "Total:".bright_black(),
            total_entries.to_string().bright_white(),
            fs_utils::format_file_size(total_size).bright_yellow()
        );

        // Confirm clearing
        if !force {
            println!();
            if !Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Are you sure you want to clear these cache entries")
                .default(false)
                .interact()
                .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
            {
                info!("Cache clearing cancelled by user");
                return Ok(());
            }
        }

        // Clear cache entries
        let pb =
            terminal_utils::create_progress_bar(total_entries as u64, "Clearing cache entries...");

        let mut cleared_entries = 0usize;
        let mut cleared_size = 0u64;

        for cache_type in &cache_types {
            if let Some(entries) = stats.entries_by_type.get(cache_type) {
                for entry in entries {
                    match self.remove_cache_entry(entry) {
                        Ok(()) => {
                            cleared_entries += 1;
                            cleared_size += entry.size;
                            pb.inc(1);
                        }
                        Err(e) => {
                            warn!("Failed to remove cache entry {}: {}", entry.name, e);
                            pb.inc(1);
                        }
                    }
                }
            }
        }

        pb.finish_and_clear();

        println!(
            "{} Cleared {} cache entries ({})",
            "✓".bright_green(),
            cleared_entries.to_string().bright_white(),
            fs_utils::format_file_size(cleared_size).bright_yellow()
        );

        Ok(())
    }

    /// Cleanup old cache entries based on policy
    pub async fn cleanup_cache(&self) -> Result<()> {
        let config = self.config_manager.config();
        let cache_config = &config.cache;

        if !cache_config.enabled {
            println!("{}", "Cache is disabled.".bright_yellow());
            return Ok(());
        }

        println!("{}", "Cleaning up cache...".bright_blue());

        let stats = self.gather_cache_stats().await?;
        let mut entries_to_remove = Vec::new();

        // Apply cleanup policy
        match cache_config.cleanup_policy {
            crate::config::CacheCleanupPolicy::Age => {
                entries_to_remove.extend(self.find_old_entries(&stats, &cache_config.ttl)?);
            }
            crate::config::CacheCleanupPolicy::Size => {
                entries_to_remove
                    .extend(self.find_oversized_entries(&stats, cache_config.max_size_mb)?);
            }
            crate::config::CacheCleanupPolicy::Both => {
                entries_to_remove.extend(self.find_old_entries(&stats, &cache_config.ttl)?);
                entries_to_remove
                    .extend(self.find_oversized_entries(&stats, cache_config.max_size_mb)?);
                entries_to_remove.dedup_by(|a, b| a.path == b.path);
            }
        }

        if entries_to_remove.is_empty() {
            println!("{}", "No cache entries need cleanup.".bright_green());
            return Ok(());
        }

        let total_size: u64 = entries_to_remove.iter().map(|e| e.size).sum();
        println!(
            "Removing {} old/oversized cache entries ({})",
            entries_to_remove.len().to_string().bright_white(),
            fs_utils::format_file_size(total_size).bright_yellow()
        );

        let pb = terminal_utils::create_progress_bar(
            entries_to_remove.len() as u64,
            "Cleaning up cache...",
        );

        let mut removed_count = 0usize;
        let mut removed_size = 0u64;

        for entry in &entries_to_remove {
            match self.remove_cache_entry(entry) {
                Ok(()) => {
                    removed_count += 1;
                    removed_size += entry.size;
                }
                Err(e) => {
                    warn!("Failed to remove cache entry {}: {}", entry.name, e);
                }
            }
            pb.inc(1);
        }

        pb.finish_and_clear();

        println!(
            "{} Cleaned up {} cache entries ({})",
            "✓".bright_green(),
            removed_count.to_string().bright_white(),
            fs_utils::format_file_size(removed_size).bright_yellow()
        );

        Ok(())
    }

    /// Gather comprehensive cache statistics
    async fn gather_cache_stats(&self) -> Result<CacheStats> {
        let mut entries_by_type: HashMap<CacheType, Vec<CacheEntry>> = HashMap::new();
        let mut total_size = 0u64;
        let mut entry_count = 0usize;
        let mut oldest_entry: Option<CacheEntry> = None;
        let mut largest_entry: Option<CacheEntry> = None;

        let cache_types = [
            CacheType::Templates,
            CacheType::Plugins,
            CacheType::Adapters,
            CacheType::Versions,
            CacheType::Downloads,
        ];

        for cache_type in &cache_types {
            let type_dir = self.cache_dir.join(cache_type.dir_name());
            if !type_dir.exists() {
                entries_by_type.insert(cache_type.clone(), Vec::new());
                continue;
            }

            let entries = self.scan_cache_directory(&type_dir, cache_type.clone())?;

            for entry in &entries {
                total_size += entry.size;
                entry_count += 1;

                // Track oldest entry
                if oldest_entry.is_none()
                    || entry.modified < oldest_entry.as_ref().unwrap().modified
                {
                    oldest_entry = Some(entry.clone());
                }

                // Track largest entry
                if largest_entry.is_none() || entry.size > largest_entry.as_ref().unwrap().size {
                    largest_entry = Some(entry.clone());
                }
            }

            entries_by_type.insert(cache_type.clone(), entries);
        }

        Ok(CacheStats {
            total_size,
            entry_count,
            entries_by_type,
            oldest_entry,
            largest_entry,
        })
    }

    /// Scan cache directory for entries
    fn scan_cache_directory(&self, dir: &Path, cache_type: CacheType) -> Result<Vec<CacheEntry>> {
        let mut entries = Vec::new();

        if !dir.exists() {
            return Ok(entries);
        }

        fn scan_recursive(
            dir: &Path,
            cache_type: &CacheType,
            entries: &mut Vec<CacheEntry>,
        ) -> Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let metadata = entry.metadata()?;

                if path.is_dir() {
                    scan_recursive(&path, cache_type, entries)?;
                } else if path.is_file() {
                    let size = metadata.len();
                    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);

                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    entries.push(CacheEntry {
                        path: path.clone(),
                        size,
                        modified,
                        cache_type: cache_type.clone(),
                        name,
                    });
                }
            }
            Ok(())
        }

        scan_recursive(dir, &cache_type, &mut entries)?;
        Ok(entries)
    }

    /// Find entries that are too old based on TTL configuration
    fn find_old_entries(
        &self,
        stats: &CacheStats,
        ttl_config: &crate::config::CacheTtlConfig,
    ) -> Result<Vec<CacheEntry>> {
        let mut old_entries = Vec::new();
        let now = SystemTime::now();

        for (cache_type, entries) in &stats.entries_by_type {
            let ttl_seconds = match cache_type {
                CacheType::Templates => ttl_config.templates,
                CacheType::Plugins => ttl_config.plugins,
                CacheType::Adapters => ttl_config.adapters,
                CacheType::Versions => ttl_config.versions,
                CacheType::Downloads => ttl_config.plugins, // Use plugins TTL for downloads
                CacheType::All => continue,
            };

            for entry in entries {
                if let Ok(duration) = now.duration_since(entry.modified)
                    && duration.as_secs() > ttl_seconds
                {
                    old_entries.push(entry.clone());
                }
            }
        }

        Ok(old_entries)
    }

    /// Find entries that exceed size limits (LRU)
    fn find_oversized_entries(
        &self,
        stats: &CacheStats,
        max_size_mb: u64,
    ) -> Result<Vec<CacheEntry>> {
        let max_size_bytes = max_size_mb * 1_048_576; // Convert MB to bytes

        if stats.total_size <= max_size_bytes {
            return Ok(Vec::new());
        }

        let mut all_entries: Vec<CacheEntry> =
            stats.entries_by_type.values().flatten().cloned().collect();

        // Sort by last modified (oldest first)
        all_entries.sort_by_key(|entry| entry.modified);

        let mut entries_to_remove = Vec::new();
        let mut current_size = stats.total_size;

        for entry in &all_entries {
            if current_size <= max_size_bytes {
                break;
            }
            entries_to_remove.push(entry.clone());
            current_size = current_size.saturating_sub(entry.size);
        }

        Ok(entries_to_remove)
    }

    /// Remove a cache entry
    fn remove_cache_entry(&self, entry: &CacheEntry) -> Result<()> {
        if entry.path.is_file() {
            fs::remove_file(&entry.path)
                .map_err(|e| NbrError::io(format!("Failed to remove file: {}", e)))?;
        } else if entry.path.is_dir() {
            fs::remove_dir_all(&entry.path)
                .map_err(|e| NbrError::io(format!("Failed to remove directory: {}", e)))?;
        }

        debug!("Removed cache entry: {}", entry.path.display());
        Ok(())
    }

    /// Display cache information
    fn display_cache_info(&self, stats: &CacheStats) {
        let config = self.config_manager.config();

        println!("{}", "Cache Configuration:".bright_green().bold());
        println!(
            "  {} {}",
            "Enabled:".bright_black(),
            if config.cache.enabled {
                "Yes".bright_green()
            } else {
                "No".bright_red()
            }
        );
        println!(
            "  {} {}",
            "Location:".bright_black(),
            self.cache_dir.display().to_string().bright_cyan()
        );
        println!(
            "  {} {} MB",
            "Size Limit:".bright_black(),
            config.cache.max_size_mb.to_string().bright_white()
        );
        println!(
            "  {} {:?}",
            "Cleanup Policy:".bright_black(),
            format!("{:?}", config.cache.cleanup_policy).bright_white()
        );
        println!();

        println!("{}", "Cache Statistics:".bright_green().bold());
        println!(
            "  {} {}",
            "Total Size:".bright_black(),
            fs_utils::format_file_size(stats.total_size).bright_yellow()
        );
        println!(
            "  {} {}",
            "Total Entries:".bright_black(),
            stats.entry_count.to_string().bright_white()
        );

        if let Some(ref oldest) = stats.oldest_entry {
            let age = SystemTime::now()
                .duration_since(oldest.modified)
                .map(|d| format!("{} days", d.as_secs() / 86400))
                .unwrap_or_else(|_| "unknown".to_string());
            println!(
                "  {} {} ({})",
                "Oldest Entry:".bright_black(),
                oldest.name.bright_white(),
                age.bright_black()
            );
        }

        if let Some(ref largest) = stats.largest_entry {
            println!(
                "  {} {} ({})",
                "Largest Entry:".bright_black(),
                largest.name.bright_white(),
                fs_utils::format_file_size(largest.size).bright_yellow()
            );
        }
        println!();

        println!("{}", "Cache by Type:".bright_green().bold());
        for cache_type in &[
            CacheType::Templates,
            CacheType::Plugins,
            CacheType::Adapters,
            CacheType::Versions,
            CacheType::Downloads,
        ] {
            if let Some(entries) = stats.entries_by_type.get(cache_type) {
                let type_size: u64 = entries.iter().map(|e| e.size).sum();
                let size_str = if type_size > 0 {
                    fs_utils::format_file_size(type_size)
                } else {
                    "0 B".to_string()
                };

                println!(
                    "  {} {} entries ({})",
                    "•".bright_blue(),
                    format!("{}: {}", cache_type.description(), entries.len()).bright_white(),
                    size_str.bright_yellow()
                );

                if !entries.is_empty() && entries.len() <= 5 {
                    // Show individual entries for small lists
                    for entry in entries.iter().take(5) {
                        let age = SystemTime::now()
                            .duration_since(entry.modified)
                            .map(|d| {
                                if d.as_secs() < 3600 {
                                    format!("{}m", d.as_secs() / 60)
                                } else if d.as_secs() < 86400 {
                                    format!("{}h", d.as_secs() / 3600)
                                } else {
                                    format!("{}d", d.as_secs() / 86400)
                                }
                            })
                            .unwrap_or_else(|_| "?".to_string());

                        println!(
                            "    {} {} ({}, {})",
                            "▪".bright_black(),
                            entry.name.bright_black(),
                            fs_utils::format_file_size(entry.size).bright_black(),
                            age.bright_black()
                        );
                    }
                }
            }
        }

        // Show cache health
        println!();
        let usage_percentage = if config.cache.max_size_mb > 0 {
            (stats.total_size as f64 / (config.cache.max_size_mb * 1_048_576) as f64) * 100.0
        } else {
            0.0
        };

        let health_status = if usage_percentage > 90.0 {
            "Critical".bright_red()
        } else if usage_percentage > 70.0 {
            "High".bright_yellow()
        } else {
            "Good".bright_green()
        };

        println!("{}", "Cache Health:".bright_green().bold());
        println!(
            "  {} {} ({:.1}% of limit)",
            "Usage:".bright_black(),
            health_status,
            usage_percentage
        );

        if usage_percentage > 80.0 {
            println!();
            println!(
                "{} Cache is getting full. Consider running: {}",
                "⚠".bright_yellow(),
                "nb cache clear".bright_cyan()
            );
        }
    }
}

/// Parse cache type from string
fn parse_cache_type(s: &str) -> Option<CacheType> {
    match s.to_lowercase().as_str() {
        "templates" | "template" => Some(CacheType::Templates),
        "plugins" | "plugin" => Some(CacheType::Plugins),
        "adapters" | "adapter" => Some(CacheType::Adapters),
        "versions" | "version" => Some(CacheType::Versions),
        "downloads" | "download" => Some(CacheType::Downloads),
        "all" => Some(CacheType::All),
        _ => None,
    }
}

/// Handle the cache command
pub async fn handle_cache(matches: &ArgMatches) -> Result<()> {
    let cache_manager = CacheManager::new().await?;

    match matches.subcommand() {
        Some(("clear", sub_matches)) => {
            let cache_types = if let Some(types_str) = sub_matches.get_many::<String>("types") {
                let mut types = Vec::new();
                for type_str in types_str {
                    if let Some(cache_type) = parse_cache_type(type_str) {
                        if cache_type == CacheType::All {
                            types = vec![
                                CacheType::Templates,
                                CacheType::Plugins,
                                CacheType::Adapters,
                                CacheType::Versions,
                                CacheType::Downloads,
                            ];
                            break;
                        } else {
                            types.push(cache_type);
                        }
                    } else {
                        return Err(NbrError::invalid_argument(format!(
                            "Unknown cache type: {}",
                            type_str
                        )));
                    }
                }
                types
            } else {
                vec![CacheType::All]
            };

            let force = sub_matches.get_flag("force");
            cache_manager.clear_cache(cache_types, force).await
        }
        Some(("info", _)) => cache_manager.show_info().await,
        Some(("cleanup", _)) => cache_manager.cleanup_cache().await,
        _ => Err(NbrError::invalid_argument("Invalid cache subcommand")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_manager_creation() {
        let manager = CacheManager::new().await;
        assert!(manager.is_ok());
    }

    #[test]
    fn test_cache_type_parsing() {
        assert_eq!(parse_cache_type("templates"), Some(CacheType::Templates));
        assert_eq!(parse_cache_type("plugins"), Some(CacheType::Plugins));
        assert_eq!(parse_cache_type("all"), Some(CacheType::All));
        assert_eq!(parse_cache_type("invalid"), None);
    }

    #[test]
    fn test_cache_type_descriptions() {
        assert!(!CacheType::Templates.description().is_empty());
        assert!(!CacheType::Plugins.description().is_empty());
        assert!(!CacheType::All.description().is_empty());
    }

    #[test]
    fn test_cache_entry_creation() {
        let entry = CacheEntry {
            path: PathBuf::from("/tmp/test"),
            size: 1024,
            modified: UNIX_EPOCH,
            cache_type: CacheType::Templates,
            name: "test".to_string(),
        };

        assert_eq!(entry.size, 1024);
        assert_eq!(entry.cache_type, CacheType::Templates);
        assert_eq!(entry.name, "test");
    }
}
