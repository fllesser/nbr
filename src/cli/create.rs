use anyhow::Context;
use clap::ArgMatches;
use colored::*;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info};

use crate::cli::adapter::{AdapterManager, RegistryAdapter};

use crate::config::NbConfig;
use crate::error::{NbrError, Result};
use crate::pyproject::{Adapter, Nonebot, PyProjectConfig, Tool, ToolNonebot};
use crate::uv;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Template {
    Bootstrap,
    Simple,
}

impl Template {
    pub fn description(&self) -> &str {
        match self {
            Template::Bootstrap => "bootstrap - Basic NoneBot project template",
            Template::Simple => "simple - Simple bot template with basic plugins",
        }
    }
}

impl Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Template::Bootstrap => write!(f, "bootstrap"),
            Template::Simple => write!(f, "simple"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOptions {
    pub name: String,
    pub template: Template,
    pub output_dir: PathBuf,
    pub force: bool,
    pub drivers: Vec<String>,
    pub adapters: Vec<RegistryAdapter>,
    pub plugins: Vec<String>,
    pub python_version: String,
    pub environment: String,
}

pub async fn handle_create(matches: &ArgMatches) -> Result<()> {
    info!("🎉 Creating NoneBot project...");

    let adapter_manager = AdapterManager::default();

    // 补齐项目选项
    let options = gather_project_options(matches, &adapter_manager).await?;

    // Check if directory already exists
    if options.output_dir.exists() && !options.force {
        let should_continue = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Directory '{}' already exists. Continue?",
                options.output_dir.display()
            ))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(e.to_string()))?;

        if !should_continue {
            error!("{}", "Create operation cancelled.");
            return Ok(());
        }
    }

    // Create the project
    create_project(&options).await?;

    info!("\n✨ Project created successfully !");
    info!("🚀 Next steps:");
    info!("     {}", format!("cd {}", options.name));
    info!("     {}", "nbr run");
    // Show additional setup instructions
    // show_setup_instructions(&options).await?;

    Ok(())
}

async fn gather_project_options(
    matches: &ArgMatches,
    adapter_manager: &AdapterManager,
) -> Result<ProjectOptions> {
    let name = if let Some(name) = matches.get_one::<String>("name") {
        name.to_owned()
    } else {
        Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Project name")
            .default("awesome-bot".to_string())
            .validate_with(|input: &String| -> Result<()> {
                if input.contains(" ") {
                    Err(NbrError::invalid_argument(
                        "Project name cannot contain spaces".to_string(),
                    ))
                } else {
                    Ok(())
                }
            })
            .interact_text()
            .context("Failed to get project name")?
    };

    // 选择模板
    let template = select_template()?;
    // 选择 Bot 创建目录，默认在当前目录下创建
    let output_dir: PathBuf = matches
        .get_one::<String>("output")
        .map(|s| PathBuf::from(s))
        .unwrap_or(std::env::current_dir()?.join(&name));
    // 是否强制创建
    let force = matches.get_flag("force");
    // 选择驱动
    let drivers = select_drivers()?;
    // 指定 Python 版本
    let python_version = matches.get_one::<String>("python").unwrap().to_owned();
    // 选择适配器
    let adapters = adapter_manager.select_adapter().await?;
    // 选择内置插件
    let plugins = select_builtin_plugins()?;
    // 选择环境类型
    let environment = select_environment()?;

    Ok(ProjectOptions {
        name,
        template,
        output_dir,
        force,
        drivers,
        adapters,
        plugins,
        python_version,
        environment,
    })
}

fn select_environment() -> Result<String> {
    let env_types = vec!["dev", "prod"];

    let selected_env_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which environment")
        .items(&env_types)
        .default(0)
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;
    Ok(env_types[selected_env_type].to_string())
}

fn select_drivers() -> Result<Vec<String>> {
    let drivers = vec!["FastAPI", "HTTPX", "websockets", "Quark", "AIOHTTP"];
    let selected_drivers = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which driver(s) would you like to use")
        .items(&drivers)
        // 默认选择前三个
        .defaults(&vec![true; 3])
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;

    let selected_drivers: Vec<String> = selected_drivers
        .into_iter()
        .map(|i| drivers[i].to_string())
        .collect();

    if selected_drivers.is_empty() {
        return select_drivers();
    }

    Ok(selected_drivers)
}

