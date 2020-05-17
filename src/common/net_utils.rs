use crate::common::consts::{MAX_CONTROL_MESSAGE, MESSAGE_LENGTH_SIZE_BYTES};
use crate::common::control::*;
use anyhow::{bail, Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use log::debug;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn client_send_message<A>(stream: &mut A, message: ClientMessage) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    send_control_message(stream, ClientEnvelope::ClientMessage(message)).await
}

pub async fn client_send_error<A>(stream: &mut A, error: ClientError) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    send_control_message(stream, ClientEnvelope::Error(error)).await
}

pub async fn server_send_message<A>(stream: &mut A, message: ServerMessage) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    send_control_message(stream, ServerEnvelope::ServerMessage(message)).await
}
pub async fn server_send_error<A>(stream: &mut A, error: ServerError) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
{
    send_control_message(stream, ServerEnvelope::Error(error)).await
}

// If the client sent an error, the result will be set with Err(ClientError) instead.
pub async fn server_read_message<A>(stream: &mut A) -> Result<ClientMessage>
where
    A: AsyncReadExt + Unpin,
{
    let envelope: ClientEnvelope = read_control_message(stream).await?;
    match envelope {
        ClientEnvelope::ClientMessage(m) => Ok(m),
        ClientEnvelope::Error(e) => Err(anyhow::Error::new(e)),
    }
}

pub async fn client_read_message<A>(stream: &mut A) -> Result<ServerMessage>
where
    A: AsyncReadExt + Unpin,
{
    let envelope: ServerEnvelope = read_control_message(stream).await?;
    match envelope {
        ServerEnvelope::ServerMessage(m) => Ok(m),
        ServerEnvelope::Error(e) => Err(anyhow::Error::new(e)),
    }
}

/// A helper that encodes a json-serializable object and sends it over the stream.
async fn send_control_message<A, T>(stream: &mut A, message: T) -> Result<()>
where
    A: AsyncWriteExt + Unpin,
    T: Serialize,
{
    let json = serde_json::to_string(&message)?;
    let payload = json.as_bytes();
    // This is our invariant. We cannot serialise big payloads here.Serialize
    assert!(payload.len() <= (u32::MAX) as usize);
    let mut buf = BytesMut::with_capacity(MESSAGE_LENGTH_SIZE_BYTES + payload.len());
    // Shipping the length first.
    buf.put_u32(payload.len() as u32);
    buf.put_slice(payload);
    debug!("Sent: {} bytes", buf.bytes().len());
    stream.write_all(&buf.bytes()).await?;
    Ok(())
}

/// A helper that encodes reads a control message off the wire and deserialize it to type T
/// if possible. You should strictly use that for 'Envelope' messages.
async fn read_control_message<A, T>(stream: &mut A) -> Result<T>
where
    A: AsyncReadExt + Unpin,
    T: DeserializeOwned,
{
    // Let's first read the message size in one syscall.
    // We know that this is inefficient but it makes handling the protocol much simpler
    // And saves us memory as we are not over allocating buffers. The control protocol
    // is not chatty anyway.
    let message_size = stream.read_u32().await?;
    // We restrict receiving control messages over 20MB (defined in consts.rs)
    if message_size > MAX_CONTROL_MESSAGE {
        bail!(
            "Unusually large protocol negotiation header: {}MB, max allowed: {}MB",
            message_size / 1024,
            MAX_CONTROL_MESSAGE,
        );
    }

    let mut buf = BytesMut::with_capacity(message_size as usize);
    let mut remaining_bytes: u64 = message_size as u64;
    let mut counter: usize = 0;
    while remaining_bytes > 0 {
        counter += 1;
        // Only read up-to the remaining-bytes, don't over read.
        // It's important that we don't read more as we don't want to mess up
        // the protocol. The next read should find the LENGTH as the first 4 bytes.
        let mut handle = stream.take(remaining_bytes);
        let bytes_read = handle.read_buf(&mut buf).await?;
        if bytes_read == 0 {
            // We have reached EOF. This is unexpected.
            // XXX: Handle
            bail!("Connected was closed by peer.");
        }
        // usize is u64 in most cases.
        remaining_bytes -= bytes_read as u64;
    }
    debug!(
        "Received a control message ({} bytes) in {} iterations",
        message_size, counter
    );
    assert_eq!(message_size as usize, buf.len());

    let obj = serde_json::from_slice(&buf)
        .with_context(|| "Invalid protocol, could not deserialise JSON")?;
    Ok(obj)
}

pub fn peer_to_string(stream: &TcpStream) -> String {
    stream
        .peer_addr()
        .map(|addr| addr.to_string())
        // The reason for or_else here is to avoid allocating the string if this was never called.
        .unwrap_or_else(|_| "<UNKNOWN>".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::control::to_server_error;
    use bytes::Bytes;
    use pretty_assertions::assert_eq;
    use serde::{Deserialize, Serialize};

    // A test serializable structure to use in testing
    #[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
    struct MockSerializable {
        username: String,
    }

    // Note the lifetime of the returned stream matches the input buffer
    fn convert_buffer_to_stream<'a>(buf: &'a Bytes) -> impl tokio::io::AsyncRead + 'a {
        // Creating a stream of 5-byte chunks to be read later.
        let chunks_stream = tokio::stream::iter(buf.chunks(5).map(|x| Ok(x)));
        tokio::io::stream_reader(chunks_stream)
    }

    #[tokio::test]
    async fn test_send_control_message() -> Result<()> {
        let mut buf = vec![];
        let obj = MockSerializable {
            username: "asoli".to_owned(),
        };
        // create a stream to serialise data into
        send_control_message(&mut buf, obj.clone()).await?;
        assert_eq!(buf.len(), 24);
        // let's receive the same value and compare.
        let mem = Bytes::from(buf);
        let mut stream = convert_buffer_to_stream(&mem);
        let data: MockSerializable = read_control_message(&mut stream).await?;
        assert_eq!(data, obj);
        Ok(())
    }

    #[tokio::test]
    async fn test_messages() -> Result<()> {
        // Send and receive server messages.
        {
            let mut buf = vec![];
            server_send_message(&mut buf, ServerMessage::Welcome).await?;
            // let's receive the same value and compare.
            let mem = Bytes::from(buf);
            let mut stream = convert_buffer_to_stream(&mem);
            let data = client_read_message(&mut stream).await?;
            assert_eq!(data, ServerMessage::Welcome);
        }
        // Send and receive errors
        {
            let mut buf = vec![];
            server_send_error(
                &mut buf,
                ServerError::AccessDenied("Something went wrong".to_owned()),
            )
            .await?;
            // let's receive the same value and compare.
            let mem = Bytes::from(buf);
            let mut stream = convert_buffer_to_stream(&mem);
            let data = client_read_message(&mut stream).await;
            assert!(data.is_err());
            assert!(matches!(to_server_error(&data),
                Some(ServerError::AccessDenied(msg)) if *msg == "Something went wrong"
            ));
        }
        Ok(())
    }
}
