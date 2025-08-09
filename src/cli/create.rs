#![allow(dead_code)]

use anyhow::{Context, Result};
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use handlebars::Handlebars;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use super::env::AdapterInfo;

#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    pub description: String,
    pub url: Option<String>,
    pub builtin: bool,
    pub adapters: Vec<String>,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectOptions {
    pub name: String,
    pub template: String,
    pub output_dir: PathBuf,
    pub force: bool,
    pub adapters: Vec<String>,
    pub plugins: Vec<String>,
}

pub async fn handle_create(matches: &ArgMatches) -> Result<()> {
    println!("{}", "🎉 Creating NoneBot project...".bright_green());

    let options = gather_project_options(matches).await?;

    info!("Creating project with options: {:?}", options);

    // Check if directory already exists
    if options.output_dir.exists() && !options.force {
        let should_continue = Confirm::new()
            .with_prompt(format!(
                "Directory '{}' already exists. Continue?",
                options.output_dir.display()
            ))
            .default(false)
            .interact()?;

        if !should_continue {
            println!("{}", "❌ Operation cancelled.".bright_red());
            return Ok(());
        }
    }

    // Create the project
    create_project(&options).await?;

    println!("{}", "✨ Project created successfully!".bright_green());
    println!("📂 Location: {}", options.output_dir.display());
    println!("\n🚀 Next steps:");
    println!("  cd {}", options.name);
    println!("  nb run");

    // Show additional setup instructions
    show_setup_instructions(&options).await?;

    Ok(())
}

async fn gather_project_options(matches: &ArgMatches) -> Result<ProjectOptions> {
    let project_name = if let Some(name) = matches.get_one::<String>("name") {
        name.clone()
    } else {
        Input::<String>::new()
            .with_prompt("Project name")
            .default("awesome-bot".to_string())
            .validate_with(|input: &String| -> Result<(), String> {
                if input.is_empty() {
                    Err("Project name cannot be empty".to_string())
                } else if input.contains(' ') {
                    Err("Project name cannot contain spaces".to_string())
                } else {
                    Ok(())
                }
            })
            .interact_text()
            .context("Failed to get project name")?
    };

    let template_name = if let Some(template) = matches.get_one::<String>("template") {
        template.clone()
    } else {
        select_template().await?
    };

    let output_dir = if let Some(dir) = matches.get_one::<String>("output") {
        PathBuf::from(dir)
    } else {
        std::env::current_dir()?.join(&project_name)
    };

    let force = matches.get_flag("force");

    // Get template info and let user select adapters/plugins
    let template = get_template_info(&template_name).await?;
    let (adapters, plugins) = select_components(&template).await?;

    Ok(ProjectOptions {
        name: project_name,
        template: template_name,
        output_dir,
        force,
        adapters,
        plugins,
    })
}

async fn select_template() -> Result<String> {
    let templates = get_available_templates().await?;

    if templates.is_empty() {
        warn!("No templates available, using default bootstrap template");
        return Ok("bootstrap".to_string());
    }

    let template_descriptions: Vec<String> = templates
        .iter()
        .map(|t| format!("{} - {}", t.name, t.description))
        .collect();

    let selection = Select::new()
        .with_prompt("Select a template")
        .default(0)
        .items(&template_descriptions)
        .interact()?;

    Ok(templates[selection].name.clone())
}

