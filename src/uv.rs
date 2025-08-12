use crate::{
    error::{NbCliError, Result},
    utils::{process_utils, terminal_utils},
};
use std::path::Path;

pub struct Uv;

#[allow(unused)]
impl Uv {
    pub async fn sync(working_dir: Option<&Path>) -> Result<()> {
        let args = vec!["sync"];
        let spinner = terminal_utils::create_spinner(&format!("Installing dependencies..."));
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

    pub async fn add(
        package: &str,
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

        args.push(package);

        let spinner = terminal_utils::create_spinner(&format!("Installing {}...", package));

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
        let args = vec!["add", &git_url];
        let spinner = terminal_utils::create_spinner(&format!("Installing {}...", repo_url));
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

    pub async fn reinstall(package: &str, working_dir: Option<&Path>) -> Result<()> {
        Self::remove(package, working_dir).await?;
        Self::add(package, false, None, working_dir).await
    }

    pub async fn remove(package: &str, working_dir: Option<&Path>) -> Result<()> {
        let args = vec!["remove"];
        let spinner = terminal_utils::create_spinner(&format!("Removing {}...", package));
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

    pub async fn show_version(package: &str, working_dir: Option<&Path>) -> Result<String> {
        let stdout = Self::show(package, working_dir).await?;

        for line in stdout.lines() {
            if line.starts_with("Version:") {
                return Ok(line.replace("Version:", "").trim().to_string());
            }
        }

        Err(NbCliError::not_found(format!(
            "Version not found for package: {}",
            package
        )))
    }

    pub async fn is_installed(package: &str, working_dir: Option<&Path>) -> Result<bool> {
        let output = Self::show(package, working_dir).await?;
        Ok(output.contains("Version"))
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
        let result = Uv::is_installed("nonebot2", working_dir()).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_show_version() {
        let result = Uv::show_version("nonebot2", working_dir()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("2."));
    }
}
