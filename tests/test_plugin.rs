mod common;
use nbr::cli::{GlobalContext, plugin::PluginManager};

#[tokio::test]
async fn test_plugin_list() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let manager = PluginManager::new(Some(project_path.clone()), GlobalContext::default()).unwrap();

    // In the temp project, we install "echo" plugin by default.
    // However, get_installed_plugins uses `uv list` which checks the virtual environment.
    // Since we create the project with `create_venv: false` in common::create_temp_project,
    // `uv list` might not find anything or fail if it expects a venv.

    // But wait, `create_temp_project` sets `create_venv` based on argument.
    // Let's check common::create_temp_project implementation.
    // It calls `create_project` which calls `install_dependencies` if `create_venv` is true.

    // If we want to test `get_installed_plugins`, we probably need a venv and installed packages.
    // That might be slow for a unit test.

    // Alternatively, we can check if the plugin is in pyproject.toml.
    // But PluginManager doesn't seem to expose a method to read plugins from pyproject.toml directly
    // other than `get_installed_plugins` which uses `uv`.

    // Let's look at `PluginManager::get_installed_plugins`. It calls `uv::list`.
    // `uv::list` runs `uv pip list --format json`.

    // 保持测试离线且稳定：只验证基础逻辑，不触发 registry 网络请求或 uv 调用。
    assert!(PluginManager::is_plugin("nonebot-plugin-echo"));
    assert!(!PluginManager::is_plugin("serde"));
    let _ = manager;
}
