//! Environment command handler for nbr
//!
//! This module handles environment management including showing system information,
//! checking dependencies, and validating the current project setup.

use crate::EnvCommands;
use crate::error::{NbrError, Result};
use crate::log::StyledText;
use crate::utils::{process_utils, terminal_utils};
use crate::uv::{self, Package};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use sysinfo::System;
use tracing::{info, warn};

/// Environment information structure
#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    /// Operating system information
    // pub os_info: OsInfo,
    /// Python environment information
    pub python_info: PythonInfo,
    /// NoneBot information
    pub nonebot_info: Option<NoneBotInfo>,
    /// Project information
    pub project_info: Option<ProjectInfo>,
    /// System resources
    pub system_info: SystemInfo,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
}

/// Python environment information
#[derive(Debug, Clone)]
pub struct PythonInfo {
    pub version: String,
    pub executable: String,
    pub virtual_env: Option<String>,
    pub uv_version: Option<String>,
    pub site_packages: Vec<Package>,
}

/// NoneBot information
#[derive(Debug, Clone)]
pub struct NoneBotInfo {
    pub version: String,
    pub location: String,
    pub adapters: Vec<Package>,
    pub plugins: Vec<Package>,
}

/// Adapter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub name: String,
    pub version: String,
    pub location: String,
    pub package_name: String,
    pub module_name: String,
}

/// Project information
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub root_path: PathBuf,
    pub bot_file: Option<PathBuf>,
    pub plugins_dir: Option<PathBuf>,
    pub is_git_repo: bool,
    pub virtual_env: Option<PathBuf>,
}

/// System information
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub total_memory: u64,
    pub available_memory: u64,
    pub cpu_count: usize,
    pub cpu_usage: f32,
    pub disk_usage: Vec<DiskUsage>,
}

/// Disk usage information
#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub usage_percentage: f32,
}

/// Environment checker
pub struct EnvironmentChecker {
    /// Working directory
    work_dir: PathBuf,
    /// System information
    system: System,
}

impl EnvironmentChecker {
    /// Create a new environment checker
    pub async fn new() -> Result<Self> {
        //let config_manager = ConfigManager::new()?;
        let work_dir = std::env::current_dir()?;
        let mut system = System::new_all();
        system.refresh_all();

        Ok(Self { work_dir, system })
    }

    /// Show environment information
    pub async fn show_info(&mut self) -> Result<()> {
        let env_info = self.gather_environment_info().await?;
        self.display_environment_info(&env_info);
        Ok(())
    }

    /// Check environment dependencies
    pub async fn check_environment(&mut self) -> Result<()> {
        let env_info = self.gather_environment_info().await?;

        let issues = self.check_for_issues(&env_info);

        if issues.is_empty() {
            info!("✓ Environment is healthy!, you can run `nbr run` to start your bot");
        } else {
            warn!("Environment issues detected:\n");

            for (i, issue) in issues.iter().enumerate() {
                StyledText::new("")
                    .red(format!("  {}.{}", i + 1, issue).as_str())
                    .println();
            }

            info!("\nRecommendations:");
            self.show_recommendations(&issues);
        }

        Ok(())
    }

    /// Gather comprehensive environment information
    async fn gather_environment_info(&mut self) -> Result<EnvironmentInfo> {
        let spinner = terminal_utils::create_spinner("Checking environment...");
        self.system.refresh_all();
        let python_info = self.get_python_info().await?;
        let nonebot_info = self.get_nonebot_info(&python_info).await.ok();
        let project_info = self.get_project_info();
        let system_info = self.get_system_info();
        let env_vars = self.get_relevant_env_vars();
        spinner.finish_and_clear();
        Ok(EnvironmentInfo {
            python_info,
            nonebot_info,
            project_info,
            system_info,
            env_vars,
        })
    }

    /// Get Python environment information
    async fn get_python_info(&self) -> Result<PythonInfo> {
        let executable = process_utils::find_python()
            .ok_or_else(|| NbrError::not_found("Python executable not found"))?;

        let version = process_utils::get_python_version(&executable)
            .await
            .unwrap_or_else(|_| "Unknown".to_string());

        let virtual_env = self
            .get_virtual_env()
            .map(|path| path.to_string_lossy().to_string());

        let uv_version = uv::self_version().await.ok().map(|v| v.trim().to_string());
        let site_packages = uv::list(false).await.unwrap_or_default();

        Ok(PythonInfo {
            version,
            executable,
            virtual_env,
            uv_version,
            site_packages,
        })
    }

