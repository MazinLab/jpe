// Defines types and functionality related to the base controller
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{marker::PhantomData, net::Ipv4Addr};
use thiserror::Error;

const PARITY: Parity = Parity::None;
const DATABITS: DataBits = DataBits::Eight;
const FLOWCONTROL: FlowControl = FlowControl::None;
const STOPBITS: StopBits = StopBits::One;
const READ_BUF_SIZE: usize = 4096;

/// Errors for the base controller api
#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Serial(#[from] serialport::Error),
    #[error("")]
    DeviceNotFound,
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    InvalidResponse(String),
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
    Error(String),
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
    /// Parses a response and returns the reesult
    fn parse_response(&self, cmd: &Command) -> BaseResult<Response> {
        todo!()
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
