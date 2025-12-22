use std::{fs, path::Path};

use crate::pyproject::PyProjectConfig;

use super::DockerCommands;
use anyhow::{Context, Result};

pub(crate) async fn handle(commands: &DockerCommands) -> Result<()> {
    let work_dir = std::env::current_dir()?;
    match commands {
        DockerCommands::Run => run_docker(&work_dir).await?,
        DockerCommands::Build => build_docker(&work_dir).await?,
        DockerCommands::Gen => generate_docker_files(&work_dir).await?,
    }
    Ok(())
}

#[allow(unused)]
pub(crate) async fn run_docker(work_dir: &Path) -> Result<()> {
    unimplemented!()
}

#[allow(unused)]
pub(crate) async fn build_docker(work_dir: &Path) -> Result<()> {
    unimplemented!()
}

pub(crate) async fn generate_docker_files(work_dir: &Path) -> Result<()> {
    // get python version
    let pyproject = PyProjectConfig::parse(Some(work_dir))?;
    create_python_version_file(work_dir, "3.12")?;
    create_dockerfile(work_dir)?;
    create_compose_file(work_dir, &pyproject.project.name)?;
    create_dockerignore(work_dir)?;
    Ok(())
}

pub(crate) fn create_python_version_file(work_dir: &Path, python_version: &str) -> Result<()> {
    fs::write(work_dir.join(".python-version"), python_version)
        .context("Failed to write .python-version")
}

pub(crate) fn create_dockerfile(work_dir: &Path) -> Result<()> {
    let dockerfile = format!(include_str!("templates/Dockerfile"));
    fs::write(work_dir.join("Dockerfile"), dockerfile).context("Failed to write Dockerfile")
}

pub(crate) fn create_compose_file(work_dir: &Path, project_name: &str) -> Result<()> {
    let compose_config = include_str!("templates/compose.yml");
    let compose_config = compose_config.replace("${PROJECT_NAME}", project_name);
    fs::write(work_dir.join("compose.yml"), compose_config).context("Failed to write compose.yml")
}

pub(crate) fn create_dockerignore(work_dir: &Path) -> Result<()> {
    let dockerignore = include_str!("templates/.dockerignore");
    fs::write(work_dir.join(".dockerignore"), dockerignore).context("Failed to write .dockerignore")
}
