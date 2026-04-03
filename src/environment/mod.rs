use crate::log::StyledText;
use crate::utils::{process_utils, terminal_utils};
use crate::uv::{self, Package};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fmt};
use sysinfo::{Disks, System};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    pub python_info: PythonInfo,
    pub nonebot_info: Option<NoneBotInfo>,
    pub project_info: Option<ProjectInfo>,
    pub system_info: SystemInfo,
    pub env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PythonInfo {
    pub version: String,
    pub executable: String,
    pub virtual_env: Option<String>,
    pub uv_version: Option<String>,
    pub site_packages: Vec<Package>,
}

impl PythonInfo {
    pub(crate) fn show(&self) {
        StyledText::new(" ")
            .text("  version:")
            .cyan(&self.version)
            .println();
        StyledText::new(" ")
            .text("  uv version:")
            .with(|text| {
                if let Some(uv_version) = self.uv_version.as_ref() {
                    text.cyan(uv_version);
                } else {
                    text.red("Not Installed");
                }
            })
            .println();
        StyledText::new(" ")
            .text("  executable:")
            .cyan(&self.executable)
            .println();
        StyledText::new(" ")
            .text("  virtual environment:")
            .with(|text| {
                if let Some(venv) = self.virtual_env.as_ref() {
                    text.cyan(venv);
                } else {
                    text.red("None");
                }
            })
            .println();

        StyledText::new(" ")
            .text("  installed Packages:")
            .cyan(self.site_packages.len().to_string())
            .println();
    }
}

#[derive(Debug, Clone)]
pub struct NoneBotInfo {
    pub version: String,
    pub location: String,
    pub adapters: Vec<Package>,
    pub plugins: Vec<Package>,
}

