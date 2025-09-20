use std::{collections::HashSet, fs, path::Path};

use clap::{Subcommand, ValueEnum};
use dialoguer::{MultiSelect, theme::ColorfulTheme};
use regex::Regex;

use crate::{
    error::{NbrError, Result},
    uv,
};
use strum::Display;

#[derive(Subcommand)]
pub enum DriverCommands {
    #[clap(about = "Install drivers")]
    Install {
        #[clap(value_enum, num_args = 1.., value_delimiter = ',')]
        drivers: Option<Vec<Driver>>,
    },
    #[clap(about = "Uninstall drivers")]
    Uninstall,
}

pub(crate) async fn handle_driver(commands: &DriverCommands) -> Result<()> {
    match commands {
        DriverCommands::Install { drivers } => DriverManager::install_driver(drivers).await,
        DriverCommands::Uninstall => DriverManager::uninstall_driver().await,
    }
}

#[derive(ValueEnum, Debug, Clone, Display)]
#[clap(rename_all = "lowercase")]
#[allow(clippy::upper_case_acronyms)]
#[strum(serialize_all = "lowercase")] // strum display, to_string()
pub enum Driver {
    FastAPI,
    HTTPX,
    WebSockets,
    Quart,
    AIOHTTP,
}

pub struct DriverManager;

impl DriverManager {
    async fn install_driver(drivers: &Option<Vec<Driver>>) -> Result<()> {
        // 选择 driver
        let drivers = match drivers {
            Some(drivers) => drivers.into_iter().map(|d| d.to_string()).collect(),
            None => DriverManager::select_drivers(&[])?,
        };

        // uv add
        let package = format!("nonebot2[{}]", drivers.join(","));
        uv::add(vec![&package]).run()?;

        // 更新 env 文件
        let env_files = [Path::new(".env.dev"), Path::new(".env.prod")];

        for env_file in &env_files {
            if !env_file.exists() {
                continue;
            }
            // 读取
            let env_content = fs::read_to_string(env_file)?;

            // DRIVER=~fastapi+~httpx+~websockets, 取出 [fastapi, httpx, websockets]
            let isd_drivers = DriverManager::extract_drivers_from_env(&env_content);

            // 合并 installed_drivers 和 drivers 去重
            let all_drivers = isd_drivers
                .into_iter()
                .chain(drivers.iter().cloned())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            println!("all_drivers: {:?}", all_drivers);

            let env_content = env_content.replace(
                r"DRIVER=[^\s]*",
                &format!(
                    "DRIVER={}",
                    DriverManager::gen_drivers_for_env(&all_drivers)
                ),
            );
            println!("env_content: {}", env_content);
            fs::write(env_file, env_content)?;
        }

        Ok(())
    }

    async fn uninstall_driver() -> Result<()> {
        todo!()
    }

    pub(super) fn select_drivers(defaults: &[bool]) -> Result<Vec<String>> {
        let drivers = Driver::value_variants();
        let selected_drivers = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Which driver(s) would you like to use")
            .items(drivers)
            // 默认选择前三个
            .defaults(defaults)
            .interact()
            .map_err(|e| NbrError::io(e.to_string()))?;

        let selected_drivers: Vec<String> = selected_drivers
            .into_iter()
            .map(|i| drivers[i].to_string())
            .collect();

        if selected_drivers.is_empty() {
            return Self::select_drivers(defaults);
        }

        Ok(selected_drivers)
    }

    /// 生成 env DRIVER 字符串
    pub(super) fn gen_drivers_for_env(drivers: &Vec<String>) -> String {
        drivers
            .iter()
            .map(|driver| String::from("~") + driver)
            .collect::<Vec<String>>()
            .join("+")
    }

    fn extract_drivers_from_env(env_content: &str) -> Vec<String> {
        // DRIVER=~fastapi+~httpx+~websockets, 取出 [fastapi, httpx, websockets]
        let re = Regex::new(r"DRIVER=(?:[^,\n]*?)~(\w+)").unwrap();
        re.captures_iter(env_content)
            .map(|cap| cap[1].to_string())
            .collect()
    }
}
