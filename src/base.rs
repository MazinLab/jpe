// Defines types and functionality related to the base controller
use crate::config::*;
use serialport::{
    DataBits, FlowControl, Parity, SerialPort, SerialPortType, StopBits, available_ports,
};
use std::{
    io::{self, ErrorKind},
    marker::PhantomData,
    net::Ipv4Addr,
    num::ParseIntError,
    str::Utf8Error,
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
// Total time to read from the serial input queue.
const READ_TIMEOUT: Duration = Duration::from_millis(200);
const DEVICE_PID: u16 = 0000;
const TERMINATOR: char = '\r';
// Used at the start of every Command
const MARKER: char = '/';

/// Errors for the base controller api
#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Serial(#[from] serialport::Error),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("Device not found.")]
    DeviceNotFound,
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    InvalidResponse(String),
    #[error("")]
    WrongConnMode { expected: ConnMode, found: ConnMode },
    #[error("{0}")]
    General(String),
    #[error("max_len: {}, idx: {}", max_len, idx)]
    BufOverflow { max_len: usize, idx: usize },
    #[error("{0}")]
    Bound(String),
    #[error("{0}")]
    Utf8(#[from] Utf8Error),
    #[error("{0}")]
    DeviceError(String),
    #[error("{0}")]
    ParseIntError(#[from] ParseIntError),
}

pub type BaseResult<T> = std::result::Result<T, Error>;

/// The response type expected for a given Command
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Response {
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
            payload: format!("{}{}{}", MARKER, payload, TERMINATOR),
        }
    }
}

