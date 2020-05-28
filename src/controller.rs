use crate::common::consts::INTERNAL_PROT_BUFFER;
use crate::common::control::{ClientMessage, ServerMessage, State, StreamStats, TestResults};
use crate::common::data::{Direction, Role};
use crate::common::net_utils::*;
use crate::common::perf_test::PerfTest;
use crate::common::stream_worker::{StreamWorker, StreamWorkerRef, WorkerMessage};
use crate::common::ui;
use anyhow::{anyhow, Context, Result};
use futures::stream::StreamExt;
use log::{debug, warn};
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::{timeout, Duration};

#[derive(Debug)]
pub enum ControllerMessage {
    // Asks the controller to create a new stream using this socket.
    CreateStream(TcpStream),
    StreamTerminated(usize),
}

pub struct TestController {
    pub sender: Sender<ControllerMessage>,
    test: PerfTest,
    receiver: Receiver<ControllerMessage>,
    stream_results: HashMap<usize, StreamStats>,
}

impl TestController {
    pub fn new(test: PerfTest) -> Self {
        let (sender, receiver) = mpsc::channel(INTERNAL_PROT_BUFFER);
        TestController {
            test,
            sender,
            receiver,
            stream_results: HashMap::new(),
        }
    }

    /// This is the test controller code, when this function terminates, the test is done.
    pub async fn run_controller(mut self) -> Result<()> {
        debug!("Controller has started");
        let mut old_state = self.test.state.clone().lock().await.clone();
        loop {
            // We need to reason about our state and decide on the next step here before
            // processing messages.
            let state = self.test.state.clone().lock().await.clone();
            debug!("Controller state: {:?}", state);
            let role = &self.test.role;
            // TODO: We don't want to lock a mutex on every iteration, this is insane, yet
            // it's not a big deal since this is only for the low-frequency control socket.
            match state {
                State::TestStart if matches!(role, Role::Server) => {
                    // We are a server, we just started the test.
                    debug!("Test is initialising");
                    self.test
                        .set_state(State::CreateStreams {
                            cookie: self.test.cookie.clone(),
                        })
                        .await?;
                    continue;
                }
                State::CreateStreams { cookie: _ } if matches!(role, Role::Server) => {
                    // We are a server, we are waiting for streams to be created.
                    debug!("Waiting for data streams to be connected");
                }
                State::CreateStreams { cookie: _ } if matches!(role, Role::Client) => {
                    // We are a client, we are being asked to create streams.
                    debug!("We should create data streams now");
                    self.create_streams().await?;
                }
                // Only process this on the transition. Start the load testing
                State::Running if old_state != State::Running => {
                    debug!("Streams have been created, starting the load test");
                    // Send the StartLoad signal to all streams.
                    ui::print_header();
                    self.broadcast_to_streams(WorkerMessage::StartLoad);
                }
                // We are asked to exchange the test results we have.
                State::ExchangeResults => {
                    // Do we have active streams yet? We should ask these to terminate and wait
                    // This is best-effort.
                    self.broadcast_to_streams(WorkerMessage::Terminate);
                    let local_results = self.collect_test_result().await?;
                    let remote_results = self.exchange_results(local_results.clone()).await?;
                    self.print_results(local_results, remote_results);
                    break;
                }
                _ => {}
            }
            // Set the old_state as the new state now since we processed the transition already.
            old_state = state;
            if self.test.role == Role::Server {
                // We are a SERVER
                let message = self.receiver.next().await;
                self.process_internal_message(message).await?;
            } else {
                // We are a CLIENT
                let internal_message = self.receiver.next();
                let server_message = client_read_message(&mut self.test.control_socket);
                select! {
                    message = internal_message => self.process_internal_message(message).await? ,
                    message = server_message => self.process_server_message(message).await?,
                    else => break,
                }
            }
        }
        println!("netperf Done!");
        Ok(())
    }

