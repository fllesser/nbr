use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use strum::Display;
use tracing::info;

use super::adapter::{AdapterManager, RegistryAdapter};
use super::common;
use super::docker;
use crate::error::Error;
use crate::pyproject::{
    BuildSystem, DependencyGroupItem, DependencyGroups, NbTomlEditor, Nonebot, Project,
    PyProjectConfig, Tool,
};
use crate::uv;

#[derive(ValueEnum, Clone, Debug)]
#[clap(rename_all = "lowercase")]
pub enum Template {
    #[clap(help = "Basic NoneBot project template")]
    Bootstrap,
    #[clap(help = "Simple bot template with basic plugins")]
    Simple,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
pub enum Driver {
    FastAPI,
    HTTPX,
    WebSockets,
    Quark,
    AIOHTTP,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Environment {
    Dev,
    Prod,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "kebab-case")]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinPlugin {
    Echo,
    SingleSession,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum DevTool {
    Ruff,
    Basedpyright,
    PreCommit,
}

impl DevTool {
    pub fn to_dependency(&self) -> String {
        match self {
            Self::Ruff => "ruff>=0.14.8".to_string(),
            Self::Basedpyright => "basedpyright>=1.35.0".to_string(),
            Self::PreCommit => "pre-commit>=4.3.0".to_string(),
        }
    }
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    #[clap()]
    name: Option<String>,
    #[clap(short, long, value_enum)]
    template: Option<Template>,
    #[clap(short, long)]
    output: Option<String>,
    #[clap(short, long)]
    force: bool,
    #[clap(short, long)]
    python: Option<String>,
    #[clap(long, value_enum, num_args = 1.., value_delimiter = ',')]
    drivers: Option<Vec<Driver>>,
    #[clap(short, long, num_args = 0.., value_delimiter = ',')]
    adapters: Option<Vec<String>>,
    #[clap(long, value_enum, num_args = 0.., value_delimiter = ',')]
    plugins: Option<Vec<BuiltinPlugin>>,
    #[clap(short, long, value_enum)]
    env: Option<Environment>,
    #[clap(long, value_enum, num_args = 0.., value_delimiter = ',')]
    dev_tools: Option<Vec<DevTool>>,
    #[clap(long, help = "Generate Dockerfile")]
    gen_dockerfile: Option<bool>,
    #[clap(long, help = "Create virtual environment now")]
    create_venv: Option<bool>,
}

pub struct ProjectOptions {
    pub name: String,
    pub template: Template,
    pub output_dir: PathBuf,
    pub drivers: Vec<String>,
    pub adapters: Vec<RegistryAdapter>,
    pub plugins: Vec<String>,
    pub python_version: String,
    pub environment: Environment,
    pub dev_tools: Vec<DevTool>,
    pub gen_dockerfile: bool,
    pub create_venv: bool,
}

pub async fn handle(args: CreateArgs) -> Result<()> {
    info!("🎉 Creating NoneBot project...");
    let adapter_manager = AdapterManager::default();
    // 补齐项目参数
    let options = gather_project_options(args, &adapter_manager).await?;
    // Create the project
    create_project(&options).await?;
    info!("\n✨ Project created successfully !");
    info!("🚀 Next steps:\n");
    info!("     {}", format!("cd {}", options.name));
    info!("     {}", "nbr run\n");
    Ok(())
}

fn check_directory_exists(output_dir: &Path) -> Result<()> {
    if output_dir.exists() {
        let should_continue = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "Directory '{}' already exists. Continue?",
                output_dir.display()
            ))
            .default(false)
            .interact()?;

        if !should_continue {
            return Err(Error::Cancelled.into());
        }
    }
    Ok(())
}

/// Confirm whether to generate Dockerfile and Docker Compose configuration
fn confirm_gen_docker() -> Result<bool> {
    let gen_dockerfile = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to generate Dockerfile and Docker Compose configuration?")
        .default(true)
        .interact()?;
    Ok(gen_dockerfile)
}

/// Confirm whether to create a virtual environment now
fn confirm_create_venv() -> Result<bool> {
    let create_venv = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to create a virtual environment now?")
        .default(true)
        .interact()?;
    Ok(create_venv)
}

