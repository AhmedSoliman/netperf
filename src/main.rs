use clap::Parser;
use cling::prelude::*;
use log::info;
use netperf::common::opts::Opts;

#[tokio::main]
async fn main() -> ClingFinished<Opts> {
    let opts = Opts::parse();
    // Setting the log-level for (all modules) from the verbosity argument.
    // -v WARN, -vv INFO, -vvv DEBUG, etc.
    let filter = opts.verbose.log_level_filter();
    env_logger::builder().filter_level(filter).init();
    info!("Log Level: {:?}", filter.to_level());
    opts.into_cling().run().await
}
