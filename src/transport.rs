/* Abstraction for the transport semantics */

use crate::{BaseResult, Error, config::*};
use bytes::{Buf, BufMut, BytesMut};
use serial2::SerialPort;
use std::fmt::Display;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

const READ_TIMEOUT: Duration = Duration::from_millis(500);
const READ_CHUNK_SIZE: usize = 64;
const MAX_FRAME_SIZE: usize = 4096;
const TERMINATOR: &'static str = "\r\n";

/// A framed response received from the controller.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Frame {
    /// Error responses, begins with "Error"
    Error(String),
    /// Carriage return delimited responses (currently a bug)
    CrDelimited(Vec<String>),
    /// Normal, non-Error responses delimited by commas
    CommaDelimited(Vec<String>),
}

/// Higher level enum for supported modules for a given command.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModuleScope {
    Any,
    Only(Vec<Module>),
}
/// Higher level enum for supported operation modes for a given command.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModeScope {
    Any,
    Only(Vec<ControllerOpMode>),
}
/// The command type that the base controller API expects
/// for dispatch and response routing.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Command {
    /// Modules that support this command
    pub(crate) allowed_mod: ModuleScope,
    /// Controller operation modes that support this command
    pub(crate) allowed_mode: ModeScope,
    pub(crate) payload: String,
}
impl Command {
    pub(crate) fn new(allowed_mod: ModuleScope, allowed_mode: ModeScope, payload: &str) -> Self {
        Self {
            allowed_mod,
            allowed_mode,
            payload: format!("{}{}", payload, TERMINATOR),
        }
    }
}
impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.payload.split_whitespace().next().unwrap_or("Unknown");
        write!(f, "{}", s)
    }
}

// Trait to unify underlying types
pub(crate) trait BufClear: Read + Write {
    fn clear_input_buffer(&mut self) -> Result<(), Error>;
    fn clear_output_buffer(&mut self) -> Result<(), Error>;
}
/// Simple trait used to simplify internal API between the user facing
/// context and the infrastructure used to communicate over the wire.
pub(crate) trait Transport: std::fmt::Debug + Send + Sync {
    fn transact(&mut self, cmd: &Command) -> Result<Frame, Error>;
}

/// Connection mode to the controller. Used internally by the controller
/// base API.
#[derive(Debug)]
pub(crate) enum ConnMode {
    Rs422,
    Usb,
    Network,
}
impl Display for ConnMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConnMode::Usb => "Usb",
            ConnMode::Network => "Network",
            ConnMode::Rs422 => "RS422",
        };
        write!(f, "{}", s)
    }
}
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
            // non Error path, but one or more commas.
            1 => Ok(Frame::CommaDelimited(
                msg.split(|c| c == ',')
                    .map(|slice| slice.to_string())
                    .collect(),
            )),
            // Carriage return delimited (bug) case, greater than one carriage return in
            // the non Error path but no commas.
            2.. => Ok(Frame::CrDelimited(
                msg.split(|c| c == '\r')
                    .map(|slice| slice.to_string())
                    .collect(),
            )),
            _ => Err(Error::InvalidResponse(format!("Malformed Response: {msg}"))),
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
        while timer.elapsed() < READ_TIMEOUT || !self.read_buf.ends_with(TERMINATOR.as_bytes()) {
            match self.transport.read(&mut chunk_buf) {
                Ok(0) => break,
                Ok(n_read) => {
                    total_b_read += n_read;
                    if total_b_read > MAX_FRAME_SIZE || n_read > self.read_buf.remaining() {
                        self.read_buf.clear();
                        let _ = self.transport.clear_input_buffer();
                        return Err(Error::BufOverflow {
                            max_len: MAX_FRAME_SIZE,
                            idx: total_b_read,
                        });
                    }

                    self.read_buf.put_slice(&chunk_buf);
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
        self.transport.write_all(cmd.payload.as_bytes())?;
        self.transport.flush()?;

        // Read raw data and try dispatching for local parsing
        self.read_chunks()?;
        let _ = self.transport.clear_input_buffer();
        self.parse_frame()
    }
}
impl<B> Transport for Connection<B>
where
    B: BufClear + Sync + Send + std::fmt::Debug,
{
    fn transact(&mut self, cmd: &Command) -> Result<Frame, Error> {
        self.transaction_handler(cmd)
    }
}

impl BufClear for TcpStream {
    /// Used to keep the request/response paradigm in sync by draining
    /// the recv buffer of the TcpStream
    fn clear_input_buffer(&mut self) -> Result<(), Error> {
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

    fn clear_output_buffer(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
impl BufClear for SerialPort {
    fn clear_input_buffer(&mut self) -> Result<(), Error> {
        self.discard_input_buffer().map_err(|e| e.into())
    }

    fn clear_output_buffer(&mut self) -> Result<(), Error> {
        self.discard_output_buffer().map_err(|e| e.into())
    }
}