async fn gather_project_options(
    args: CreateArgs,
    adapter_manager: &AdapterManager,
) -> Result<ProjectOptions> {
    let name = match args.name.clone() {
        Some(name) => name,
        None => input_project_name()?,
    };

    let output_dir = args
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&name).to_path_buf());

    if !args.force {
        // 如果 output_dir 已经存在，则提示用户是否继续
        check_directory_exists(&output_dir)?;
    }
    // 指定 Python 版本
    let python_version = match args.python {
        Some(version) => version,
        None => common::select_python_version()?,
    };
    // 选择模板
    let template = match args.template {
        Some(template) => template,
        None => select_template()?,
    };
    // 选择驱动
    let drivers = match args.drivers {
        Some(drivers) => drivers.into_iter().map(|d| d.to_string()).collect(),
        None => select_drivers()?,
    };

    let adapters = match args.adapters {
        Some(adapters) => {
            let registry_adapter_map = adapter_manager.fetch_registry_adapters(false).await?;
            adapters
                .into_iter()
                .filter(|a| registry_adapter_map.contains_key(a))
                .map(|a| registry_adapter_map[&a].clone())
                .collect()
        }
        None => adapter_manager
            .select_adapters(false, false)
            .await?
            .into_iter()
            .map(|a| a.to_owned())
            .collect(),
    };

    // 选择内置插件
    let plugins = match args.plugins {
        Some(plugins) => plugins.into_iter().map(|p| p.to_string()).collect(),
        None => select_builtin_plugins()?,
    };
    // 选择环境类型
    let environment = match args.env {
        Some(env) => env,
        None => select_environment()?,
    };
    // 选择开发工具
    let dev_tools = match args.dev_tools {
        Some(dev_tools) => dev_tools,
        None => select_dev_tools()?,
    };
    // 是否生成 Dockerfile
    let gen_dockerfile = match args.gen_dockerfile {
        Some(gen_dockerfile) => gen_dockerfile,
        None => confirm_gen_docker()?,
    };
    // 是否创建虚拟环境
    let create_venv = match args.create_venv {
        Some(create_venv) => create_venv,
        None => confirm_create_venv()?,
    };

    Ok(ProjectOptions {
        name,
        template,
        output_dir,
        drivers,
        adapters,
        plugins,
        python_version,
        environment,
        dev_tools,
        gen_dockerfile,
        create_venv,
    })
}

