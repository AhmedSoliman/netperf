use crate::common::Opts;
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
}

impl TestParameters {
    pub fn from_opts(opts: &Opts, default_block_size: usize) -> Self {
        TestParameters {
            // TODO: Make this configurable through opts
            direction: Direction::ClientToServer,
            omit_seconds: 0,
            time_seconds: opts.client_opts.time,
            parallel: opts.client_opts.parallel,
            block_size: opts.client_opts.length.unwrap_or(default_block_size),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
