pub mod client;
pub mod common;
pub mod controller;
pub mod server;

use crate::common::opts::{ClientOpts, CommonOpts, ServerOpts};

async fn run(
    common_opts: &CommonOpts,
    server_opts: &ServerOpts,
    client_opts: &ClientOpts,
) -> Result<(), anyhow::Error> {
    if server_opts.server {
        // Server mode... Blocking until we terminate.
        crate::server::run_server(common_opts).await
    } else {
        // Client mode...
        crate::client::run_client(common_opts, client_opts).await
    }
}
