use crate::common::consts;
use crate::common::control::*;
use crate::common::data::{Role, TestParameters};
use crate::common::net_utils::*;
use crate::common::*;
use crate::controller::{ControllerMessage, TestController};
use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
use std::net::Ipv6Addr;
use std::sync::{Arc, Weak};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::timeout;

pub async fn run_server(opts: Opts) -> Result<()> {
    // We use IPv6Addr::UNSPECIFIED here to listen on all
    // IPv4 and IPv6 local interfaces. (dual stack)
    let mut listener = TcpListener::bind((Ipv6Addr::UNSPECIFIED, opts.common_opts.port)).await?;
    ui::print_server_banner(opts.common_opts.port);
    // Handles a single test instance
    //
    // Note that in netperf, we don't run tests concurrently.
    // We will not be accepting more connections unless the test in-flight finishes execution.
    let mut in_flight_test_state: Weak<Mutex<State>> = Weak::new();
    // Initially we will have a None sender until we have a test running in-flight.
    let mut controller_channel: Option<Sender<ControllerMessage>> = None;
    while let Ok((mut inbound, _)) = listener.accept().await {
        let peer = peer_to_string(&inbound);

        // Do we have a test in-flight?
        match in_flight_test_state.upgrade() {
            Some(state_lock) => {
                if let State::CreateStreams { ref cookie } = *state_lock.lock().await {
                    // Validate the cookie in this case.
                    // Read the cookie from the Hello message and compare to cookie.
                    let client_cookie = read_cookie(&mut inbound).await?;
                    // Authentication
                    if client_cookie == *cookie {
                        // Create the stream.
                        server_send_message(&mut inbound, ServerMessage::Welcome).await?;
                        let mut controller = controller_channel.clone().unwrap();
                        controller.try_send(ControllerMessage::CreateStream(inbound))?;
                    } else {
                        let _ = server_send_error(
                            &mut inbound,
                            ServerError::AccessDenied("Test already in-flight".to_owned()),
                        )
                        .await;
                    }
                } else {
                    // We already have a test in-flight, close the connection immediately.
                    // Note that here we don't read anything from the socket to avoid
                    // unnecessarily being blocked on the client not sending any data.
                    info!("Test already in-flight, rejecting connection from {}", peer);
                    let _ = server_send_error(
                        &mut inbound,
                        ServerError::AccessDenied("Test already in-flight".to_owned()),
                    )
                    .await;
                }
            }
            None => {
                // No in-flight test, let's create one.
                info!("Accepted connection from {}", peer);
                // Do we have an active test running already?
                // If not, let's start a test session and wait for parameters from the client
                match create_test(inbound).await {
                    Ok(test) => {
                        info!("[{}] Test Created", peer);
                        in_flight_test_state = Arc::downgrade(&test.state);
                        // Keep a weak-ref to this test here.
                        // Async dispatch.
                        let controller = TestController::new(test);
                        controller_channel = Some(controller.sender.clone());
                        tokio::spawn(async move {
                            let _ = controller.run_controller().await;
                        });
                    }
                    Err(e) => {
                        error!("[{}] {}", peer, e);
                    }
                };
            }
        };
    }
    Ok(())
}

async fn create_test(mut control_socket: TcpStream) -> Result<PerfTest> {
    // It's important that we finish this initial negotiation quickly, we are setting
    // up a race between these reads and a timeout timer (5 seconds) for each read to
    // ensure we don't end up waiting forever and not accepting new potential tests.

    let cookie = read_cookie(&mut control_socket).await?;
    debug!("Hello received: {}", cookie);
    // Sending the WELCOME message first since this is an accepted attempt.
    server_send_message(&mut control_socket, ServerMessage::Welcome).await?;
    // Reading the test parameters length
    let params = timeout(
        consts::PROTOCOL_TIMEOUT,
        read_test_parameters(&mut control_socket),
    )
    .await
    .context("Timed out waiting for the protocol negotiation!")??;
    Ok(PerfTest::new(
        None, // No client_address, we are a server. [Not needed]
        cookie,
        control_socket,
        Role::Server,
        params,
    ))
}

async fn read_test_parameters(stream: &mut TcpStream) -> Result<TestParameters> {
    // Since we don't know the block size yet,
    // we will need to assume the message length size.
    match server_read_message(stream).await? {
        ClientMessage::SendParameters(params) => Ok(params),
        e => Err(anyhow!(
            "Unexpected message, we expect SendParameters, instead we got {:?}",
            e
        )),
    }
}

async fn read_cookie(socket: &mut TcpStream) -> Result<String> {
    match server_read_message(socket).await {
        Ok(ClientMessage::Hello { cookie }) => Ok(cookie),
        Ok(e) => Err(anyhow!(
            "Client sent the wrong welcome message, got: {:?}",
            e
        )),
        Err(e) => Err(anyhow!("Failed to finish initial negotiation: {:?}", e)),
    }
}
