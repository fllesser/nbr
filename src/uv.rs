use crate::{
    error::{NbrError, Result},
    utils::{process_utils, terminal_utils},
};
use colored::Colorize;
use serde::Deserialize;
use std::path::Path;

pub struct Uv;

const UV_NOT_FOUND_MESSAGE: &str = "uv not found. You can run \n  curl -LsSf https://astral.sh/uv/install.sh | sh \nto install or get more information from https://astral.sh/blog/uv";

#[allow(unused)]
impl Uv {
    pub async fn check_self_installed() -> Result<()> {
        let output = process_utils::execute_command_with_output("uv", &["--version"], None, 5)
            .await
            .map_err(|_| NbrError::environment(UV_NOT_FOUND_MESSAGE))?;
        Ok(())
    }

    pub async fn get_self_version() -> Result<String> {
        let output = process_utils::execute_command_with_output("uv", &["--version"], None, 5)
            .await
            .map_err(|_| NbrError::environment(UV_NOT_FOUND_MESSAGE))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim().to_string())
    }

    pub fn sync(working_dir: Option<&Path>, python_version: Option<&str>) -> Result<()> {
        let mut args = vec!["sync"];
        if let Some(version) = python_version {
            args.push("--python");
            args.push(version);
        }
        process_utils::execute_interactive("uv", &args, working_dir)?;
        Ok(())
    }

    pub fn add(
        packages: Vec<&str>,
        upgrade: bool,
        index_url: Option<&str>,
        working_dir: Option<&Path>,
    ) -> Result<()> {
        let mut args = vec!["add"];

        if upgrade {
            args.push("--upgrade");
        }

        if let Some(index) = index_url {
            args.push("--index-url");
            args.push(index);
        }

        args.extend(packages.clone());

        process_utils::execute_interactive("uv", &args, working_dir)
    }

    pub fn add_from_github(repo_url: &str, working_dir: Option<&Path>) -> Result<()> {
        let git_url = format!("git+{}", repo_url);
        process_utils::execute_interactive("uv", &["add", &git_url], working_dir)
    }

    pub fn reinstall(package: &str, working_dir: Option<&Path>) -> Result<()> {
        Self::remove(vec![package], working_dir)?;
        Self::add(vec![package], false, None, working_dir)
    }

    pub fn remove(packages: Vec<&str>, working_dir: Option<&Path>) -> Result<()> {
        let mut args = vec!["remove"];
        args.extend(packages.clone());
        process_utils::execute_interactive("uv", &args, working_dir)
    }

    pub async fn show(package: &str, working_dir: Option<&Path>) -> Result<String> {
        let output = process_utils::execute_command_with_output(
            "uv",
            &["pip", "show", package],
            working_dir,
            30,
        )
        .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_string())
    }

    /// Get package detailed info
    pub async fn show_package_info(package: &str, working_dir: Option<&Path>) -> Result<Package> {
        let output = process_utils::execute_command_with_output(
            "uv",
            &["pip", "show", package],
            working_dir,
            5,
        )
        .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines();
        let name = lines.next().unwrap().split(" ").last().unwrap().to_owned();
        let version = lines.next().unwrap().split(" ").last().unwrap().to_owned();
        let latest_version = None;
        let location = lines
            .next()
            .unwrap()
            .split(" ")
            .last()
            .map(|s| s.to_owned());
        let requires = lines
            .next()
            .unwrap()
            .split(" ")
            .last()
            .map(|s| s.to_owned().split(",").map(|s| s.to_owned()).collect());
        let requires_by = lines
            .next()
            .unwrap()
            .split(" ")
            .last()
            .map(|s| s.to_owned().split(",").map(|s| s.to_owned()).collect());

        Ok(Package {
            name,
            version,
            latest_version,
            location,
            requires,
            requires_by,
        })
    }

    pub async fn is_installed(package: &str, working_dir: Option<&Path>) -> bool {
        let output = Self::show(package, working_dir).await;
        output.is_ok() && output.unwrap().contains("Version")
    }

    pub async fn list(working_dir: Option<&Path>, outdated: bool) -> Result<Vec<Package>> {
        let mut args: Vec<&str> = vec!["pip", "list", "--format=json"];
        let mut spinner = None;
        if outdated {
            args.push("--outdated");
            spinner = Some(terminal_utils::create_spinner(
                "Checking for outdated packages...",
            ));
        }

        let output =
            process_utils::execute_command_with_output("uv", &args, working_dir, 30).await?;

        if let Some(spinner) = spinner {
            spinner.finish_and_clear();
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let packages: Vec<Package> = serde_json::from_str(&stdout)?;
        Ok(packages)
    }
}

#[derive(Debug, Clone, Deserialize, Eq, Hash)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub latest_version: Option<String>,
    pub location: Option<String>,
    pub requires: Option<Vec<String>>,
    pub requires_by: Option<Vec<String>>,
}

impl PartialEq for Package {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Package {
    pub fn is_outdated(&self) -> bool {
        if let Some(latest_version) = self.latest_version.as_ref() {
            &self.version != latest_version
        } else {
            false
        }
    }

    /// Display package info
    /// name installedeversion (available version)
    pub fn display_info(&self) {
        let installed_version = format!("v{}", self.version).bright_green();
        let available_version = if self.is_outdated() {
            format!("(available: v{})", self.latest_version.as_ref().unwrap())
                .bright_yellow()
                .to_string()
        } else {
            "".to_string()
        };
        println!(
            "  {} {} {}",
            self.name.bright_blue(),
            installed_version,
            available_version
        );
    }
}

#[cfg(test)]
mod tests {

    use std::path::PathBuf;

    use super::*;

    fn working_dir() -> PathBuf {
        std::env::current_dir().unwrap().join("awesome-bot")
    }

    #[tokio::test]
    async fn test_is_installed() {
        let work_dir = working_dir();
        let is_installed = Uv::is_installed("not-exist-package", Some(&work_dir)).await;
        assert!(!is_installed);
        let is_installed = Uv::is_installed("nonebot2", Some(&work_dir)).await;
        assert!(is_installed);
    }

    #[tokio::test]
    async fn test_get_installed_version() {
        let work_dir = working_dir();
        let package = Uv::show_package_info("nonebot2", Some(&work_dir)).await;
        assert!(package.is_ok());
        assert!(dbg!(package).unwrap().version.contains("2."));
        let package = Uv::show_package_info("not-exist-package", Some(&work_dir)).await;
        assert!(package.is_err());
    }

    #[tokio::test]
    async fn test_get_self_version() {
        let result = Uv::get_self_version().await;
        assert!(result.is_ok());
        dbg!(result.unwrap());
    }

    #[tokio::test]
    async fn test_list() {
        let work_dir = working_dir();
        let outdated_package = Uv::list(Some(&work_dir), true).await;
        assert!(outdated_package.is_ok());
        dbg!(outdated_package.unwrap());
        let all_package = Uv::list(Some(&work_dir), false).await;
        assert!(all_package.is_ok());
        dbg!(all_package.unwrap());
    }

    #[test]
    fn test_add() {
        let work_dir = working_dir();
        let result = Uv::add(
            vec!["nonebot-plugin-status", "nonebot-plugin-abs"],
            false,
            None,
            Some(&work_dir),
        );
        assert!(result.is_ok());
        dbg!(result.unwrap());
    }

    #[test]
    fn test_package_display_info() {
        let package = Package {
            name: "nonebot-plugin-status".to_string(),
            version: "0.1.0".to_string(),
            latest_version: Some("0.2.0".to_string()),
            location: None,
            requires: None,
            requires_by: None,
        };
        package.display_info();
    }
}
