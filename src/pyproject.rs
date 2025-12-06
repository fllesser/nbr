use crate::error::{NbrError, Result as NbrResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use toml_edit::{Array, Document, DocumentMut, InlineTable, Table};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct PyProjectConfig {
    pub project: Project,
    pub dependency_groups: Option<DependencyGroups>,
    pub build_system: Option<BuildSystem>,
    pub tool: Option<Tool>,
}

impl Default for PyProjectConfig {
    fn default() -> Self {
        Self {
            project: Project::default(),
            dependency_groups: Some(DependencyGroups::default()),
            build_system: Some(BuildSystem::default()),
            tool: Some(Tool::default()),
        }
    }
}

/// Represents a single item in a dependency group, which can be either
/// a PEP 508 dependency specifier string or an include-group reference
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum DependencyGroupItem {
    /// A standard PEP 508 dependency specifier (e.g., "pytest>=7.0")
    String(String),
    /// A dependency group include (e.g., { include-group = "test" })
    IncludeGroup {
        #[serde(rename = "include-group")]
        include_group: String,
    },
}

/// Dependency groups as defined in PEP 735
/// Each group contains a list of dependency items (strings or include-group references)
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DependencyGroups {
    #[serde(flatten)]
    pub groups: HashMap<String, Vec<DependencyGroupItem>>,
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
            description: String::from("a nonebot project"),
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
            requires: vec!["uv_build>=0.9.0,<0.10.0".to_string()],
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
                "{} is not exists",
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
    pub fn with_str(content: &str, save_path: &Path) -> NbrResult<Self> {
        let toml_path = save_path.to_path_buf();
        let doc = Document::parse(content)
            .map_err(|e| NbrError::config(format!("Failed to parse pyproject.toml: {}", e)))?;
        let doc_mut = doc.into_mut();
        Ok(Self { toml_path, doc_mut })
    }

    pub fn with_work_dir(work_dir: Option<&Path>) -> NbrResult<Self> {
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

        Self::with_str(&content, &toml_path)
    }

    fn nonebot_table_mut(&mut self) -> NbrResult<&mut Table> {
        self.doc_mut["tool"]["nonebot"]
            .as_table_mut()
            .ok_or(NbrError::config("tool.nonebot is not table"))
    }

    fn adapters_array_mut(&mut self) -> NbrResult<&mut Array> {
        self.nonebot_table_mut()?
            .get_mut("adapters")
            .ok_or(NbrError::config("adapters is not found in tool.nonebot"))
            .and_then(|item| {
                item.as_array_mut()
                    .ok_or(NbrError::config("adapters is not array"))
            })
    }

    fn plugins_array_mut(&mut self) -> NbrResult<&mut Array> {
        self.nonebot_table_mut()?
            .get_mut("plugins")
            .ok_or(NbrError::config("plugins is not found in tool.nonebot"))
            .and_then(|item| {
                item.as_array_mut()
                    .ok_or(NbrError::config("plugins is not array"))
            })
    }

    fn save(&self) -> NbrResult<()> {
        std::fs::write(self.toml_path.clone(), self.doc_mut.to_string())?;
        Ok(())
    }

    fn fmt_toml_array(array: &mut toml_edit::Array) {
        array.iter_mut().for_each(|a| {
            let decor_mut = a.decor_mut();
            decor_mut.set_prefix("\n  ");
            decor_mut.set_suffix("");
        });
        if let Some(last) = array.iter_mut().last() {
            last.decor_mut().set_suffix("\n");
        }
    }

    pub fn add_adapters(&mut self, adapters: Vec<Adapter>) -> NbrResult<()> {
        let adapters = adapters.into_iter().collect::<HashSet<Adapter>>();
        let adapters_arr_mut = self.adapters_array_mut()?;

        // 交互逻辑 已经排除了已经安装的 adapter
        for adapter in adapters {
            let mut inline_table = InlineTable::new();
            inline_table.insert("name", adapter.name);
            inline_table.insert("module_name", adapter.module_name);
            adapters_arr_mut.push(inline_table);
        }
        Self::fmt_toml_array(adapters_arr_mut);

        // 写回文件
        self.save()
    }

    pub fn remove_adapters(&mut self, adapter_names: Vec<&str>) -> NbrResult<()> {
        let adapters_arr_mut = self.adapters_array_mut()?;
        adapters_arr_mut.retain(|a| {
            !adapter_names.contains(&a.as_inline_table().unwrap()["name"].as_str().unwrap())
        });
        self.save()
    }

    pub fn add_plugins(&mut self, plugins: Vec<&str>) -> NbrResult<()> {
        let mut plugins = plugins.into_iter().collect::<HashSet<&str>>();
        let plugins_arr_mut = self.plugins_array_mut()?;

        let plugin_names = plugins_arr_mut
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect::<Vec<&str>>();
        plugins.retain(|p| !plugin_names.contains(p));
        plugins_arr_mut.extend(plugins);
        Self::fmt_toml_array(plugins_arr_mut);

        self.save()
    }

    pub fn remove_plugins(&mut self, plugins: Vec<&str>) -> NbrResult<()> {
        let plugins_arr_mut = self.plugins_array_mut()?;
        plugins_arr_mut.retain(|p| !plugins.contains(&p.as_str().unwrap()));
        Self::fmt_toml_array(plugins_arr_mut);
        self.save()
    }

    /// 重置 tool.nonebot.plugins
    pub fn reset_plugins(&mut self, plugins: Vec<&str>) -> NbrResult<()> {
        let plugins_arr_mut = self.plugins_array_mut()?;
        plugins_arr_mut.clear();
        plugins_arr_mut.extend(plugins);
        Self::fmt_toml_array(plugins_arr_mut);
        self.save()
    }

    /// 重置 tool.nonebot.adapters
    #[allow(unused)]
    pub fn reset_adapters(&mut self, adapters: Vec<Adapter>) -> NbrResult<()> {
        let adapters_arr_mut = self.adapters_array_mut()?;
        adapters_arr_mut.clear();
        adapters_arr_mut.extend(adapters.into_iter().map(|adapter| {
            let mut inline_table = InlineTable::new();
            inline_table.insert("name", adapter.name);
            inline_table.insert("module_name", adapter.module_name);
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
        let mut editor = NbTomlEditor::with_work_dir(Some(toml_path)).unwrap();
        editor
            .add_adapters(vec![Adapter {
                name: "OneBot V11".to_string(),
                module_name: "nonebot.adapters.onebot.v12".to_string(),
            }])
            .unwrap();
        editor
            .add_adapters(vec![Adapter {
                name: "OneBot V12".to_string(),
                module_name: "nonebot.adapters.onebot.v12".to_string(),
            }])
            .unwrap();
    }

    #[test]
    fn test_add_plugins() {
        let toml_path = Path::new("awesome-bot");
        let mut editor = NbTomlEditor::with_work_dir(Some(toml_path)).unwrap();

        editor
            .add_plugins(vec![
                "nonebot_plugin_status",
                "nonebot_plugin_alconna",
                "nonebot_plugin_waiter",
            ])
            .unwrap();
    }

    #[test]
    fn test_remove_plugins() {
        let toml_path = Path::new("awesome-bot");
        let mut editor = NbTomlEditor::with_work_dir(Some(toml_path)).unwrap();
        editor
            .remove_plugins(vec!["nonebot_plugin_status"])
            .unwrap();
    }

    #[test]
    fn test_parse_toml_to_nonebot() {
        let toml_path = Path::new("awesome-bot");
        let pyproject = PyProjectConfig::parse(Some(toml_path)).unwrap();
        let nonebot = pyproject.nonebot().unwrap();
        dbg!(nonebot);
    }

    #[test]
    fn test_dependency_groups_with_include() {
        let toml_content = r#"
[project]
name = "test-project"
version = "0.1.0"
description = "Test project"
requires-python = ">=3.10"
dependencies = []

[dependency-groups]
test = ["pytest>=7.0", "coverage"]
typing = ["mypy", "types-requests"]
dev = [
    { include-group = "test" },
    { include-group = "typing" },
    "ruff"
]
"#;
        let pyproject = PyProjectConfig::parse_from_str(toml_content).unwrap();
        let dep_groups = pyproject.dependency_groups.unwrap();

        // Check test group
        let test_group = dep_groups.groups.get("test").unwrap();
        assert_eq!(test_group.len(), 2);
        assert!(matches!(&test_group[0], DependencyGroupItem::String(s) if s == "pytest>=7.0"));
        assert!(matches!(&test_group[1], DependencyGroupItem::String(s) if s == "coverage"));

        // Check dev group with include-group
        let dev_group = dep_groups.groups.get("dev").unwrap();
        assert_eq!(dev_group.len(), 3);
        assert!(
            matches!(&dev_group[0], DependencyGroupItem::IncludeGroup { include_group } if include_group == "test")
        );
        assert!(
            matches!(&dev_group[1], DependencyGroupItem::IncludeGroup { include_group } if include_group == "typing")
        );
        assert!(matches!(&dev_group[2], DependencyGroupItem::String(s) if s == "ruff"));
    }

    #[test]
    fn test_dependency_groups_serialization() {
        // Create a PyProjectConfig with dependency groups
        let mut pyproject = PyProjectConfig::default();
        let mut groups = std::collections::HashMap::new();

        // Add test group
        groups.insert(
            "test".to_string(),
            vec![
                DependencyGroupItem::String("pytest>=7.0".to_string()),
                DependencyGroupItem::String("coverage".to_string()),
            ],
        );

        // Add dev group with include-group
        groups.insert(
            "dev".to_string(),
            vec![
                DependencyGroupItem::IncludeGroup {
                    include_group: "test".to_string(),
                },
                DependencyGroupItem::String("ruff".to_string()),
            ],
        );

        pyproject.dependency_groups = Some(DependencyGroups { groups });

        // Serialize to TOML
        let toml_str = toml::to_string(&pyproject).unwrap();

        println!("Serialized TOML:\n{}", toml_str);

        // Verify the serialized TOML contains the expected structure
        assert!(toml_str.contains("[dependency-groups]"));
        assert!(toml_str.contains("test = ["));
        assert!(toml_str.contains("\"pytest>=7.0\""));
        assert!(toml_str.contains("dev = ["));
        assert!(toml_str.contains("include-group = \"test\""));

        // Parse it back and verify
        let parsed: PyProjectConfig = toml::from_str(&toml_str).unwrap();
        let parsed_groups = parsed.dependency_groups.unwrap();
        assert_eq!(parsed_groups.groups.len(), 2);
    }

    #[test]
    fn test_dev_group_includes_test_first() {
        // Simulate what generate_pyproject_file does
        let mut pyproject = PyProjectConfig::default();

        let dev_deps = vec!["ruff>=0.14.8".to_string(), "pre-commit>=4.3.0".to_string()];

        // Create test dependency group
        let test_group_items: Vec<DependencyGroupItem> = vec![
            DependencyGroupItem::String("pytest>=7.0".to_string()),
            DependencyGroupItem::String("coverage".to_string()),
        ];

        // Convert dev_deps strings to DependencyGroupItem::String
        let mut dev_group_items: Vec<DependencyGroupItem> = vec![
            // Include test group first
            DependencyGroupItem::IncludeGroup {
                include_group: String::from("test"),
            },
        ];

        // Add dev dependencies
        dev_group_items.extend(dev_deps.into_iter().map(DependencyGroupItem::String));

        // Insert both test and dev groups
        let dep_groups = pyproject.dependency_groups.as_mut().unwrap();
        dep_groups
            .groups
            .insert("test".to_string(), test_group_items);
        dep_groups.groups.insert("dev".to_string(), dev_group_items);

        // Verify the order in memory
        let dev_group = dep_groups.groups.get("dev").unwrap();
        assert_eq!(dev_group.len(), 3);

        // First item should be include-group
        assert!(
            matches!(&dev_group[0], DependencyGroupItem::IncludeGroup { include_group } if include_group == "test")
        );

        // Then the dev dependencies
        assert!(matches!(&dev_group[1], DependencyGroupItem::String(s) if s == "ruff>=0.14.8"));
        assert!(
            matches!(&dev_group[2], DependencyGroupItem::String(s) if s == "pre-commit>=4.3.0")
        );

        // Serialize and check order is preserved
        let toml_str = toml::to_string(&pyproject).unwrap();

        // The include-group should appear before other items in the serialized form
        let dev_line_start = toml_str.find("dev = [").expect("dev group not found");
        let include_pos = toml_str[dev_line_start..]
            .find("include-group")
            .expect("include-group not found");
        let ruff_pos = toml_str[dev_line_start..]
            .find("ruff")
            .expect("ruff not found");

        assert!(
            include_pos < ruff_pos,
            "include-group should come before ruff in serialized TOML"
        );
    }
}
