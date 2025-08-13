use anyhow::Context;
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use crate::cli::adapter::{AdapterManager, RegistryAdapter};

use crate::config::NbConfig;
use crate::error::{NbrError, Result};
use crate::pyproject::{Adapter, Nonebot, PyProjectConfig, Tool, ToolNonebot};
use crate::utils::terminal_utils;
use crate::uv::Uv;

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    pub description: String,
    pub url: Option<String>,
    pub builtin: bool,
    pub adapters: Vec<String>,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOptions {
    pub name: String,
    pub template: String,
    pub output_dir: PathBuf,
    pub force: bool,
    pub adapters: Vec<RegistryAdapter>,
    pub plugins: Vec<String>,
}

impl ProjectOptions {
    pub fn display(&self) {
        println!("\n{}", "Nonebot project options:".bright_green());
        println!("  name: {}", self.name.bright_blue());
        println!("  template: {}", self.template.bright_blue());
        println!(
            "  output_dir: {}",
            self.output_dir.display().to_string().bright_blue()
        );
        let adapters = self
            .adapters
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>();
        println!("  adapters: {}", adapters.join(", ").bright_blue());
        println!("  plugins: {}", self.plugins.join(", ").bright_blue());
    }
}

pub async fn handle_create(matches: &ArgMatches) -> Result<()> {
    println!("{}", "🎉 Creating NoneBot project...".bright_green());

    let adapter_manager = AdapterManager::new()?;

    let options = gather_project_options(matches, &adapter_manager).await?;

    options.display();

    // Check if directory already exists
    if options.output_dir.exists() && !options.force {
        let should_continue = Confirm::new()
            .with_prompt(format!(
                "Directory '{}' already exists. Continue?",
                options.output_dir.display()
            ))
            .default(false)
            .interact()
            .map_err(|e| NbrError::io(e.to_string()))?;

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
    println!("  nbr run");

    // Show additional setup instructions
    // show_setup_instructions(&options).await?;

    Ok(())
}

async fn gather_project_options(
    matches: &ArgMatches,
    adapter_manager: &AdapterManager,
) -> Result<ProjectOptions> {
    let project_name = if let Some(name) = matches.get_one::<String>("name") {
        name.clone()
    } else {
        Input::<String>::new()
            .with_prompt("Project name")
            .default("awesome-bot".to_string())
            .validate_with(|input: &String| -> Result<()> {
                if input.is_empty() {
                    Err(NbrError::invalid_argument(
                        "Project name cannot be empty".to_string(),
                    ))
                } else if input.contains(' ') {
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

    println!();

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
    let (adapters, plugins) = select_components(&template, adapter_manager).await?;

    let registry_adapters = adapter_manager.fetch_regsitry_adapters().await?;
    let adapters = adapters
        .iter()
        .map(|a| registry_adapters.get(a).unwrap().clone())
        .collect();

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
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?;

    Ok(templates[selection].name.clone())
}

async fn select_components(
    _template: &Template,
    adapter_manager: &AdapterManager,
) -> Result<(Vec<String>, Vec<String>)> {
    // Select adapters
    let spinner = terminal_utils::create_spinner(&format!("Fetching adapters from registry..."));
    let registry_adapters = adapter_manager.fetch_regsitry_adapters().await?;
    spinner.finish_and_clear();

    let mut adapter_names: Vec<String> = registry_adapters.keys().cloned().collect();
    adapter_names.sort();

    let selected_adapters = if !adapter_names.is_empty() {
        println!("\n{}\n", "🔌 Select adapters to install:".bright_cyan());
        let selections = MultiSelect::new()
            .with_prompt("Adapters")
            .items(&adapter_names)
            //.defaults(&vec![true; adapter_names.len().min(1)]) // Select first adapter by default
            .interact()
            .map_err(|e| NbrError::io(e.to_string()))?;

        selections
            .into_iter()
            .map(|i| adapter_names[i].to_string())
            .collect()
    } else {
        vec!["OneBot V11".to_string()] // Default adapter
    };

    let builtin_plugins = vec!["echo", "single_session"];

    println!(
        "\n{}\n",
        "📦 Select builtin plugins to install:".bright_cyan()
    );

    let selected_plugins = MultiSelect::new()
        .with_prompt("Plugins (recommended)")
        .items(&builtin_plugins)
        .defaults(&vec![true; adapter_names.len().min(1)])
        .interact()
        .map_err(|e| NbrError::io(e.to_string()))?
        .into_iter()
        .map(|i| builtin_plugins[i].to_string())
        .collect();

    Ok((selected_adapters, selected_plugins))
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
            plugins: vec!["nonebot-plugin-status".to_string()],
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
        .ok_or_else(|| NbrError::not_found(format!("Template '{}' not found", name)))
}

async fn create_project(options: &ProjectOptions) -> Result<()> {
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
    let package_name = options.name.replace("-", "_");
    // Create directory structure
    create_project_structure(&options.output_dir, &package_name)?;

    // Generate files
    generate_bot_file(&options.output_dir)?;
    generate_pyproject_file(&options)?;
    generate_nb_config_file(&options)?;
    generate_env_files(&options.output_dir)?;
    generate_readme_file(&options)?;
    generate_gitignore(&options.output_dir)?;

    // Install dependencies
    Uv::sync(Some(&options.output_dir)).await?;

    Ok(())
}

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
                plugin_dirs: vec![format!("src/{}/plugins", options.name.replace("-", "_"))],
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

async fn create_full_project(options: &ProjectOptions) -> Result<()> {
    // Start with simple template
    create_simple_project(options).await?;

    Ok(())
}

fn create_project_structure(base_dir: &Path, module_name: &str) -> Result<()> {
    let dirs = vec![
        base_dir.join("src/plugins"),
        base_dir.join(format!("src/{}", module_name)),
        //base_dir.join("data"),
        //base_dir.join("resources"),
        //base_dir.join("tests"),
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

fn generate_bot_file(output_dir: &Path) -> Result<()> {
    // let content = handlebars.render("bot.py", data)?;
    let content = include_str!("nbfile/bot.py");
    fs::write(output_dir.join("bot.py"), content)?;
    Ok(())
}

fn generate_pyproject_file(options: &ProjectOptions) -> Result<()> {
    let mut pyproject = PyProjectConfig::default();
    pyproject.project.name = options.name.to_string();

    let mut dependencies = HashSet::new();
    // 补齐插件, 适配器相关表
    for adapter in &options.adapters {
        let adapter_dep = format!("{}>={}", adapter.project_link, adapter.version);
        dependencies.insert(adapter_dep);
    }
    pyproject.project.dependencies.extend(dependencies);

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

    ToolNonebot::parse(Some(options.output_dir.clone().join("pyproject.toml")))?
        .add_adapters(adapters)?;
    Ok(())
}

fn generate_env_files(output_dir: &Path) -> Result<()> {
    let env_dev = include_str!("nbfile/env.dev");
    let env_prod = include_str!("nbfile/env.prod");

    fs::write(output_dir.join(".env"), env_dev)?;
    fs::write(output_dir.join(".env.prod"), env_prod)?;

    Ok(())
}

fn generate_readme_file(options: &ProjectOptions) -> Result<()> {
    let project_name = options.name.clone();

    let readme = format!(
        include_str!("nbfile/readme.template"),
        project_name, project_name, project_name, project_name, project_name
    );

    fs::write(options.output_dir.join("README.md"), readme)?;
    Ok(())
}

fn generate_gitignore(output_dir: &Path) -> Result<()> {
    let gitignore = include_str!("nbfile/gitignore");

    fs::write(output_dir.join(".gitignore"), gitignore)?;
    Ok(())
}

#[allow(unused)]
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
        include_str!("nbfile/dockerfile.template"),
        project_name, project_name, project_name, project_name
    );

    fs::write(output_dir.join("Dockerfile"), dockerfile)?;
    Ok(())
}

fn create_example_plugin(output_dir: &Path) -> Result<()> {
    let plugins_dir = output_dir.join("src/plugins");

    let hello_plugin = include_str!("nbfile/hello.py");

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
        "💡 Need help? Check the README.md file for more information.".bright_blue()
    );

    Ok(())
}
