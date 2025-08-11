use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

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
        let config_content = toml::to_string(self).map_err(|e| {
            NbCliError::config(format!("Failed to serialize pyproject config: {}", e))
        })?;

        fs::write(&config_path, config_content)
            .map_err(|e| NbCliError::config(format!("Failed to write pyproject config: {}", e)))
    }
}
