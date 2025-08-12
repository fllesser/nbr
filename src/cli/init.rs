//! Init command handler for nb-cli
//!
//! This module handles initializing NoneBot projects in the current directory,
//! creating necessary files and directory structure.

use crate::config::{ConfigManager, NbConfig};
use crate::error::{NbCliError, Result};
use crate::utils::{fs_utils, git_utils, string_utils, template_utils};
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, Input, MultiSelect};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Project initialization handler
pub struct InitHandler {
    /// Configuration manager
    config_manager: ConfigManager,
    /// Working directory
    work_dir: PathBuf,
    /// Project options
    options: InitOptions,
}

/// Project initialization options
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Project name
    pub name: String,
    /// Project description
    pub description: Option<String>,
    /// Author name
    pub author_name: Option<String>,
    /// Author email
    pub author_email: Option<String>,
    /// Python version requirement
    pub python_version: String,
    /// Initial adapters to install
    pub adapters: Vec<String>,
    /// Initial plugins to install
    pub plugins: Vec<String>,
    /// Initialize git repository
    pub init_git: bool,
    /// Create virtual environment
    pub create_venv: bool,
    /// Use poetry for dependency management
    pub use_poetry: bool,
    /// Force overwrite existing files
    pub force: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            name: "nonebot-project".to_string(),
            description: None,
            author_name: None,
            author_email: None,
            python_version: ">=3.8".to_string(),
            adapters: vec!["console".to_string()],
            plugins: vec![],
            init_git: true,
            create_venv: false,
            use_poetry: false,
            force: false,
        }
    }
}

impl InitHandler {
    /// Create a new init handler
    pub async fn new(force: bool) -> Result<Self> {
        let mut config_manager = ConfigManager::new()?;
        config_manager.load().await?;

        let work_dir = env::current_dir()
            .map_err(|e| NbCliError::io(format!("Failed to get current directory: {}", e)))?;

        let mut options = InitOptions::default();
        options.force = force;

        // Use directory name as default project name
        if let Some(dir_name) = work_dir.file_name().and_then(|n| n.to_str()) {
            options.name = string_utils::to_snake_case(dir_name);
        }

        // Use config author info if available
        let config = config_manager.config();
        if let Some(ref author) = config.user.author {
            options.author_name = Some(author.name.clone());
            options.author_email = author.email.clone();
        }

        Ok(Self {
            config_manager,
            work_dir,
            options,
        })
    }

    /// Initialize project interactively
    pub async fn init_project(&mut self, project_name: Option<&str>) -> Result<()> {
        println!("{}", "Initializing NoneBot project...".bright_blue().bold());
        println!();

        // Set project name if provided
        if let Some(name) = project_name {
            self.options.name = name.to_string();
        }

        // Check if directory is empty or force is enabled
        if !self.options.force && !self.is_directory_suitable()? {
            return Err(NbCliError::already_exists(
                "Directory is not empty. Use --force to initialize anyway.",
            ));
        }

        // Gather project information
        self.gather_project_info()?;

        // Confirm initialization
        self.show_project_summary();

        if !Confirm::new()
            .with_prompt("Initialize project with these settings?")
            .default(true)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            info!("Project initialization cancelled");
            return Ok(());
        }

        // Create project structure
        self.create_project_structure().await?;

        // Generate project files
        self.generate_project_files().await?;

        // Initialize git repository
        if self.options.init_git {
            self.init_git_repository()?;
        }

        // Create virtual environment
        if self.options.create_venv {
            self.create_virtual_environment().await?;
        }

        // Save project configuration
        self.save_project_config().await?;

        // Show completion message
        self.show_completion_message();

