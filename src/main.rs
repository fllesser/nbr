use clap::Parser;
use nbr::{cli::Cli, log, uv};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    log::init_logging(cli.verbose);

    if let Err(err) = run(cli).await {
        tracing::error!("{err}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    uv::self_version().await?;
    cli.run().await?;
    Ok(())
}
