use crate::{
    error::{NbrError, Result},
    utils::{process_utils, terminal_utils},
};
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

    pub async fn sync(working_dir: Option<&Path>) -> Result<()> {
        let args = vec!["sync"];
        let spinner = terminal_utils::create_spinner(&"Installing dependencies...".to_string());
        let output = process_utils::execute_command_with_output(
            "uv",
            &args,
            working_dir,
            1800, // 30 minutes timeout
        )
        .await;
        spinner.finish_and_clear();

        output.map(|_| ())
    }

    pub async fn add(
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
        
        let spinner =
            terminal_utils::create_spinner(&format!("Installing {}...", packages.join(", ")));

        let output = process_utils::execute_command_with_output(
            "uv",
            &args,
            working_dir,
            300, // 5 minutes timeout
        )
        .await;

        spinner.finish_and_clear();

        output.map(|_| ())
    }

    pub async fn add_from_github(repo_url: &str, working_dir: Option<&Path>) -> Result<()> {
        let git_url = format!("git+{}", repo_url);
        let spinner = terminal_utils::create_spinner(&format!("Installing {}...", repo_url));
        let output = process_utils::execute_command_with_output(
            "uv",
            &["add", &git_url],
            working_dir,
            300, // 5 minutes timeout
        )
        .await;
        spinner.finish_and_clear();

        output.map(|_| ())
    }

    pub async fn reinstall(package: &str, working_dir: Option<&Path>) -> Result<()> {
        Self::remove(vec![package], working_dir).await?;
        Self::add(vec![package], false, None, working_dir).await
    }

    pub async fn remove(packages: Vec<&str>, working_dir: Option<&Path>) -> Result<()> {
        let spinner =
            terminal_utils::create_spinner(&format!("Removing {}...", packages.join(", ")));

        let mut args = vec!["remove"];
        args.extend(packages.clone());

        let output = process_utils::execute_command_with_output(
            "uv",
            &args,
            working_dir,
            300, // 5 minutes timeout
        )
        .await;
        spinner.finish_and_clear();

        output.map(|_| ())
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

    pub async fn list(working_dir: Option<&Path>) -> Result<Vec<String>> {
        let output =
            process_utils::execute_command_with_output("uv", &["pip", "list"], working_dir, 30)
                .await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|line| line.to_string()).collect())
    }

    pub async fn get_installed_version(
        package: &str,
        working_dir: Option<&Path>,
    ) -> Result<String> {
        let stdout = Self::show(package, working_dir).await?;

        for line in stdout.lines() {
            if line.starts_with("Version:") {
                return Ok(line.replace("Version:", "").trim().to_string());
            }
        }

        Err(NbrError::not_found(format!(
            "Version not found for package: {}",
            package
        )))
    }

    pub async fn is_installed(package: &str, working_dir: Option<&Path>) -> bool {
        let output = Self::show(package, working_dir).await;
        output.is_ok() && output.unwrap().contains("Version")
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn working_dir() -> Option<&'static Path> {
        // let current_dir = std::env::current_dir().unwrap();
        // current_dir.join("awesome-bot")
        Some(Path::new("awesome-bot"))
    }

    #[tokio::test]
    async fn test_is_installed() {
        let is_installed = Uv::is_installed("not-exist-package", working_dir()).await;
        assert!(!is_installed);
        let is_installed = Uv::is_installed("nonebot2", working_dir()).await;
        assert!(is_installed);
    }

    #[tokio::test]
    async fn test_get_installed_version() {
        let result = Uv::get_installed_version("nonebot2", working_dir()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("2."));
        let result = Uv::get_installed_version("not-exist-package", working_dir()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("0.1.0"));
    }
}
