use anyhow::{Context, Result};
use std::{fs, path::Path};

use crate::log::StyledText;
use crate::pyproject::PyProjectConfig;

pub fn create_python_pin_file(work_dir: &Path, python_version: &str) -> Result<()> {
    fs::write(work_dir.join(".python-version"), python_version)
        .context("Failed to write .python-version")
}

pub fn create_dockerfile(work_dir: &Path) -> Result<()> {
    let dockerfile = include_str!("cli/templates/dockerfile");
    fs::write(work_dir.join("Dockerfile"), dockerfile).context("Failed to write Dockerfile")
}

pub fn create_compose_file(work_dir: &Path, project_name: &str) -> Result<()> {
    let compose_config = include_str!("cli/templates/compose.yml");
    let compose_config = compose_config.replace("${PROJECT_NAME}", project_name);
    fs::write(work_dir.join("compose.yml"), compose_config).context("Failed to write compose.yml")
}

pub fn create_dockerignore(work_dir: &Path) -> Result<()> {
    let dockerignore = include_str!("cli/templates/.dockerignore");
    fs::write(work_dir.join(".dockerignore"), dockerignore).context("Failed to write .dockerignore")
}

pub fn generate_docker_files(work_dir: &Path, python_version: Option<&str>) -> Result<()> {
    let pyproject = PyProjectConfig::parse(Some(work_dir))?;
    if !work_dir.join(".python-version").exists() {
        let version =
            python_version.context("Python version is required to create .python-version")?;
        create_python_pin_file(work_dir, version)?;
    }
    create_dockerfile(work_dir)?;
    create_compose_file(work_dir, &pyproject.project.name)?;
    create_dockerignore(work_dir)?;
    StyledText::new(" ")
        .green_bold("✓ Successfully generated Docker configs")
        .println();
    Ok(())
}
