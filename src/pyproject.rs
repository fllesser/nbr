use crate::error::{NbrError, Result as NbrResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    vec,
};

use toml_edit::{Document, DocumentMut, InlineTable, Table};

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

#[derive(Serialize, Deserialize, Default, Debug, Clone, Eq, PartialEq, Hash)]
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

#[derive(Debug, Clone)]
pub struct ToolNonebot {
    toml_path: PathBuf,
    doc_mut: DocumentMut,
}

#[allow(dead_code)]
impl ToolNonebot {
    pub fn parse(toml_path: Option<PathBuf>) -> NbrResult<Self> {
        let toml_path = toml_path
            .clone()
            .unwrap_or_else(|| Path::new("pyproject.toml").to_path_buf());
        let content = std::fs::read_to_string(toml_path.clone())
            .map_err(|e| NbrError::config(format!("Failed to read pyproject.toml: {}", e)))?;

        let doc = Document::parse(content)
            .map_err(|e| NbrError::config(format!("Failed to parse pyproject.toml: {}", e)))?;

        let doc_mut = doc.into_mut();

        Ok(Self { toml_path, doc_mut })
    }

    fn nonebot_table_mut(&mut self) -> NbrResult<&mut Table> {
        let nonebot = self.doc_mut["tool"]["nonebot"].as_table_mut().unwrap();
        Ok(nonebot)
    }

    pub fn nonebot(&self) -> NbrResult<Nonebot> {
        let nonebot = self.doc_mut["tool"]["nonebot"].as_table().unwrap();
        let nonebot = toml::from_str(nonebot.to_string().as_str())?;
        Ok(nonebot)
    }

    fn save(&self) -> NbrResult<()> {
        std::fs::write(self.toml_path.clone(), self.doc_mut.to_string())?;
        Ok(())
    }

    pub fn add_adapters(&mut self, adapters: Vec<Adapter>) -> NbrResult<()> {
        // 去重
        let adapters = adapters.into_iter().collect::<HashSet<Adapter>>();
        let nonebot = self.nonebot_table_mut()?;

        if let Some(adapters_array) = nonebot.get_mut("adapters") {
            let adapters_arr_mut = adapters_array.as_array_mut().unwrap();
            for adapter in adapters {
                if adapters_arr_mut
                    .iter()
                    .any(|a| a.as_inline_table().unwrap()["name"].as_str().unwrap() == adapter.name)
                {
                    continue;
                }
                let mut inline_table = InlineTable::new();
                inline_table.insert("name", adapter.name.into());
                inline_table.insert("module_name", adapter.module_name.into());
                adapters_arr_mut.push(inline_table);
            }
        }

        // 写回文件
        self.save()
    }

    pub fn remove_adapters(&mut self, names: Vec<&str>) -> NbrResult<()> {
        let nonebot = self.nonebot_table_mut()?;
        let adapters_array = nonebot.get_mut("adapters").unwrap();
        let adapters_arr_mut = adapters_array.as_array_mut().unwrap();
        for name in names {
            adapters_arr_mut
                .retain(|a| a.as_inline_table().unwrap()["name"].as_str().unwrap() != name);
        }
        self.save()
    }

    pub fn add_plugins(&mut self, plugins: Vec<String>) -> NbrResult<()> {
        // 去重
        let plugins = plugins.into_iter().collect::<HashSet<String>>();
        let nonebot = self.nonebot_table_mut()?;

        if let Some(plugins_array) = nonebot.get_mut("plugins") {
            let plugins_arr_mut = plugins_array.as_array_mut().unwrap();
            for plugin in plugins {
                if plugins_arr_mut
                    .iter()
                    .any(|p| p.as_str().unwrap() == plugin)
                {
                    continue;
                }
                plugins_arr_mut.push(plugin);
            }
        }

        self.save()
    }

    pub fn remove_plugins(&mut self, plugins: Vec<String>) -> NbrResult<()> {
        let nonebot_table = self.nonebot_table_mut()?;
        let plugins_array = nonebot_table.get_mut("plugins").unwrap();
        let plugins_arr_mut = plugins_array.as_array_mut().unwrap();
        for plugin in plugins {
            plugins_arr_mut.retain(|p| p.as_str().unwrap() != plugin);
        }
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_adapters() {
        let current_dir = std::env::current_dir().unwrap();
        let toml_path = current_dir.join("awesome-bot").join("pyproject.toml");
        let mut tool_nonebot = ToolNonebot::parse(Some(toml_path)).unwrap();
        tool_nonebot
            .add_adapters(vec![Adapter {
                name: "OneBot V12".to_string(),
                module_name: "nonebot.adapters.onebot.v12".to_string(),
            }])
            .unwrap();
    }

    #[test]
    fn test_add_plugins() {
        let current_dir = std::env::current_dir().unwrap();
        let toml_path = current_dir.join("awesome-bot").join("pyproject.toml");
        let mut tool_nonebot = ToolNonebot::parse(Some(toml_path)).unwrap();
        tool_nonebot
            .add_plugins(vec!["nonebot-plugin-status".to_string()])
            .unwrap();
    }

    #[test]
    fn test_parse_toml_to_nonebot() {
        let current_dir = std::env::current_dir().unwrap();
        let toml_path = current_dir.join("awesome-bot").join("pyproject.toml");
        let tool_nonebot = ToolNonebot::parse(Some(toml_path)).unwrap();
        let nonebot = tool_nonebot.nonebot().unwrap();
        dbg!(nonebot);
    }
}