    /// Get NoneBot information
    async fn get_nonebot_info(&self, python_info: &PythonInfo) -> Result<NoneBotInfo> {
        let package = uv::show_package_info("nonebot2").await?;
        // Check if NoneBot is installed
        let version = package.version;
        let location = package.location.unwrap_or("Unknown".to_string());

        let adapters = self.get_installed_adapters(&python_info.site_packages);
        let plugins = self.get_installed_plugins(&python_info.site_packages);

        Ok(NoneBotInfo {
            version,
            location,
            adapters,
            plugins,
        })
    }

    /// Get installed adapters
    fn get_installed_adapters(&self, packages: &[Package]) -> Vec<Package> {
        packages
            .iter()
            .filter(|p| p.name.starts_with("nonebot-adapter-"))
            .cloned()
            .collect()
    }

    /// Get installed plugins
    fn get_installed_plugins(&self, packages: &[Package]) -> Vec<Package> {
        packages
            .iter()
            .filter(|p| p.name.starts_with("nonebot") && p.name.contains("plugin"))
            .cloned()
            .collect()
    }

    /// Get project information
    fn get_project_info(&self) -> Option<ProjectInfo> {
        let mut bot_file = None;
        let mut plugins_dir = None;

        // Check for bot files
        let bot_path = self.work_dir.join("bot.py");
        if bot_path.exists() {
            bot_file = Some(bot_path);
        }

        // Check for plugins directory
        let plugins_path = self.work_dir.join("src").join("plugins");
        if plugins_path.exists() && plugins_path.is_dir() {
            plugins_dir = Some(plugins_path);
        }

        // Check if it's a git repository
        let is_git_repo = self.work_dir.join(".git").exists();

        // Check for virtual environment
        let virtual_env = self.get_virtual_env();

        let project_name = self
            .work_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        Some(ProjectInfo {
            name: project_name,
            root_path: self.work_dir.clone(),
            bot_file,
            plugins_dir,
            is_git_repo,
            virtual_env,
        })
    }

    fn get_virtual_env(&self) -> Option<PathBuf> {
        let venv_path = self.work_dir.join(".venv");
        if venv_path.exists() && venv_path.is_dir() {
            Some(venv_path)
        } else {
            None
        }
    }
    /// Get system information
    fn get_system_info(&self) -> SystemInfo {
        let total_memory = self.system.total_memory();
        let available_memory = self.system.available_memory();
        let cpu_count = self.system.cpus().len();
        let cpu_usage = self.system.global_cpu_usage();
        let disk_usage = vec![];

        SystemInfo {
            total_memory,
            available_memory,
            cpu_count,
            cpu_usage,
            disk_usage,
        }
    }

    /// Get relevant environment variables
    fn get_relevant_env_vars(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        let relevant_vars = [
            "PYTHONPATH",
            "VIRTUAL_ENV",
            "ENVIRONMENT",
            "LOG_LEVEL",
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "http_proxy",
            "https_proxy",
        ];

        for var_name in &relevant_vars {
            if let Ok(value) = env::var(var_name) {
                // Hide sensitive values
                let display_value = if var_name.to_lowercase().contains("token")
                    || var_name.to_lowercase().contains("secret")
                    || var_name.to_lowercase().contains("password")
                {
                    "*".repeat(value.len().min(8))
                } else {
                    value
                };
                env_vars.insert(var_name.to_string(), display_value);
            }
        }

        env_vars
    }

