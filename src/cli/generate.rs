//! Generate command handler for nb-cli
//!
//! This module handles generating bot entry files and other project files
//! with customizable templates and configurations.

use crate::config::ConfigManager;
use crate::error::{NbCliError, Result};
use crate::utils::template_utils;
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, Input, Select};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Bot file templates
const BOT_TEMPLATES: &[(&str, &str)] = &[
    ("basic", "Basic bot template with minimal setup"),
    (
        "advanced",
        "Advanced bot template with plugins and middleware",
    ),
    (
        "production",
        "Production-ready bot template with logging and error handling",
    ),
    ("custom", "Interactive custom bot template"),
];

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

        // Host and port
        context.insert("default_host".to_string(), config.user.default_host.clone());
        context.insert(
            "default_port".to_string(),
            config.user.default_port.to_string(),
        );

        // Interactive context for custom template
        if template == "custom" {
            self.gather_custom_context(&mut context)?;
        }

        // Environment variables
        context.insert("env_file".to_string(), ".env".to_string());
        context.insert("log_level".to_string(), config.user.log_level.clone());

        Ok(context)
    }

    /// Gather custom context from user input
    fn gather_custom_context(&self, context: &mut HashMap<String, String>) -> Result<()> {
        println!("{}", "Custom bot configuration:".bright_blue());

        // Bot name
        let bot_name: String = Input::new()
            .with_prompt("Bot name")
            .default(
                context
                    .get("project_name")
                    .cloned()
                    .unwrap_or_else(|| "MyBot".to_string()),
            )
            .interact_text()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        context.insert("bot_name".to_string(), bot_name);

        // Enable logging
        let enable_logging = Confirm::new()
            .with_prompt("Enable advanced logging?")
            .default(true)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        context.insert("enable_logging".to_string(), enable_logging.to_string());

        // Enable middleware
        let enable_middleware = Confirm::new()
            .with_prompt("Enable request/response middleware?")
            .default(false)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        context.insert(
            "enable_middleware".to_string(),
            enable_middleware.to_string(),
        );

        // Enable plugin system
        let enable_plugins = Confirm::new()
            .with_prompt("Enable plugin system?")
            .default(true)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        context.insert("enable_plugins".to_string(), enable_plugins.to_string());

        Ok(())
    }

    /// Generate bot content based on template
    fn generate_bot_content(
        &self,
        template: &str,
        context: &HashMap<String, String>,
    ) -> Result<String> {
        let template_content = match template {
            "basic" => self.get_basic_bot_template(),
            "advanced" => self.get_advanced_bot_template(),
            "production" => self.get_production_bot_template(),
            "custom" => self.get_custom_bot_template(context),
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
        r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
{{project_name}} Bot

A simple NoneBot2 application.
"""

import nonebot
from nonebot.adapters.console import Adapter as ConsoleAdapter

# Initialize NoneBot
nonebot.init()

# Register adapters
driver = nonebot.get_driver()
driver.register_adapter(ConsoleAdapter)

# Load plugins
# nonebot.load_builtin_plugins("echo")

if __name__ == "__main__":
    nonebot.run(host="{{default_host}}", port={{default_port}})
"#
        .to_string()
    }

    /// Get advanced bot template
    fn get_advanced_bot_template(&self) -> String {
        r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
{{project_name}} Bot

Advanced NoneBot2 application with plugins and middleware.
"""

import nonebot
from nonebot.log import logger, default_format
from nonebot.adapters.console import Adapter as ConsoleAdapter

# Custom logging
logger.add(
    "logs/error.log",
    rotation="00:00",
    diagnosis=False,
    level="ERROR",
    format=default_format
)

# Initialize NoneBot
nonebot.init(_env_file=".env")

# Register adapters
driver = nonebot.get_driver()
driver.register_adapter(ConsoleAdapter)

# Load built-in plugins
nonebot.load_builtin_plugins("echo")

# Load plugins from plugins directory
# nonebot.load_plugins("plugins")

# Load plugins from external packages
# nonebot.load_plugin("nonebot_plugin_help")

@driver.on_startup
async def startup():
    logger.info("Bot is starting up...")

@driver.on_shutdown
async def shutdown():
    logger.info("Bot is shutting down...")

if __name__ == "__main__":
    logger.info("Starting {{project_name}} Bot...")
    nonebot.run()
"#
        .to_string()
    }

    /// Get production bot template
    fn get_production_bot_template(&self) -> String {
        r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
{{project_name}} Bot

Production-ready NoneBot2 application with comprehensive error handling.
"""

import sys
import asyncio
from pathlib import Path

import nonebot
from nonebot.log import logger, default_format
from nonebot.adapters.console import Adapter as ConsoleAdapter

# Ensure logs directory exists
Path("logs").mkdir(exist_ok=True)

# Configure logging
logger.add(
    "logs/bot.log",
    rotation="1 day",
    retention="30 days",
    level="INFO",
    format=default_format,
    backtrace=True,
    diagnose=True
)

logger.add(
    "logs/error.log",
    rotation="1 day",
    retention="7 days",
    level="ERROR",
    format=default_format,
    backtrace=True,
    diagnose=True
)

# Initialize NoneBot with environment file
nonebot.init(_env_file=".env")

# Get driver and config
driver = nonebot.get_driver()
config = driver.config

# Register adapters
driver.register_adapter(ConsoleAdapter)

# Load plugins
nonebot.load_builtin_plugins("echo")

# Load custom plugins
try:
    nonebot.load_plugins("src/plugins")
    logger.info("Custom plugins loaded successfully")
except Exception as e:
    logger.warning(f"Failed to load custom plugins: {e}")

# Global exception handler
@driver.on_startup
async def startup():
    logger.info(f"{{project_name}} Bot v{{project_version}} is starting...")
    logger.info(f"Environment: {config.environment}")

@driver.on_shutdown
async def shutdown():
    logger.info("Bot is shutting down gracefully...")

# Handle uncaught exceptions
def handle_exception(exc_type, exc_value, exc_traceback):
    if issubclass(exc_type, KeyboardInterrupt):
        sys.__excepthook__(exc_type, exc_value, exc_traceback)
        return

    logger.error(
        "Uncaught exception",
        exc_info=(exc_type, exc_value, exc_traceback)
    )

sys.excepthook = handle_exception

def handle_task_exception(task):
    try:
        task.result()
    except asyncio.CancelledError:
        pass
    except Exception as e:
        logger.error(f"Task exception: {e}", exc_info=True)

if __name__ == "__main__":
    try:
        logger.info("Starting {{project_name}} Bot...")
        nonebot.run()
    except Exception as e:
        logger.error(f"Failed to start bot: {e}", exc_info=True)
        sys.exit(1)
"#
        .to_string()
    }

    /// Get custom bot template based on user preferences
    fn get_custom_bot_template(&self, context: &HashMap<String, String>) -> String {
        let mut template = String::new();

        // Header
        template.push_str(&format!(
            r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
{} Bot

{}
"""

"#,
            context.get("bot_name").unwrap_or(&"MyBot".to_string()),
            context
                .get("project_description")
                .unwrap_or(&"A NoneBot2 application".to_string())
        ));

        // Imports
        template.push_str("import nonebot\n");

        if context
            .get("enable_logging")
            .unwrap_or(&"false".to_string())
            == "true"
        {
            template.push_str("from nonebot.log import logger, default_format\n");
        }

        template.push_str("from nonebot.adapters.console import Adapter as ConsoleAdapter\n\n");

        // Logging setup
        if context
            .get("enable_logging")
            .unwrap_or(&"false".to_string())
            == "true"
        {
            template.push_str(
                r#"# Configure logging
logger.add(
    "logs/bot.log",
    rotation="1 day",
    level="{{log_level}}",
    format=default_format
)

"#,
            );
        }

        // Initialize NoneBot
        template.push_str(
            r#"# Initialize NoneBot
nonebot.init(_env_file="{{env_file}}")

# Register adapters
driver = nonebot.get_driver()
driver.register_adapter(ConsoleAdapter)

"#,
        );

        // Plugin loading
        if context
            .get("enable_plugins")
            .unwrap_or(&"false".to_string())
            == "true"
        {
            template.push_str(
                r#"# Load plugins
nonebot.load_builtin_plugins("echo")
# nonebot.load_plugins("plugins")

"#,
            );
        }

        // Middleware
        if context
            .get("enable_middleware")
            .unwrap_or(&"false".to_string())
            == "true"
        {
            template.push_str(
                r#"# Startup and shutdown events
@driver.on_startup
async def startup():
    logger.info("Bot is starting up...")

@driver.on_shutdown
async def shutdown():
    logger.info("Bot is shutting down...")

"#,
            );
        }

        // Main block
        template.push_str(
            r#"if __name__ == "__main__":
"#,
        );

        if context
            .get("enable_logging")
            .unwrap_or(&"false".to_string())
            == "true"
        {
            template.push_str(
                r#"    logger.info("Starting {{bot_name}} Bot...")
"#,
            );
        }

        template.push_str(
            r#"    nonebot.run(host="{{default_host}}", port={{default_port}})
"#,
        );

        template
    }

    /// Show next steps after file generation
    fn show_next_steps(&self, template: &str, filename: &str) {
        println!();
        println!("{}", "Next steps:".bright_yellow().bold());

        match template {
            "basic" => {
                println!("  1. Install NoneBot2: uv add nonebot2[fastapi]");
                println!("  2. Run your bot: python {}", filename);
            }
            "advanced" => {
                println!("  1. Install dependencies: uv add nonebot2[fastapi]");
                println!("  2. Create plugins directory: mkdir plugins");
                println!("  3. Configure .env file with your settings");
                println!("  4. Run your bot: python {}", filename);
            }
            "production" => {
                println!("  1. Install dependencies: uv add nonebot2[fastapi]");
                println!("  2. Create src/plugins directory: mkdir -p src/plugins");
                println!("  3. Configure .env file with production settings");
                println!("  4. Set up proper logging rotation");
                println!("  5. Run your bot: python {}", filename);
            }
            "custom" => {
                println!("  1. Install required dependencies");
                println!("  2. Configure your .env file");
                println!("  3. Set up any required directories");
                println!("  4. Run your bot: python {}", filename);
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
        context.insert("default_host".to_string(), "127.0.0.1".to_string());
        context.insert("default_port".to_string(), "8080".to_string());

        let content = handler.generate_bot_content("basic", &context).unwrap();

        assert!(content.contains("TestBot Bot"));
        assert!(content.contains("127.0.0.1"));
        assert!(content.contains("8080"));
    }

    #[test]
    fn test_template_selection() {
        assert_eq!(BOT_TEMPLATES.len(), 4);
        assert!(BOT_TEMPLATES.iter().any(|(name, _)| *name == "basic"));
        assert!(BOT_TEMPLATES.iter().any(|(name, _)| *name == "advanced"));
        assert!(BOT_TEMPLATES.iter().any(|(name, _)| *name == "production"));
        assert!(BOT_TEMPLATES.iter().any(|(name, _)| *name == "custom"));
    }
}
