use anyhow::{Context, Result};
use clap::ArgMatches;
use colored::*;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::pyproject::{Adapter, PyProjectConfig};

use super::env::AdapterInfo;

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
    pub adapters: Vec<AdapterInfo>,
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
    println!("  uv sync");
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
    let adapters_map = get_available_adapters_map().await?;
    let adapters = adapters
        .iter()
        .map(|a| adapters_map.get(a).unwrap().clone())
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
    let recommended_plugins = vec!["nonebot-plugin-status", "nonebot-plugin-abs"];

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
    let adapters = vec![
        AdapterInfo {
            name: "OneBot V11".to_string(),
            version: "2.4.6".to_string(),
            location: "https://github.com/nonebot/adapter-onebot".to_string(),
            package_name: "nonebot-adapter-onebot".to_string(),
            module_name: "nonebot.adapters.onebot.v11".to_string(),
        },
        AdapterInfo {
            name: "OneBot V12".to_string(),
            version: "2.4.6".to_string(),
            location: "https://github.com/nonebot/adapter-onebot".to_string(),
            package_name: "nonebot-adapter-onebot".to_string(),
            module_name: "nonebot.adapters.onebot.v12".to_string(),
        },
    ];

    Ok(adapters)
}

async fn get_available_adapters_map() -> Result<HashMap<String, AdapterInfo>> {
    let adapters = get_available_adapters().await?;
    let adapters_map = adapters
        .iter()
        .map(|a| (a.name.clone(), a.clone()))
        .collect();
    Ok(adapters_map)
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

    let package_name = options.name.replace("-", "_");
    let mut data = HashMap::<&str, &dyn erased_serde::Serialize>::new();
    data.insert("adapters", &options.adapters);

    // Create directory structure
    create_project_structure(&options.output_dir, &package_name)?;
    // Generate files
    generate_bot_file(&handlebars, &data, &options.output_dir)?;
    generate_pyproject_file(&options)?;
    generate_env_files(&options.output_dir)?;
    generate_readme_file(&options)?;
    generate_gitignore(&options.output_dir)?;
    //generate_dockerfile(&handlebars, &data, &options.output_dir)?;

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

fn register_templates(handlebars: &mut Handlebars) -> Result<()> {
    // Bot file template
    let bot_py_template = include_str!("templates/botpy.template");
    handlebars.register_template_string("bot.py", bot_py_template)?;
    // Register helper functions
    handlebars.register_helper("adapter_pascal_case", Box::new(adapter_pascal_case_helper));
    handlebars.register_helper(
        "adapter_package_name",
        Box::new(adapter_package_name_helper),
    );
    handlebars.register_helper("adapter_module_name", Box::new(adapter_module_name_helper));
    Ok(())
}

#[allow(unused)]
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

fn adapter_pascal_case_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let adapter = serde_json::from_value::<AdapterInfo>(param.value().clone())?;
    let pascal_case = adapter
        .name
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

fn adapter_package_name_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let adapter = serde_json::from_value::<AdapterInfo>(param.value().clone())?;
    out.write(&adapter.package_name)?;
    Ok(())
}

fn adapter_module_name_helper(
    h: &handlebars::Helper,
    _: &handlebars::Handlebars,
    _: &handlebars::Context,
    _: &mut handlebars::RenderContext,
    out: &mut dyn handlebars::Output,
) -> handlebars::HelperResult {
    let param = h
        .param(0)
        .ok_or_else(|| handlebars::RenderError::new("Expected parameter"))?;
    let adapter = serde_json::from_value::<AdapterInfo>(param.value().clone())?;
    out.write(&adapter.module_name)?;
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

fn generate_bot_file(
    handlebars: &Handlebars,
    data: &HashMap<&str, &dyn erased_serde::Serialize>,
    output_dir: &Path,
) -> Result<()> {
    let content = handlebars.render("bot.py", data)?;
    fs::write(output_dir.join("bot.py"), content)?;
    Ok(())
}

fn generate_pyproject_file(options: &ProjectOptions) -> Result<()> {
    let mut pyproject = PyProjectConfig::default();
    pyproject.project.name = options.name.to_string();

    // 补齐插件, 适配器相关表
    for adapter in &options.adapters {
        pyproject
            .project
            .dependencies
            .push(adapter.package_name.to_string());
        pyproject.tool.nonebot.adapters.push(Adapter {
            name: adapter.package_name.to_string(),       // Onebot v11
            module_name: adapter.module_name.to_string(), // nonebot.adapters.onebot.v11
        });
    }
    for plugin in &options.plugins {
        pyproject.project.dependencies.push(plugin.to_string());
        pyproject
            .tool
            .nonebot
            .plugins
            .push(plugin.replace("-", "_"));
    }

    let content = toml::to_string(&pyproject)?;
    fs::write(options.output_dir.join("pyproject.toml"), content)?;
    Ok(())
}

fn generate_env_files(output_dir: &Path) -> Result<()> {
    let env_dev = include_str!("templates/env.dev");
    let env_prod = include_str!("templates/env.prod");

    fs::write(output_dir.join(".env"), env_dev)?;
    fs::write(output_dir.join(".env.prod"), env_prod)?;

    Ok(())
}

fn generate_readme_file(options: &ProjectOptions) -> Result<()> {
    let project_name = options.name.clone();

    let readme = format!(
        include_str!("templates/readme.template"),
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
        include_str!("templates/dockerfile.template"),
        project_name, project_name, project_name, project_name
    );

    fs::write(output_dir.join("Dockerfile"), dockerfile)?;
    Ok(())
}

fn create_example_plugin(output_dir: &Path) -> Result<()> {
    let plugins_dir = output_dir.join("src/plugins");

    let hello_plugin = include_str!("templates/plugin.example");

    fs::write(plugins_dir.join("hello.py"), hello_plugin)?;
    Ok(())
}

async fn show_setup_instructions(options: &ProjectOptions) -> Result<()> {
    println!("\n{}", "📋 Setup Instructions:".bright_yellow());
    println!("1. Configure your bot in the .env file");
    if !options.adapters.is_empty() {
        println!("2. Set up your adapters:");
        for adapter in &options.adapters {
            println!("   • {}: Configure {}", adapter.name, adapter.location);
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
            adapters: vec![AdapterInfo {
                name: "OneBot V11".to_string(),
                version: "2.4.6".to_string(),
                location: "https://github.com/nonebot/adapter-onebot".to_string(),
                package_name: "nonebot-adapter-onebot".to_string(),
                module_name: "nonebot.adapters.onebot.v11".to_string(),
            }],
            plugins: vec![
                "nonebot-plugin-status".to_string(),
                "nonebot-plugin-abs".to_string(),
            ],
        };

        create_bootstrap_project(&options).await.unwrap();

        // Check if essential files were created
        assert!(temp_dir.path().join("bot.py").exists());
        assert!(temp_dir.path().join("pyproject.toml").exists());
        assert!(temp_dir.path().join(".env").exists());
        assert!(temp_dir.path().join(".gitignore").exists());
    }
}
