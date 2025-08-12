//! Generate command handler for nb-cli
//!
//! This module handles generating bot entry files and other project files
//! with customizable templates and configurations.

use crate::config::ConfigManager;
use crate::error::{NbCliError, Result};
use crate::utils::template_utils;
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, Select};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Bot file templates
const BOT_TEMPLATES: &[(&str, &str)] = &[("general", "Basic bot template with minimal setup")];

/// Generate file handler
pub struct GenerateHandler {
    /// Configuration manager
    config_manager: ConfigManager,
    /// Working directory
    work_dir: PathBuf,
}

impl GenerateHandler {
    /// Create a new generate handler
    pub async fn new() -> Result<Self> {
        let mut config_manager = ConfigManager::new()?;
        config_manager.load().await?;

        let work_dir = env::current_dir()
            .map_err(|e| NbCliError::io(format!("Failed to get current directory: {}", e)))?;

        Ok(Self {
            config_manager,
            work_dir,
        })
    }

    /// Generate bot entry file
    pub async fn generate_bot_file(
        &self,
        filename: &str,
        template_type: Option<&str>,
        force: bool,
    ) -> Result<()> {
        let bot_path = self.work_dir.join(filename);

        // Check if file already exists
        if bot_path.exists() && !force {
            if !Confirm::new()
                .with_prompt(&format!("File '{}' already exists. Overwrite?", filename))
                .default(false)
                .interact()
                .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
            {
                info!("File generation cancelled");
                return Ok(());
            }
        }

        // Select template
        let template = if let Some(template_name) = template_type {
            template_name.to_string()
        } else {
            self.select_template()?
        };

        // Generate context for template
        let context = self.create_template_context(&template).await?;

        // Generate bot file content
        let content = self.generate_bot_content(&template, &context)?;

        // Write file
        fs::write(&bot_path, content)
            .map_err(|e| NbCliError::io(format!("Failed to write bot file: {}", e)))?;

        println!(
            "{} Generated bot file: {}",
            "✓".bright_green(),
            filename.bright_blue()
        );

        // Show next steps
        self.show_next_steps(&template, filename);

        Ok(())
    }

    /// Select template interactively
    fn select_template(&self) -> Result<String> {
        println!("{}", "Select bot template:".bright_blue());

        let templates: Vec<&str> = BOT_TEMPLATES.iter().map(|(name, _)| *name).collect();
        let descriptions: Vec<&str> = BOT_TEMPLATES.iter().map(|(_, desc)| *desc).collect();

        let selection = Select::new()
            .items(&descriptions)
            .default(0)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        Ok(templates[selection].to_string())
    }

    /// Create template context
    async fn create_template_context(&self, template: &str) -> Result<HashMap<String, String>> {
        let mut context = HashMap::new();
        let config = self.config_manager.config();

        // Basic context
        context.insert("template_type".to_string(), template.to_string());

        // Project information
        context.insert("project_name".to_string(), "awsome-bot".to_string());
        context.insert("project_version".to_string(), "0.1.0".to_string());
        context.insert(
            "project_description".to_string(),
            "A NoneBot2 application".to_string(),
        );

        // Author information
        if let Some(ref author) = config.user.author {
            context.insert("author_name".to_string(), author.name.clone());
            if let Some(ref email) = author.email {
                context.insert("author_email".to_string(), email.clone());
            }
        }

        // Environment variables
        context.insert("env_file".to_string(), ".env".to_string());
        context.insert("log_level".to_string(), config.user.log_level.clone());

        Ok(context)
    }

    /// Generate bot content based on template
    fn generate_bot_content(
        &self,
        template: &str,
        context: &HashMap<String, String>,
    ) -> Result<String> {
        let template_content = match template {
            "general" => self.get_basic_bot_template(),
            "advanced" => self.get_advanced_bot_template(),
            "production" => self.get_production_bot_template(),
            _ => {
                return Err(NbCliError::template(format!(
                    "Unknown template: {}",
                    template
                )));
            }
        };

        template_utils::render_template(&template_content, context)
    }

    /// Get basic bot template
    fn get_basic_bot_template(&self) -> String {
        include_str!("nbfile/bot.py").to_string()
    }

    /// Get advanced bot template
    fn get_advanced_bot_template(&self) -> String {
        include_str!("nbfile/bot.py").to_string()
    }

    /// Get production bot template
    fn get_production_bot_template(&self) -> String {
        include_str!("nbfile/bot.py").to_string()
    }

    /// Show next steps after file generation
    fn show_next_steps(&self, template: &str, _filename: &str) {
        println!();
        println!("{}", "Next steps:".bright_yellow().bold());

        match template {
            "general" => {
                println!("  1. Install dependencies: uv sync");
                println!("  2. Run your bot: nbuv run");
            }
            "advanced" => {
                println!("  1. Install dependencies: uv sync");
                println!("  2. Run your bot: nbuv run");
            }
            "production" => {
                println!("  1. Install dependencies: uv sync");
                println!("  2. Run your bot: nbuv run");
            }
            _ => {}
        }

        println!();
        println!(
            "{}",
            "For more help, visit: https://nonebot.dev/".bright_cyan()
        );
    }
}

/// Handle the generate command
pub async fn handle_generate(matches: &ArgMatches) -> Result<()> {
    let filename = matches
        .get_one::<String>("file")
        .map(|s| s.as_str())
        .unwrap_or("bot.py");

    let force = matches.get_flag("force");

    // Validate filename
    if !filename.ends_with(".py") {
        return Err(NbCliError::invalid_argument(
            "Bot file must have .py extension",
        ));
    }

    let handler = GenerateHandler::new().await?;

    println!("{}", "Generating bot entry file...".bright_blue());
    handler.generate_bot_file(filename, None, force).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_basic_template() {
        let handler = GenerateHandler::new().await.unwrap();
        let mut context = HashMap::new();
        context.insert("project_name".to_string(), "TestBot".to_string());

        let content = handler.generate_bot_content("general", &context).unwrap();

        assert!(content.contains("TestBot Bot"));
    }

    #[test]
    fn test_template_selection() {
        assert_eq!(BOT_TEMPLATES.len(), 1);
        assert!(BOT_TEMPLATES.iter().any(|(name, _)| *name == "general"));
    }
}
