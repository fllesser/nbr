use crate::error::{NbrError, Result};
use crate::log::StyledText;
use crate::pyproject::PyProjectConfig;

use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
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
            .interact()
            .map_err(|e| NbrError::io(format!("Failed to read user input: {}", e)))?
    {
        error!("File generation cancelled.");
        return Ok(());
    }

    // Generate bot file content
    let content = generate_bot_content(work_dir)?;

    // Write file
    fs::write(&bot_path, content)
        .map_err(|e| NbrError::io(format!("Failed to write bot file: {}", e)))?;

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
        .ok_or(NbrError::not_found("No tool.nonebot in pyproject.toml"))?;

    let name_module_tuples = nonebot
        .adapters
        .as_ref()
        .unwrap_or(&vec![])
        .iter()
        .map(|a| (a.alias(), a.module_name.to_owned()))
        .collect::<Vec<_>>();

    let adapters_import = name_module_tuples
        .iter()
        .map(|(alias, module)| format!("from {module} import Adapter as {alias}"))
        .collect::<Vec<_>>()
        .join("\n");

    let adapters_register = name_module_tuples
        .iter()
        .map(|(alias, _)| format!("driver.register_adapter({alias})"))
        .collect::<Vec<_>>()
        .join("\n");

    let builtin_plugins_load = nonebot
        .builtin_plugins
        .as_ref()
        .unwrap_or(&vec![])
        .iter()
        .map(|plugin| format!(r#""{plugin}""#))
        .reduce(|a, b| format!("{a}, {b}"))
        .map(|s| format!("nonebot.load_builtin_plugins({s})"))
        .unwrap_or_default();

    let content = format!(
        include_str!("templates/bot"),
        adapters_import, adapters_register, builtin_plugins_load
    );
    Ok(content)
}

/// Handle the generate command
pub async fn handle_generate(force: bool) -> Result<()> {
    let work_dir = std::env::current_dir()?;
    generate_bot_file(&work_dir, force).await
}