impl NoneBotInfo {
    pub(crate) fn show(&self) {
        StyledText::new(" ")
            .text("  version:")
            .cyan(&self.version)
            .println();
        StyledText::new(" ")
            .text("  location:")
            .cyan(&self.location)
            .println();

        if !self.adapters.is_empty() {
            StyledText::new("")
                .text(format!("  installed {} adapters:", self.adapters.len()))
                .println();
            for adapter in &self.adapters {
                StyledText::new(" ")
                    .text("    •")
                    .cyan(&adapter.name)
                    .green(format!("(v{})", adapter.version))
                    .println();
            }
        }

        if !self.plugins.is_empty() {
            StyledText::new("")
                .text(format!("  installed {} plugins:", self.plugins.len()))
                .println();
            for plugin in &self.plugins {
                StyledText::new(" ")
                    .text("    •")
                    .cyan(&plugin.name)
                    .green(format!("(v{})", plugin.version))
                    .println();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub root_path: PathBuf,
    pub bot_file: Option<PathBuf>,
    pub plugins_dir: Option<PathBuf>,
    pub is_git_repo: bool,
}

impl ProjectInfo {
    pub(crate) fn show(&self) {
        StyledText::new(" ")
            .text("  name:")
            .cyan(&self.name)
            .println();
        StyledText::new(" ")
            .text("  root path:")
            .cyan(self.root_path.display().to_string())
            .println();

        if let Some(ref bot_file) = self.bot_file {
            StyledText::new(" ")
                .text("  bot file:")
                .cyan(bot_file.display().to_string())
                .println();
        }

        if let Some(ref plugins_dir) = self.plugins_dir {
            StyledText::new(" ")
                .text("  plugins directory:")
                .cyan(plugins_dir.display().to_string())
                .println();
        }

        StyledText::new(" ")
            .text("  git repository:")
            .with(|text| {
                if self.is_git_repo {
                    text.green("Yes");
                } else {
                    text.red("No");
                }
            })
            .println();
    }
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub total_memory: u64,
    pub available_memory: u64,
    pub cpu_count: usize,
    pub cpu_usage: f32,
    pub disk_usage: Vec<DiskUsage>,
}

impl SystemInfo {
    pub(crate) fn show(&self) {
        StyledText::new(" ")
            .text("  cpu:")
            .cyan(format!(
                "{} cores / {:.2}% usage",
                self.cpu_count, self.cpu_usage
            ))
            .println();

        let total_gb = self.total_memory as f64 / 1_073_741_824.0;
        let available_gb = self.available_memory as f64 / 1_073_741_824.0;
        StyledText::new(" ")
            .text("  memory:")
            .cyan(format!(
                "available: {available_gb:.3} GB / total: {total_gb:.3} GB",
            ))
            .println();

        if !self.disk_usage.is_empty() {
            StyledText::new("").text("  disk usage:").println();
            for disk in &self.disk_usage {
                let total_gb = disk.total_space as f64 / 1_073_741_824.0;
                let available_gb = disk.available_space as f64 / 1_073_741_824.0;
                let used_gb = total_gb - available_gb;
                StyledText::new(" ")
                    .text("    •")
                    .cyan(format!("{:.2}% used", disk.usage_percentage))
                    .cyan(format!("({used_gb:.2} / {total_gb:.2} GB)"))
                    .cyan(format!("at {}", disk.mount_point))
                    .println();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub usage_percentage: f32,
}

pub enum Issue {
    PythonVersionTooLow,
    NoneBotNotInstalled,
    VirtualEnvNotActivated,
    NoVirtualEnvironmentDetected,
    UvNotInstalled,
    GitNotInstalled,
    GitRepoNotInitialized,
    PluginsDirNotConfigured,
    LowSystemMemory,
    LowDiskSpace,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PythonVersionTooLow => write!(f, "Python version too low (< 3.10)"),
            Self::NoneBotNotInstalled => write!(f, "NoneBot is not installed"),
            Self::VirtualEnvNotActivated => write!(f, "Virtual environment is not activated"),
            Self::NoVirtualEnvironmentDetected => write!(f, "No virtual environment detected"),
            Self::UvNotInstalled => write!(f, "uv is not installed"),
            Self::GitNotInstalled => write!(f, "Git is not installed"),
            Self::GitRepoNotInitialized => write!(f, "Git repository is not initialized"),
            Self::PluginsDirNotConfigured => write!(f, "Plugins directory is not configured"),
            Self::LowSystemMemory => write!(f, "Low system memory available (< 512 MB)"),
            Self::LowDiskSpace => write!(f, "Low disk space available (< 512 MB)"),
        }
    }
}

impl Issue {
    pub fn show_recommendation(&self) {
        match self {
            Issue::PythonVersionTooLow => {
                StyledText::new("")
                    .text("  • Install Python 3.10 or later from ")
                    .cyan("https://python.org")
                    .println();
            }
            Issue::NoneBotNotInstalled => {
                StyledText::new("")
                    .text("  • Install NoneBot2: ")
                    .cyan("uv add nonebot2[fastapi]")
                    .println();
            }
            Issue::UvNotInstalled => {
                StyledText::new("")
                    .text("  • Install uv from ")
                    .cyan("https://astral.sh/blog/uv")
                    .println();
            }
            Issue::NoVirtualEnvironmentDetected => {
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
            }
            Issue::LowSystemMemory => {
                StyledText::new("")
                    .text("  • Close unnecessary applications to free up memory")
                    .println();
            }
            Issue::LowDiskSpace => {
                StyledText::new("")
                    .text("  • Free up disk space by removing unnecessary files")
                    .println();
            }
            Issue::PluginsDirNotConfigured => {
                StyledText::new("")
                    .text("  • Configure plugins directory in bot.py: ")
                    .cyan("PLUGINS_DIR = \"plugins\"")
                    .println();
            }
            Issue::VirtualEnvNotActivated => {
                StyledText::new("")
                    .text("  • Activate the virtual environment: ")
                    .cyan("source .venv/bin/activate")
                    .text(" (Linux/Mac) or ")
                    .cyan(".venv\\Scripts\\activate")
                    .text(" (Windows)")
                    .println();
            }
            Issue::GitNotInstalled => {
                StyledText::new("")
                    .text("  • Install Git from ")
                    .cyan("https://git-scm.com")
                    .println();
            }
            Issue::GitRepoNotInitialized => {
                StyledText::new("")
                    .text("  • Initialize a Git repository: ")
                    .cyan("git init")
                    .println();
            }
        }
    }
}

pub struct EnvironmentChecker {
    work_dir: PathBuf,
    system: System,
    disks: Disks,
}

impl EnvironmentChecker {
    pub fn new(work_dir: PathBuf) -> Result<Self> {
        let mut system = System::new_all();
        system.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        Ok(Self {
            work_dir,
            system,
            disks,
        })
    }

    pub async fn show_info(&mut self) -> Result<()> {
        let env_info = self.gather_environment_info().await?;
        Self::display_environment_info(&env_info);
        Ok(())
    }

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
            for issue in issues {
                issue.show_recommendation();
            }
        }
        Ok(())
    }

    async fn gather_environment_info(&mut self) -> Result<EnvironmentInfo> {
        let spinner = terminal_utils::create_spinner("Checking environment...");
        self.system.refresh_all();
        let python_info = self.get_python_info().await?;
        let nonebot_info = self.get_nonebot_info(&python_info).await.ok();
        let project_info = self.get_project_info();
        let system_info = self.get_system_info();
        let env_vars = Self::get_relevant_env_vars();
        spinner.finish_and_clear();
        Ok(EnvironmentInfo {
            python_info,
            nonebot_info,
            project_info,
            system_info,
            env_vars,
        })
    }

    async fn get_python_info(&self) -> Result<PythonInfo> {
        let executable = find_python_executable(&self.work_dir)?;
        let version = process_utils::get_python_version(&executable)
            .await
            .unwrap_or_else(|_| "Unknown".to_string());
        let virtual_env = self
            .get_virtual_env()
            .map(|path| path.to_string_lossy().into_owned());
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

    async fn get_nonebot_info(&self, python_info: &PythonInfo) -> Result<NoneBotInfo> {
        let package = uv::show_package_info("nonebot2", Some(&self.work_dir)).await?;
        let version = package.version;
        let location = package.location.unwrap_or("Unknown".to_string());
        let adapters = Self::get_installed_adapters(&python_info.site_packages);
        let plugins = Self::get_installed_plugins(&python_info.site_packages);
        Ok(NoneBotInfo {
            version,
            location,
            adapters,
            plugins,
        })
    }

    fn get_installed_adapters(packages: &[Package]) -> Vec<Package> {
        packages
            .iter()
            .filter(|p| p.name.starts_with("nonebot-adapter-"))
            .cloned()
            .collect()
    }

    fn get_installed_plugins(packages: &[Package]) -> Vec<Package> {
        packages
            .iter()
            .filter(|p| p.name.starts_with("nonebot") && p.name.contains("plugin"))
            .cloned()
            .collect()
    }

    fn get_project_info(&self) -> Option<ProjectInfo> {
        let name = self
            .work_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();
        let root_path = self.work_dir.clone();
        let bot_path = self.work_dir.join("bot.py");
        let bot_file = if bot_path.exists() {
            Some(bot_path)
        } else {
            None
        };
        let plugins_path = self.work_dir.join("src").join("plugins");
        let plugins_dir = if plugins_path.exists() && plugins_path.is_dir() {
            Some(plugins_path)
        } else {
            None
        };
        let is_git_repo = self.work_dir.join(".git").exists();

        Some(ProjectInfo {
            name,
            root_path,
            bot_file,
            plugins_dir,
            is_git_repo,
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

    fn get_system_info(&self) -> SystemInfo {
        let total_memory = self.system.total_memory();
        let available_memory = self.system.available_memory();
        let cpu_count = self.system.cpus().len();
        let cpu_usage = self.system.global_cpu_usage();
        let disk_usage = self
            .disks
            .iter()
            .map(|disk| {
                let mount_point = disk.mount_point().to_string_lossy().to_string();
                let available_space = disk.available_space();
                let total_space = disk.total_space();
                let usage_percentage =
                    (total_space - available_space) as f32 / total_space as f32 * 100.0;
                DiskUsage {
                    mount_point,
                    total_space,
                    available_space,
                    usage_percentage,
                }
            })
            .collect();
        SystemInfo {
            total_memory,
            available_memory,
            cpu_count,
            cpu_usage,
            disk_usage,
        }
    }

    fn get_relevant_env_vars() -> HashMap<String, String> {
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
                env_vars.insert(var_name.to_string(), value);
            }
        }
        env_vars
    }

    fn display_environment_info(env_info: &EnvironmentInfo) {
        info!("Python Environment:");
        env_info.python_info.show();
        info!("\nNoneBot:");
        if let Some(ref nonebot) = env_info.nonebot_info {
            nonebot.show();
        } else {
            StyledText::new(" ")
                .text("  status:")
                .red("Not installed")
                .println();
        }
        info!("\nProject:");
        if let Some(ref project) = env_info.project_info {
            project.show();
        } else {
            StyledText::new(" ")
                .text("  status:")
                .red("No NoneBot project detected")
                .println();
        }
        info!("\nSystem Resources:");
        env_info.system_info.show();
        if !env_info.env_vars.is_empty() {
            info!("\nEnvironment Variables:");
            for (key, value) in &env_info.env_vars {
                StyledText::new(" ")
                    .text(format!(" • {}:", key))
                    .cyan(value)
                    .println();
            }
        }
    }

    fn check_for_issues(&self, env_info: &EnvironmentInfo) -> Vec<Issue> {
        let mut issues = Vec::new();
        if !env_info.python_info.version.contains("3.") {
            issues.push(Issue::PythonVersionTooLow);
        }
        if env_info.nonebot_info.is_none() {
            issues.push(Issue::NoneBotNotInstalled);
        }
        if env_info.python_info.uv_version.is_none() {
            issues.push(Issue::UvNotInstalled);
        }
        if env_info.python_info.virtual_env.is_none() {
            issues.push(Issue::NoVirtualEnvironmentDetected);
        }
        let available_gb = env_info.system_info.available_memory as f64 / 1_073_741_824.0;
        if available_gb < 0.5 {
            issues.push(Issue::LowSystemMemory);
        }
        for disk in &env_info.system_info.disk_usage {
            if disk.usage_percentage > 95.0 {
                issues.push(Issue::LowDiskSpace);
            }
        }
        issues
    }
}

pub fn find_python_executable(work_dir: &Path) -> Result<String> {
    #[cfg(target_os = "windows")]
    let python_executable = work_dir.join(".venv").join("Scripts").join("python.exe");
    #[cfg(not(target_os = "windows"))]
    let python_executable = work_dir.join(".venv").join("bin").join("python");

    if python_executable.exists() {
        return Ok(python_executable.to_string_lossy().to_string());
    }
    process_utils::find_python().context(
        "Python executable not found. Please use `uv python install 3.1x` to install Python",
    )
}