    fn print_results(&self, local: TestResults, remote: TestResults) {
        ui::print_summary(&local, &remote, &self.test.params.direction);
    }

    async fn collect_test_result(&mut self) -> Result<TestResults> {
        // Join all streams and collect results.
        for (id, worker_ref) in self.test.streams.drain() {
            debug!("Waiting on stream {} to terminate", id);
            match timeout(Duration::from_secs(5), worker_ref.join_handle).await {
                Err(_) => warn!(
                    "Timeout waiting on stream {} to terminate, ignoring it.",
                    id
                ),
                Ok(Ok(Ok(result))) => {
                    self.stream_results.insert(id, result);
                    debug!("Stream {} joined.", id);
                }
                Ok(Ok(Err(e))) => {
                    warn!(
                        "Stream {} terminated with error ({}) ignoring its results!",
                        id, e
                    );
                }
                Ok(Err(e)) => warn!("Failed to join stream {}: {}", id, e),
            }
        }
        let results = TestResults {
            streams: self.stream_results.clone(),
        };
        Ok(results)
    }

    /// Exchanges the test results with the other party returning the other party's result.
    async fn exchange_results(&mut self, local_results: TestResults) -> Result<TestResults> {
        if self.test.role == Role::Client {
            // Send ours then read the server's
            client_send_message(
                &mut self.test.control_socket,
                ClientMessage::SendResults(local_results),
            )
            .await?;
            match client_read_message(&mut self.test.control_socket).await? {
                ServerMessage::SendResults(results) => Ok(results),
                e => Err(anyhow!(
                    "Invalid protocol message was sent from the server: {:?}",
                    e
                )),
            }
        } else {
            // On the server side, we read the client's results first.
            let remote_result = match server_read_message(&mut self.test.control_socket).await? {
                ClientMessage::SendResults(results) => Ok(results),
                e => Err(anyhow!(
                    "Invalid protocol message was sent from the server: {:?}",
                    e
                )),
            };
            // Send ours then read the server's
            server_send_message(
                &mut self.test.control_socket,
                ServerMessage::SendResults(local_results),
            )
            .await?;
            remote_result
        }
    }

    fn broadcast_to_streams(&mut self, message: WorkerMessage) {
        for (id, worker_ref) in &mut self.test.streams {
            worker_ref
                .channel
                .try_send(message.clone())
                .unwrap_or_else(|e| {
                    debug!(
                        "Failed to terminate stream {}, it might have been terminated already: {}",
                        id, e
                    )
                });
        }
    }

    /// A handler when we receive a ControllerMessage from other components.
    async fn process_internal_message(
        &mut self,
        message: Option<ControllerMessage>,
    ) -> Result<(), anyhow::Error> {
        let message = message.unwrap();
        match message {
            ControllerMessage::CreateStream(stream) => self.accept_stream(stream).await?,
            ControllerMessage::StreamTerminated(id) => {
                let handle = self.test.streams.remove(&id);
                if let Some(worker_ref) = handle {
                    // join the task to retrieve the result.
                    let result = worker_ref.join_handle.await.with_context(|| {
                        format!("Couldn't join an internal task for stream: {}", id)
                    })?;
                    if let Ok(result) = result {
                        self.stream_results.insert(id, result);
                    } else {
                        warn!(
                            "Stream {} has terminated with an error, we cannot fetch its total \
                             stream stats. This means that the results might be partial",
                            id
                        );
                    }
                }
                if self.test.streams.is_empty() && self.test.role == Role::Server {
                    // No more active streams, let's exchange results and finalise test.
                    let _ = self.test.set_state(State::ExchangeResults).await;
                }
            }
        }
        Ok(())
    }

