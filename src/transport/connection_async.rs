use super::*;
use crate::{BaseResult, Error};
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ErrorKind}, // tokio::io::Error <=> std::io::Error
    net::TcpStream,
    time::timeout,
};

use serial2_tokio::SerialPort;
/// Abstracts the low-level reading and writing semantics in an async context.
#[derive(Debug)]
pub(crate) struct ConnectionAsync<B: AsyncBufClear + Sync + Send + std::fmt::Debug> {
    read_buf: BytesMut,
    transport: B,
}
impl<B> ConnectionAsync<B>
where
    B: AsyncBufClear + Sync + Send + std::fmt::Debug,
{
    pub fn new(transport: B) -> Self {
        Self {
            transport,
            read_buf: BytesMut::with_capacity(MAX_FRAME_SIZE),
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
                Err(_) => break
            }
        }
        Ok(())
    }
}
impl<B> AsyncTransport for ConnectionAsync<B>
where
    B: AsyncBufClear + Sync + Send + std::fmt::Debug,
{
    // Handles the interplay between polling the device and capturing the
    // acknowledgment that most API functions will use.
    fn transact<'a>(
        &'a mut self,
        cmd: &'a Command,
    ) -> Pin<Box<dyn Future<Output = BaseResult<Frame>> + 'a>> {
        Box::pin(async move {
            self.transport.clear_input_buffer().await?;
            self.transport.clear_output_buffer().await?;
            self.transport.write_all(cmd.payload.as_bytes()).await?;
            self.transport.flush().await?;

            // Read raw data and try dispatching for local parsing
            self.read_chunks().await?;
            self.parse_frame()
        })
    }
}

impl AsyncBufClear for TcpStream {
    async fn clear_input_buffer(&mut self) -> Result<(), Error> {
        let mut chunk_buf: [u8; READ_CHUNK_SIZE] = [0; READ_CHUNK_SIZE];
        // Drain any remanining data from stream.
        loop {
            match self.try_read(&mut chunk_buf) {
                // Stream has been closed or has zero bytes to read.
                Ok(0) => break,
                // Discard any data that is read
                Ok(_) => continue,
                // No data to read, waiting on OS to present more data.
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(Error::Io(e)),
            }
        }
        Ok(())
    }

    async fn clear_output_buffer(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
impl AsyncBufClear for SerialPort {
    async fn clear_input_buffer(&mut self) -> BaseResult<()> {
        self.discard_input_buffer().map_err(|e| e.into())
    }

    async fn clear_output_buffer(&mut self) -> BaseResult<()> {
        self.discard_output_buffer().map_err(|e| e.into())
    }
}