    /// Display environment information
    fn display_environment_info(&self, env_info: &EnvironmentInfo) {
        // Operating System

        // Python Environment
        info!("Python Environment:");
        StyledText::new(" ")
            .text("  version:")
            .cyan(&env_info.python_info.version)
            .println();
        StyledText::new(" ")
            .text("  uv version:")
            .cyan(
                env_info
                    .python_info
                    .uv_version
                    .as_ref()
                    .unwrap_or(&"not installed".to_string()),
            )
            .println();
        StyledText::new(" ")
            .text("  executable:")
            .cyan(&env_info.python_info.executable)
            .println();
        StyledText::new(" ")
            .text("  virtual environment:")
            .with(|text| {
                if let Some(venv) = env_info.python_info.virtual_env.as_ref() {
                    text.cyan(venv);
                } else {
                    text.red("None");
                }
            })
            .println();

        StyledText::new(" ")
            .text("  installed Packages:")
            .cyan(&env_info.python_info.site_packages.len().to_string())
            .println();

        println!();

        // NoneBot Information
        if let Some(ref nonebot) = env_info.nonebot_info {
            info!("NoneBot:");
            StyledText::new(" ")
                .text("  version:")
                .cyan(&nonebot.version)
                .println();
            StyledText::new(" ")
                .text("  location:")
                .cyan(&nonebot.location)
                .println();

            if !nonebot.adapters.is_empty() {
                StyledText::new("")
                    .text(format!("  installed {} adapters:", nonebot.adapters.len()).as_str())
                    .println();
                for adapter in &nonebot.adapters {
                    StyledText::new(" ")
                        .text("    •")
                        .cyan(&adapter.name)
                        .green(format!("(v{})", adapter.version).as_str())
                        .println();
                }
            }

            if !nonebot.plugins.is_empty() {
                StyledText::new("")
                    .text(format!("  installed {} plugins:", nonebot.plugins.len()).as_str())
                    .println();
                for plugin in &nonebot.plugins {
                    StyledText::new(" ")
                        .text("    •")
                        .cyan(&plugin.name)
                        .green(format!("(v{})", plugin.version).as_str())
                        .println();
                }
            }
        } else {
            StyledText::new("").green_bold("NoneBot:").println();
            StyledText::new(" ")
                .text("  status:")
                .red("Not installed")
                .println();
        }
        println!();

        // Project Information
        if let Some(ref project) = env_info.project_info {
            info!("Project:");
            StyledText::new(" ")
                .text("  name:")
                .cyan(&project.name)
                .println();
            StyledText::new(" ")
                .text("  root path:")
                .cyan(&project.root_path.display().to_string())
                .println();

            if let Some(ref bot_file) = project.bot_file {
                StyledText::new(" ")
                    .text("  bot file:")
                    .cyan(&bot_file.display().to_string())
                    .println();
            }

            if let Some(ref plugins_dir) = project.plugins_dir {
                StyledText::new(" ")
                    .text("  plugins directory:")
                    .cyan(&plugins_dir.display().to_string())
                    .println();
            }

            StyledText::new(" ")
                .text("  git repository:")
                .with(|text| {
                    if project.is_git_repo {
                        text.green("Yes");
                    } else {
                        text.red("No");
                    }
                })
                .println();

            if let Some(ref venv) = project.virtual_env {
                StyledText::new(" ")
                    .text("  virtual environment:")
                    .cyan(&venv.display().to_string())
                    .println();
            }
        } else {
            info!("Project:");
            StyledText::new(" ")
                .text("  status:")
                .red("No NoneBot project detected")
                .println();
        }
        println!();

        // System Resources
        info!("System Resources:");
        StyledText::new(" ")
            .text("  cpu:")
            .cyan(
                format!(
                    "num {} / {:.2}% usage",
                    env_info.system_info.cpu_count, env_info.system_info.cpu_usage
                )
                .as_str(),
            )
            .println();

        let total_gb = env_info.system_info.total_memory as f64 / 1_073_741_824.0;
        let available_gb = env_info.system_info.available_memory as f64 / 1_073_741_824.0;
        StyledText::new(" ")
            .text("  memory:")
            .cyan(
                format!(
                    "available: {:.3} GB / total: {:.3} GB",
                    available_gb, total_gb
                )
                .as_str(),
            )
            .println();

        if !env_info.system_info.disk_usage.is_empty() {
            StyledText::new("").text("  disk usage:").println();
            for disk in &env_info.system_info.disk_usage {
                let total_gb = disk.total_space as f64 / 1_073_741_824.0;
                let available_gb = disk.available_space as f64 / 1_073_741_824.0;
                StyledText::new(" ")
                    .text("    •")
                    .cyan(format!("{}% used", disk.usage_percentage).as_str())
                    .cyan(format!("({:.2} / {:.2} GB)", total_gb - available_gb, total_gb).as_str())
                    .cyan(format!(" GB) at {}", disk.mount_point).as_str())
                    .println();
            }
        }
        println!();

        // Environment Variables
        if !env_info.env_vars.is_empty() {
            info!("Environment Variables:");
            for (key, value) in &env_info.env_vars {
                StyledText::new(" ")
                    .text(&format!(" • {}:", key))
                    .cyan(value)
                    .println();
            }
        }
    }

