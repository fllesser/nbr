use crate::cli::EnvCommands;
pub use crate::environment::{EnvironmentChecker, find_python_executable};
use anyhow::Result;

pub async fn handle(commands: &EnvCommands) -> Result<()> {
    let work_dir = std::env::current_dir()?;
    let mut checker = EnvironmentChecker::new(work_dir)?;
    match commands {
        EnvCommands::Info => checker.show_info().await?,
        EnvCommands::Check => checker.check_environment().await?,
    }
    Ok(())
}
