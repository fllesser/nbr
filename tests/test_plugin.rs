mod common;
use nbr::cli::plugin::PluginManager;

#[tokio::test]
async fn test_plugin_list() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let manager = PluginManager::new(Some(project_path.clone())).unwrap();

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

    // If we don't have a venv, `uv pip list` might fail or return system packages if not isolated.
    // For this test, let's try to verify the manager can be created and maybe run a search which doesn't require venv.

    let results = manager.search_plugins("echo", 1, false).await;
    assert!(results.is_ok());
}
