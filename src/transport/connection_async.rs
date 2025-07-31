use super::*;
use crate::{BaseResult, Error};
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ErrorKind}, // tokio::io::Error <=> std::io::Error
    net::TcpStream,
    time::timeout,
};

/// Abstracts the low-level reading and writing semantics
#[derive(Debug)]
pub(crate) struct ConnectionAsync<B: AsyncBufClear + Sync + Send + std::fmt::Debug + Unpin> {
    read_buf: BytesMut,
    transport: B,
}
impl<B> ConnectionAsync<B>
where
    B: AsyncBufClear + Sync + Send + std::fmt::Debug + Unpin,
{
    pub fn new(transport: B) -> Self {
        Self {
            transport,
            read_buf: BytesMut::with_capacity(MAX_FRAME_SIZE * 2),
        }
    }
    /// Attempts to frame bytes in the read buffer.
    fn parse_frame(&mut self) -> BaseResult<Frame> {
        let msg = std::str::from_utf8(&self.read_buf)?
            .strip_suffix(TERMINATOR)
            .ok_or(Error::InvalidResponse("Terminator not found".to_string()))?;

        // Error case returns early
        if msg.starts_with("Error") {
            return Ok(Frame::Error(msg.to_string()));
        }

        match msg.chars().filter(|c| *c == '\r').count() {
            // Comma-delimited case when there is only one carriage return in the
            // non Error path (previously removed), but one or more commas.
            0 => Ok(Frame::CommaDelimited(
                msg.split(|c| c == ',')
                    .map(|slice| slice.to_string())
                    .collect(),
            )),
            // Carriage return delimited (bug) case, greater than one carriage return in
            // the non Error path (one previously removed) but no commas.
            1.. => Ok(Frame::CrDelimited(
                msg.split(|c| c == '\r')
                    .map(|slice| slice.to_string())
                    .collect(),
            )),
        }
    }

    /// Low-level reader for all connections
    async fn read_chunks(&mut self) -> BaseResult<()> {
        self.read_buf.clear();

        while !self.read_buf.ends_with(TERMINATOR.as_bytes()) {
            match timeout(READ_TIMEOUT, self.transport.read_buf(&mut self.read_buf)).await {
                Ok(read_result) => {
                    match read_result {
                        // This case indicates either EOF OR buf remaining capacity is 0.
                        Ok(0) => break,
                        // Read some bytes into buffer and advanced internal cursor appropriately
                        Ok(_) => continue,
                        Err(ref e)
                            if e.kind() == ErrorKind::WouldBlock
                                || e.kind() == ErrorKind::TimedOut =>
                        {
                            continue;
                        }
                        Err(e) => {
                            // Clearing as required by the tokio API
                            self.read_buf.clear();
                            return Err(Error::Io(e));
                        }
                    }
                }
                // Read timer elapsed
                Err(_) => break,
            }
        }
        Ok(())
    }
    // Handles the interplay between polling the device and capturing the
    // acknowledgment that most API functions will use.
    pub(crate) async fn transaction_handler(&mut self, cmd: &Command) -> BaseResult<Frame> {
        todo!()
    }
}
impl<B> AsyncTransport for ConnectionAsync<B>
where
    B: AsyncBufClear + Sync + Send + std::fmt::Debug + Unpin,
{
    async fn transact(&mut self, cmd: &Command) -> BaseResult<Frame> {
        self.transaction_handler(cmd).await
    }
}

impl AsyncBufClear for TcpStream {
    async fn clear_input_buffer(&mut self) -> Result<(), Error> {
        todo!()
    }

    async fn clear_output_buffer(&mut self) -> Result<(), Error> {
        todo!()
    }
}
