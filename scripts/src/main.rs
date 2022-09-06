mod command;

use crate::command::Commands;
use anyhow::Context;
use clap::Parser;
use tracing::metadata::LevelFilter;
use tracing_subscriber::FmtSubscriber;

/// Args.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Log level.
    #[clap(long, default_value = "INFO")]
    log_level: LevelFilter,

    #[clap(subcommand)]
    command: Commands,
}

fn setup_log(args: &Args) -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(args.log_level)
        .finish();

    tracing::subscriber::set_global_default(subscriber).context("setting default subscriber failed")
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    setup_log(&args)?;
    args.command.run()?;
    Ok(())
}
