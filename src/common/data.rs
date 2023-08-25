use crate::common::opts::ClientOpts;
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq)]
pub enum Role {
    Server,
    Client,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum Direction {
    /// Traffic flows from Client => Server (the default)
    ClientToServer,
    /// Traffic flows from Server => Client.
    ServerToClient,
    /// Both ways.
    Bidirectional,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TestParameters {
    pub direction: Direction,
    /// omit the first n seconds.
    pub omit_seconds: u32,
    pub time_seconds: u64,
    // The number of data streams
    pub parallel: u16,
    pub block_size: usize,
    pub client_version: String,
    pub no_delay: bool,
    pub socket_buffers: Option<usize>,
}

impl TestParameters {
    pub fn from_opts(opts: &ClientOpts, default_block_size: usize) -> Self {
        let direction = if opts.bidir {
            Direction::Bidirectional
        } else if opts.reverse {
            Direction::ServerToClient
        } else {
            Direction::ClientToServer
        };
        TestParameters {
            direction,
            omit_seconds: 0,
            time_seconds: opts.time,
            parallel: opts.parallel,
            block_size: opts.length.unwrap_or(default_block_size),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            no_delay: opts.no_delay,
            socket_buffers: opts.socket_buffers,
        }
    }
}
