#![allow(dead_code)]

use serde::Deserialize;

use crate::{
    error::{NbrError, Result},
    log::StyledText,
    utils::{process_utils, terminal_utils},
};
use std::{
    hash::{Hash, Hasher},
    path::Path,
};

/// Install pre-commit hooks
pub fn pre_commit_install(output_dir: &Path) -> Result<()> {
    let args = vec!["run", "pre-commit", "install"];
    CmdBuilder::uv(args).working_dir(output_dir).run()
}

pub fn add(packages: Vec<&str>) -> AddBuilder<'_> {
    AddBuilder::new(packages)
}

pub fn remove(packages: Vec<&str>) -> CmdBuilder<'_> {
    let mut args = vec!["remove"];
    args.extend(packages.clone());
    CmdBuilder::uv(args)
}

pub fn sync(python_version: Option<&str>) -> CmdBuilder<'_> {
    let mut args = vec!["sync"];
    if let Some(version) = python_version {
        args.push("--python");
        args.push(version);
    }
    CmdBuilder::uv(args)
}

pub fn show(package: &str) -> CmdBuilder<'_> {
    let args = vec!["pip", "show", package];
    CmdBuilder::uv(args)
}

pub fn reinstall(package: &str) -> Result<()> {
    add(vec![package]).reinstall(true).run()
}

pub fn upgrade(packages: Vec<&str>) -> Result<()> {
    let mut args = vec!["lock"];
    args.extend(packages.iter().flat_map(|p| ["-P", p]));
    CmdBuilder::uv(args).run()?;
    sync(None).run()
}

pub async fn is_installed(package: &str) -> bool {
    show(package).run_async().await.is_ok()
}

pub async fn self_version() -> Result<String> {
    let args = vec!["self", "version", "--short"];
    CmdBuilder::uv(args).run_async().await.map_err(|_| {
        let message = concat!(
            "uv not found. You can run\n\n",
            "   curl -LsSf https://astral.sh/uv/install.sh | sh\n\n",
            "to install or get more information from https://astral.sh/blog/uv",
        );
        NbrError::environment(message)
    })
}

pub async fn list(outdated: bool) -> Result<Vec<Package>> {
    let args: Vec<&str> = vec!["pip", "list", "--format=json"];
    let mut builder = CmdBuilder::uv(args);
    let stdout = if outdated {
        builder
            .arg("--outdated")
            .timeout(300)
            .run_async_with_spinner("Checking for outdated packages...")
            .await?
    } else {
        builder.run_async().await?
    };

    Ok(serde_json::from_str(&stdout)?)
}

pub async fn show_package_info(package: &str, working_dir: Option<&Path>) -> Result<Package> {
    let stdout = show(package)
        .working_dir_opt(working_dir)
        .run_async()
        .await?;

    let mut lines = stdout.lines();
    let name = lines
        .next()
        .unwrap()
        .trim_start_matches("Name: ")
        .to_owned();
    let version = lines
        .next()
        .unwrap()
        .trim_start_matches("Version: ")
        .to_owned();
    let latest_version = None;
    let location = Some(
        lines
            .next()
            .unwrap()
            .trim_start_matches("Location: ")
            .to_owned(),
    );
    let requires = Some(
        lines
            .next()
            .unwrap()
            .trim_start_matches("Requires:")
            .trim()
            .split(", ")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned())
            .collect::<Vec<String>>(),
    );
    let requires_by = Some(
        lines
            .next()
            .unwrap()
            .trim_start_matches("Required-by:")
            .trim()
            .split(", ")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned())
            .collect::<Vec<String>>(),
    );

    Ok(Package {
        name,
        version,
        latest_version,
        location,
        requires,
        requires_by,
    })
}

#[derive(Debug, Clone, Deserialize, Eq)]
#[allow(unused)]
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

impl Hash for Package {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
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
        StyledText::new("")
            .text("  ")
            .cyan(&self.name)
            .text(" ")
            .green("v")
            .green(&self.version)
            .text(" ")
            .with(|text| {
                if self.is_outdated() {
                    text.yellow(&format!("(v{})", self.latest_version.as_ref().unwrap()));
                }
            })
            .println();
    }
}

pub struct CmdBuilder<'a> {
    pub cmd: &'a str,
    pub args: Vec<&'a str>,
    pub working_dir: Option<&'a Path>,
    pub timeout_secs: u16,
}

impl<'a> CmdBuilder<'a> {
    /// Create a new CmdBuilder
    pub fn uv(args: Vec<&'a str>) -> Self {
        Self {
            cmd: "uv",
            args,
            working_dir: None,
            timeout_secs: 5,
        }
    }

