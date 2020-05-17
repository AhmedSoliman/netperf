use crate::common::consts::DEFAULT_BLOCK_SIZE;
use crate::common::control::*;
use crate::common::data::{Role, TestParameters};
use crate::common::net_utils::*;
use crate::common::*;
use crate::controller::TestController;
use anyhow::Result;
use log::debug;
use tokio::net::TcpStream;

pub async fn run_client(opts: Opts) -> Result<(), anyhow::Error> {
    // We are sure this is set at this point.
    let client_host = opts.client_opts.client.as_ref().unwrap();
    let port = opts.common_opts.port;
    let address = format!("{}:{}", client_host, port);
    print!("Connecting to ({}:{}) ...", client_host, port);
    let mut control_socket = TcpStream::connect(address.clone()).await?;
    println!("Connected!");
    let cookie = uuid::Uuid::new_v4().to_hyphenated().to_string();
    client_send_message(
        &mut control_socket,
        ClientMessage::Hello {
            cookie: cookie.clone(),
        },
    )
    .await?;
    debug!("Hello sent!");
    // Wait for the initial Welcome message. If the server is busy, this will
    // return an Error of AccessDenied and the client will terminate.
    let _: ServerMessage = client_read_message(&mut control_socket).await?;
    debug!("Welcome received!");

    // Sending the header of the test paramters in JSON
    // The format is size(4 bytes)+JSON
    let params = TestParameters::from_opts(&opts, DEFAULT_BLOCK_SIZE);
    client_send_message(
        &mut control_socket,
        ClientMessage::SendParameters(params.clone()),
    )
    .await?;
    debug!("Params sent!");

    let perf_test = PerfTest::new(Some(address), cookie, control_socket, Role::Client, params);
    let controller = TestController::new(perf_test);
    let handle = tokio::spawn(async move {
        controller
            .run_controller()
            .await
            .expect("Test terminated unexpectedly!");
    });
    // Wait for the test to finish.
    handle.await?;
    Ok(())
}
