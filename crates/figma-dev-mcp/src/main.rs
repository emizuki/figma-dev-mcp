mod cli;
mod runtime;

use clap::Parser;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    figma_dev_mcp::logging::init();
    let _cli = cli::Cli::parse();
    runtime::run().await
}