    /// Check for environment issues
    fn check_for_issues(&self, env_info: &EnvironmentInfo) -> Vec<String> {
        let mut issues = Vec::new();

        // Check Python version
        if !env_info.python_info.version.contains("3.") {
            issues.push("Python 3.10+ is required for NoneBot2".to_string());
        } else {
            // Extract version number for more detailed check
            if let Some(version_str) = env_info.python_info.version.split_whitespace().nth(1)
                && let Some(version_parts) = version_str.split('.').collect::<Vec<_>>().get(0..2)
                && let (Ok(major), Ok(minor)) = (
                    version_parts[0].parse::<u32>(),
                    version_parts[1].parse::<u32>(),
                )
                && (major < 3 || (major == 3 && minor < 8))
            {
                issues.push("Python 3.10+ is recommended for NoneBot2".to_string());
            }
        }

        // Check if NoneBot is installed
        if env_info.nonebot_info.is_none() {
            issues.push("NoneBot2 is not installed".to_string());
        }

        // Check if uv is available
        if env_info.python_info.uv_version.is_none() {
            issues.push("uv is not available".to_string());
        }

        // Check virtual environment
        if env_info.python_info.virtual_env.is_none() {
            issues.push(
                "No virtual environment detected (recommended for project isolation)".to_string(),
            );
        }

        // Check system resources
        let available_gb = env_info.system_info.available_memory as f64 / 1_073_741_824.0;
        if available_gb < 0.5 {
            issues.push("Low system memory available (< 512 MB)".to_string());
        }

        // Check disk space
        for disk in &env_info.system_info.disk_usage {
            if disk.usage_percentage > 95.0 {
                issues.push(format!(
                    "Disk space critically low on {} ({:.1}% used)",
                    disk.mount_point, disk.usage_percentage
                ));
            }
        }

        issues
    }

    /// Show recommendations based on issues
    fn show_recommendations(&self, issues: &[String]) {
        for issue in issues {
            if issue.contains("Python 3.10+") {
                StyledText::new("")
                    .text("  • Install Python 3.10 or later from ")
                    .cyan("https://python.org")
                    .println();
            } else if issue.contains("NoneBot2 is not installed") {
                StyledText::new("")
                    .text("  • Install NoneBot2: ")
                    .cyan("uv add nonebot2[fastapi]")
                    .println();
            } else if issue.contains("uv is not available") {
                StyledText::new("")
                    .text("  • Install uv from ")
                    .cyan("https://astral.sh/blog/uv")
                    .println();
            } else if issue.contains("virtual environment") {
                StyledText::new("")
                    .text("  • Create a virtual environment: ")
                    .cyan("uv venv")
                    .println();
                StyledText::new("")
                    .text("  • Activate it: ")
                    .cyan("source .venv/bin/activate")
                    .text(" (Linux/Mac) or ")
                    .cyan(".venv\\Scripts\\activate")
                    .text(" (Windows)")
                    .println();
            } else if issue.contains("memory") {
                println!("  • Close unnecessary applications to free up memory");
                println!("  • Consider upgrading system RAM");
            } else if issue.contains("Disk space") {
                println!("  • Free up disk space by removing unnecessary files");
                println!("  • Consider moving the project to a drive with more space");
            } else if issue.contains("bot entry file") {
                StyledText::new("")
                    .text("  • Create a bot entry file: ")
                    .cyan("nb generate bot.py")
                    .println();
            } else if issue.contains(".env") {
                StyledText::new("")
                    .text("  • Create environment file: ")
                    .cyan("cp .env.example .env")
                    .println();
                StyledText::new("")
                    .text("  • Or create a new project: ")
                    .cyan("nb create")
                    .println();
            }
        }
    }
}

/// Handle the env command
pub async fn handle_env(commands: &EnvCommands) -> Result<()> {
    let mut checker = EnvironmentChecker::new().await?;

    match commands {
        EnvCommands::Info => checker.show_info().await,
        EnvCommands::Check => checker.check_environment().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_environment_checker_creation() {
        let checker = EnvironmentChecker::new().await;
        assert!(checker.is_ok());
    }

    #[test]
    fn test_issue_detection() {
        let env_info = EnvironmentInfo {
            // os_info: OsInfo {
            //     name: "Test OS".to_string(),
            //     version: "1.0".to_string(),
            //     architecture: "x64".to_string(),
            //     kernel_version: "1.0.0".to_string(),
            // },
            python_info: PythonInfo {
                version: "Python 3.10.12".to_string(),
                executable: "python".to_string(),
                virtual_env: None,
                uv_version: None,
                site_packages: vec![],
            },
            nonebot_info: None,
            project_info: None,
            system_info: SystemInfo {
                total_memory: 1_073_741_824,   // 1 GB
                available_memory: 104_857_600, // 100 MB
                cpu_count: 4,
                cpu_usage: 50.0,
                disk_usage: vec![],
            },
            env_vars: HashMap::new(),
        };

        let checker = EnvironmentChecker {
            work_dir: PathBuf::new(),
            system: System::new_all(),
        };

        let issues = checker.check_for_issues(&env_info);
        assert!(!issues.is_empty()); // Should have issues with Python 2.7
    }
}
