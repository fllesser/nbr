use anyhow::Context;
use clap::ArgMatches;
use colored::*;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{error, info};

use crate::cli::adapter::{AdapterManager, RegistryAdapter};

use crate::error::{NbrError, Result};
use crate::pyproject::{Adapter, NbTomlEditor, PyProjectConfig};
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
    pub dev_tools: Vec<String>,
}

pub async fn handle_create(matches: &ArgMatches) -> Result<()> {
    info!("🎉 Creating NoneBot project...");

    let adapter_manager = AdapterManager::default();

    // 补齐项目选项
    let options = gather_project_options(matches, &adapter_manager).await?;

    // Check if directory already exists
    if !check_directory_exists(&options)? {
        return Ok(());
    }

    // Create the project
    create_project(&options).await?;

    info!("\n✨ Project created successfully !");
    info!("🚀 Next steps:\n");
    info!("     {}", format!("cd {}", options.name));
    info!("     {}", "nbr run\n");
    // Show additional setup instructions
    // show_setup_instructions(&options).await?;

    Ok(())
}

fn check_directory_exists(options: &ProjectOptions) -> Result<bool> {
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
            return Ok(false);
        }
    }
    Ok(true)
}

async fn gather_project_options(
    matches: &ArgMatches,
    adapter_manager: &AdapterManager,
) -> Result<ProjectOptions> {
    let name = matches
        .get_one::<String>("name")
        .map(|s| s.to_owned())
        .unwrap_or(input_project_name()?);
    // 选择模板
    let template = select_template()?;
    // 选择 Bot 创建目录，默认在当前目录下创建
    let output_dir: PathBuf = matches
        .get_one::<String>("output")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?.join(&name));
    // 是否强制创建
    let force = matches.get_flag("force");
    // 选择驱动
    let drivers = select_drivers()?;
    // 指定 Python 版本
    let python_version = matches
        .get_one::<String>("python")
        .map(|s| s.to_owned())
        .unwrap_or(select_python_version()?);
    // 选择适配器
    let adapters = adapter_manager.select_adapters(false).await?;
    // 选择内置插件
    let plugins = select_builtin_plugins()?;
    // 选择环境类型
    let environment = select_environment()?;
    // 选择开发工具
    let dev_tools = select_dev_tools()?;

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
        dev_tools,
    })
}

fn input_project_name() -> Result<String> {
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
        .context("Failed to get project name")
        .map_err(|e| NbrError::io(e.to_string()))
}

fn select_environment() -> Result<String> {
    let env_types = vec!["dev", "prod"];

    let selected_env_type = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which environment are you in now")
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
        .defaults(&[true; 3])
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

fn select_python_version() -> Result<String> {
    let python_versions = vec!["3.10", "3.11", "3.12", "3.13", "3.14"];
    let selected_python_version = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which Python version would you like to use")
        .items(&python_versions)
        .default(0)
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;
    Ok(python_versions[selected_python_version].to_string())
}

fn select_dev_tools() -> Result<Vec<String>> {
    let dev_tools = vec!["ruff", "basedpyright", "pre-commit", "pylance"];
    let selected_dev_tools = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which dev tool(s) would you like to use")
        .items(&dev_tools)
        .defaults(&[true; 3])
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;
    let selected_dev_tools: Vec<String> = selected_dev_tools
        .into_iter()
        .map(|i| dev_tools[i].to_string())
        .collect();
    Ok(selected_dev_tools)
}

// 选择内置插件
fn select_builtin_plugins() -> Result<Vec<String>> {
    let builtin_plugins = vec!["echo", "single_session"];
    let selected_plugins = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which builtin plugin(s) would you like to use")
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
    generate_env_files(options)?;
    generate_readme_file(options)?;
    generate_gitignore(&options.output_dir)?;

    if options.dev_tools.contains(&"ruff".to_string()) {
        append_ruff_config(&options.output_dir)?;
    }
    if options.dev_tools.contains(&"basedpyright".to_string()) {
        append_pyright_config(&options.output_dir)?;
    }
    if options.dev_tools.contains(&"pre-commit".to_string()) {
        generate_pre_commit_config(&options.output_dir)?;
    }

    // Install dependencies
    uv::sync(Some(&options.python_version))
        .working_dir(&options.output_dir)
        .run()?;
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
    let nonebot_mut = pyproject.tool.as_mut().unwrap().nonebot.as_mut().unwrap();
    nonebot_mut.plugin_dirs = Some(vec![format!("src/plugins")]);
    nonebot_mut.builtin_plugins = Some(options.plugins.clone());

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

    NbTomlEditor::parse(Some(&options.output_dir))?.add_adapters(adapters)?;
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
        include_str!("templates/env"),
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
        include_str!("templates/readme"),
        project_name, project_name, project_name, project_name, project_name
    );

    fs::write(options.output_dir.join("README.md"), readme)?;
    Ok(())
}

fn generate_pre_commit_config(output_dir: &Path) -> Result<()> {
    let pre_commit_config = include_str!("templates/pre_commit_config");
    fs::write(
        output_dir.join(".pre-commit-config.yaml"),
        pre_commit_config,
    )?;
    Ok(())
}

fn generate_gitignore(output_dir: &Path) -> Result<()> {
    let gitignore = include_str!("templates/gitignore");

    fs::write(output_dir.join(".gitignore"), gitignore)?;
    Ok(())
}

fn append_ruff_config(output_dir: &Path) -> Result<()> {
    let content = include_str!("templates/pyproject/tool_ruff");
    append_content_to_pyproject(output_dir, content)?;
    Ok(())
}

fn append_pyright_config(output_dir: &Path) -> Result<()> {
    let content = include_str!("templates/pyproject/tool_pyright");
    append_content_to_pyproject(output_dir, content)?;
    Ok(())
}

fn append_content_to_pyproject(output_dir: &Path, content: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true) // 设置为追加模式
        .create(true) // 如果文件不存在则创建
        .open(output_dir.join("pyproject.toml"))?;
    file.write_all(format!("\n{}", content).as_bytes())?;
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
            dev_tools: vec![],
        };
        generate_env_files(&options).unwrap();
    }

    #[test]
    fn test_generate_ruff_config() {
        append_ruff_config(&PathBuf::from("awesome-bot")).unwrap();
    }
}
