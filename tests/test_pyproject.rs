mod common;
use nbr::pyproject::{Adapter, NbTomlEditor, PyProjectConfig};

#[tokio::test]
async fn test_add_adapters() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let mut editor = NbTomlEditor::with_work_dir(Some(&project_path)).unwrap();
    editor
        .add_adapters(vec![Adapter {
            name: "OneBot V11".to_string(),
            module_name: "nonebot.adapters.onebot.v11".to_string(),
        }])
        .unwrap();
    editor
        .add_adapters(vec![Adapter {
            name: "OneBot V12".to_string(),
            module_name: "nonebot.adapters.onebot.v12".to_string(),
        }])
        .unwrap();
}

#[tokio::test]
async fn test_add_plugins() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let mut editor = NbTomlEditor::with_work_dir(Some(&project_path)).unwrap();

    editor
        .add_plugins(vec![
            "nonebot_plugin_status",
            "nonebot_plugin_alconna",
            "nonebot_plugin_waiter",
        ])
        .unwrap();
}

#[tokio::test]
async fn test_remove_plugins() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let mut editor = NbTomlEditor::with_work_dir(Some(&project_path)).unwrap();

    // First add a plugin to remove
    editor.add_plugins(vec!["nonebot_plugin_status"]).unwrap();

    editor
        .remove_plugins(vec!["nonebot_plugin_status"])
        .unwrap();
}

#[tokio::test]
async fn test_parse_toml_to_nonebot() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let pyproject = PyProjectConfig::parse(Some(&project_path)).unwrap();
    let nonebot = pyproject.nonebot().unwrap();
    assert!(!nonebot.adapters.as_ref().unwrap().is_empty());
}