fn select_template() -> Result<Template> {
    let template_descriptions: Vec<String> = vec![
        Template::Bootstrap.description().to_string(),
        Template::Simple.description().to_string(),
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a template")
        .default(0)
        .items(&template_descriptions)
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;

    match selection {
        0 => Ok(Template::Bootstrap),
        1 => Ok(Template::Simple),
        _ => unreachable!(),
    }
}

// 选择内置插件
fn select_builtin_plugins() -> Result<Vec<String>> {
    let builtin_plugins = vec!["echo", "single_session"];
    let selected_plugins = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select builtin plugins")
        .items(&builtin_plugins)
        .defaults(&vec![true; builtin_plugins.len().min(1)])
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?
        .into_iter()
        .map(|i| builtin_plugins[i].to_string())
        .collect();
    Ok(selected_plugins)
}

async fn create_project(options: &ProjectOptions) -> Result<()> {
    fs::create_dir_all(&options.output_dir).context("Failed to create output directory")?;

    match options.template {
        Template::Bootstrap => create_bootstrap_project(options).await?,
        Template::Simple => create_simple_project(options).await?,
    }

    Ok(())
}

async fn create_bootstrap_project(options: &ProjectOptions) -> Result<()> {
    let package_name = options.name.replace("-", "_");
    // Create structure
    create_project_structure(&options.output_dir, &package_name)?;
    generate_pyproject_file(options)?;
    generate_env_files(&options)?;
    generate_readme_file(options)?;
    generate_gitignore(&options.output_dir)?;

    // Install dependencies
    uv::sync(Some(&options.python_version))
        .working_dir(&options.output_dir)
        .run()?;
    Ok(())
}

#[allow(unused)]
fn generate_nb_config_file(options: &ProjectOptions) -> Result<()> {
    let nb_config = NbConfig {
        tool: Tool {
            nonebot: Nonebot {
                adapters: options
                    .adapters
                    .iter()
                    .map(|a| Adapter {
                        name: a.name.clone(),
                        module_name: a.module_name.clone(),
                    })
                    .collect(),
                plugins: vec![],
                plugin_dirs: vec![format!("src/plugins")],
                builtin_plugins: options.plugins.clone(),
            },
        },
    };
    fs::write(
        options.output_dir.join("nb.toml"),
        toml::to_string_pretty(&nb_config)?,
    )?;
    Ok(())
}

async fn create_simple_project(options: &ProjectOptions) -> Result<()> {
    // Start with bootstrap template
    create_bootstrap_project(options).await?;
    // Add example plugin
    create_example_plugin(&options.output_dir)?;

    Ok(())
}

fn create_project_structure(base_dir: &Path, module_name: &str) -> Result<()> {
    let dirs = vec![
        base_dir.join("src/plugins"),
        base_dir.join(format!("src/{}", module_name)),
    ];

    for dir in dirs {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
    }
    fs::write(
        base_dir.join(format!("src/{}/__init__.py", module_name)),
        "",
    )?;
    Ok(())
}

fn generate_pyproject_file(options: &ProjectOptions) -> Result<()> {
    let mut pyproject = PyProjectConfig::default();
    pyproject.project.name = options.name.to_string();

    // 补齐驱动依赖
    let drivers = options.drivers.join(",").to_string().to_lowercase();
    pyproject
        .project
        .dependencies
        .push(format!("nonebot2[{}]>=2.4.3", drivers));

    let adapter_deps = options
        .adapters
        .iter()
        .map(|a| format!("{}>={}", a.project_link, a.version))
        .collect::<HashSet<String>>(); // 沟槽的 onebot 12
    // 补齐依赖
    pyproject.project.dependencies.extend(adapter_deps);

    // 补齐 tool.nonebot
    pyproject.tool.nonebot.plugin_dirs = vec![format!("src/plugins")];
    pyproject.tool.nonebot.builtin_plugins = options.plugins.clone();

    // 写入文件
    let content = toml::to_string(&pyproject)?;
    fs::write(options.output_dir.join("pyproject.toml"), content)?;

    let adapters = options
        .adapters
        .iter()
        .map(|a| Adapter {
            name: a.name.clone(),
            module_name: a.module_name.clone(),
        })
        .collect();

    ToolNonebot::parse(Some(&options.output_dir))?.add_adapters(adapters)?;
    Ok(())
}

fn generate_env_files(options: &ProjectOptions) -> Result<()> {
    let driver = options
        .drivers
        .iter()
        .map(|d| format!("~{}", d.to_lowercase()))
        .collect::<Vec<String>>()
        .join("+");
    let log_level = match options.environment.as_str() {
        "dev" => "DEBUG",
        "prod" => "INFO",
        _ => unreachable!(),
    };
    let file_name = format!(".env.{}", options.environment);
    let env_content = format!(
        include_str!("templates/env_template"),
        driver, log_level, options.name,
    );
    fs::write(
        options.output_dir.join(".env"),
        format!("ENVIRONMENT={}", options.environment),
    )?;
    fs::write(options.output_dir.join(file_name), env_content)?;

    Ok(())
}

fn generate_readme_file(options: &ProjectOptions) -> Result<()> {
    let project_name = options.name.clone();

    let readme = format!(
        include_str!("templates/readme_template"),
        project_name, project_name, project_name, project_name, project_name
    );

    fs::write(options.output_dir.join("README.md"), readme)?;
    Ok(())
}

fn generate_gitignore(output_dir: &Path) -> Result<()> {
    let gitignore = include_str!("templates/gitignore");

    fs::write(output_dir.join(".gitignore"), gitignore)?;
    Ok(())
}

fn create_example_plugin(output_dir: &Path) -> Result<()> {
    let plugins_dir = output_dir.join("src/plugins");

    let hello_plugin = include_str!("templates/hello.py");

    fs::write(plugins_dir.join("hello.py"), hello_plugin)?;
    Ok(())
}

#[allow(unused)]
async fn show_setup_instructions(options: &ProjectOptions) -> Result<()> {
    println!("\n{}", "📋 Setup Instructions:".bright_yellow());
    println!("1. Configure your bot in the .env file");
    if !options.adapters.is_empty() {
        println!("2. Set up your adapters:");
        for adapter in &options.adapters {
            println!("   • {}: Configure {}", adapter.name, adapter.project_link);
        }
    }
    println!("3. Run 'nb run' to start your bot");
    println!(
        "\n{}",
        "💡 Need help? Check the README.md file for more information.".cyan()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_env_files() {
        let options = ProjectOptions {
            name: "awesome-bot".to_string(),
            template: Template::Bootstrap,
            output_dir: PathBuf::from("awesome-bot"),
            force: false,
            drivers: vec![
                "FastAPI".to_string(),
                "HTTPX".to_string(),
                "webosockets".to_string(),
            ],
            adapters: vec![],
            plugins: vec![],
            python_version: "3.10".to_string(),
            environment: "dev".to_string(),
        };
        generate_env_files(&options).unwrap();
    }
}