async fn select_components(_template: &Template) -> Result<(Vec<String>, Vec<String>)> {
    // Select adapters
    let available_adapters = get_available_adapters().await?;
    let adapter_names: Vec<&str> = available_adapters.iter().map(|a| a.name.as_str()).collect();

    let selected_adapters = if !adapter_names.is_empty() {
        println!("\n{}", "🔌 Select adapters to install:".bright_cyan());
        let selections = MultiSelect::new()
            .with_prompt("Adapters")
            .items(&adapter_names)
            .defaults(&vec![true; adapter_names.len().min(1)]) // Select first adapter by default
            .interact()?;

        selections
            .into_iter()
            .map(|i| adapter_names[i].to_string())
            .collect()
    } else {
        vec!["OneBot V11".to_string()] // Default adapter
    };

    // Select plugins
    let recommended_plugins = vec![
        "nonebot-plugin-echo",
        "nonebot-plugin-status",
        "nonebot-plugin-help",
    ];

    println!("\n{}", "📦 Select plugins to install:".bright_cyan());
    let selected_plugins = MultiSelect::new()
        .with_prompt("Plugins (recommended)")
        .items(&recommended_plugins)
        .defaults(&vec![false; recommended_plugins.len()])
        .interact()?
        .into_iter()
        .map(|i| recommended_plugins[i].to_string())
        .collect();

    Ok((selected_adapters, selected_plugins))
}

async fn get_available_adapters() -> Result<Vec<super::env::AdapterInfo>> {
    let adapters = vec![AdapterInfo {
        name: "OneBot V11".to_string(),
        version: "2.4.6".to_string(),
        location: "https://github.com/nonebot/nonebot2".to_string(),
    }];

    Ok(adapters)
}

async fn get_available_templates() -> Result<Vec<Template>> {
    let templates = vec![
        Template {
            name: "bootstrap".to_string(),
            description: "Basic NoneBot project template".to_string(),
            url: None,
            builtin: true,
            adapters: vec!["OneBot V11".to_string()],
            plugins: vec![],
        },
        Template {
            name: "simple".to_string(),
            description: "Simple bot template with basic plugins".to_string(),
            url: None,
            builtin: true,
            adapters: vec!["OneBot V11".to_string()],
            plugins: vec!["nonebot-plugin-echo".to_string()],
        },
        Template {
            name: "full".to_string(),
            description: "Full-featured template with multiple adapters and plugins".to_string(),
            url: None,
            builtin: true,
            adapters: vec!["OneBot V11".to_string(), "Console".to_string()],
            plugins: vec![
                "nonebot-plugin-echo".to_string(),
                "nonebot-plugin-status".to_string(),
            ],
        },
    ];

    // TODO: Fetch remote templates from registry
    debug!("Available templates: {:?}", templates);

    Ok(templates)
}

async fn get_template_info(name: &str) -> Result<Template> {
    let templates = get_available_templates().await?;
    templates
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", name))
}

async fn create_project(options: &ProjectOptions) -> Result<()> {
    info!(
        "Creating project directory: {}",
        options.output_dir.display()
    );
    fs::create_dir_all(&options.output_dir).context("Failed to create output directory")?;

    match options.template.as_str() {
        "bootstrap" => create_bootstrap_project(options).await?,
        "simple" => create_simple_project(options).await?,
        "full" => create_full_project(options).await?,
        _ => {
            warn!(
                "Unknown template '{}', falling back to bootstrap",
                options.template
            );
            create_bootstrap_project(options).await?
        }
    }

    Ok(())
}

async fn create_bootstrap_project(options: &ProjectOptions) -> Result<()> {
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);

    // Register built-in templates
    register_templates(&mut handlebars)?;

    let mut data: HashMap<&str, &dyn erased_serde::Serialize> = HashMap::new();
    data.insert("project_name", &options.name);
    let package_name = options.name.replace("-", "_");
    data.insert("package_name", &package_name);
    data.insert("adapters", &options.adapters);
    data.insert("plugins", &options.plugins);

    // Create directory structure
    create_project_structure(&options.output_dir)?;

    // Generate files
    generate_bot_file(&handlebars, &data, &options.output_dir)?;
    generate_pyproject_file(&handlebars, &data, &options.output_dir)?;
    generate_env_files(&handlebars, &data, &options.output_dir)?;
    generate_readme_file(&handlebars, &data, &options.output_dir)?;
    generate_gitignore(&options.output_dir)?;
    generate_dockerfile(&handlebars, &data, &options.output_dir)?;

    Ok(())
}

async fn create_simple_project(options: &ProjectOptions) -> Result<()> {
    // Start with bootstrap template
    create_bootstrap_project(options).await?;

    // Add example plugin
    create_example_plugin(&options.output_dir)?;

    Ok(())
}

