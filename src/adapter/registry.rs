use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::log::StyledText;
use crate::project::SelectedAdapter;
use crate::pyproject::Adapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryAdapter {
    pub module_name: String,
    pub project_link: String,
    pub name: String,
    pub desc: String,
    pub author: String,
    pub homepage: Option<String>,
    pub tags: Vec<HashMap<String, String>>,
    pub is_official: bool,
    pub time: String,
    pub version: String,
}

impl From<&RegistryAdapter> for Adapter {
    fn from(adapter: &RegistryAdapter) -> Self {
        Self {
            name: adapter.name.clone(),
            module_name: adapter.module_name.clone(),
        }
    }
}

impl From<&RegistryAdapter> for SelectedAdapter {
    fn from(adapter: &RegistryAdapter) -> Self {
        Self {
            module_name: adapter.module_name.clone(),
            name: adapter.name.clone(),
            project_link: adapter.project_link.clone(),
            version: adapter.version.clone(),
        }
    }
}

impl RegistryAdapter {
    pub fn display(&self) {
        StyledText::new(" ")
            .cyan_bold("  •")
            .cyan_bold(&self.name)
            .text(format!("({})", self.project_link).as_str())
            .green(format!("v{}", self.version).as_str())
            .println();
    }
}
