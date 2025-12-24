use crate::log::StyledText;
use crate::pyproject::PyProjectConfig;
use anyhow::{Context, Result};

use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use std::fmt::Write;
use std::fs;
use std::path::Path;
use tracing::error;

/// Generate bot entry file
pub async fn generate_bot_file(work_dir: &Path, force: bool) -> Result<()> {
    let filename = "bot.py";
    let bot_path = work_dir.join(filename);

    // Check if file already exists
    if bot_path.exists()
        && !force
        && !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("File '{filename}' already exists. Overwrite"))
            .default(false)
            .interact()?
    {
        error!("File generation cancelled.");
        return Ok(());
    }

    // Generate bot file content
    let content = generate_bot_content(work_dir)?;

    // Write file
    fs::write(&bot_path, content).context("Failed to write bot file")?;

    StyledText::new(" ")
        .green_bold("✓ Successfully generated bot file:")
        .cyan_bold(filename)
        .println();

    Ok(())
}

pub fn generate_bot_content(work_dir: &Path) -> Result<String> {
    let pyproject = PyProjectConfig::parse(Some(work_dir))?;
    let nonebot = pyproject
        .nonebot()
        .context("No tool.nonebot in pyproject.toml")?;

    let name_module_tuples = nonebot
        .adapters
        .as_ref()
        .unwrap_or(&vec![])
        .iter()
        .map(|a| (a.alias(), a.module_name.to_owned()))
        .collect::<Vec<_>>();

    let mut adapters_import = String::new();
    let mut adapters_register = String::new();
    let mut iter = name_module_tuples.iter().peekable();
    while let Some((alias, module)) = iter.next() {
        write!(adapters_import, "from {module} import Adapter as {alias}")?;
        write!(adapters_register, "driver.register_adapter({alias})")?;
        if iter.peek().is_some() {
            adapters_import.push('\n');
            adapters_register.push('\n');
        }
    }

    let mut builtin_plugins_load = String::new();
    if let Some(builtin_plugins) = nonebot.builtin_plugins.as_ref() {
        write!(builtin_plugins_load, "nonebot.load_builtin_plugins(")?;
        let mut iter = builtin_plugins.iter().peekable();
        while let Some(plugin) = iter.next() {
            write!(builtin_plugins_load, r#""{plugin}""#)?;
            if iter.peek().is_some() {
                write!(builtin_plugins_load, ", ")?;
            }
        }
        builtin_plugins_load.push(')');
    }

    let content = format!(
        include_str!("templates/bot"),
        adapters_import, adapters_register, builtin_plugins_load
    );
    Ok(content)
}

/// Handle the generate command
pub async fn handle(force: bool) -> Result<()> {
    let work_dir = std::env::current_dir()?;
    generate_bot_file(&work_dir, force).await?;
    Ok(())
}
