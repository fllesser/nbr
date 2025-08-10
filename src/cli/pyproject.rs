use serde::{Deserialize, Serialize};

#[serde(rename_all = "kebab-case")]
#[derive(Serialize, Deserialize, Default)]
pub struct PyProject {
    pub project: Project,
    pub tool: Tool,
    pub build_system: BuildSystem,
}

#[serde(rename_all = "kebab-case")]
#[derive(Serialize, Deserialize)]
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

#[serde(rename_all = "kebab-case")]
#[derive(Serialize, Deserialize, Default)]
pub struct Tool {
    pub nonebot: Nonebot,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Nonebot {
    pub adapters: Vec<Adapter>,
    pub plugins: Vec<String>,
    pub plugin_dirs: Vec<String>,
    pub builtin_plugins: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Adapter {
    pub name: String,
    pub module_name: String,
}

#[serde(rename_all = "kebab-case")]
#[derive(Serialize, Deserialize)]
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