async fn create_full_project(options: &ProjectOptions) -> Result<()> {
    // Start with simple template
    create_simple_project(options).await?;

    // Add more sophisticated examples
    create_advanced_examples(&options.output_dir)?;

    Ok(())
}

fn register_templates(handlebars: &mut Handlebars) -> Result<()> {
    // Bot file template
    handlebars.register_template_string(
        "bot.py",
        r#"#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import nonebot
{{#each adapters}}
from nonebot.adapters.{{snake_case this}} import Adapter as {{pascal_case this}}_Adapter
{{/each}}

# Custom your logger
#
# from nonebot.log import logger, default_format
# logger.add("error.log",
#            rotation="00:00",
#            diagnose=False,
#            level="ERROR",
#            format=default_format)

# You can pass some keyword args config to init function
nonebot.init()
app = nonebot.get_asgi()

driver = nonebot.get_driver()
{{#each adapters}}
driver.register_adapter({{pascal_case this}}_Adapter)
{{/each}}

nonebot.load_from_toml("pyproject.toml")

# Modify some config / config depends on loaded configs
#
# config = driver.config
# do something...

if __name__ == "__main__":
    nonebot.logger.warning("Always use `nb run` to start the bot instead of manually running!")
    nonebot.run(app="__mp_main__:app")
"#,
    )?;

    // pyproject.toml template
    handlebars.register_template_string("pyproject.toml", r#"[project]
name = "{{project_name}}"
version = "0.1.0"
description = ""
authors = []
readme = "README.md"
requires-python = ">=3.10"
dependencies = [
    "nonebot2>=2.3.0"
    "{{#each adapters}}"
    "{{adapter_package this}}>=2.0.0"
    "{{/each}}"
    "{{#each plugins}}"
    {{this}} = "*"
    {{/each}}
]


[tool.nonebot]
adapters = [
{{#each adapters}}
    { name = "{{this}}", module_name = "{{module_name this}}", project_link = "{{adapter_package this}}", desc = "{{this}} 协议" },
{{/each}}
]
plugins = [
{{#each plugins}}
    "{{this}}",
{{/each}}
]
plugin_dirs = ["{{package_name}}/plugins"]
builtin_plugins = ["echo"]

[build-system]
requires = ["uv_build>=0.8.3,<0.9.0"]
build-backend = "uv_build"
"#)?;

    // Register helper functions
    handlebars.register_helper("snake_case", Box::new(snake_case_helper));
    handlebars.register_helper("pascal_case", Box::new(pascal_case_helper));
    handlebars.register_helper("adapter_package", Box::new(adapter_package_helper));
    handlebars.register_helper("module_name", Box::new(module_name_helper));

    Ok(())
}

fn snake_case_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let value = param.value().as_str().unwrap_or("");
    let snake_case = value.to_lowercase().replace(" ", "_").replace("-", "_");
    out.write(&snake_case)?;
    Ok(())
}

fn pascal_case_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let value = param.value().as_str().unwrap_or("");
    let pascal_case = value
        .split_whitespace()
        .map(|word| {
            let mut chars: Vec<char> = word.chars().collect();
            if let Some(first_char) = chars.first_mut() {
                *first_char = first_char.to_uppercase().next().unwrap_or(*first_char);
            }
            chars.into_iter().collect::<String>()
        })
        .collect::<String>();
    out.write(&pascal_case)?;
    Ok(())
}

fn adapter_package_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let adapter_name = param.value().as_str().unwrap_or("");

    let package = match adapter_name.to_lowercase().as_str() {
        "onebot v11" | "onebot v12" => "nonebot-adapter-onebot",
        "console" => "nonebot-adapter-console",
        "telegram" => "nonebot-adapter-telegram",
        "discord" => "nonebot-adapter-discord",
        _ => "nonebot-adapter-unknown",
    };

    out.write(package)?;
    Ok(())
}

fn module_name_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let adapter_name = param.value().as_str().unwrap_or("");

    let module = match adapter_name.to_lowercase().as_str() {
        "onebot v11" => "nonebot.adapters.onebot.v11",
        "onebot v12" => "nonebot.adapters.onebot.v12",
        "console" => "nonebot.adapters.console",
        "telegram" => "nonebot.adapters.telegram",
        "discord" => "nonebot.adapters.discord",
        _ => "nonebot.adapters.unknown",
    };

    out.write(module)?;
    Ok(())
}

fn create_project_structure(base_dir: &Path) -> Result<()> {
    let dirs = vec![
        base_dir.join("plugins"),
        base_dir.join("data"),
        base_dir.join("resources"),
        base_dir.join("tests"),
    ];

    for dir in dirs {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
    }

    Ok(())
}

fn generate_bot_file(
    handlebars: &Handlebars,
    data: &HashMap<&str, &dyn erased_serde::Serialize>,
    output_dir: &Path,
) -> Result<()> {
    let content = handlebars.render("bot.py", data)?;
    fs::write(output_dir.join("bot.py"), content)?;
    Ok(())
}

fn generate_pyproject_file(
    handlebars: &Handlebars,
    data: &HashMap<&str, &dyn erased_serde::Serialize>,
    output_dir: &Path,
) -> Result<()> {
    let content = handlebars.render("pyproject.toml", data)?;
    fs::write(output_dir.join("pyproject.toml"), content)?;
    Ok(())
}

fn generate_env_files(
    _handlebars: &Handlebars,
    _data: &HashMap<&str, &dyn erased_serde::Serialize>,
    output_dir: &Path,
) -> Result<()> {
    let env_dev = r#"ENVIRONMENT=dev
LOG_LEVEL=DEBUG

# Driver
DRIVER=~httpx+~websockets

# Adapter configurations
# OneBot V11
# ONEBOT_ACCESS_TOKEN=
# ONEBOT_SECRET=

# Superusers
SUPERUSERS=["123456789"]

# Command prefix
COMMAND_PREFIX=[""]
NICKNAME=["Bot"]

# API settings
HOST=127.0.0.1
PORT=8080
"#;

    let env_prod = r#"ENVIRONMENT=prod
LOG_LEVEL=INFO

# Driver
DRIVER=~httpx+~websockets

# Adapter configurations
# OneBot V11
# ONEBOT_ACCESS_TOKEN=your_token_here
# ONEBOT_SECRET=your_secret_here

# Superusers (replace with actual user IDs)
SUPERUSERS=["123456789"]

# Command prefix
COMMAND_PREFIX=[""]
NICKNAME=["Bot"]

# API settings
HOST=0.0.0.0
PORT=8080
"#;

    fs::write(output_dir.join(".env"), env_dev)?;
    fs::write(output_dir.join(".env.prod"), env_prod)?;

    Ok(())
}

fn generate_readme_file(
    _handlebars: &Handlebars,
    data: &HashMap<&str, &dyn erased_serde::Serialize>,
    output_dir: &Path,
) -> Result<()> {
    let project_name = data
        .get("project_name")
        .and_then(|v| serde_json::to_string(v).ok())
        .unwrap_or("nb-bot-project".to_string());

    let readme = format!(
        r#"# {}

基于 NoneBot2 的聊天机器人

## 快速开始

1. 安装依赖
```bash
uv sync
# 或者
uv pip install -r requirements.txt
```

2. 配置机器人
复制 `.env.prod` 到 `.env` 并编辑配置文件，填入你的机器人连接信息。

3. 运行机器人
```bash
nb run
```

## 配置说明

### 环境变量

- `ENVIRONMENT`: 运行环境 (dev/prod)
- `LOG_LEVEL`: 日志级别 (DEBUG/INFO/WARNING/ERROR)
- `DRIVER`: 驱动器配置
- `SUPERUSERS`: 超级用户列表
- `COMMAND_PREFIX`: 命令前缀

### 适配器配置

请根据你使用的平台配置相应的适配器参数。

## 开发

### 添加插件

- 在 `plugins/` 目录下添加你的插件文件
- 或者使用 `nb plugin install <plugin_name>` 安装插件

### 项目结构

```
{}/
├── bot.py              # 机器人入口文件
├── pyproject.toml      # 项目配置文件
├── .env               # 开发环境配置
├── .env.prod          # 生产环境配置
├── plugins/           # 插件目录
├── data/             # 数据目录
├── resources/        # 资源文件目录
└── tests/            # 测试文件目录
```

## 部署

### 使用 Docker

```bash
docker build -t {} .
docker run -d --name {} -p 8080:8080 {}
```

### 使用 systemd (Linux)

1. 复制 `nb-bot.service` 到 `/etc/systemd/system/`
2. 启动服务：`sudo systemctl start nb-bot`
3. 设置开机自启：`sudo systemctl enable nb-bot`

## 许可证

MIT License
"#,
        project_name, project_name, project_name, project_name, project_name
    );

    fs::write(output_dir.join("README.md"), readme)?;
    Ok(())
}

fn generate_gitignore(output_dir: &Path) -> Result<()> {
    let gitignore = r#"# Byte-compiled / optimized / DLL files
__pycache__/
*.py[cod]
*$py.class

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

# Unit test / coverage reports
htmlcov/
.tox/
.coverage
.coverage.*
.cache
nosetests.xml
coverage.xml
*.cover
.hypothesis/
.pytest_cache/

# Environment variables
.env
.env.local
.env.prod

# IDEs
.vscode/
.idea/
*.swp
*.swo
*~

# Logs
*.log
logs/

# Database
*.db
*.sqlite
*.sqlite3

# NoneBot
data/
resources/temp/

# macOS
.DS_Store

# Windows
Thumbs.db
ehthumbs.db

# Poetry
poetry.lock

# Temporary files
*.tmp
*.temp
"#;

    fs::write(output_dir.join(".gitignore"), gitignore)?;
    Ok(())
}

fn generate_dockerfile(
    _handlebars: &Handlebars,
    data: &HashMap<&str, &dyn erased_serde::Serialize>,
    output_dir: &Path,
) -> Result<()> {
    let project_name = data
        .get("project_name")
        .and_then(|v| serde_json::to_string(v).ok())
        .unwrap_or("nb-bot-project".to_string());

    let dockerfile = format!(
        r#"FROM python:3.11-slim

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements
COPY pyproject.toml ./
COPY poetry.lock* ./

# Install Python dependencies
RUN pip install uv && \
    uv sync --no-dev

# Copy application
COPY . .

# Create non-root user
RUN useradd --create-home --shell /bin/bash {}
RUN chown -R {}:{} /app
USER {}

EXPOSE 8080

CMD ["python", "bot.py"]
"#,
        project_name, project_name, project_name, project_name
    );

    fs::write(output_dir.join("Dockerfile"), dockerfile)?;
    Ok(())
}

fn create_example_plugin(output_dir: &Path) -> Result<()> {
    let plugins_dir = output_dir.join("plugins");

    let hello_plugin = r#"from nonebot import on_command
from nonebot.adapters import Message
from nonebot.params import CommandArg
from nonebot.plugin import PluginMetadata

__plugin_meta__ = PluginMetadata(
    name="Hello Plugin",
    description="A simple hello plugin",
    usage="Send 'hello' to get a greeting",
)

hello = on_command("hello", aliases={"hi"}, priority=10, block=True)

@hello.handle()
async def hello_handler(args: Message = CommandArg()):
    msg = args.extract_plain_text()
    if msg:
        await hello.finish(f"Hello, {msg}!")
    else:
        await hello.finish("Hello, World!")
"#;

    fs::write(plugins_dir.join("hello.py"), hello_plugin)?;
    Ok(())
}

fn create_advanced_examples(output_dir: &Path) -> Result<()> {
    let plugins_dir = output_dir.join("plugins");

    // Create a weather plugin example
    let weather_plugin = r#"from nonebot import on_command
from nonebot.adapters import Message
from nonebot.params import CommandArg
from nonebot.plugin import PluginMetadata
import httpx

__plugin_meta__ = PluginMetadata(
    name="Weather Plugin",
    description="Get weather information",
    usage="Send 'weather <city>' to get weather info",
)

weather = on_command("weather", priority=10, block=True)

@weather.handle()
async def weather_handler(args: Message = CommandArg()):
    city = args.extract_plain_text().strip()
    if not city:
        await weather.finish("请输入城市名称！")

    try:
        # This is just an example - you'd need to use a real weather API
        async with httpx.AsyncClient() as client:
            # Replace with actual weather API
            await weather.finish(f"{city} 的天气信息：\n晴，25°C\n（这是示例数据）")
    except Exception as e:
        await weather.finish(f"获取天气信息失败：{e}")
"#;

    fs::write(plugins_dir.join("weather.py"), weather_plugin)?;

    // Create a status plugin example
    let status_plugin = r#"from nonebot import on_command
from nonebot.plugin import PluginMetadata
import psutil
import time

__plugin_meta__ = PluginMetadata(
    name="Status Plugin",
    description="Show bot status information",
    usage="Send 'status' to get bot status",
)

status = on_command("status", priority=10, block=True)

@status.handle()
async def status_handler():
    # Get system information
    cpu_percent = psutil.cpu_percent(interval=1)
    memory = psutil.virtual_memory()
    disk = psutil.disk_usage('/')

    uptime = time.time() - psutil.boot_time()
    uptime_hours = uptime // 3600
    uptime_minutes = (uptime % 3600) // 60

    status_msg = f"""🤖 Bot Status:

📊 System Info:
• CPU: {cpu_percent}%
• Memory: {memory.percent}% ({memory.used // 1024 // 1024}MB / {memory.total // 1024 // 1024}MB)
• Disk: {disk.percent}% ({disk.used // 1024 // 1024 // 1024}GB / {disk.total // 1024 // 1024 // 1024}GB)

⏰ Uptime: {uptime_hours:.0f}h {uptime_minutes:.0f}m
"""

    await status.finish(status_msg)
"#;

    fs::write(plugins_dir.join("status.py"), status_plugin)?;
    Ok(())
}

async fn show_setup_instructions(options: &ProjectOptions) -> Result<()> {
    println!("\n{}", "📋 Setup Instructions:".bright_yellow());
    println!("1. Configure your bot in the .env file");
    if !options.adapters.is_empty() {
        println!("2. Set up your adapters:");
        for adapter in &options.adapters {
            match adapter.as_str() {
                "OneBot V11" => {
                    println!("   • OneBot V11: Configure ONEBOT_ACCESS_TOKEN and ONEBOT_SECRET");
                }
                "Console" => {
                    println!("   • Console: Ready to use for testing");
                }
                "Telegram" => {
                    println!("   • Telegram: Configure TELEGRAM_BOT_TOKEN");
                }
                _ => {
                    println!(
                        "   • {}: Check adapter documentation for configuration",
                        adapter
                    );
                }
            }
        }
    }
    if !options.plugins.is_empty() {
        println!("3. Installed plugins: {}", options.plugins.join(", "));
    }
    println!("4. Run 'nb run' to start your bot");
    println!(
        "\n{}",
        "💡 Need help? Check the README.md file for more information.".bright_blue()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_bootstrap_project() {
        let temp_dir = TempDir::new().unwrap();
        let options = ProjectOptions {
            name: "test-bot".to_string(),
            template: "bootstrap".to_string(),
            output_dir: temp_dir.path().to_path_buf(),
            force: true,
            adapters: vec!["OneBot V11".to_string()],
            plugins: vec![],
        };

        create_bootstrap_project(&options).await.unwrap();

        // Check if essential files were created
        assert!(temp_dir.path().join("bot.py").exists());
        assert!(temp_dir.path().join("pyproject.toml").exists());
        assert!(temp_dir.path().join(".env").exists());
        assert!(temp_dir.path().join(".gitignore").exists());
    }
}
