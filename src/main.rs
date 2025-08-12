use anyhow::Result;
use clap::{Arg, Command};
use colored::*;
use tracing::info;

mod cli;
mod config;
mod error;
mod pyproject;
mod utils;

use cli::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BANNER: &str = r#"
d8b   db  .d88b.  d8b   db d88888b d8888b.  .d88b.  d888888b
888o  88 .8P  Y8. 888o  88 88'     88  `8D .8P  Y8. `~~88~~'
88V8o 88 88    88 88V8o 88 88ooooo 88oooY' 88    88    88
88 V8o88 88    88 88 V8o88 88~~~~~ 88~~~b. 88    88    88
88  V888 `8b  d8' 88  V888 88.     88   8D `8b  d8'    88
VP   V8P  `Y88P'  VP   V8P Y88888P Y8888P'  `Y88P'     YP
"#;

fn build_cli() -> Command {
    Command::new("nb")
        .version(VERSION)
        .author("fllesser")
        .about("CLI for NoneBot2 - Rust implementation")
        .before_help(BANNER.bright_cyan().to_string())
        .arg_required_else_help(true)
        .subcommand(
            Command::new("create")
                .about("Create a new NoneBot project")
                .arg(
                    Arg::new("name")
                        .help("Project name")
                        .required(false)
                        .index(1),
                )
                .arg(
                    Arg::new("template")
                        .long("template")
                        .short('t')
                        .help("Template to use")
                        .value_name("TEMPLATE"),
                )
                .arg(
                    Arg::new("output")
                        .long("output")
                        .short('o')
                        .help("Output directory")
                        .value_name("DIR"),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .short('f')
                        .help("Force creation even if directory exists")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("run")
                .about("Run the bot in current folder")
                .arg(
                    Arg::new("file")
                        .help("Bot entry file")
                        .required(false)
                        .index(1),
                )
                .arg(
                    Arg::new("reload")
                        .long("reload")
                        .short('r')
                        .help("Enable auto-reload")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("host")
                        .long("host")
                        .help("Host to bind")
                        .value_name("HOST")
                        .default_value("127.0.0.1"),
                )
                .arg(
                    Arg::new("port")
                        .long("port")
                        .short('p')
                        .help("Port to bind")
                        .value_name("PORT")
                        .default_value("8080"),
                ),
        )
        .subcommand(
            Command::new("plugin")
                .about("Manage bot plugins")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("install")
                        .about("Install a plugin")
                        .arg(
                            Arg::new("name")
                                .help("Plugin name or PyPI package")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("index")
                                .long("index")
                                .short('i')
                                .help("PyPI index URL")
                                .value_name("URL"),
                        )
                        .arg(
                            Arg::new("upgrade")
                                .long("upgrade")
                                .short('U')
                                .help("Upgrade if already installed")
                                .action(clap::ArgAction::SetTrue),
                        ),
                )
                .subcommand(
                    Command::new("uninstall")
                        .about("Uninstall a plugin")
                        .arg(Arg::new("name").help("Plugin name").required(true).index(1)),
                )
                .subcommand(
                    Command::new("list").about("List installed plugins").arg(
                        Arg::new("outdated")
                            .long("outdated")
                            .help("Show outdated plugins only")
                            .action(clap::ArgAction::SetTrue),
                    ),
                )
                .subcommand(
                    Command::new("search")
                        .about("Search plugins")
                        .arg(
                            Arg::new("query")
                                .help("Search query")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("limit")
                                .long("limit")
                                .short('l')
                                .help("Limit search results")
                                .value_name("NUM")
                                .default_value("10"),
                        ),
                )
                .subcommand(
                    Command::new("update")
                        .about("Update plugins")
                        .arg(
                            Arg::new("all")
                                .long("all")
                                .help("Update all plugins")
                                .action(clap::ArgAction::SetTrue),
                        )
                        .arg(
                            Arg::new("name")
                                .help("Plugin name to update")
                                .required(false)
                                .index(1),
                        ),
                ),
        )
        .subcommand(
            Command::new("adapter")
                .about("Manage bot adapters")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("install").about("Install an adapter").arg(
                        Arg::new("name")
                            .help("Adapter name")
                            .required(true)
                            .index(1),
                    ),
                )
                .subcommand(
                    Command::new("uninstall").about("Uninstall an adapter").arg(
                        Arg::new("name")
                            .help("Adapter name")
                            .required(true)
                            .index(1),
                    ),
                )
                .subcommand(
                    Command::new("list")
                        .about("List available and installed adapters")
                        .arg(
                            Arg::new("all")
                                .long("all")
                                .short('a')
                                .help("List all adapters, including uninstalled ones")
                                .action(clap::ArgAction::SetTrue),
                        ),
                ),
        )
        .subcommand(
            Command::new("generate")
                .about("Generate bot entry file")
                .arg(
                    Arg::new("file")
                        .help("Output file name")
                        .default_value("bot.py")
                        .index(1),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .short('f')
                        .help("Overwrite existing file")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("init")
                .about("Initialize a NoneBot project in current directory")
                .arg(
                    Arg::new("name")
                        .help("Project name")
                        .required(false)
                        .index(1),
                )
                .arg(
                    Arg::new("force")
                        .long("force")
                        .short('f')
                        .help("Force initialization even if files exist")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("env")
                .about("Environment management")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(Command::new("info").about("Show environment information"))
                .subcommand(Command::new("check").about("Check environment dependencies")),
        )
        .subcommand(
            Command::new("cache")
                .about("Cache management")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(Command::new("clear").about("Clear all caches"))
                .subcommand(Command::new("info").about("Show cache information")),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Enable verbose output")
                .action(clap::ArgAction::Count),
        )
        .arg(
            Arg::new("quiet")
                .long("quiet")
                .short('q')
                .help("Suppress output")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("verbose"),
        )
}

fn setup_logging(_verbose_level: u8, quiet: bool) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*};

    if quiet {
        return Ok(());
    }

    // let filter = match verbose_level {
    //     0 => "nb_cli=info",
    //     1 => "nb_cli=debug",
    //     _ => "nb_cli=trace",
    // };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        //.with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)))
        .init();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = build_cli().get_matches();

    let verbose = matches.get_count("verbose");
    let quiet = matches.get_flag("quiet");

    setup_logging(verbose, quiet)?;

    info!("Starting nb-cli v{}", VERSION);

    match matches.subcommand() {
        Some(("create", sub_matches)) => create::handle_create(sub_matches).await?,
        Some(("run", sub_matches)) => run::handle_run(sub_matches).await?,
        Some(("plugin", sub_matches)) => plugin::handle_plugin(sub_matches).await?,
        Some(("adapter", sub_matches)) => adapter::handle_adapter(sub_matches).await?,
        Some(("generate", sub_matches)) => generate::handle_generate(sub_matches).await?,
        Some(("init", sub_matches)) => init::handle_init(sub_matches).await?,
        Some(("env", sub_matches)) => env::handle_env(sub_matches).await?,
        Some(("cache", sub_matches)) => cache::handle_cache(sub_matches).await?,
        _ => {
            println!("{}", BANNER.bright_cyan());
            println!("{}", "Welcome to NoneBot CLI!".bright_green());
            println!("{}", "Use --help for more information.".bright_blue());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_creation() {
        let cmd = build_cli();
        assert_eq!(cmd.get_name(), "nb");
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