    /// 无任何输出
    pub fn quiet(&mut self) -> &mut Self {
        self.args.insert(0, "--quiet");
        self
    }

    pub fn arg(&mut self, arg: &'a str) -> &mut Self {
        self.args.push(arg);
        self
    }

    pub fn args(&mut self, args: Vec<&'a str>) -> &mut Self {
        self.args.extend(args);
        self
    }

    /// Set the working directory
    pub fn working_dir(&mut self, working_dir: &'a Path) -> &mut Self {
        self.working_dir = Some(working_dir);
        self
    }

    pub fn working_dir_opt(&mut self, working_dir: Option<&'a Path>) -> &mut Self {
        self.working_dir = working_dir;
        self
    }

    /// Set the timeout in seconds
    pub fn timeout(&mut self, timeout_secs: u16) -> &mut Self {
        self.timeout_secs = timeout_secs;
        self
    }

    pub fn run(&self) -> Result<()> {
        process_utils::execute_interactive(self.cmd, &self.args, self.working_dir)
    }

    /// Run the command asynchronously and return the stdout as a string
    pub async fn run_async(&self) -> Result<String> {
        let output = process_utils::execute_command_with_output(
            self.cmd,
            &self.args,
            self.working_dir,
            self.timeout_secs as u64,
        )
        .await?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run the command asynchronously and return the stdout as a string with a spinner
    pub async fn run_async_with_spinner(&self, spinner_message: &str) -> Result<String> {
        let spinner = terminal_utils::create_spinner(spinner_message);
        let output = self.run_async().await?;
        spinner.finish_and_clear();
        Ok(output)
    }
}

pub struct AddBuilder<'a> {
    pub packages: Vec<&'a str>,
    pub upgrade: bool,
    pub index_url: Option<&'a str>,
    pub working_dir: Option<&'a Path>,
    pub extras: Option<Vec<&'a str>>,
    pub reinstall: bool,
}

impl<'a> AddBuilder<'a> {
    pub fn new(packages: Vec<&'a str>) -> Self {
        Self {
            packages,
            upgrade: false,
            index_url: None,
            working_dir: None,
            extras: None,
            reinstall: false,
        }
    }

    pub fn upgrade(&mut self, upgrade: bool) -> &mut Self {
        self.upgrade = upgrade;
        self
    }

    pub fn index_url_opt(&mut self, index_url: Option<&'a str>) -> &mut Self {
        self.index_url = index_url;
        self
    }

    pub fn index_url(&mut self, index_url: &'a str) -> &mut Self {
        self.index_url = Some(index_url);
        self
    }

    pub fn working_dir(&mut self, working_dir: &'a Path) -> &mut Self {
        self.working_dir = Some(working_dir);
        self
    }

    pub fn extras(&mut self, extras: Vec<&'a str>) -> &mut Self {
        self.extras = Some(extras);
        self
    }

    pub fn reinstall(&mut self, reinstall: bool) -> &mut Self {
        self.reinstall = reinstall;
        self
    }

    pub fn run(&self) -> Result<()> {
        let mut args: Vec<&str> = vec!["add"];
        args.extend(self.packages.clone());
        if self.upgrade {
            args.push("--upgrade");
        }
        if let Some(index_url) = self.index_url {
            args.push("--index-url");
            args.push(index_url);
        }
        if let Some(ref extras) = self.extras {
            let extras = extras.iter().flat_map(|e| ["--extra", e]);
            args.extend(extras);
        }
        if self.reinstall {
            args.push("--reinstall");
        }
        process_utils::execute_interactive("uv", &args, self.working_dir)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn working_dir() -> &'static Path {
        Path::new("awesome-bot")
    }

    #[tokio::test]
    async fn test_self_version() {
        let version = self_version().await.unwrap();
        println!("uv version: {}", version);
    }

    #[tokio::test]
    async fn test_show_package_info() {
        let package = show_package_info("nonebot2", Some(working_dir()))
            .await
            .unwrap();
        package.display_info();
        dbg!(package);
    }

    #[tokio::test]
    async fn test_list() {
        let packages = list(false).await.unwrap();
        println!("{:?}", packages);
    }

    #[tokio::test]
    async fn test_add_and_remove() {
        // add
        let result = add(vec!["nonebot-plugin-abs"])
            .working_dir(working_dir())
            .upgrade(true)
            .run();
        assert!(result.is_ok());

        // remove
        let result = remove(vec!["nonebot-plugin-abs"])
            .working_dir(working_dir())
            .run();
        assert!(result.is_ok());
    }
}