/// Abstract, central representation of the Controller
#[derive(Debug)]
pub struct BaseController {
    /// Mode used to connect to the controller
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
// ======= Internal API =======
impl BaseController {
    /// Checks whether a command is valid given the current state of the hardware
    fn check_command(&self, cmd: &Command, slot: Option<Slot>) -> bool {
        let opmode_check = match &cmd.allowed_mode {
            ModeScope::Any => true,
            ModeScope::Only(modes) => modes.contains(&self.op_mode),
        };
        let mod_check = match &cmd.allowed_mod {
            ModuleScope::Any => true,
            ModuleScope::Only(mods) => {
                // Check which module is in the given slot and check if it's in the list of
                // supported modules for this command.
                // This is a bit ugly, may need to refactor for readability.
                match slot {
                    Some(Slot::One) if matches!(self.modules[0], Some(m) if mods.contains(&m)) => {
                        true
                    }
                    Some(Slot::Two) if matches!(self.modules[1], Some(m) if mods.contains(&m)) => {
                        true
                    }
                    Some(Slot::Three) if matches!(self.modules[2], Some(m) if mods.contains(&m)) => {
                        true
                    }
                    Some(Slot::Four) if matches!(self.modules[3], Some(m) if mods.contains(&m)) => {
                        true
                    }
                    Some(Slot::Five) if matches!(self.modules[4], Some(m) if mods.contains(&m)) => {
                        true
                    }
                    Some(Slot::Six) if matches!(self.modules[5], Some(m) if mods.contains(&m)) => {
                        true
                    }
                    // This case should never match (None in slot should only be paired with the Any case),
                    // but returning true if so.
                    None => true,
                    _ => false,
                }
            }
        };
        mod_check && opmode_check
    }
    /// Parses a response in read buffer and returns the result
    fn parse_response(&self, bytes_read: usize) -> BaseResult<Response> {
        // First, make sure index into the buffer is valid, then try to convert
        // from bytes to &str since all bytes should be ASCII.
        let msg = std::str::from_utf8(self.read_buffer.get(..bytes_read).ok_or(
            Error::BufOverflow {
                max_len: self.read_buffer.len(),
                idx: bytes_read,
            },
        )?)?;

        // Error case returns early
        if msg.starts_with("Error") {
            return Ok(Response::Error(
                msg.strip_suffix(TERMINATOR)
                    .ok_or(Error::InvalidResponse("Bad terminator".to_string()))?
                    .to_string(),
            ));
        }

        // Comma-delimited case when there is only one carriage return in the
        // non Error path. More than one, the CrDelimited (bug) case
        match msg.chars().filter(|c| *c == TERMINATOR).count() {
            1 => Ok(Response::CommaDelimited(
                msg.strip_suffix(TERMINATOR)
                    .ok_or(Error::InvalidResponse("Bad terminator".to_string()))?
                    .split(|c| c == ',')
                    .map(|slice| slice.to_string())
                    .collect(),
            )),
            _ => Ok(Response::CrDelimited(
                msg.strip_suffix(TERMINATOR)
                    .ok_or(Error::InvalidResponse("Bad terminator".to_string()))?
                    .split(|c| c == '\r')
                    .map(|slice| slice.to_string())
                    .collect(),
            )),
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

    // Handles the interplay between polling the device and capturing the
    // acknowledgment that most API functions will use.
    fn comm_handler(&mut self, cmd: &Command) -> BaseResult<Response> {
        // encode and send data on wire
        match self.conn_mode {
            ConnMode::Serial => {
                if let Some(ref mut handle) = self.serial_conn {
                    handle.clear(serialport::ClearBuffer::Output)?;
                    handle.write_all(cmd.payload.as_bytes())?;
                } else {
                    return Err(Error::WrongConnMode {
                        expected: ConnMode::Serial,
                        found: ConnMode::Network,
                    });
                }
            }
            ConnMode::Network => {
                todo!()
            }
        }
        // Read raw data and try dispatching for local parsing
        let bytes_read = self.read_into_buffer()?;
        self.parse_response(bytes_read)
    }
}

// ======= External API =======
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
    /// Handler to abstract the boilerplate used in most command methods. The length bounds check allows
    /// for the use of direct indexing into the resulting return value as a result.
    fn handle_command(
        &mut self,
        cmd: &Command,
        n_resp_vals: Option<usize>,
        slot: Option<Slot>,
    ) -> BaseResult<Vec<String>> {
        // Check to verify if command is valid
        if !self.check_command(cmd, slot) {
            return Err(Error::InvalidParams(
                "Invalid command for current controller state".to_string(),
            ));
        }
        let resp = self.comm_handler(&cmd)?;
        match resp {
            Response::Error(s) => Err(Error::DeviceError(s)),
            Response::CrDelimited(v) | Response::CommaDelimited(v) => {
                // None in expected return vals implies it can be variable, return as-is.
                if let Some(n_vals) = n_resp_vals {
                    if v.len() != n_vals {
                        return Err(Error::InvalidResponse(format!(
                            "Expected {} values, got {}",
                            n_vals,
                            v.len()
                        )));
                    } else {
                        Ok(v)
                    }
                } else {
                    return Ok(v);
                }
            }
        }
    }
    /// Returns the firmware version of the controller and updates internal value.
    pub fn get_fw_version(&mut self) -> BaseResult<String> {
        if !self.fw_vers.is_empty() {
            Ok(self.fw_vers.clone())
        } else {
            // Build Command and send to controller
            let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "VER");
            // Extract, set, and return value. Direct indexing safe due to bounds check by the handle command
            // method.
            let mut v = self.handle_command(&cmd, Some(1), None)?;
            self.fw_vers = v[0].clone();
            Ok(v.remove(0))
        }
    }
    /// Returns firmware version information of module in given slot. Returns None if slot is empty.
    pub fn get_mod_fw_version(&mut self, slot: Slot) -> BaseResult<Option<String>> {
        let idx = u8::from(slot.clone()) as usize;
        if self.modules[idx - 1].is_some() {
            let cmd = Command::new(
                ModuleScope::Any,
                ModeScope::Any,
                format!("FIV {}", idx).as_str(),
            );
            let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
            Ok(Some(v.remove(0)))
        } else {
            Ok(None)
        }
    }
    /// Returns a list of all installed modules and updates internal module container
    pub fn get_module_list(&mut self) -> BaseResult<Vec<String>> {
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "MODLIST");
        let v = self.handle_command(&cmd, Some(6), None)?;

