use log::info;

pub mod client;
pub mod common;
pub mod controller;
pub mod server;

use crate::common::opts::{ClientOpts, CommonOpts, ServerOpts};
use cling::Collected;

async fn run(
    Collected(verbosity): Collected<clap_verbosity_flag::Verbosity>,
    common_opts: &CommonOpts,
    server_opts: &ServerOpts,
    client_opts: &ClientOpts,
) -> Result<(), anyhow::Error> {
    // Setting the log-level for (all modules) from the verbosity argument.
    // -v WARN, -vv INFO, -vvv DEBUG, etc.
    let filter = verbosity.log_level_filter();
    env_logger::builder().filter_level(filter).init();
    info!("Log Level={}", filter.to_level().unwrap());

    if server_opts.server {
        // Server mode... Blocking until we terminate.
        crate::server::run_server(common_opts).await
    } else {
        // Client mode...
        crate::client::run_client(common_opts, client_opts).await
    }
}
