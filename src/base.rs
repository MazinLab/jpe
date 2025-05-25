// Defines types and functionality related to the base controller
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{
    io::{self, ErrorKind},
    marker::PhantomData,
    net::Ipv4Addr,
    str::{FromStr, Utf8Error},
    time::{Duration, Instant},
};
use thiserror::Error;

const PARITY: Parity = Parity::None;
const DATABITS: DataBits = DataBits::Eight;
const FLOWCONTROL: FlowControl = FlowControl::None;
const STOPBITS: StopBits = StopBits::One;
const READ_BUF_SIZE: usize = 4096;
// Used with serial readers to set the chunk size for reading from the serial buffer
const READ_CHUNK_SIZE: usize = 64;
const READ_TIMEOUT: Duration = Duration::from_millis(200);

/// Errors for the base controller api
#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Serial(#[from] serialport::Error),
    #[error("")]
    Io(#[from] io::Error),
    #[error("")]
    DeviceNotFound,
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    InvalidResponse(String),
    #[error("")]
    WrongConnMode { expected: ConnMode, found: ConnMode },
    #[error("{0}")]
    General(String),
    #[error("")]
    BufOverflow { max_len: usize, idx: usize },
    #[error("")]
    Utf8(#[from] Utf8Error),
}

pub type BaseResult<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq)]
/// Reperesents the different types of Module supported by the controller
pub(crate) enum Module {
    All,
    Cadm,
    Rsm,
    Oem,
    Psm,
    Edm,
}
impl TryFrom<String> for Module {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        // The device spec uses ASCII
        let s = s.to_ascii_lowercase();
        match s {
            _ if s.starts_with("cadm") => Ok(Self::Cadm),
            _ if s.starts_with("rsm") => Ok(Self::Rsm),
            _ if s.starts_with("oem") => Ok(Self::Oem),
            _ if s.starts_with("psm") => Ok(Self::Psm),
            _ if s.starts_with("edm") => Ok(Self::Edm),
            _ => Err(Error::InvalidParams(format!("Unknown module: {}", s))),
        }
    }
}

/// The operation modes supported by the controller
#[derive(Debug, Clone, PartialEq)]
pub enum ControllerOpMode {
    Basedrive,
    Servodrive,
    Flexdrive,
}

/// Serial connection mode to the controller. Used in type-state-builder
/// pattern for controller creation
#[derive(Debug, Clone, PartialEq)]
struct Serial;
/// Network connection mode to the controller. Used in type-state-builder
/// pattern for controller creation
#[derive(Debug, Clone, PartialEq)]
struct Network;
/// Connection mode to the controller. Used internally by the controller
/// base API.
#[derive(Debug, Clone, PartialEq)]
enum ConnMode {
    Serial,
    Network,
}

/// The module slot within the controller
#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}

/// The response type expected for a given Command
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    /// Error responses, begins with "Error"
    Error(String),
    /// Carriage return delimited responses (currently a bug)
    CrDelimited(Vec<String>),
    /// Normal, non-Error responses delimited by commas
    CommaDelimited(Vec<String>),
}

/// Higher level enum for supported modules
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModuleScope {
    Any,
    Only(Vec<Module>),
}
/// Higher level enum for supported operation modes
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
    pub(crate) allowed_module: ModuleScope,
    /// Controller operation modes that support this command
    pub(crate) allowed_mode: ModeScope,
    pub(crate) payload: String,
    pub(crate) resp_type: Response,
}

