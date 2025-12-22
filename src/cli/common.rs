use anyhow::Result;
use dialoguer::{Select, theme::ColorfulTheme};

pub(crate) fn select_python_version() -> Result<String> {
    let python_versions = vec!["3.10", "3.11", "3.12", "3.13", "3.14"];
    let selected_python_version = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which Python version would you like to use")
        .items(&python_versions)
        .default(0)
        .interact()?;
    Ok(python_versions[selected_python_version].to_string())
}
