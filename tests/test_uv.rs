mod common;
use nbr::uv;

#[tokio::test]
async fn test_uv_operations_snapshot() {
    let (_dir, project_path) = common::create_temp_project(true).await;

    let package = uv::show_package_info("nonebot2", Some(&project_path))
        .await
        .unwrap();

    insta::assert_yaml_snapshot!("uv_show_package_info", package, {
        ".location" => "[LOCATION]",
        ".version" => "[VERSION]",
    });
}