/// Abstract, central representation of the Controller
#[derive(Debug)]
pub struct BaseController {
    conn_mode: ConnMode,
    op_mode: ControllerOpMode,
    /// Firmware version of all modules
    fw_vers: String,
    ip_addr: Option<Ipv4Addr>,
    /// Network connection handle (if using network)
    net_conn: Option<()>, // Not sure which type this will be, based on UDP or TCP support.
    /// Name of the serial port (if in serial mode)
    com_port: Option<String>,
    /// Serial connection handle (if using serial)
    serial_conn: Option<Box<dyn SerialPort>>,
    /// Device serial number
    serial_num: Option<String>,
    baud_rate: Option<u32>,
    read_buffer: Vec<u8>,
    /// Internal representation of the installed modules
    modules: [Option<Module>; 6],
}
impl BaseController {
    fn new(
        conn_mode: ConnMode,
        ip_addr: Option<Ipv4Addr>,
        com_port: Option<String>,
        serial_conn: Option<Box<dyn SerialPort>>,
        net_conn: Option<()>,
        serial_num: Option<String>,
        baud_rate: Option<u32>,
    ) -> Self {
        Self {
            conn_mode,
            op_mode: ControllerOpMode::Basedrive,
            fw_vers: "".to_string(),
            ip_addr,
            com_port,
            serial_conn,
            net_conn,
            serial_num,
            baud_rate,
            read_buffer: vec![0; READ_BUF_SIZE],
            modules: [None; 6],
        }
    }
    /// Polls the device to get the installed modules
    fn get_modules(&mut self) -> BaseResult<()> {
        todo!()
    }
    /// Checks whether a command is valid given the current state of the hardware
    fn check_command(&self, cmd: &Command, slot: Slot) -> bool {
        let opmode_check = match &cmd.allowed_mode {
            ModeScope::Any => true,
            ModeScope::Only(modes) => modes.contains(&self.op_mode),
        };
        let mod_check = match &cmd.allowed_module {
            ModuleScope::Any => true,
            ModuleScope::Only(mods) => {
                // Check which module is in the given slot and check if it's in the list of
                // supported modules for this command.
                // This is a bit ugly, may need to refactor for readability.
                match slot {
                    Slot::One if matches!(self.modules[0], Some(m) if mods.contains(&m)) => true,
                    Slot::Two if matches!(self.modules[1], Some(m) if mods.contains(&m)) => true,
                    Slot::Three if matches!(self.modules[2], Some(m) if mods.contains(&m)) => true,
                    Slot::Four if matches!(self.modules[3], Some(m) if mods.contains(&m)) => true,
                    Slot::Five if matches!(self.modules[4], Some(m) if mods.contains(&m)) => true,
                    Slot::Six if matches!(self.modules[5], Some(m) if mods.contains(&m)) => true,
                    _ => false,
                }
            }
        };
        mod_check && opmode_check
    }
    /// Parses a response in read buffer and return the result
    fn parse_response(&self, cmd: &Command, bytes_read: usize) -> BaseResult<Response> {
        // First, make sure index into the buffer is valid, then try to convert
        // from bytes to &str since all parsing should be ASCII.
        let msg = std::str::from_utf8(self.read_buffer.get(..bytes_read).ok_or(
            Error::BufOverflow {
                max_len: self.read_buffer.len(),
                idx: bytes_read,
            },
        )?)?;

        if msg.starts_with("Error") {
            return Ok(Response::Error(msg.to_string()));
        }
        // Comma-delimited case when there is only one carriage return in the
        // non Error path. More than one, the CrDelimited (bug) case
        match msg.chars().filter(|c| *c == '\r').count() {
            1 => {
                // Trim the terminator
                let trimmed = msg.strip_suffix('\r');
                if let Some(trimmed) = trimmed {
                    // Split the msg on commas and collect into vec
                    // of Strings
                    let ret: Vec<String> = trimmed
                        .split(|c| c == ',')
                        .map(|slice| slice.to_string())
                        .collect();
                    return Ok(Response::CommaDelimited(ret));
                } else {
                    return Err(Error::InvalidResponse("Bad terminator".to_string()));
                }
            }
            _ => {
                // Trim the terminator
                let trimmed = msg.strip_suffix('\r');
                if let Some(trimmed) = trimmed {
                    // Split the msg on commas and collect into vec
                    // of Strings
                    let ret: Vec<String> = trimmed
                        .split(|c| c == '\r')
                        .map(|slice| slice.to_string())
                        .collect();
                    return Ok(Response::CrDelimited(ret));
                } else {
                    return Err(Error::InvalidResponse("Bad terminator".to_string()));
                }
            }
        }
    }
    /// Higher level read function that reads from any given media into the
    /// internal read buffer.
    fn read_into_buffer(&mut self) -> BaseResult<usize> {
        todo!()
    }
    /// Low-level reader for the USB connection mode
    fn read_usb_chunks(&mut self) -> BaseResult<usize> {
        // Clear the internal read buffer and create a local chunk buffer.
        self.read_buffer.fill(0);
        let mut chunk_buf: [u8; READ_CHUNK_SIZE] = [0; READ_CHUNK_SIZE];

        // Loop to read in chunks and iteratively add to internal read buffer
        // until total timeout is reached.
        let read_timer_start = Instant::now();
        let mut total_bytes_read = 0usize;
        let reader = self.serial_conn.as_mut().ok_or(Error::WrongConnMode {
            expected: ConnMode::Serial,
            found: ConnMode::Network,
        })?;

        loop {
            match reader.read(&mut chunk_buf) {
                Ok(chunk_bytes_read) => {
                    if let Some(buf_slice) = self
                        .read_buffer
                        .get_mut(total_bytes_read..total_bytes_read + chunk_bytes_read)
                    {
                        // Happy path, haven't exceeded read buffer capacity
                        buf_slice.copy_from_slice(&chunk_buf[..chunk_bytes_read]);
                        total_bytes_read += chunk_bytes_read;
                    } else {
                        // Read buffer overrun case, read from chunk buf until
                        // input buf is full and break early.
                        if let Some(bytes_left) = (total_bytes_read + chunk_bytes_read)
                            .checked_sub(self.read_buffer.len())
                        {
                            // Know the exact number of bytes to read, can use unsafe accesses
                            self.read_buffer[total_bytes_read..total_bytes_read + bytes_left]
                                .copy_from_slice(&chunk_buf[..bytes_left]);
                            total_bytes_read += bytes_left
                        } else {
                            return Err(Error::General(
                                "Logic error in read buf overrun case, got negative difference between buf len and total bytes read.".to_string(),
                            ));
                        }
                        break;
                    }
                }
                // If chunk times out, just keep iterating until total timeout
                Err(ref e) if e.kind() == ErrorKind::TimedOut => (),
                Err(e) => return Err(Error::Io(e)),
            }
            if read_timer_start.elapsed() > READ_TIMEOUT {
                break;
            }
        }
        // Clear the input buffer of any residual junk and return bytes read
        reader.clear(serialport::ClearBuffer::Input)?;
        Ok(total_bytes_read)
    }
}

