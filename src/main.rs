use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;

mod cli;
mod config;
mod error;
mod log;
mod pyproject;
mod utils;
mod uv;

use cli::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
// nbr banner
const BANNER: &str = r#"
d8b   db  .d88b.  d8b   db d88888b d8888b.  .d88b.  d888888b
888o  88 .8P  Y8. 888o  88 88'     88  `8D .8P  Y8. `~~88~~'
88V8o 88 88    88 88V8o 88 88ooooo 88oooY' 88    88    88
88 V8o88 88    88 88 V8o88 88~~~~~ 88~~~b. 88    88    88
88  V888 `8b  d8' 88  V888 88.     88   8D `8b  d8'    88
VP   V8P  `Y88P'  VP   V8P Y88888P Y8888P'  `Y88P'     YP
"#;

const AUTHOR: &str = "fllesser";
const ABOUT: &str = "CLI for NoneBot2 - Rust implementation";

#[derive(Parser)]
#[command(name = "nbr", version = VERSION, about = ABOUT, author = AUTHOR, before_help = BANNER.bright_cyan().to_string(), arg_required_else_help = true)]
pub struct CLI {
    #[clap(subcommand)]
    pub commands: NbrCommands,
    #[clap(
        short,
        long,
        default_value = "0",
        help = "Verbose level, 0: INFO, 1: DEBUG, 2: TRACE"
    )]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum NbrCommands {
    #[clap(about = "Create a new project")]
    Create(create::CreateArgs),
    #[clap(about = "Run the bot")]
    Run {
        #[clap()] // 位置参数
        file: Option<String>,
        #[clap(short, long)]
        reload: bool,
    },
    #[clap(about = "Manage plugins")]
    Plugin {
        #[clap(subcommand)]
        plugin_commands: plugin::PluginCommands,
    },
    #[clap(about = "Manage adapters")]
    Adapter {
        #[clap(subcommand)]
        adapter_commands: adapter::AdapterCommands,
    },
    #[clap(about = "Generate bot entry file")]
    Generate {
        #[clap(short, long)]
        force: bool,
    },
    #[clap(about = "unimplemented")]
    Init {
        #[clap(short, long)]
        name: String,
        #[clap(short, long)]
        force: bool,
    },
    #[clap(about = "Check environment")]
    Env {
        #[clap(subcommand)]
        env_commands: EnvCommands,
    },
    #[clap(about = "unimplemented")]
    Cache {
        #[clap(subcommand)]
        cache_commands: CacheCommands,
    },
}

#[derive(Subcommand)]
pub enum EnvCommands {
    Info,
    Check,
}

#[derive(Subcommand)]
pub enum CacheCommands {
    Clear,
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = CLI::parse();

    log::init_logging(cli.verbose);

    // Check if uv is installed
    uv::self_version().await?;

    match cli.commands {
        NbrCommands::Create(create_args) => create::handle_create(create_args).await?,
        NbrCommands::Run { file, reload } => run::handle_run(file, reload).await?,
        NbrCommands::Plugin { plugin_commands } => plugin::handle_plugin(&plugin_commands).await?,
        NbrCommands::Adapter { adapter_commands } => {
            adapter::handle_adapter(&adapter_commands).await?
        }
        NbrCommands::Generate { force } => generate::handle_generate(force).await?,
        NbrCommands::Init { .. } => unimplemented!(),
        NbrCommands::Env { env_commands } => env::handle_env(&env_commands).await?,
        NbrCommands::Cache { cache_commands } => cache::handle_cache(&cache_commands).await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
