mod common;
use nbr::cli::{GlobalContext, adapter::AdapterManager};

#[tokio::test]
async fn test_adapter_list() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let manager =
        AdapterManager::new(Some(project_path.clone()), GlobalContext::default()).unwrap();

    let installed_adapters = manager.get_installed_adapters_names();
    assert!(!installed_adapters.is_empty());
    assert!(installed_adapters.contains(&"OneBot V11"));
}