/// Type-State Builder for the Controller type based on connection mode.
pub struct BaseControllerBuilder<T> {
    conn_mode: ConnMode,
    ip_addr: Option<Ipv4Addr>,
    net_conn: Option<()>,
    com_port: Option<String>,
    serial_num: Option<String>,
    baud_rate: Option<u32>,
    /// Used since we don't care about using T in the type
    _marker: PhantomData<T>,
}
impl BaseControllerBuilder<Serial> {
    pub fn new(com_port: Option<String>, serial_num: Option<String>, baud_rate: u32) -> Self {
        Self {
            com_port,
            conn_mode: ConnMode::Serial,
            ip_addr: None,
            net_conn: None,
            serial_num,
            baud_rate: Some(baud_rate),
            _marker: PhantomData,
        }
    }
    /// Builds the controller type and tries to connect over serial.
    pub fn build(self) -> BaseResult<BaseController> {
        // Try and find the serial port that the device is connected
        let port = match (self.com_port.as_ref(), self.serial_num.as_ref()) {
            (Some(c), _) => c.clone(),
            (None, Some(s)) => Self::walk_com_ports(s).ok_or(Error::DeviceNotFound)?,
            _ => {
                return Err(Error::InvalidParams(
                    "Need serial port or serial number to connect to device".to_string(),
                ));
            }
        };

        // Try to bind to a serial port handle and return newly built instance
        Ok(BaseController::new(
            self.conn_mode,
            self.ip_addr,
            self.com_port,
            Some(
                serialport::new(
                    port,
                    self.baud_rate
                        .expect("Baud rate required to get to serial build method."),
                )
                .data_bits(DATABITS)
                .parity(PARITY)
                .flow_control(FLOWCONTROL)
                .stop_bits(STOPBITS)
                .open()?,
            ),
            self.net_conn,
            self.serial_num,
            self.baud_rate,
        ))
    }
    /// Walks available serial ports and tries to find the device based on the
    /// given serial number.
    fn walk_com_ports(serial_num: &str) -> Option<String> {
        todo!()
    }
}
impl BaseControllerBuilder<Network> {
    fn new() -> BaseController {
        todo!("Need to determine whether the controller supports TCP or UDP...")
    }
}
