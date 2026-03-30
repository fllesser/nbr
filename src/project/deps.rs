use crate::pyproject::{DependencyGroupItem, DependencyGroups};
use std::collections::HashSet;

use super::options::{DevTool, SelectedAdapter};

pub fn collect_dependencies(adapters: &[SelectedAdapter], drivers: &[String]) -> Vec<String> {
    let mut dependencies = vec![];
    let drivers = drivers.join(",").to_lowercase();
    dependencies.push(format!("nonebot2[{}]>=2.4.3", drivers));

    let adapter_deps = adapters
        .iter()
        .map(|a| format!("{}>={}", a.project_link, a.version))
        .collect::<HashSet<String>>();

    dependencies.extend(adapter_deps);
    dependencies
}

pub fn collect_dependency_groups(dev_tools: &[DevTool]) -> DependencyGroups {
    let mut dep_groups = DependencyGroups::default();
    let mut dev_deps: Vec<DependencyGroupItem> = dev_tools
        .iter()
        .map(|t| DependencyGroupItem::String(t.to_dependency().to_owned()))
        .collect();
    dev_deps.push(DependencyGroupItem::IncludeGroup {
        include_group: "test".to_string(),
    });

    dep_groups.groups.insert(
        "test".to_string(),
        vec![
            DependencyGroupItem::String("nonebug>=0.3.7,<1.0.0".to_string()),
            DependencyGroupItem::String("pytest-asyncio>=1.3.0,<2.0.0".to_string()),
        ],
    );
    dep_groups.groups.insert("dev".to_string(), dev_deps);
    dep_groups
}
