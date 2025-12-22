mod common;
use nbr::pyproject::{Adapter, NbTomlEditor, PyProjectConfig};

#[tokio::test]
async fn test_add_adapters() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let mut editor = NbTomlEditor::with_work_dir(Some(&project_path)).unwrap();

    editor
        .add_adapters(vec![Adapter {
            name: "OneBot V12".to_string(),
            module_name: "nonebot.adapters.onebot.v12".to_string(),
        }])
        .unwrap();
    let pyproject = PyProjectConfig::parse(Some(&project_path)).unwrap();
    assert_eq!(
        pyproject.nonebot().unwrap().adapters.as_ref().unwrap(),
        &vec![
            Adapter {
                name: "OneBot V11".to_string(),
                module_name: "nonebot.adapters.onebot.v11".to_string(),
            },
            Adapter {
                name: "OneBot V12".to_string(),
                module_name: "nonebot.adapters.onebot.v12".to_string(),
            }
        ]
    );
}

#[tokio::test]
async fn test_add_plugins() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let mut editor = NbTomlEditor::with_work_dir(Some(&project_path)).unwrap();
    let plugins = vec![
        "nonebot_plugin_status",
        "nonebot_plugin_alconna",
        "nonebot_plugin_waiter",
    ];
    editor.add_plugins(plugins.clone()).unwrap();
    let pyproject = PyProjectConfig::parse(Some(&project_path)).unwrap();
    let pyproject_plugins = pyproject.nonebot().unwrap().plugins.as_ref().unwrap();
    assert_eq!(pyproject_plugins.len(), plugins.len());
    pyproject_plugins
        .iter()
        .for_each(|p| assert!(plugins.contains(&p.as_str())));
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

    let pyproject = PyProjectConfig::parse(Some(&project_path)).unwrap();
    let pyproject_plugins = pyproject.nonebot().unwrap().plugins.as_ref().unwrap();
    assert_eq!(pyproject_plugins.len(), 0);
}
