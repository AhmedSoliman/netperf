use crate::common::control::StreamStats;
use crate::common::data::TestParameters;
use crate::common::ui;
use anyhow::{bail, Result};
use futures::FutureExt;
use log::{debug, warn};
use std::convert::TryInto;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::stream::StreamExt;
use tokio::sync::mpsc::{error::TryRecvError, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{delay_for, timeout, Instant};

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    StartLoad,
    Terminate,
}

/// Represents a connected stream connection.
pub struct StreamWorkerRef {
    pub channel: Sender<WorkerMessage>,
    pub join_handle: JoinHandle<Result<StreamStats>>,
}

pub struct StreamWorker {
    pub id: usize,
    pub stream: TcpStream,
    pub params: TestParameters,
    pub is_sending: bool,
    receiver: Receiver<WorkerMessage>,
}

impl StreamWorker {
    pub fn new(
        id: usize,
        stream: TcpStream,
        params: TestParameters,
        is_sending: bool,
        receiver: Receiver<WorkerMessage>,
    ) -> Self {
        StreamWorker {
            id,
            stream,
            params,
            is_sending,
            receiver,
        }
    }

    pub async fn run_worker(mut self) -> Result<StreamStats> {
        // Let's pre-allocate a buffer for 1 block;
        let block_size = self.params.block_size;
        let mut buffer: Vec<u8> = vec![0; block_size];
        // Let's fill the buffer with random block if we are a sender
        if self.is_sending {
            let mut random = File::open("/dev/urandom").await?;
            let count = random.read_exact(&mut buffer).await?;
            // The urandom buffer should be available to read the exact buffer we want
            assert_eq!(count, block_size);
        }

        self.configure_stream_socket()?;
        // First thing is that we need to wait for the `StartLoad` signal to start sending or
        // receiving data. The `StartLoad` signal comes in after the server receives all the
        // expected data stream connections as exchanged through the TestParameters.
        debug!(
            "Data stream {} created ({}), waiting for the StartLoad signal!",
            self.id,
            if self.is_sending {
                "sending"
            } else {
                "receiving"
            }
        );
        let signal = self.receiver.next().await;
        if !matches!(signal, Some(WorkerMessage::StartLoad)) {
            bail!("Internal communication channel for stream was terminated unexpectedly!");
        }
        // TODO: Connect to the cmdline args.
        let interval = Duration::from_secs(1);
        let start_time = Instant::now();
        let timeout_timer = delay_for(Duration::from_secs(self.params.time_seconds));
        let mut bytes_transferred: usize = 0;
        let mut syscalls: usize = 0;
        let mut current_interval_start = Instant::now();
        let mut current_interval_bytes_transferred: usize = 0;
        let mut current_interval_syscalls: usize = 0;
        loop {
            // Are we done?
            if timeout_timer.is_elapsed() {
                debug!("Test time is up!");
                break;
            }
            let internal_message = self.receiver.try_recv();
            match internal_message {
                Ok(WorkerMessage::StartLoad) => {
                    warn!(
                        "Unexpected StartLoad from controller, we are already running with load!"
                    );
                }
                Ok(WorkerMessage::Terminate) => {
                    break;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => {}
            };

            // We don't want to be waiting for data forever, if we don't have data, after the
            // timeout, let's continue looping
            let read_or_write = if self.is_sending {
                self.stream.write(&buffer).left_future()
            } else {
                // Read up-to the remaining bytes from the socket.
                self.stream.read(&mut buffer).right_future()
            };
            // If we cannot read within 5ms, we skip this loop and try again. This ensures that we
            // will still terminate this stream when the total time passes.
            if let Ok(bytes_count) = timeout(Duration::from_millis(100), read_or_write).await {
                let bytes_count = bytes_count?;
                current_interval_bytes_transferred += bytes_count;
                bytes_transferred += bytes_count;
                if bytes_count > 0 {
                    syscalls += 1;
                    current_interval_syscalls += 1;
                } else {
                    // zero means that the connection is terminated. Let's wrap this up.
                    warn!("Stream {}'s connection has been closed.", self.id);
                    break;
                }
            } else {
                debug!("Stream [] taking longer than 100ms to produce data...");
            }
            // Stats
            // Check if we should print stats or not.
            let now = Instant::now();
            if now > current_interval_start + interval {
                // Collect the stats, print. Then reset the interval.
                let current_interval = now - current_interval_start;
                ui::print_stats(
                    Some(self.id),
                    (current_interval_start - start_time)
                        .as_millis()
                        .try_into()
                        .unwrap(),
                    current_interval.as_millis().try_into().unwrap(),
                    current_interval_bytes_transferred,
                    self.is_sending,
                    current_interval_syscalls,
                    block_size,
                );
                current_interval_bytes_transferred = 0;
                current_interval_syscalls = 0;
                current_interval_start = now;
            }
        }
        let duration = Instant::now() - start_time;
        let stats = StreamStats {
            sender: self.is_sending,
            duration_millis: duration.as_millis().try_into().unwrap(),
            bytes_transferred,
            syscalls,
        };

        // Drain the sockets if we are the receiving end, we need to do that to avoid failing the
        // sender stream that might still be sending data.
        if !self.is_sending {
            while self.stream.read(&mut buffer).await? != 0 {}
        }
        Ok(stats)
    }

    fn configure_stream_socket(&mut self) -> Result<()> {
        if self.params.no_delay {
            self.stream.set_nodelay(self.params.no_delay)?;
        }
        // Configure the control socket to use the send and receive buffers.
        if let Some(socket_buffers) = self.params.socket_buffers {
            self.stream.set_send_buffer_size(socket_buffers)?;
            self.stream.set_recv_buffer_size(socket_buffers)?;
        }
        Ok(())
    }
}
