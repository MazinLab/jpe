use super::*;
use crate::{BaseResult, Error};
use bytes::{BufMut, BytesMut};
use serial2::SerialPort;
use std::{
    io::{ErrorKind, Read},
    net::TcpStream,
    time::Instant,
};

/// Abstracts the low-level reading and writing semantics
#[derive(Debug)]
pub(crate) struct Connection<B: BufClear + Sync + Send + std::fmt::Debug> {
    read_buf: BytesMut,
    transport: B,
}
impl<B> Connection<B>
where
    B: BufClear + Sync + Send + std::fmt::Debug,
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
    fn read_chunks(&mut self) -> BaseResult<()> {
        // Loop to read in chunks and iteratively add to internal read buffer
        // until total timeout is reached, terminator is found, or number of bytes
        // read exceeds limit.
        let timer = Instant::now();
        let mut total_b_read = 0usize;
        self.read_buf.clear();

        let mut chunk_buf = [0u8; READ_CHUNK_SIZE];

        // Canonical chunked read loop
        while timer.elapsed() < READ_TIMEOUT && !self.read_buf.ends_with(TERMINATOR.as_bytes()) {
            match self.transport.read(&mut chunk_buf) {
                Ok(0) => break,
                Ok(n_read) => {
                    total_b_read += n_read;
                    if total_b_read > MAX_FRAME_SIZE {
                        return Err(Error::BufOverflow {
                            max_len: MAX_FRAME_SIZE,
                            idx: total_b_read,
                        });
                    }

                    self.read_buf.put_slice(&chunk_buf[..n_read]);
                }
                // Chunk read blocked, continue to next chunk read
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => continue,
                // In case the low-level handler does not send EOF appropriately
                Err(ref e) if e.kind() == ErrorKind::TimedOut => continue,
                Err(e) => {
                    return Err(Error::Io(e));
                }
            }
        }

        Ok(())
    }
    // Handles the interplay between polling the device and capturing the
    // acknowledgment that most API functions will use.
    pub(crate) fn transaction_handler(&mut self, cmd: &Command) -> BaseResult<Frame> {
        // encode and send data on wire
        self.transport.clear_output_buffer()?;
        self.transport.clear_input_buffer()?;
        self.transport.write_all(cmd.payload.as_bytes())?;
        self.transport.flush()?;

        // Read raw data and try dispatching for local parsing
        self.read_chunks()?;
        self.parse_frame()
    }
}
impl<B> Transport for Connection<B>
where
    B: BufClear + Sync + Send + std::fmt::Debug,
{
    fn transact(&mut self, cmd: &Command) -> BaseResult<Frame> {
        self.transaction_handler(cmd)
    }
}

impl BufClear for TcpStream {
    /// Used to keep the request/response paradigm in sync by draining
    /// the recv buffer of the TcpStream
    fn clear_input_buffer(&mut self) -> BaseResult<()> {
        let mut chunk_buf: [u8; READ_CHUNK_SIZE] = [0; READ_CHUNK_SIZE];

        // Drain any remanining data from stream.
        loop {
            match self.read(&mut chunk_buf) {
                // Stream has been closed.
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

    fn clear_output_buffer(&mut self) -> BaseResult<()> {
        Ok(())
    }
}
impl BufClear for SerialPort {
    fn clear_input_buffer(&mut self) -> BaseResult<()> {
        self.discard_input_buffer().map_err(|e| e.into())
    }

    fn clear_output_buffer(&mut self) -> BaseResult<()> {
        self.discard_output_buffer().map_err(|e| e.into())
    }
}
