use crate::error::{NbrError, Result as NbrResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    vec,
};

use toml_edit::{Document, DocumentMut, InlineTable, Table};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PyProjectConfig {
    pub project: Project,
    pub dependency_groups: Option<DependencyGroups>,
    pub tool: Option<Tool>,
    pub build_system: Option<BuildSystem>,
}

impl Default for PyProjectConfig {
    fn default() -> Self {
        Self {
            project: Project::default(),
            tool: Some(Tool::default()),
            build_system: Some(BuildSystem::default()),
            dependency_groups: Some(DependencyGroups::default()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DependencyGroups {
    pub dev: Option<Vec<String>>,
    pub test: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Project {
    pub name: String,
    pub version: String,
    pub description: String,
    pub requires_python: String,
    pub dependencies: Vec<String>,

    pub authors: Option<Vec<Author>>,
    pub readme: Option<String>,
    pub urls: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Author {
    pub name: String,
    pub email: String,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: String::from("awesome-bot"),
            version: String::from("0.1.0"),
            description: String::from("your bot description"),
            requires_python: String::from(">=3.10"),
            dependencies: vec![],

            authors: Some(vec![]),
            readme: Some(String::from("README.md")),
            urls: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Tool {
    pub nonebot: Option<Nonebot>,
}

impl Default for Tool {
    fn default() -> Self {
        Self {
            nonebot: Some(Nonebot::default()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Nonebot {
    pub adapters: Option<Vec<Adapter>>,
    pub plugins: Option<Vec<String>>,
    pub plugin_dirs: Option<Vec<String>>,
    pub builtin_plugins: Option<Vec<String>>,
}

impl Default for Nonebot {
    fn default() -> Self {
        Self {
            adapters: Some(vec![]),
            plugins: Some(vec![]),
            plugin_dirs: Some(vec![]),
            builtin_plugins: Some(vec![]),
        }
    }
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

impl PyProjectConfig {
    /// 解析 pyproject.toml 文件
    ///
    /// # Arguments
    ///
    /// * `work_dir` - 工作目录，如果为 None，则使用当前目录
    ///
    /// # Returns
    ///
    /// 返回解析后的 PyProjectConfig 结构体
    pub fn parse(work_dir: Option<&Path>) -> NbrResult<Self> {
        let toml_path = if let Some(work_dir) = work_dir {
            work_dir.join("pyproject.toml")
        } else {
            Path::new("pyproject.toml").to_path_buf()
        };

        if !toml_path.exists() {
            return Err(NbrError::config(format!(
                "pyproject.toml not found in {}",
                toml_path.display()
            )));
        }

        let content = std::fs::read_to_string(toml_path)
            .map_err(|e| NbrError::config(format!("Failed to read pyproject.toml: {}", e)))?;

        Self::parse_from_str(&content)
    }

    pub fn parse_from_str(content: &str) -> NbrResult<Self> {
        toml::from_str(content).map_err(|e| {
            NbrError::config(format!(
                "Failed to parse pyproject.toml to PyProjectConfig: {e}"
            ))
        })
    }

    /// 解析当前目录的 pyproject.toml 文件
    ///
    /// # Returns
    ///
    /// 返回解析后的 PyProjectConfig 结构体
    #[allow(unused)]
    pub fn parse_current_dir() -> NbrResult<Self> {
        Self::parse(None)
    }

    pub fn nonebot(&self) -> Option<&Nonebot> {
        self.tool.as_ref().and_then(|tool| tool.nonebot.as_ref())
    }
}

#[derive(Debug, Clone)]
pub struct NbTomlEditor {
    toml_path: PathBuf,
    doc_mut: DocumentMut,
}

impl NbTomlEditor {
    #[allow(unused)]
    pub fn parse_current_dir() -> NbrResult<Self> {
        Self::parse(None)
    }

    pub fn parse(work_dir: Option<&Path>) -> NbrResult<Self> {
        let toml_path = if let Some(work_dir) = work_dir {
            work_dir.join("pyproject.toml")
        } else {
            Path::new("pyproject.toml").to_path_buf()
        };

        let mut content = std::fs::read_to_string(toml_path.clone())
            .map_err(|e| NbrError::config(format!("Failed to read pyproject.toml: {}", e)))?;

        // 如果 pyproject.toml 中没有 [tool.nonebot] 表，则添加
        if !content.contains("[tool.nonebot]") {
            content.push_str(
                format!(
                    include_str!("cli/templates/pyproject/tool_nonebot"),
                    "", "", ""
                )
                .as_str(),
            );
        }

        let doc = Document::parse(&content)
            .map_err(|e| NbrError::config(format!("Failed to parse pyproject.toml: {}", e)))?;

        let doc_mut = doc.into_mut();

        Ok(Self { toml_path, doc_mut })
    }

    fn nonebot_table_mut(&mut self) -> NbrResult<&mut Table> {
        let nonebot = self.doc_mut["tool"]["nonebot"].as_table_mut().unwrap();
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

    /// 重置 tool.nonebot.plugins
    pub fn reset_plugins(&mut self, plugins: Vec<String>) -> NbrResult<()> {
        let nonebot_table = self.nonebot_table_mut()?;
        let plugins_array = nonebot_table.get_mut("plugins").unwrap();
        let plugins_arr_mut = plugins_array.as_array_mut().unwrap();
        plugins_arr_mut.clear();
        plugins_arr_mut.extend(plugins);
        self.save()
    }

    /// 重置 tool.nonebot.adapters
    #[allow(unused)]
    pub fn reset_adapters(&mut self, adapters: Vec<Adapter>) -> NbrResult<()> {
        let nonebot_table = self.nonebot_table_mut()?;
        let adapters_array = nonebot_table.get_mut("adapters").unwrap();
        let adapters_arr_mut = adapters_array.as_array_mut().unwrap();
        adapters_arr_mut.clear();
        adapters_arr_mut.extend(adapters.into_iter().map(|adapter| {
            let mut inline_table = InlineTable::new();
            inline_table.insert("name", adapter.name.into());
            inline_table.insert("module_name", adapter.module_name.into());
            inline_table
        }));
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_adapters() {
        let toml_path = Path::new("awesome-bot");
        let mut tool_nonebot = NbTomlEditor::parse(Some(&toml_path)).unwrap();
        tool_nonebot
            .add_adapters(vec![Adapter {
                name: "OneBot V12".to_string(),
                module_name: "nonebot.adapters.onebot.v12".to_string(),
            }])
            .unwrap();
    }

    #[test]
    fn test_add_plugins() {
        let toml_path = Path::new("awesome-bot");
        let mut tool_nonebot = NbTomlEditor::parse(Some(&toml_path)).unwrap();
        tool_nonebot
            .add_plugins(vec!["nonebot-plugin-status".to_string()])
            .unwrap();
    }

    #[test]
    fn test_parse_toml_to_nonebot() {
        let toml_path = Path::new("awesome-bot");
        let pyproject = PyProjectConfig::parse(Some(&toml_path)).unwrap();
        let nonebot = pyproject.nonebot().unwrap();
        dbg!(nonebot);
    }
}
