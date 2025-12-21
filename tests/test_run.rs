mod common;
use nbr::cli::run::load_environment_variables;
use std::fs;

#[tokio::test]
async fn test_load_environment_variables() {
    let (_dir, project_path) = common::create_temp_project(false).await;

    // Add some custom env vars to .env
    let env_path = project_path.join(".env");
    let mut content = fs::read_to_string(&env_path).unwrap();
    content.push_str("\nTEST_VAR=test_value\nANOTHER_VAR=\"quoted value\"\n");
    fs::write(&env_path, content).unwrap();

    let env_vars = load_environment_variables(&project_path).unwrap();
    assert!(env_vars.contains_key("TEST_VAR"));
    assert_eq!(env_vars.get("TEST_VAR").unwrap(), "test_value");
    assert!(env_vars.contains_key("ANOTHER_VAR"));
    assert_eq!(env_vars.get("ANOTHER_VAR").unwrap(), "quoted value");
}
