mod common;
use nbr::uv;

#[tokio::test]
async fn test_uv_operations_snapshot() {
    let (_dir, project_path) = common::create_temp_project(true).await;

    // Test show_package_info
    let package = uv::show_package_info("nonebot2", Some(&project_path))
        .await
        .unwrap();

    insta::assert_yaml_snapshot!("uv_show_package_info", package, {
        ".location" => "[LOCATION]",
        ".version" => "[VERSION]",
    });

    // Test list
    let mut packages = uv::list(false).await.unwrap();
    // Filter or truncate to make it more stable
    packages.retain(|p| p.name == "nonebot2" || p.name.contains("adapter"));
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    insta::assert_yaml_snapshot!("uv_list", packages, {
        "[].location" => "[LOCATION]",
        "[].version" => "[VERSION]",
    });
}
