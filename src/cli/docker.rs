use super::DockerCommands;
use crate::{cli::common, docker};
use anyhow::Result;
use std::path::Path;

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
    let mut python_version = None;
    if !work_dir.join(".python-version").exists() {
        python_version = Some(common::select_python_version()?);
    }
    docker::generate_docker_files(work_dir, python_version.as_deref())
}
