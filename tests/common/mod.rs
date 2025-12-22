use nbr::cli::adapter::RegistryAdapter;
use nbr::cli::create::{
    BuiltinPlugin, DevTool, Environment, ProjectOptions, Template, create_project,
};
use std::path::PathBuf;
use tempfile::{TempDir, tempdir};

pub async fn create_temp_project(create_venv: bool) -> (TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let output_dir = dir.path().to_path_buf();

    let options = ProjectOptions {
        name: "test-bot".to_string(),
        template: Template::Bootstrap,
        output_dir: output_dir.clone(),
        drivers: vec!["fastapi".to_string()],
        adapters: vec![RegistryAdapter {
            name: "OneBot V11".to_string(),
            module_name: "nonebot.adapters.onebot.v11".to_string(),
            project_link: "nonebot-adapter-onebot".to_string(),
            version: "2.4.6".to_string(),
            author: "yanyongyu".to_string(),
            desc: "OneBot V11 协议".to_string(),
            homepage: Some("https://onebot.adapters.nonebot.dev".to_string()),
            tags: vec![],
            is_official: true,
            time: "2024-10-24T07:34:56.115315Z".to_string(),
        }],
        plugins: vec![BuiltinPlugin::Echo.to_string()],
        python_version: "3.12".to_string(),
        environment: Environment::Dev,
        dev_tools: vec![DevTool::Ruff],
        gen_dockerfile: true,
        create_venv,
    };

    create_project(&options).await.unwrap();
    (dir, output_dir)
}
