use crate::common::control::{ServerMessage, State};
use crate::common::data::*;
use crate::common::net_utils::*;
use crate::common::stream_worker::StreamWorkerRef;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Debug)]
pub struct StreamStats {
    pub start: Instant,
    pub from: Instant,
    pub duration: Duration,
    pub bytes_transferred: usize,
}

/// A master object that holds the state of this perf test run.
pub struct PerfTest {
    // The connection address to the client (Only set on clients)
    pub client_address: Option<String>,
    // A unique identifier for this test run, used by clients to authenticate data streams.
    pub cookie: String,
    // Represents where we are in the lifecyle of a test.
    pub state: Arc<Mutex<State>>,
    // The control socket stream.
    pub control_socket: TcpStream,
    // Defines whether we are a server or client in this test run.
    pub role: Role,
    // The test configuration.
    pub params: TestParameters,
    // The set of data streams
    pub streams: HashMap<usize, StreamWorkerRef>,
    // The number of streams at which we are sending data
    pub num_send_streams: u16,
    // The number of streams at which we are receiving data
    pub num_receive_streams: u16,
}

impl PerfTest {
    pub fn new(
        client_address: Option<String>,
        cookie: String,
        control_socket: TcpStream,
        role: Role,
        params: TestParameters,
    ) -> Self {
        // Let's assume we are client.
        let mut num_send_streams: u16 = 0;
        let mut num_receive_streams: u16 = 0;
        match params.direction {
            Direction::ClientToServer => num_send_streams = params.parallel,
            Direction::ServerToClient => num_receive_streams = params.parallel,
            Direction::Bidirectional => {
                num_send_streams = params.parallel;
                num_receive_streams = params.parallel;
            }
        }

        if matches!(role, Role::Server) {
            // swap the streams ;)
            std::mem::swap(&mut num_send_streams, &mut num_receive_streams);
        }
        PerfTest {
            client_address,
            cookie,
            state: Arc::new(Mutex::new(State::TestStart)),
            control_socket,
            params,
            role,
            streams: HashMap::new(),
            num_send_streams,
            num_receive_streams,
        }
    }
    /// Sets the state of the state machine and syncs that to the client through the control socket
    pub async fn set_state(&mut self, state: State) -> Result<()> {
        if self.role == Role::Server {
            // We need to sync that state to the client.
            server_send_message(
                &mut self.control_socket,
                ServerMessage::SetState(state.clone()),
            )
            .await?;
        }
        // We can perform state transition validation here if necessary.
        let mut locked_state = self.state.lock().await;
        *locked_state = state;
        Ok(())
    }
    /// Returns the current state of this test
    pub async fn get_state_clone(&self) -> State {
        self.state.lock().await.clone()
    }
}
