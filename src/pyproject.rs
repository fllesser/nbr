use std::{
    fs,
    path::{Path, PathBuf},
    vec,
};

use serde::{Deserialize, Serialize};
use toml_edit::{Array, Document, DocumentMut, Table, value};

use crate::error::{NbCliError, Result};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PyProjectConfig {
    pub project: Project,
    pub tool: Tool,
    pub build_system: BuildSystem,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Project {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub dependencies: Vec<String>,
    pub requires_python: String,
    pub readme: String,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: String::from("awesome-bot"),
            version: String::from("0.1.0"),
            description: String::from("your bot description"),
            authors: Vec::from([]),
            dependencies: Vec::from(["nonebot2[fastapi, httpx, websockets]>=2.4.0".to_string()]),
            requires_python: String::from(">=3.10"),
            readme: String::from("README.md"),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Tool {
    pub nonebot: Nonebot,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Nonebot {
    pub adapters: Vec<Adapter>,
    pub plugins: Vec<String>,
    pub plugin_dirs: Vec<String>,
    pub builtin_plugins: Vec<String>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Adapter {
    pub name: String,
    pub module_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct BuildSystem {
    pub requires: Vec<String>,
    pub build_backend: String,
}

impl Default for BuildSystem {
    fn default() -> Self {
        Self {
            requires: vec!["uv_build>=0.8.3,<0.9.0".to_string()],
            build_backend: "uv_build".to_string(),
        }
    }
}
impl PyProjectConfig {
    pub async fn load() -> Result<Option<Self>> {
        let current_dir = std::env::current_dir()
            .map_err(|e| NbCliError::config(format!("Failed to get current directory: {}", e)))?;
        let config_path = current_dir.join("pyproject.toml");
        PyProjectConfig::parse(&config_path).map(Some)
    }

    pub fn parse(config_path: &Path) -> Result<Self> {
        let content = fs::read_to_string(config_path)
            .map_err(|e| NbCliError::config(format!("Failed to read pyproject.toml: {}", e)))?;

        let parsed: toml::Value = toml::from_str(&content)
            .map_err(|e| NbCliError::config(format!("Failed to parse TOML: {}", e)))?;

        parsed
            .try_into()
            .map_err(|e| NbCliError::config(format!("Failed to parse pyproject.toml: {}", e)))
    }

    pub async fn save(&self) -> Result<()> {
        let current_dir = std::env::current_dir()
            .map_err(|e| NbCliError::config(format!("Failed to get current directory: {}", e)))?;

        let config_path = current_dir.join("pyproject.toml");
        let config_content = toml::to_string_pretty(self).map_err(|e| {
            NbCliError::config(format!("Failed to serialize pyproject config: {}", e))
        })?;

        fs::write(&config_path, config_content)
            .map_err(|e| NbCliError::config(format!("Failed to write pyproject config: {}", e)))
    }

    #[allow(dead_code)]
    pub async fn add_plugin(plugin_module_name: &str) -> Result<()> {
        let mut pyproject = PyProjectConfig::load().await?.unwrap();
        pyproject
            .tool
            .nonebot
            .plugins
            .push(plugin_module_name.to_string());
        pyproject.save().await
    }

    #[allow(dead_code)]
    pub async fn add_adapter(adapter_name: &str, adapter_module_name: &str) -> Result<()> {
        let mut pyproject = PyProjectConfig::load().await?.unwrap();
        pyproject.tool.nonebot.adapters.push(Adapter {
            name: adapter_name.to_string(),
            module_name: adapter_module_name.to_string(),
        });
        pyproject.save().await
    }

    #[allow(dead_code)]
    pub async fn remove_plugin(plugin_module_name: &str) -> Result<()> {
        let mut pyproject = PyProjectConfig::load().await?.unwrap();
        pyproject
            .tool
            .nonebot
            .plugins
            .retain(|p| p != &plugin_module_name);
        pyproject.save().await
    }

    #[allow(dead_code)]
    pub async fn remove_adapter(adapter_name: &str) -> Result<()> {
        let mut pyproject = PyProjectConfig::load().await?.unwrap();
        pyproject
            .tool
            .nonebot
            .adapters
            .retain(|a| a.name != adapter_name);
        pyproject.save().await
    }
}

#[allow(dead_code)]
pub struct ToolNonebot {
    pub toml_path: PathBuf,
    pub doc_mut: DocumentMut,
}

#[allow(dead_code)]
impl ToolNonebot {
    pub fn parse(toml_path: Option<PathBuf>) -> Result<Self> {
        let toml_path = toml_path
            .clone()
            .unwrap_or_else(|| Path::new("pyproject.toml").to_path_buf());
        let content = std::fs::read_to_string(toml_path.clone())
            .map_err(|e| NbCliError::config(format!("Failed to read pyproject.toml: {}", e)))?;

        let doc = Document::parse(content)
            .map_err(|e| NbCliError::config(format!("Failed to parse pyproject.toml: {}", e)))?;

        let doc_mut = doc.into_mut();
        Ok(Self { toml_path, doc_mut })
    }

    fn nonebot_table(&mut self) -> Result<&mut Table> {
        let nonebot = self
            .doc_mut
            .get_mut("tool")
            .unwrap()
            .get_mut("nonebot")
            .unwrap();
        Ok(nonebot.as_table_mut().unwrap())
    }

    fn save(&self) -> Result<()> {
        std::fs::write(self.toml_path.clone(), self.doc_mut.to_string())?;
        Ok(())
    }

    ///
    /// ```toml
    /// [tool.nonebot]
    /// adapters = [
    ///     {name = "OneBot V11", module_name = "nonebot.adapters.onebot.v11"},
    ///     {name = "OneBot V12", module_name = "nonebot.adapters.onebot.v12"},
    /// ]
    /// ```
    pub fn add_adapters(&mut self, wait_add_adapters: Vec<Adapter>) -> Result<()> {
        let nonebot = self.nonebot_table()?;

        let adapters = if let Some(adapters) = nonebot.get_mut("adapters") {
            adapters.as_array_of_tables_mut().unwrap()
        } else {
            nonebot["adapters"] = value(Array::new());
            nonebot["adapters"].as_array_of_tables_mut().unwrap()
        };

        // 添加新的适配器配置
        for adapter in wait_add_adapters {
            let mut table = Table::new();
            table.insert("name", value(adapter.name));
            table.insert("module_name", value(adapter.module_name));
            adapters.push(table)
        }

        // 写回文件
        self.save()
    }

    pub fn add_plugins(&mut self, wait_add_plugins: Vec<String>) -> Result<()> {
        let nonebot = self.nonebot_table()?;

        if let Some(plugins) = nonebot.get_mut("plugins") {
            for plugin in wait_add_plugins {
                plugins.as_array_mut().unwrap().push(plugin);
            }
        }

        self.save()
    }
}
