use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;

use crate::docker;
use crate::pyproject::{BuildSystem, NbTomlEditor, Nonebot, Project, PyProjectConfig, Tool};
use crate::uv;

use super::deps;
use super::options::{Environment, ProjectOptions, Template};

impl ProjectOptions {
    pub async fn create(&self) -> Result<()> {
        fs::create_dir_all(&self.output_dir).context("Failed to create output directory")?;

        match self.template {
            Template::Bootstrap => self.create_with_bootstrap().await?,
            Template::Simple => self.create_with_simple().await?,
        }

        Ok(())
    }

    async fn create_with_bootstrap(&self) -> Result<()> {
        self.create_structure()?;
        self.create_pyproject_config()?;
        self.create_env_files()?;
        self.create_readme_file()?;
        self.create_gitignore()?;
        self.create_dev_tools_config()?;
        self.create_docker_config()?;
        self.install_dependencies()?;
        Ok(())
    }

    async fn create_with_simple(&self) -> Result<()> {
        self.create_with_bootstrap().await?;
        self.create_example_plugin()?;
        Ok(())
    }

    fn create_structure(&self) -> Result<()> {
        let base_dir = &self.output_dir;
        let module_name = self.name.replace("-", "_");

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

    fn create_pyproject_config(&self) -> Result<()> {
        let dependencies = deps::collect_dependencies(&self.adapters, &self.drivers);
        let dependency_groups = deps::collect_dependency_groups(&self.dev_tools);

        let pyproject = PyProjectConfig {
            project: Project {
                name: self.name.to_string(),
                version: String::from("0.1.0"),
                description: String::from("a nonebot project"),
                authors: None,
                readme: Some("README.md".to_string()),
                urls: None,
                requires_python: format!(">={}", self.python_version),
                dependencies,
            },
            dependency_groups: Some(dependency_groups),
            build_system: Some(BuildSystem::default()),
            tool: Some(Tool {
                nonebot: Some(Nonebot {
                    builtin_plugins: Some(self.plugins.clone()),
                    plugin_dirs: Some(vec!["src/plugins".to_string()]),
                    adapters: Some(vec![]),
                    plugins: Some(vec![]),
                }),
            }),
        };
        let content = toml::to_string(&pyproject)?;
        let save_path = self.output_dir.join("pyproject.toml");
        NbTomlEditor::with_str(&content, &save_path)?
            .add_adapters(self.adapters.iter().map(|a| a.into()).collect::<Vec<_>>())?;
        Ok(())
    }

    fn install_dependencies(&self) -> Result<()> {
        if self.create_venv {
            uv::sync(Some(&self.python_version))
                .working_dir(&self.output_dir)
                .run()?;
        }
        Ok(())
    }

    fn create_env_files(&self) -> Result<()> {
        let driver = self
            .drivers
            .iter()
            .map(|d| format!("~{}", d.to_lowercase()))
            .collect::<Vec<String>>()
            .join("+");
        let log_level = match self.environment {
            Environment::Dev => "DEBUG",
            Environment::Prod => "INFO",
        };
        let file_name = format!(".env.{}", self.environment);
        let env_content = format!(
            include_str!("../cli/templates/.env"),
            driver, log_level, self.name
        );
        fs::write(
            self.output_dir.join(".env"),
            format!("ENVIRONMENT={}", self.environment),
        )?;
        fs::write(self.output_dir.join(file_name), env_content)?;

        Ok(())
    }

    fn create_docker_config(&self) -> Result<()> {
        if self.gen_dockerfile {
            docker::create_dockerfile(&self.output_dir)?;
            docker::create_dockerignore(&self.output_dir)?;
            docker::create_python_pin_file(&self.output_dir, &self.python_version)?;
            docker::create_compose_file(&self.output_dir, &self.name)?;
        }
        Ok(())
    }

    fn create_readme_file(&self) -> Result<()> {
        let project_name = self.name.clone();

        let readme = format!(
            include_str!("../cli/templates/readme"),
            project_name, project_name, project_name, project_name, project_name
        );

        fs::write(self.output_dir.join("README.md"), readme)?;
        Ok(())
    }

    fn create_dev_tools_config(&self) -> Result<()> {
        for tool in self.dev_tools.iter() {
            match tool {
                super::options::DevTool::Ruff => self.append_ruff_config()?,
                super::options::DevTool::Basedpyright => self.append_pyright_config()?,
                super::options::DevTool::PreCommit => self.create_pre_commit_config()?,
            }
        }
        Ok(())
    }

    fn create_pre_commit_config(&self) -> Result<()> {
        let pre_commit_config = include_str!("../cli/templates/pre_commit_config");
        fs::write(
            self.output_dir.join(".pre-commit-config.yaml"),
            pre_commit_config,
        )?;
        Ok(())
    }

    fn create_gitignore(&self) -> Result<()> {
        let gitignore = include_str!("../cli/templates/gitignore");
        fs::write(self.output_dir.join(".gitignore"), gitignore)?;
        Ok(())
    }

    fn append_ruff_config(&self) -> Result<()> {
        let content = include_str!("../cli/templates/pyproject/tool_ruff");
        self.append_content_to_pyproject(content)?;
        Ok(())
    }

    fn append_pyright_config(&self) -> Result<()> {
        let content = include_str!("../cli/templates/pyproject/tool_pyright");
        self.append_content_to_pyproject(content)?;
        Ok(())
    }

    fn append_content_to_pyproject(&self, content: &str) -> Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.output_dir.join("pyproject.toml"))?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    fn create_example_plugin(&self) -> Result<()> {
        let plugins_dir = self.output_dir.join("src/plugins");
        let hello_plugin = include_str!("../cli/templates/hello.py");
        fs::write(plugins_dir.join("hello.py"), hello_plugin)?;
        Ok(())
    }
}
