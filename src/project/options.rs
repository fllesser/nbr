use clap::ValueEnum;
use std::path::PathBuf;
use strum::Display;

#[derive(Debug, Clone)]
pub struct SelectedAdapter {
    pub module_name: String,
    pub name: String,
    pub project_link: String,
    pub version: String,
}

impl From<&SelectedAdapter> for crate::pyproject::Adapter {
    fn from(adapter: &SelectedAdapter) -> Self {
        Self {
            name: adapter.name.clone(),
            module_name: adapter.module_name.clone(),
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
#[clap(rename_all = "lowercase")]
pub enum Template {
    #[clap(help = "Basic NoneBot project template")]
    Bootstrap,
    #[clap(help = "Simple bot template with basic plugins")]
    Simple,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
pub enum Driver {
    FastAPI,
    HTTPX,
    WebSockets,
    Quark,
    AIOHTTP,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Environment {
    Dev,
    Prod,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "kebab-case")]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinPlugin {
    Echo,
    SingleSession,
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum DevTool {
    Ruff,
    Basedpyright,
    PreCommit,
}

impl DevTool {
    pub fn to_dependency(&self) -> &'static str {
        match self {
            Self::Ruff => "ruff>=0.14.8",
            Self::Basedpyright => "basedpyright>=1.35.0",
            Self::PreCommit => "pre-commit>=4.3.0",
        }
    }
}

pub struct ProjectOptions {
    pub name: String,
    pub template: Template,
    pub output_dir: PathBuf,
    pub drivers: Vec<String>,
    pub adapters: Vec<SelectedAdapter>,
    pub plugins: Vec<String>,
    pub python_version: String,
    pub environment: Environment,
    pub dev_tools: Vec<DevTool>,
    pub gen_dockerfile: bool,
    pub create_venv: bool,
}