    async fn process_server_message(
        &mut self,
        message: Result<ServerMessage, anyhow::Error>,
    ) -> Result<(), anyhow::Error> {
        let message = message.with_context(|| "Server terminated!")?;
        match message {
            ServerMessage::SetState(state) => {
                // Update our client state, ignore/panic the error on purpose. The error should
                // never happen on the client side.
                self.test.set_state(state).await?;
            }
            e => println!("Received an unexpected message from the server {:?}", e),
        }
        Ok(())
    }

    // Executed only on the server
    async fn accept_stream(&mut self, stream: TcpStream) -> Result<()> {
        assert_eq!(self.test.role, Role::Server);
        let total_needed_streams: usize =
            (self.test.num_send_streams + self.test.num_receive_streams) as usize;
        self.create_and_register_stream_worker(stream)?;
        if self.test.streams.len() == total_needed_streams {
            // We have all streams ready, switch state and start load.
            self.test.set_state(State::Running).await?;
        }
        Ok(())
    }

    /// Creates a connection to the server that serves as a data stream to test the network.
    /// This method initialises the connection and performs the authentication as well.
    async fn connect_data_stream(&self) -> Result<TcpStream> {
        let address = self.test.client_address.clone().unwrap();
        debug!("Opening a data stream to ({}) ...", address);
        let mut stream = TcpStream::connect(address).await?;
        // Send the hello and authenticate.
        client_send_message(
            &mut stream,
            ClientMessage::Hello {
                cookie: self.test.cookie.clone(),
            },
        )
        .await?;
        // Wait for the initial Welcome message. If the server is busy, this will
        // return an Error of AccessDenied and the client will terminate.
        let _: ServerMessage = client_read_message(&mut stream).await?;
        debug!("Data stream created to {}", peer_to_string(&stream));
        Ok(stream)
    }

    fn create_and_register_stream_worker(&mut self, stream: TcpStream) -> Result<()> {
        // The first (num_send_streams) are sending (meaning that we `Client` are the sending
        // end of this stream)
        let streams_created = self.test.streams.len();
        let mut is_sending = streams_created < self.test.num_send_streams as usize;
        // The case for bidirectional is not that straight forward.
        // This is the only case where we have both sending and receiving streams, in this scenario
        // we need to flip the flag on the server. To explain:
        //
        // Client creates 1 sending, 1 recieving streams. The client will connect them in that order.
        // The server need to first accept the first one, in this case we want to accept it as
        // receiving.
        // As we exhaust the receiving, we will create the sending streams.
        // Hence this flip trick!
        if self.test.role == Role::Server && self.test.params.direction == Direction::Bidirectional
        {
            // Flip!
            is_sending = !is_sending;
        }
        let id = streams_created;
        let (sender, receiver) = mpsc::channel(INTERNAL_PROT_BUFFER);
        let worker = StreamWorker::new(id, stream, self.test.params.clone(), is_sending, receiver);
        let mut controller = self.sender.clone();
        let handle = tokio::spawn(async move {
            let result = worker.run_worker().await;
            controller
                .try_send(ControllerMessage::StreamTerminated(id))
                .unwrap_or_else(|e| debug!("Failed to communicate with controller: {}", e));
            result
        });
        let worker_ref = StreamWorkerRef {
            channel: sender,
            join_handle: handle,
        };
        self.test.streams.insert(id, worker_ref);
        Ok(())
    }

    /// Executed only on the client. Establishes N connections to the server according to the
    /// exchanged `TestParameters`
    async fn create_streams(&mut self) -> Result<()> {
        assert_eq!(self.test.role, Role::Client);
        // Let's connect the send streams first. We will connect and authenticate streams
        // sequentially for simplicity. The server expects all the (client-to-server) streams
        // to be created first. That's an implicit assumption as part of the protocol.
        let total_needed_streams: usize =
            (self.test.num_send_streams + self.test.num_receive_streams) as usize;

        while self.test.streams.len() < total_needed_streams {
            let stream = self.connect_data_stream().await?;
            self.create_and_register_stream_worker(stream)?;
        }
        Ok(())
    }
}
