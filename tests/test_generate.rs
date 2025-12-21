mod common;
use nbr::cli::generate::generate_bot_file;
use std::fs;

#[tokio::test]
async fn test_generate_bot_file_snapshot() {
    let (_dir, project_path) = common::create_temp_project(false).await;

    // Generate bot file
    generate_bot_file(&project_path, true).await.unwrap();

    // Snapshot bot.py
    let bot_py_content = fs::read_to_string(project_path.join("bot.py")).unwrap();
    insta::assert_snapshot!(bot_py_content);
}
