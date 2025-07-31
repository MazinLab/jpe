pub(crate) mod connection;
pub(crate) mod connection_async;
pub(crate) use connection::Connection;
pub(crate) use connection_async::ConnectionAsync;

use crate::{
    BaseResult, Error,
    base::{ModeScope, ModuleScope},
};
use std::{
    fmt::Display,
    io::{Read, Write},
    time::Duration,
};

use tokio::io::{AsyncRead, AsyncWrite};

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

// Trait to unify clearing API to underlying transports
pub(crate) trait BufClear: Read + Write {
    fn clear_input_buffer(&mut self) -> Result<(), Error>;
    fn clear_output_buffer(&mut self) -> Result<(), Error>;
}
// Async version of `BufClear` trait.
pub(crate) trait AsyncBufClear: AsyncRead + AsyncWrite + Unpin {
    async fn clear_input_buffer(&mut self) -> Result<(), Error>;
    async fn clear_output_buffer(&mut self) -> Result<(), Error>;
}

/// Simple trait used to simplify internal API between the user facing
/// context and the infrastructure used to communicate over the wire.
pub(crate) trait Transport: std::fmt::Debug + Send + Sync {
    fn transact(&mut self, cmd: &Command) -> BaseResult<Frame>;
}
/// Async version of `Transport` trait.
pub(crate) trait AsyncTransport: std::fmt::Debug + Send + Sync + Unpin {
    async fn transact(&mut self, cmd: &Command) -> BaseResult<Frame>;
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
