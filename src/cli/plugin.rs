use super::GlobalContext;
use crate::plugin::InstallOptions;
pub use crate::plugin::PluginManager;
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum PluginCommands {
    #[clap(about = "Install a plugin")]
    Install {
        #[clap(help = "Plugin name")]
        name: String,
        #[clap(short, long, help = "Specify the index url")]
        index: Option<String>,
        #[clap(short, long, help = "Upgrade the plugin")]
        upgrade: bool,
        #[clap(short, long, help = "Reinstall the plugin")]
        reinstall: bool,
        #[clap(short, long, help = "Fetch plugins from remote")]
        fetch_remote: bool,
    },
    #[clap(about = "Uninstall a plugin")]
    Uninstall {
        #[clap(help = "Plugin name")]
        name: String,
    },
    #[clap(about = "List installed plugins, show outdated plugins if --outdated is set")]
    List {
        #[clap(short, long, help = "Show outdated plugins")]
        outdated: bool,
    },
    #[clap(about = "Search plugins in registry")]
    Search {
        #[clap(help = "Search keyword")]
        query: String,
        #[clap(
            short,
            long,
            default_value = "10",
            help = "Limit the number of search results"
        )]
        limit: usize,
        #[clap(short, long, help = "Fetch plugins from remote")]
        fetch_remote: bool,
    },
    #[clap(about = "Update plugin(s)")]
    Update {
        #[clap(help = "Plugin name")]
        name: Option<String>,
        #[clap(short, long, help = "Update all plugins")]
        all: bool,
        #[clap(short, long, help = "Reinstall the plugin")]
        reinstall: bool,
    },
    #[clap(about = "Reset nonebot plugins, remove invalid plugins and add missing plugins")]
    Reset,
    #[clap(about = "Create a new plugin")]
    Create,
}

pub async fn handle(commands: &PluginCommands, ctx: GlobalContext) -> Result<()> {
    let mut manager = PluginManager::new(None, ctx)?;
    match commands {
        PluginCommands::Install {
            name,
            index,
            upgrade,
            reinstall,
            fetch_remote,
        } => {
            let options = InstallOptions::new(name, *upgrade, *reinstall, index.as_deref())?;
            manager.install(options, *fetch_remote).await?
        }
        PluginCommands::Uninstall { name } => manager.uninstall(name).await?,
        PluginCommands::List { outdated } => manager.list(*outdated).await?,
        PluginCommands::Search {
            query,
            limit,
            fetch_remote,
        } => manager.search_plugins(query, *limit, *fetch_remote).await?,
        PluginCommands::Update {
            name,
            all,
            reinstall,
        } => manager.update(name.as_deref(), *all, *reinstall).await?,
        PluginCommands::Reset => manager.reset().await?,
        PluginCommands::Create => {
            unimplemented!()
        }
    }
    Ok(())
}