fn input_project_name() -> anyhow::Result<String> {
    Input::<String>::with_theme(&ColorfulTheme::default())
        .with_prompt("Project name")
        .default("awesome-bot".to_string())
        .validate_with(|input: &String| -> Result<()> {
            if input.contains(" ") {
                anyhow::bail!("Project name cannot contain spaces")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .context("Failed to get project name")
}

fn select_environment() -> Result<Environment> {
    let envs = Environment::value_variants();

    let selected_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Which environment are you in now")
        .items(envs)
        .default(0)
        .interact()?;
    Ok(envs[selected_idx].clone())
}

fn select_drivers() -> Result<Vec<String>> {
    let drivers = Driver::value_variants();
    let selected_drivers = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which driver(s) would you like to use")
        .items(drivers)
        // 默认选择前三个
        .defaults(&[true; 3])
        .interact()?;

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
    let template_prompts = vec![
        "bootstrap - Basic NoneBot project template",
        "simple - Simple bot template with basic plugins",
    ];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a template")
        .default(0)
        .items(&template_prompts)
        .interact()?;

    match selection {
        0 => Ok(Template::Bootstrap),
        1 => Ok(Template::Simple),
        _ => unreachable!(),
    }
}

fn select_dev_tools() -> Result<Vec<DevTool>> {
    let dev_tools = DevTool::value_variants();
    let selected_dev_tools = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which dev tool(s) would you like to use")
        .items(dev_tools)
        .defaults(&[true; 3])
        .interact()?;
    let selected_dev_tools = selected_dev_tools
        .into_iter()
        .map(|i| dev_tools[i].to_owned())
        .collect();
    Ok(selected_dev_tools)
}

// 选择内置插件
fn select_builtin_plugins() -> Result<Vec<String>> {
    let builtin_plugins = BuiltinPlugin::value_variants();
    let selected_plugins = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which builtin plugin(s) would you like to use")
        .items(builtin_plugins)
        .defaults(&vec![true; builtin_plugins.len().min(1)])
        .interact()?
        .into_iter()
        .map(|i| builtin_plugins[i].to_string())
        .collect();
    Ok(selected_plugins)
}

pub async fn create_project(options: &ProjectOptions) -> Result<()> {
    fs::create_dir_all(&options.output_dir).context("Failed to create output directory")?;

    match options.template {
        Template::Bootstrap => create_bootstrap_project(options).await?,
        Template::Simple => create_simple_project(options).await?,
    }

    Ok(())
}

async fn create_bootstrap_project(options: &ProjectOptions) -> Result<()> {
    create_project_structure(options)?;
    create_pyporject_config(options)?;
    create_env_files(options)?;
    create_readme_file(options)?;
    create_gitignore(&options.output_dir)?;
    create_dev_tools_config(options)?;
    create_dockerfile(options)?;
    install_dependencies(options)?;
    Ok(())
}

fn install_dependencies(options: &ProjectOptions) -> Result<()> {
    if options.create_venv {
        uv::sync(Some(&options.python_version))
            .working_dir(&options.output_dir)
            .run()?;
    }
    Ok(())
}

async fn create_simple_project(options: &ProjectOptions) -> Result<()> {
    // Start with bootstrap template
    create_bootstrap_project(options).await?;
    // Add example plugin
    create_example_plugin(&options.output_dir)?;

    Ok(())
}

fn create_project_structure(options: &ProjectOptions) -> Result<()> {
    let base_dir = &options.output_dir;
    let module_name = options.name.replace("-", "_");

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

fn collect_dependencies(options: &ProjectOptions) -> Vec<String> {
    // 补齐驱动依赖
    let mut dependencies = vec![];
    let drivers = options.drivers.join(",").to_string().to_lowercase();
    dependencies.push(format!("nonebot2[{}]>=2.4.3", drivers));

    let adapter_deps = options
        .adapters
        .iter()
        .map(|a| format!("{}>={}", a.project_link, a.version))
        .collect::<HashSet<String>>(); // 沟槽的 onebot 12 适配器

    dependencies.extend(adapter_deps);
    dependencies
}

// 收集依赖组
fn collect_dependency_groups(options: &ProjectOptions) -> DependencyGroups {
    let mut dep_groups = DependencyGroups::default();
    let mut dev_deps: Vec<DependencyGroupItem> = options
        .dev_tools
        .iter()
        .map(|t| DependencyGroupItem::String(t.to_dependency()))
        .collect();
    dev_deps.push(DependencyGroupItem::IncludeGroup {
        include_group: "test".to_string(),
    });

    dep_groups.groups.insert(
        "test".to_string(),
        vec![
            DependencyGroupItem::String("nonebug>=0.3.7,<1.0.0".to_string()),
            DependencyGroupItem::String("pytest-asyncio>=1.3.0,<2.0.0".to_string()),
        ],
    );
    dep_groups.groups.insert("dev".to_string(), dev_deps);
    dep_groups
}

fn create_pyporject_config(options: &ProjectOptions) -> Result<()> {
    let pyproject = PyProjectConfig {
        project: Project {
            name: options.name.to_string(),
            version: String::from("0.1.0"),
            description: String::from("a nonebot project"),
            authors: None,
            readme: Some("README.md".to_string()),
            urls: None,
            requires_python: format!(">={}", options.python_version),
            dependencies: collect_dependencies(options),
        },
        dependency_groups: Some(collect_dependency_groups(options)),
        build_system: Some(BuildSystem::default()),
        tool: Some(Tool {
            nonebot: Some(Nonebot {
                builtin_plugins: Some(options.plugins.clone()),
                plugin_dirs: Some(vec![format!("src/plugins")]),
                adapters: Some(vec![]),
                plugins: Some(vec![]),
            }),
        }),
    };
    let content = toml::to_string(&pyproject)?;
    let save_path = options.output_dir.join("pyproject.toml");
    NbTomlEditor::with_str(&content, &save_path)?.add_adapters(
        options
            .adapters
            .iter()
            .map(|a| a.into())
            .collect::<Vec<_>>(),
    )?;
    Ok(())
}

fn create_env_files(options: &ProjectOptions) -> Result<()> {
    let driver = options
        .drivers
        .iter()
        .map(|d| format!("~{}", d.to_lowercase()))
        .collect::<Vec<String>>()
        .join("+");
    let log_level = match options.environment {
        Environment::Dev => "DEBUG",
        Environment::Prod => "INFO",
    };
    let file_name = format!(".env.{}", options.environment);
    let env_content = format!(
        include_str!("templates/.env"),
        driver, log_level, options.name,
    );
    fs::write(
        options.output_dir.join(".env"),
        format!("ENVIRONMENT={}", options.environment),
    )?;
    fs::write(options.output_dir.join(file_name), env_content)?;

    Ok(())
}

fn create_readme_file(options: &ProjectOptions) -> Result<()> {
    let project_name = options.name.clone();

    let readme = format!(
        include_str!("templates/readme"),
        project_name, project_name, project_name, project_name, project_name
    );

    fs::write(options.output_dir.join("README.md"), readme)?;
    Ok(())
}

fn create_dev_tools_config(options: &ProjectOptions) -> Result<()> {
    for tool in options.dev_tools.iter() {
        match tool {
            DevTool::Ruff => append_ruff_config(&options.output_dir)?,
            DevTool::Basedpyright => append_pyright_config(&options.output_dir)?,
            DevTool::PreCommit => create_pre_commit_config(&options.output_dir)?,
        }
    }
    Ok(())
}

fn create_dockerfile(options: &ProjectOptions) -> Result<()> {
    if options.gen_dockerfile {
        docker::create_dockerfile(&options.output_dir)?;
        docker::create_dockerignore(&options.output_dir)?;
        docker::create_python_pin_file(&options.output_dir, &options.python_version)?;
        docker::create_compose_file(&options.output_dir, &options.name)?;
    }
    Ok(())
}

fn create_pre_commit_config(output_dir: &Path) -> Result<()> {
    let pre_commit_config = include_str!("templates/pre_commit_config");
    fs::write(
        output_dir.join(".pre-commit-config.yaml"),
        pre_commit_config,
    )?;
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
    file.write_all(content.as_bytes())?;
    Ok(())
}

fn create_gitignore(output_dir: &Path) -> Result<()> {
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
