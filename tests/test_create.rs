mod common;
use std::fs;

#[tokio::test]
async fn test_create_project_snapshot() {
    let (_dir, output_dir) = common::create_temp_project(false).await;

    // Snapshot pyproject.toml
    let pyproject_content = fs::read_to_string(output_dir.join("pyproject.toml")).unwrap();
    let pyproject_toml: toml::Value = toml::from_str(&pyproject_content).unwrap();
    insta::assert_yaml_snapshot!(pyproject_toml);

    // Snapshot .env.dev
    let env_dev_content = fs::read_to_string(output_dir.join(".env.dev")).unwrap();
    insta::assert_snapshot!(env_dev_content);

    // Snapshot Dockerfile
    let dockerfile_content = fs::read_to_string(output_dir.join("Dockerfile")).unwrap();
    insta::assert_snapshot!(dockerfile_content);
}