        // Iterate over the internal module collection and update with new values
        // from the controller. The modules in the interim vector below are guaranteed to be valid modules due to early return.
        // Length is also guaranteed to be correct due to command handler method.
        v.iter()
            .map(|mod_str| Module::try_from(mod_str.clone()))
            .collect::<BaseResult<Vec<Module>>>()?
            .iter()
            .enumerate()
            .for_each(|(idx, new_mod)| self.modules[idx] = Some(new_mod.clone()));
        Ok(v)
    }
    /// Returns a list of supported actuator and stage types
    pub fn get_supported_stages(&mut self) -> BaseResult<Vec<String>> {
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "STAGES");
        Ok(self.handle_command(&cmd, None, None)?)
    }
    /// Returns IP configuration for the LAN interface.
    /// Response: [MODE],[IP address],[Subnet Mask],[Gateway],[MAC Address]
    pub fn get_ip_config(&mut self) -> BaseResult<Vec<String>> {
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "IPR");
        Ok(self.handle_command(&cmd, Some(5), None)?)
    }
    /// Sets the IP configuration for the LAN interface
    pub fn set_ip_config(
        &mut self,
        addr_mode: IpAddrMode,
        ip_addr: Ipv4Addr,
        mask: Ipv4Addr,
        gateway: Ipv4Addr,
    ) -> BaseResult<String> {
        let cmd = match addr_mode {
            IpAddrMode::Dhcp => Command::new(
                ModuleScope::Any,
                ModeScope::Any,
                format!(
                    "{} {} {} {} {}",
                    "IPS", "DHCP", "0.0.0.0", "0.0.0.0", "0.0.0.0"
                )
                .as_str(),
            ),
            IpAddrMode::Static => Command::new(
                ModuleScope::Any,
                ModeScope::Any,
                format!(
                    "{} {} {} {} {}",
                    "IPS",
                    "STATIC",
                    ip_addr.to_string(),
                    mask.to_string(),
                    gateway.to_string()
                )
                .as_str(),
            ),
        };
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0))
    }
    /// Get baudrate setting for the USB or RS-422 interface
    pub fn get_baud_rate(&mut self, ifc: SerialInterface) -> BaseResult<u32> {
        let cmd = match ifc {
            SerialInterface::Rs422 => Command::new(ModuleScope::Any, ModeScope::Any, "GBR RS422"),
            SerialInterface::Usb => Command::new(ModuleScope::Any, ModeScope::Any, "GBR USB"),
        };
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0).parse()?)
    }
    /// Set the baudrate for the USB or RS-422 interface on the controller.
    pub fn set_baud_rate(&mut self, ifc: SerialInterface, baud: u32) -> BaseResult<String> {
        if BAUD_BOUNDS.contains(&baud) {
            let cmd = match ifc {
                SerialInterface::Rs422 => Command::new(
                    ModuleScope::Any,
                    ModeScope::Any,
                    format!("SBR RS422 {}", baud).as_str(),
                ),
                SerialInterface::Usb => Command::new(
                    ModuleScope::Any,
                    ModeScope::Any,
                    format!("SBR USB {}", baud).as_str(),
                ),
            };
            let mut v = self.handle_command(&cmd, Some(1), None)?;
            Ok(v.remove(0))
        } else {
            Err(Error::Bound(format!(
                "Out of range for baudrate: {}-{}, got {}",
                BAUD_BOUNDS.start(),
                BAUD_BOUNDS.end(),
                baud
            )))
        }
    }
    /// Instructs a module to update its firmware based. Firmware must be uploaded
    /// to the controller via the web interface and must match the passed filename.
    /// TODO: Figure out how handle the response; the controller will respond only
    /// once the firmware is fully updated (long time.)
    pub fn start_mod_fw_update(&mut self, fname: &str, slot: Slot) -> BaseResult<()> {
        let idx = u8::from(slot.clone()) as usize;
        if self.modules[idx - 1].is_some() {
            let cmd = Command::new(
                ModuleScope::Any,
                ModeScope::Any,
                format!("FU {} {}", slot, fname).as_str(),
            );
            let _ = self.handle_command(&cmd, None, Some(slot))?;
            Ok(())
        } else {
            Err(Error::InvalidParams(format!("Slot {} is empty", slot)))
        }
    }
    /// Get the fail-safe state of the CADM2 module.
    pub fn get_fail_safe_state(&mut self, slot: Slot) -> BaseResult<String> {
        let idx = u8::from(slot.clone()) as usize;
        if self.modules[idx - 1].is_some() {
            let cmd = Command::new(
                ModuleScope::Only(vec![Module::Cadm]),
                ModeScope::Any,
                format!("GFS {}", slot).as_str(),
            );
            let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
            Ok(v.remove(0))
        } else {
            Err(Error::InvalidParams(format!("Slot {} is empty", slot)))
        }
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
    /// Used since we don't care about using T in data members
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
            (None, Some(s)) => Self::walk_com_ports(Some(s)).ok_or(Error::DeviceNotFound)?,
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
    fn walk_com_ports(serial_num: Option<&str>) -> Option<String> {
        let ports = available_ports().ok()?;
        let ports_with_pid: Vec<&str> = ports
            .iter()
            .filter_map(|port| {
                match &port.port_type {
                    // Check that a device exists on a USB port with the well-known PID
                    SerialPortType::UsbPort(info) if info.pid == DEVICE_PID => {
                        Some((port.port_name.as_str(), info.serial_number.as_ref()))
                    }
                    _ => None,
                }
            })
            .filter_map(|(name, found_sn)| {
                // Check if a passed serial number matches that of any remaining elements
                // If caller does not pass a serial number, perform no filtering.
                match (serial_num, found_sn) {
                    (Some(passed_sn), Some(found_sn)) if passed_sn == found_sn => Some(name),
                    (None, _) => Some(name),
                    _ => None,
                }
            })
            .collect();

        // Pull out the path of the first COM port if it exists.
        ports_with_pid
            .get(0)
            .and_then(|port| Some(port.to_string()))
    }
}
impl BaseControllerBuilder<Network> {
    fn new() -> BaseController {
        todo!("Need to determine whether the controller supports TCP or UDP...")
    }
}
