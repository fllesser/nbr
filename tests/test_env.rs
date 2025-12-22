mod common;
use nbr::cli::env::EnvironmentChecker;

#[tokio::test]
async fn test_environment_checker_creation() {
    let (_dir, project_path) = common::create_temp_project(false).await;
    let checker = EnvironmentChecker::new(project_path);
    assert!(checker.is_ok());
}
