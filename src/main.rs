use anyhow::Result;
use clap::Parser;
use log::info;
use netperf::common::opts::Opts;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let opts = Opts::parse();
    // Setting the log-level for (all modules) from the verbosity argument.
    // -v WARN, -vv INFO, -vvv DEBUG, etc.
    let log_level = opts.verbose.log_level().unwrap_or(log::Level::Error);
    env_logger::builder()
        .filter_level(log_level.to_level_filter())
        .init();
    info!("LogLevel: {}", log_level);
    if opts.server_opts.server {
        // Server mode... Blocking until we terminate.
        netperf::server::run_server(opts).await
    } else {
        // Client mode...
        netperf::client::run_client(opts).await
    }
}
