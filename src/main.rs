use clap::Parser;
use nbr::{cli::Cli, log, uv};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    log::init_logging(cli.verbose);
    uv::self_version().await?;
    cli.run().await?;
    Ok(())
}
