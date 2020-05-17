//!
//! This file holds the protocol handling for the control socket.
//!
//! The control socket is the first socket that gets created once
//! a test is started. Communication and coordination between the
//! client and server happens through this socket. The protocol is
//! very simple.
//!
//! For every frame:
//! LENGTH (u32) + JSON Object representing one of the messages
//! defined in this file.
//!
//! The parsing of this socket will incur 2 syscalls for every frame
//! this is chosen for convenience and simplicity. We first read the
//! length (u32) then parse the JSON message and match it against the
//! enum defined in the enums `ServerMessage` and `ClientMessage`.

use crate::common::data::TestParameters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct StreamStats {
    pub sender: bool,
    pub duration_millis: u64,
    pub bytes_transferred: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TestResults {
    pub streams: HashMap<usize, StreamStats>,
}

/// Error messages set by server.
#[derive(Serialize, Deserialize, Debug, Error, Eq, PartialEq)]
pub enum ServerError {
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Cannot accept a stream connection: {0}")]
    CannotAcceptStream(String),
}

/// Error messages set by clients.
#[derive(Serialize, Deserialize, Debug, Error, Eq, PartialEq)]
pub enum ClientError {
    #[error("Cannot create a stream connection: {0}")]
    CannotCreateStream(String),
}

/// This is the top-level message that gets serialised on the wire,
/// The reason this exists is to decode whether we have an Error or
/// a valid response in the protocol decoder and translate the error
/// into (std::error::Error) instead of passing this Error as a normal message.
///
/// This technique also makes it such that users can perform exhaustive
/// pattern matching on all message types without having to handle the
/// error case.
#[derive(Serialize, Deserialize, Debug)]
pub enum ClientEnvelope {
    ClientMessage(ClientMessage),
    Error(ClientError),
}

/// See docs for `ClientEnvelope`
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerEnvelope {
    ServerMessage(ServerMessage),
    Error(ServerError),
}

/// A control message that can be sent from clients.
/// CLIENT => SERVER
#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    /// The first message that the client needs to send to the server
    /// upon successful connection. The cookie is a random UUID that
    /// the client uses to identify itself and the subsequent stream
    /// connections.
    Hello { cookie: String },
    /// Sending the test parameters
    SendParameters(TestParameters),
    /// Sending the test results
    SendResults(TestResults),
}

/// A control message that can be sent from servers.
/// SERVER => CLIENT
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum ServerMessage {
    /// The server's response to Hello.
    Welcome,
    SetState(State),
    SendResults(TestResults),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum State {
    // Parameters have been exchanged, but server is not ready yet to ask for data stream
    // connections.
    TestStart,
    // Asks the client to establish the data stream connections.
    CreateStreams { cookie: String },
    // All connections are established, stream the data and measure.
    Running,
    // We are asked to exchange the TestResults between server and client. Client will initiate this
    // exchange once it receives a transition into this state.
    ExchangeResults,
    DisplayResults,
}

// Attempts to extract ServerError from Result<_, anyhow::Error>
pub fn to_server_error<T>(result: &Result<T, anyhow::Error>) -> Option<&ServerError> {
    match result {
        Err(e) => match e.downcast_ref::<ServerError>() {
            Some(s) => Some(s),
            _ => None,
        },
        _ => None,
    }
}

// Attempts to extract ClientError from Result<_, anyhow::Error>
pub fn to_client_error<A>(result: &Result<A, anyhow::Error>) -> Option<&ClientError> {
    match result {
        Err(e) => match e.downcast_ref::<ClientError>() {
            Some(s) => Some(s),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn test_error_extraction() {
        // Server Errors
        let a: Result<(), anyhow::Error> = Ok(());
        assert!(matches!(to_server_error(&a), None));

        let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Test");
        let a: Result<(), anyhow::Error> = Err(err.into());
        assert!(matches!(to_server_error(&a), None));

        let a: Result<(), anyhow::Error> = Err(anyhow!("Missing Stuff!"));
        assert!(matches!(to_server_error(&a), None));

        let a: Result<(), anyhow::Error> = Err(anyhow::Error::new(ServerError::AccessDenied(
            "Something went wrong!".to_owned(),
        )));

        assert!(matches!(
            to_server_error(&a),
            Some(ServerError::AccessDenied(msg)) if *msg == "Something went wrong!".to_owned()));

        // Client Errors
        let a: Result<(), anyhow::Error> = Ok(());
        assert!(matches!(to_client_error(&a), None));

        let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Test");
        let a: Result<(), anyhow::Error> = Err(err.into());
        assert!(matches!(to_client_error(&a), None));

        let a: Result<(), anyhow::Error> = Err(anyhow!("Missing Stuff!"));
        assert!(matches!(to_client_error(&a), None));

        let a: Result<(), anyhow::Error> = Err(anyhow::Error::new(
            ClientError::CannotCreateStream("Something went wrong!".to_owned()),
        ));

        assert!(matches!(
            to_client_error(&a),
            Some(ClientError::CannotCreateStream(msg)) if *msg == "Something went wrong!".to_owned()));
    }
}
