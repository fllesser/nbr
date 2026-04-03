pub use crate::adapter::AdapterManager;
pub use crate::adapter::RegistryAdapter;
use crate::cli::GlobalContext;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AdapterCommands {
    #[clap(about = "Install adapters")]
    Install {
        #[clap(short, long, help = "Fetch adapters from remote")]
        fetch_remote: bool,
    },
    #[clap(about = "Uninstall adapters")]
    Uninstall,
    #[clap(about = "List installed adapters, show all adapters if --all is set")]
    List {
        #[clap(short, long, help = "Show all adapters")]
        all: bool,
    },
}

/// Handle the adapter command
pub async fn handle(commands: &AdapterCommands, ctx: GlobalContext) -> Result<()> {
    let adapter_manager = AdapterManager::new(None, ctx)?;

    match commands {
        AdapterCommands::Install { fetch_remote } => {
            adapter_manager.install_adapters(*fetch_remote).await?
        }
        AdapterCommands::Uninstall => adapter_manager.uninstall_adapters().await?,
        AdapterCommands::List { all } => adapter_manager.list_adapters(*all).await?,
    }
    Ok(())
}