        Ok(())
    }

    /// Check if directory is suitable for initialization
    fn is_directory_suitable(&self) -> Result<bool> {
        let entries = fs::read_dir(&self.work_dir)
            .map_err(|e| NbCliError::io(format!("Failed to read directory: {}", e)))?;

        let mut file_count = 0;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Ignore hidden files and common directories
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') || name == "__pycache__" {
                    continue;
                }
            }

            file_count += 1;
        }

        Ok(file_count == 0)
    }

    /// Gather project information from user
    fn gather_project_info(&mut self) -> Result<()> {
        println!("{}", "Project Information:".bright_green());

        // Project name
        let name: String = Input::new()
            .with_prompt("Project name")
            .default(self.options.name.clone())
            .validate_with(|input: &String| {
                string_utils::validate_project_name(input)
                    .map(|_| ())
                    .map_err(|e| format!("Invalid project name: {}", e))
            })
            .interact_text()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        self.options.name = name;

        // Project description
        let description: String = Input::new()
            .with_prompt("Project description")
            .allow_empty(true)
            .interact_text()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        if !description.trim().is_empty() {
            self.options.description = Some(description.trim().to_string());
        }

        // Author information
        if self.options.author_name.is_none() {
            let author: String = Input::new()
                .with_prompt("Author name")
                .allow_empty(true)
                .interact_text()
                .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

            if !author.trim().is_empty() {
                self.options.author_name = Some(author.trim().to_string());
            }
        }

        if self.options.author_email.is_none() {
            let email: String = Input::new()
                .with_prompt("Author email")
                .allow_empty(true)
                .interact_text()
                .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

            if !email.trim().is_empty() {
                self.options.author_email = Some(email.trim().to_string());
            }
        }

        println!();
        println!("{}", "Project Setup:".bright_green());

        // Select adapters
        self.select_adapters()?;

        // Select plugins
        self.select_plugins()?;

        // Development options
        self.options.init_git = Confirm::new()
            .with_prompt("Initialize git repository?")
            .default(self.options.init_git)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        self.options.create_venv = Confirm::new()
            .with_prompt("Create virtual environment?")
            .default(self.options.create_venv)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        self.options.use_poetry = Confirm::new()
            .with_prompt("Use Poetry for dependency management?")
            .default(self.options.use_poetry)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        Ok(())
    }

    /// Select adapters to install
    fn select_adapters(&mut self) -> Result<()> {
        let available_adapters = vec![
            "console",
            "onebot-v11",
            "onebot-v12",
            "telegram",
            "discord",
            "dingtalk",
            "feishu",
            "kaiheila",
        ];

        let descriptions = vec![
            "Console adapter for testing",
            "OneBot V11 protocol adapter",
            "OneBot V12 protocol adapter",
            "Telegram Bot API adapter",
            "Discord Bot adapter",
            "DingTalk adapter",
            "Feishu adapter",
            "Kaiheila adapter",
        ];

        println!("{}", "Select adapters to install:".bright_blue());

        let defaults = vec![false]; // Console adapter by default
        let selections = MultiSelect::new()
            .items(&descriptions)
            .defaults(&defaults)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

        self.options.adapters = selections
            .iter()
            .map(|&i| available_adapters[i].to_string())
            .collect();

        Ok(())
    }

    /// Select plugins to install
    fn select_plugins(&mut self) -> Result<()> {
        let common_plugins = vec!["echo", "help", "manager", "status", "reload"];

        let descriptions = vec![
            "Echo plugin for testing",
            "Help command plugin",
            "Plugin manager",
            "Bot status plugin",
            "Hot reload plugin",
        ];

        if Confirm::new()
            .with_prompt("Install common plugins?")
            .default(false)
            .interact()
            .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?
        {
            println!("{}", "Select plugins to install:".bright_blue());

            let selections = MultiSelect::new()
                .items(&descriptions)
                .interact()
                .map_err(|e| NbCliError::io(format!("Failed to read user input: {}", e)))?;

            self.options.plugins = selections
                .iter()
                .map(|&i| common_plugins[i].to_string())
                .collect();
        }

        Ok(())
    }

    /// Show project summary
    fn show_project_summary(&self) {
        println!();
        println!("{}", "Project Summary:".bright_cyan().bold());
        println!(
            "  {} {}",
            "Name:".bright_black(),
            self.options.name.bright_white()
        );

        if let Some(ref desc) = self.options.description {
            println!(
                "  {} {}",
                "Description:".bright_black(),
                desc.bright_white()
            );
        }

        if let Some(ref author) = self.options.author_name {
            println!("  {} {}", "Author:".bright_black(), author.bright_white());
        }

        if !self.options.adapters.is_empty() {
            println!(
                "  {} {}",
                "Adapters:".bright_black(),
                self.options.adapters.join(", ").bright_yellow()
            );
        }

        if !self.options.plugins.is_empty() {
            println!(
                "  {} {}",
                "Plugins:".bright_black(),
                self.options.plugins.join(", ").bright_yellow()
            );
        }

        println!(
            "  {} {}",
            "Git:".bright_black(),
            if self.options.init_git { "Yes" } else { "No" }.bright_white()
        );

        println!(
            "  {} {}",
            "Virtual Env:".bright_black(),
            if self.options.create_venv {
                "Yes"
            } else {
                "No"
            }
            .bright_white()
        );

        println!(
            "  {} {}",
            "Poetry:".bright_black(),
            if self.options.use_poetry { "Yes" } else { "No" }.bright_white()
        );

        println!();
    }

    /// Create project directory structure
    async fn create_project_structure(&self) -> Result<()> {
        info!("Creating project structure...");

        let dirs = vec!["src", "src/plugins", "tests", "docs", "logs", "data"];

        for dir in dirs {
            let dir_path = self.work_dir.join(dir);
            fs_utils::ensure_dir(&dir_path)?;
            debug!("Created directory: {}", dir);
        }

        Ok(())
    }

    /// Generate project files
    async fn generate_project_files(&self) -> Result<()> {
        info!("Generating project files...");

        // Generate bot entry file
        self.generate_bot_file().await?;

        // Generate configuration files
        self.generate_config_files().await?;

        // Generate documentation
        self.generate_documentation().await?;

        // Generate development files
        self.generate_dev_files().await?;

        Ok(())
    }

    /// Generate bot entry file
    async fn generate_bot_file(&self) -> Result<()> {
        let template = self.get_bot_template();
        let context = self.create_template_context();
        let content = template_utils::render_template(&template, &context)?;

        let bot_file = self.work_dir.join("bot.py");
        fs::write(&bot_file, content)
            .map_err(|e| NbCliError::io(format!("Failed to write bot file: {}", e)))?;

        debug!("Generated bot.py");
        Ok(())
    }

    /// Generate configuration files
    async fn generate_config_files(&self) -> Result<()> {
        // Generate .env file
        let env_content = self.generate_env_content();
        let env_file = self.work_dir.join(".env");
        fs::write(&env_file, env_content)
            .map_err(|e| NbCliError::io(format!("Failed to write .env file: {}", e)))?;

        // Generate .env.example file
        let env_example_content = self.generate_env_example_content();
        let env_example_file = self.work_dir.join(".env.example");
        fs::write(&env_example_file, env_example_content)
            .map_err(|e| NbCliError::io(format!("Failed to write .env.example file: {}", e)))?;

        // Generate pyproject.toml or requirements.txt
        if self.options.use_poetry {
            let pyproject_content = self.generate_pyproject_toml();
            let pyproject_file = self.work_dir.join("pyproject.toml");
            fs::write(&pyproject_file, pyproject_content).map_err(|e| {
                NbCliError::io(format!("Failed to write pyproject.toml file: {}", e))
            })?;
        } else {
            let requirements_content = self.generate_requirements_txt();
            let requirements_file = self.work_dir.join("requirements.txt");
            fs::write(&requirements_file, requirements_content).map_err(|e| {
                NbCliError::io(format!("Failed to write requirements.txt file: {}", e))
            })?;
        }

        debug!("Generated configuration files");
        Ok(())
    }

    /// Generate documentation files
    async fn generate_documentation(&self) -> Result<()> {
        // Generate README.md
        let readme_content = self.generate_readme();
        let readme_file = self.work_dir.join("README.md");
        fs::write(&readme_file, readme_content)
            .map_err(|e| NbCliError::io(format!("Failed to write README.md file: {}", e)))?;

        debug!("Generated documentation files");
        Ok(())
    }

    /// Generate development files
    async fn generate_dev_files(&self) -> Result<()> {
        // Generate .gitignore
        let gitignore_content = self.generate_gitignore();
        let gitignore_file = self.work_dir.join(".gitignore");
        fs::write(&gitignore_file, gitignore_content)
            .map_err(|e| NbCliError::io(format!("Failed to write .gitignore file: {}", e)))?;

        // Generate Dockerfile
        let dockerfile_content = self.generate_dockerfile();
        let dockerfile_file = self.work_dir.join("Dockerfile");
        fs::write(&dockerfile_file, dockerfile_content)
            .map_err(|e| NbCliError::io(format!("Failed to write Dockerfile: {}", e)))?;

        // Generate docker-compose.yml
        let docker_compose_content = self.generate_docker_compose();
        let docker_compose_file = self.work_dir.join("docker-compose.yml");
        fs::write(&docker_compose_file, docker_compose_content)
            .map_err(|e| NbCliError::io(format!("Failed to write docker-compose.yml: {}", e)))?;

        debug!("Generated development files");
        Ok(())
    }

    /// Create template context
    fn create_template_context(&self) -> HashMap<String, String> {
        let mut context = HashMap::new();

        context.insert("project_name".to_string(), self.options.name.clone());
        context.insert(
            "project_name_pascal".to_string(),
            string_utils::to_pascal_case(&self.options.name),
        );

        if let Some(ref description) = self.options.description {
            context.insert("project_description".to_string(), description.clone());
        } else {
            context.insert(
                "project_description".to_string(),
                format!("A NoneBot2 project: {}", self.options.name),
            );
        }

        if let Some(ref author) = self.options.author_name {
            context.insert("author_name".to_string(), author.clone());
        }

        if let Some(ref email) = self.options.author_email {
            context.insert("author_email".to_string(), email.clone());
        }

        context.insert(
            "python_version".to_string(),
            self.options.python_version.clone(),
        );
        context.insert("adapters".to_string(), self.options.adapters.join(", "));
        context.insert("plugins".to_string(), self.options.plugins.join(", "));
        context.insert(
            "use_poetry".to_string(),
            self.options.use_poetry.to_string(),
        );

        context
    }

    /// Get bot template
    fn get_bot_template(&self) -> String {
        format!(
            r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
{{{{project_name}}}} - {{{{project_description}}}}

Author: {{{{author_name}}}}
"""

import nonebot
from nonebot.log import logger, default_format
{}

# Setup logging
logger.add(
    "logs/{{{{project_name}}}}.log",
    rotation="1 day",
    retention="30 days",
    level="INFO",
    format=default_format
)

# Initialize NoneBot
nonebot.init(_env_file=".env")

# Get driver and register adapters
driver = nonebot.get_driver()
{}

# Load plugins
{}

@driver.on_startup
async def startup():
    logger.info("{{{{project_name_pascal}}}} Bot is starting...")

@driver.on_shutdown
async def shutdown():
    logger.info("{{{{project_name_pascal}}}} Bot is shutting down...")

if __name__ == "__main__":
    logger.info("Starting {{{{project_name_pascal}}}} Bot...")
    nonebot.run()
"#,
            self.generate_adapter_imports(),
            self.generate_adapter_registrations(),
            self.generate_plugin_loading()
        )
    }

    /// Generate adapter imports
    fn generate_adapter_imports(&self) -> String {
        let mut imports = Vec::new();

        for adapter in &self.options.adapters {
            let import = match adapter.as_str() {
                "console" => "from nonebot.adapters.console import Adapter as ConsoleAdapter",
                "onebot-v11" => {
                    "from nonebot.adapters.onebot.v11 import Adapter as OneBotV11Adapter"
                }
                "onebot-v12" => {
                    "from nonebot.adapters.onebot.v12 import Adapter as OneBotV12Adapter"
                }
                "telegram" => "from nonebot.adapters.telegram import Adapter as TelegramAdapter",
                "discord" => "from nonebot.adapters.discord import Adapter as DiscordAdapter",
                "dingtalk" => "from nonebot.adapters.ding import Adapter as DingTalkAdapter",
                "feishu" => "from nonebot.adapters.feishu import Adapter as FeishuAdapter",
                "kaiheila" => "from nonebot.adapters.kaiheila import Adapter as KaiheilaAdapter",
                _ => continue,
            };
            imports.push(import);
        }

        imports.join("\n")
    }

    /// Generate adapter registrations
    fn generate_adapter_registrations(&self) -> String {
        let mut registrations = Vec::new();

        for adapter in &self.options.adapters {
            let registration = match adapter.as_str() {
                "console" => "driver.register_adapter(ConsoleAdapter)",
                "onebot-v11" => "driver.register_adapter(OneBotV11Adapter)",
                "onebot-v12" => "driver.register_adapter(OneBotV12Adapter)",
                "telegram" => "driver.register_adapter(TelegramAdapter)",
                "discord" => "driver.register_adapter(DiscordAdapter)",
                "dingtalk" => "driver.register_adapter(DingTalkAdapter)",
                "feishu" => "driver.register_adapter(FeishuAdapter)",
                "kaiheila" => "driver.register_adapter(KaiheilaAdapter)",
                _ => continue,
            };
            registrations.push(registration);
        }

        registrations.join("\n")
    }

    /// Generate plugin loading
    fn generate_plugin_loading(&self) -> String {
        let mut loading = vec!["nonebot.load_plugins(\"src/plugins\")".to_string()];

        for plugin in &self.options.plugins {
            let load_stmt = match plugin.as_str() {
                "echo" => "nonebot.load_builtin_plugins(\"echo\")",
                "help" => "nonebot.load_plugin(\"nonebot_plugin_help\")",
                "manager" => "nonebot.load_plugin(\"nonebot_plugin_manager\")",
                "status" => "nonebot.load_plugin(\"nonebot_plugin_status\")",
                "reload" => "nonebot.load_plugin(\"nonebot_plugin_reload\")",
                _ => continue,
            };
            loading.push(load_stmt.to_string());
        }

        loading.join("\n")
    }

    /// Generate .env content
    fn generate_env_content(&self) -> String {
        let mut content = vec![
            "# NoneBot Environment Configuration".to_string(),
            "".to_string(),
            "# Basic Settings".to_string(),
            "ENVIRONMENT=dev".to_string(),
            "HOST=127.0.0.1".to_string(),
            "PORT=8080".to_string(),
            "LOG_LEVEL=INFO".to_string(),
            "".to_string(),
        ];

        // Add adapter-specific configuration
        for adapter in &self.options.adapters {
            match adapter.as_str() {
                "onebot-v11" => {
                    content.extend_from_slice(&[
                        "# OneBot V11 Configuration".to_string(),
                        "ONEBOT_ACCESS_TOKEN=your_access_token".to_string(),
                        "ONEBOT_SECRET=your_secret".to_string(),
                        "ONEBOT_WS_URLS=[\"ws://127.0.0.1:6700/\"]".to_string(),
                        "".to_string(),
                    ]);
                }
                "telegram" => {
                    content.extend_from_slice(&[
                        "# Telegram Configuration".to_string(),
                        "TELEGRAM_BOT_TOKEN=your_bot_token".to_string(),
                        "".to_string(),
                    ]);
                }
                "discord" => {
                    content.extend_from_slice(&[
                        "# Discord Configuration".to_string(),
                        "DISCORD_BOT_TOKEN=your_bot_token".to_string(),
                        "".to_string(),
                    ]);
                }
                _ => {}
            }
        }

        content.join("\n")
    }

    /// Generate .env.example content
    fn generate_env_example_content(&self) -> String {
        self.generate_env_content()
            .replace("your_access_token", "YOUR_ACCESS_TOKEN_HERE")
            .replace("your_secret", "YOUR_SECRET_HERE")
            .replace("your_bot_token", "YOUR_BOT_TOKEN_HERE")
    }

    /// Generate pyproject.toml content
    fn generate_pyproject_toml(&self) -> String {
        let dependencies = self.generate_dependencies();
        let context = self.create_template_context();

        let template = format!(
            r#"[tool.poetry]
name = "{{{{project_name}}}}"
version = "0.1.0"
description = "{{{{project_description}}}}"
authors = ["{author}"]
readme = "README.md"
packages = [{{{{name}} = "src"}}]

[tool.poetry.dependencies]
python = "{python_version}"
{dependencies}

[tool.poetry.group.dev.dependencies]
pytest = "^7.0"
pytest-asyncio = "^0.21"
black = "^23.0"
isort = "^5.12"
flake8 = "^6.0"

[build-system]
requires = ["poetry-core"]
build-backend = "poetry.core.masonry.api"

[tool.nonebot]
plugin_dirs = ["src/plugins"]
plugins = []

[tool.black]
line-length = 88
target-version = ["py38", "py39", "py310", "py311"]

[tool.isort]
profile = "black"
line_length = 88
"#,
            author = context.get("author_name").unwrap_or(&"Unknown".to_string()),
            python_version = self.options.python_version,
            dependencies = dependencies.join("\n")
        );

        template_utils::render_template(&template, &context).unwrap_or_else(|_| template)
    }

    /// Generate requirements.txt content
    fn generate_requirements_txt(&self) -> String {
        self.generate_dependencies().join("\n")
    }

    /// Generate dependencies list
    fn generate_dependencies(&self) -> Vec<String> {
        let mut deps = vec!["nonebot2[fastapi]>=2.0.0".to_string()];

        for adapter in &self.options.adapters {
            let dep = match adapter.as_str() {
                "onebot-v11" => "nonebot-adapter-onebot>=2.0.0",
                "onebot-v12" => "nonebot-adapter-ob12>=1.0.0",
                "telegram" => "nonebot-adapter-telegram>=4.0.0",
                "discord" => "nonebot-adapter-discord>=0.1.0",
                "dingtalk" => "nonebot-adapter-ding>=1.0.0",
                "feishu" => "nonebot-adapter-feishu>=2.0.0",
                "kaiheila" => "nonebot-adapter-kaiheila>=0.2.0",
                _ => continue,
            };
            deps.push(dep.to_string());
        }

        for plugin in &self.options.plugins {
            let dep = match plugin.as_str() {
                "help" => "nonebot-plugin-help",
                "manager" => "nonebot-plugin-manager",
                "status" => "nonebot-plugin-status",
                "reload" => "nonebot-plugin-reload",
                _ => continue,
            };
            deps.push(dep.to_string());
        }

        deps
    }

    /// Generate README.md content
    fn generate_readme(&self) -> String {
        let context = self.create_template_context();
        let template = format!(
            r#"# {{{{project_name_pascal}}}}

{{{{project_description}}}}

## Features

- Built with NoneBot2
- Support for multiple adapters: {}
- Extensible plugin system
- Docker support included

## Quick Start

### Installation

1. Clone this repository
2. Install dependencies:
   {}

3. Configure your bot:
   ```bash
   cp .env.example .env
   # Edit .env with your bot configuration
   ```

4. Run the bot:
   ```bash
   python bot.py
   ```

### Development

Run in development mode with auto-reload:
```bash
nb run --reload
```

### Docker

Build and run with Docker:
```bash
docker-compose up --build
```

## Configuration

Edit the `.env` file to configure your bot:

{}

## Project Structure

```
{{{{project_name}}}}/
├── src/
│   └── plugins/          # Custom plugins
├── tests/                # Test files
├── docs/                 # Documentation
├── logs/                 # Log files
├── data/                 # Bot data
├── bot.py                # Bot entry point
├── .env                  # Environment configuration
├── requirements.txt      # Dependencies
├── Dockerfile           # Docker configuration
└── README.md            # This file
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

This project is licensed under the MIT License.
"#,
            self.options.adapters.join(", "),
            if self.options.use_poetry {
                "poetry install"
            } else {
                "uv pip install -r requirements.txt"
            },
            self.generate_config_documentation()
        );

        template_utils::render_template(&template, &context).unwrap_or_else(|_| template)
    }

    /// Generate configuration documentation
    fn generate_config_documentation(&self) -> String {
        let mut docs = Vec::new();

        docs.push("| Variable | Description | Example |".to_string());
        docs.push("| --- | --- | --- |".to_string());
        docs.push("| `ENVIRONMENT` | Runtime environment | `dev`, `prod` |".to_string());
        docs.push("| `HOST` | Server host | `127.0.0.1` |".to_string());
        docs.push("| `PORT` | Server port | `8080` |".to_string());
        docs.push("| `LOG_LEVEL` | Logging level | `INFO`, `DEBUG` |".to_string());

        for adapter in &self.options.adapters {
            match adapter.as_str() {
                "onebot-v11" => {
                    docs.push(
                        "| `ONEBOT_ACCESS_TOKEN` | OneBot access token | `your_token` |"
                            .to_string(),
                    );
                    docs.push("| `ONEBOT_SECRET` | OneBot secret | `your_secret` |".to_string());
                }
                "telegram" => {
                    docs.push(
                        "| `TELEGRAM_BOT_TOKEN` | Telegram bot token | `123456:ABC-DEF...` |"
                            .to_string(),
                    );
                }
                "discord" => {
                    docs.push(
                        "| `DISCORD_BOT_TOKEN` | Discord bot token | `your_token` |".to_string(),
                    );
                }
                _ => {}
            }
        }

        docs.join("\n")
    }

    /// Generate .gitignore content
    fn generate_gitignore(&self) -> String {
        r#"# Byte-compiled / optimized / DLL files
__pycache__/
*.py[cod]
*$py.class

# C extensions
*.so

# Distribution / packaging
.Python
build/
develop-eggs/
dist/
downloads/
eggs/
.eggs/
lib/
lib64/
parts/
sdist/
var/
wheels/
*.egg-info/
.installed.cfg
*.egg
MANIFEST

# PyInstaller
*.manifest
*.spec

# Installer logs
pip-log.txt
pip-delete-this-directory.txt
.uv/

# Unit test / coverage reports
htmlcov/
.tox/
.nox/
.coverage
.coverage.*
.cache
nosetests.xml
coverage.xml
*.cover
.hypothesis/
.pytest_cache/

# Translations
*.mo
*.pot

# Django stuff:
*.log
local_settings.py
db.sqlite3

# Flask stuff:
instance/
.webassets-cache

# Scrapy stuff:
.scrapy

# Sphinx documentation
docs/_build/

# PyBuilder
target/

# Jupyter Notebook
.ipynb_checkpoints

# IPython
profile_default/
ipython_config.py

# pyenv
.python-version

# celery beat schedule file
celerybeat-schedule

# SageMath parsed files
*.sage.py

# Environments
.env
.venv
env/
venv/
ENV/
env.bak/
venv.bak/

# Spyder project settings
.spyderproject
.spyproject

# Rope project settings
.ropeproject

# mkdocs documentation
/site

# mypy
.mypy_cache/
.dmypy.json
dmypy.json

# NoneBot
logs/
data/
.nb/

# IDE
.vscode/
.idea/
*.swp
*.swo
*~

# OS
.DS_Store
.DS_Store?
._*
.Spotlight-V100
.Trashes
ehthumbs.db
Thumbs.db
"#
        .to_string()
    }

    /// Generate Dockerfile content
    fn generate_dockerfile(&self) -> String {
        let context = self.create_template_context();
        let template = r#"FROM python:3.11-slim

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements first for better caching
COPY requirements.txt .
RUN pip install uv && uv pip install --no-cache-dir -r requirements.txt

# Copy application code
COPY . .

# Create necessary directories
RUN mkdir -p logs data

# Set environment variables
ENV PYTHONPATH=/app
ENV PYTHONUNBUFFERED=1

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 \
    CMD python -c "import requests; requests.get('http://localhost:8080/health')" || exit 1

# Run the bot
CMD ["python", "bot.py"]
"#;

        template_utils::render_template(template, &context).unwrap_or_else(|_| template.to_string())
    }

    /// Generate docker-compose.yml content
    fn generate_docker_compose(&self) -> String {
        let context = self.create_template_context();
        let template = r#"version: '3.8'

services:
  {{project_name}}:
    build: .
    container_name: {{project_name}}-bot
    restart: unless-stopped
    env_file:
      - .env
    ports:
      - "8080:8080"
    volumes:
      - ./logs:/app/logs
      - ./data:/app/data
    networks:
      - bot-network

networks:
  bot-network:
    driver: bridge

# Add database services if needed
# postgres:
#   image: postgres:15-alpine
#   container_name: {{project_name}}-db
#   restart: unless-stopped
#   environment:
#     POSTGRES_DB: {{project_name}}
#     POSTGRES_USER: bot
#     POSTGRES_PASSWORD: password
#   volumes:
#     - postgres_data:/var/lib/postgresql/data
#   networks:
#     - bot-network

# volumes:
#   postgres_data:
"#;

        template_utils::render_template(template, &context).unwrap_or_else(|_| template.to_string())
    }

    /// Initialize git repository
    fn init_git_repository(&self) -> Result<()> {
        info!("Initializing git repository...");

        if git_utils::is_git_repository(&self.work_dir) {
            warn!("Git repository already exists");
            return Ok(());
        }

        git_utils::init_repository(&self.work_dir, false)?;

        // Create initial commit
        let git_dir = self.work_dir.join(".git");
        if git_dir.exists() {
            info!("Git repository initialized successfully");
        }

        Ok(())
    }

    /// Create virtual environment
    async fn create_virtual_environment(&self) -> Result<()> {
        info!("Creating virtual environment...");

        let python_cmd = crate::utils::process_utils::find_python()
            .ok_or_else(|| NbCliError::not_found("Python executable not found"))?;

        let venv_path = self.work_dir.join("venv");

        // Create virtual environment
        crate::utils::process_utils::execute_command_with_output(
            &python_cmd,
            &["-m", "venv", "venv"],
            Some(&self.work_dir),
            60,
        )
        .await?;

        info!("Virtual environment created at: {:?}", venv_path);

        // Install dependencies if requirements.txt exists
        let requirements_file = self.work_dir.join("requirements.txt");
        if requirements_file.exists() {
            debug!("Installing dependencies in virtual environment...");

            let uv_cmd = "uv";

            crate::utils::process_utils::execute_command_with_output(
                uv_cmd,
                &["pip", "install", "-r", "requirements.txt"],
                Some(&self.work_dir),
                300,
            )
            .await?;

            info!("Dependencies installed successfully");
        }

        Ok(())
    }

    /// Save project configuration
    async fn save_project_config(&mut self) -> Result<()> {
        self.config_manager
            .update_nb_config(|config| *config = NbConfig::default())?;

        self.config_manager.save().await?;

        info!("Project configuration saved");
        Ok(())
    }

    /// Show completion message
    fn show_completion_message(&self) {
        println!();
        println!(
            "{}",
            "✓ Project initialized successfully!".bright_green().bold()
        );
        println!();
        println!("{}", "Next Steps:".bright_yellow().bold());

        if self.options.create_venv {
            if cfg!(windows) {
                println!(
                    "  1. Activate virtual environment: {}",
                    "venv\\Scripts\\activate".bright_cyan()
                );
            } else {
                println!(
                    "  1. Activate virtual environment: {}",
                    "source venv/bin/activate".bright_cyan()
                );
            }
        } else {
            println!(
                "  1. Install dependencies: {}",
                if self.options.use_poetry {
                    "poetry install".bright_cyan()
                } else {
                    "uv pip install -r requirements.txt".bright_cyan()
                }
            );
        }

        println!("  2. Configure your bot: {}", "edit .env".bright_cyan());
        println!("  3. Run your bot: {}", "python bot.py".bright_cyan());

        if self.options.init_git {
            println!(
                "  4. Make initial commit: {}",
                "git add . && git commit -m \"Initial commit\"".bright_cyan()
            );
        }

        println!();
        println!("For more help: {}", "nb --help".bright_blue());
        println!("Documentation: {}", "https://nonebot.dev/".bright_blue());
    }
}

/// Handle the init command
pub async fn handle_init(matches: &ArgMatches) -> Result<()> {
    let project_name = matches.get_one::<String>("name").map(|s| s.as_str());
    let force = matches.get_flag("force");

    let mut handler = InitHandler::new(force).await?;
    handler.init_project(project_name).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_handler_creation() {
        let handler = InitHandler::new(false).await;
        assert!(handler.is_ok());
    }

    #[test]
    fn test_init_options_default() {
        let options = InitOptions::default();
        assert_eq!(options.name, "nonebot-project");
        assert!(options.init_git);
        assert!(!options.create_venv);
        assert!(!options.use_poetry);
    }

    #[test]
    fn test_template_context_creation() {
        let options = InitOptions {
            name: "test_bot".to_string(),
            description: Some("A test bot".to_string()),
            author_name: Some("Test Author".to_string()),
            ..Default::default()
        };

        let handler = InitHandler {
            config_manager: ConfigManager::new().unwrap(),
            work_dir: PathBuf::new(),
            options,
        };

        let context = handler.create_template_context();
        assert_eq!(context.get("project_name"), Some(&"test_bot".to_string()));
        assert_eq!(
            context.get("project_description"),
            Some(&"A test bot".to_string())
        );
        assert_eq!(context.get("author_name"), Some(&"Test Author".to_string()));
    }

    #[test]
    fn test_dependency_generation() {
        let options = InitOptions {
            adapters: vec!["console".to_string(), "onebot-v11".to_string()],
            plugins: vec!["help".to_string()],
            ..Default::default()
        };

        let handler = InitHandler {
            config_manager: ConfigManager::new().unwrap(),
            work_dir: PathBuf::new(),
            options,
        };

        let deps = handler.generate_dependencies();
        assert!(deps.contains(&"nonebot2[fastapi]>=2.0.0".to_string()));
        assert!(deps.contains(&"nonebot-adapter-onebot>=2.0.0".to_string()));
        assert!(deps.contains(&"nonebot-plugin-help".to_string()));
    }
}
