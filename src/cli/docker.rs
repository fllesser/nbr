use super::DockerCommands;
use crate::{cli::common, log::StyledText, pyproject::PyProjectConfig};
use anyhow::{Context, Result};
use std::{fs, path::Path};

pub(crate) fn handle(commands: &DockerCommands) -> Result<()> {
    let work_dir = std::env::current_dir()?;
    match commands {
        DockerCommands::Run => run_docker(&work_dir)?,
        DockerCommands::Build => build_docker(&work_dir)?,
        DockerCommands::Gen => generate_docker_files(&work_dir)?,
    }
    Ok(())
}

#[allow(unused)]
pub(crate) fn run_docker(work_dir: &Path) -> Result<()> {
    unimplemented!()
}

#[allow(unused)]
pub(crate) fn build_docker(work_dir: &Path) -> Result<()> {
    unimplemented!()
}

pub(crate) fn generate_docker_files(work_dir: &Path) -> Result<()> {
    let pyproject = PyProjectConfig::parse(Some(work_dir))?;
    // if .python-version file not exists, select it
    if !work_dir.join(".python-version").exists() {
        let python_version = common::select_python_version()?;
        create_python_pin_file(work_dir, &python_version)?;
    }

    create_dockerfile(work_dir)?;
    create_compose_file(work_dir, &pyproject.project.name)?;
    create_dockerignore(work_dir)?;

    StyledText::new(" ")
        .green_bold("✓ Successfully generated Docker configs")
        .println();

    Ok(())
}

pub(crate) fn create_python_pin_file(work_dir: &Path, python_version: &str) -> Result<()> {
    fs::write(work_dir.join(".python-version"), python_version)
        .context("Failed to write .python-version")
}

pub(crate) fn create_dockerfile(work_dir: &Path) -> Result<()> {
    let dockerfile = include_str!("templates/dockerfile");
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
