use super::GlobalContext;
use super::common;
use crate::adapter::AdapterManager;
use crate::error::Error;
pub use crate::project::{
    BuiltinPlugin, DevTool, Driver, Environment, ProjectOptions, SelectedAdapter, Template,
};
use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use std::path::{Path, PathBuf};
use tracing::info;

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

pub async fn handle(args: CreateArgs, global_context: GlobalContext) -> Result<()> {
    info!("🎉 Creating NoneBot project...");
    let _ = global_context;
    let project = gather_project_options(args).await?;

    // Create the project
    project.create().await?;
    info!("\n✨ Project created successfully !");
    info!("🚀 Next steps:\n");
    info!("     {}", format!("cd {}", project.name));
    info!("     {}", "nbr run\n");
    Ok(())
}

async fn gather_project_options(args: CreateArgs) -> Result<ProjectOptions> {
    let name = args
        .name
        .clone()
        .map(Ok)
        .unwrap_or_else(input_project_name)?;
    let output_dir = args
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&name).to_path_buf());
    if !args.force {
        check_directory_exists(&output_dir)?;
    }

    let python_version = args
        .python
        .map(Ok)
        .unwrap_or_else(common::select_python_version)?;
    let template = args.template.map(Ok).unwrap_or_else(select_template)?;
    let drivers = args
        .drivers
        .map(|drivers| drivers.into_iter().map(|d| d.to_string()).collect())
        .map(Ok)
        .unwrap_or_else(select_drivers)?;
    let adapters = AdapterManager::default()
        .resolve_selected_adapters(args.adapters)
        .await?;
    let plugins = args
        .plugins
        .map(|plugins| plugins.into_iter().map(|p| p.to_string()).collect())
        .map(Ok)
        .unwrap_or_else(select_builtin_plugins)?;
    let environment = args.env.map(Ok).unwrap_or_else(select_environment)?;
    let dev_tools = args.dev_tools.map(Ok).unwrap_or_else(select_dev_tools)?;
    let gen_dockerfile = args
        .gen_dockerfile
        .map(Ok)
        .unwrap_or_else(confirm_gen_docker)?;
    let create_venv = args
        .create_venv
        .map(Ok)
        .unwrap_or_else(confirm_create_venv)?;

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
