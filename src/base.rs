// Defines types and functionality related to the base controller
use crate::config::*;
use pyo3::prelude::*;
use serialport::{
    DataBits, FlowControl, Parity, SerialPort, SerialPortType, StopBits, available_ports,
};
use std::{
    io::{self, ErrorKind, Read},
    net::{AddrParseError, Ipv4Addr, TcpStream},
    num::{ParseFloatError, ParseIntError},
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
const READ_CHUNK_TIMEOUT: Duration = Duration::from_millis(20);
// Total time to read from the serial input queue.
const READ_TIMEOUT: Duration = Duration::from_millis(200);
const DEVICE_PID: u16 = 0000;
const TCP_PORT: u16 = 2000;
const TERMINATOR: &'static str = "\r\n";

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
    #[error("{0}")]
    ParseFloatError(#[from] ParseFloatError),
    #[error("{0}")]
    AddrParseError(#[from] AddrParseError),
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
            payload: format!("{}{}", payload, TERMINATOR),
        }
    }
}
// Type-state Builder states for the BaseControllerBuilder
pub(crate) struct Init;
pub(crate) struct Serial;
pub(crate) struct Network;

/// Abstract, central representation of the Controller
#[derive(Debug)]
#[pyclass(unsendable)] // Not supporting movement between threads at this time in Python.
pub struct BaseController {
    /// Mode used to connect to the controller
    conn_mode: ConnMode,
    op_mode: ControllerOpMode,
    /// Firmware version of all modules
    fw_vers: String,
    ip_addr: Option<Ipv4Addr>,
    /// Network connection handle (if using network)
    net_conn: Option<TcpStream>,
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
    supported_stages: Vec<String>,
}
// ======= Internal API =======
impl BaseController {
    fn new(
        conn_mode: ConnMode,
        ip_addr: Option<Ipv4Addr>,
        com_port: Option<String>,
        serial_conn: Option<Box<dyn SerialPort>>,
        net_conn: Option<TcpStream>,
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
            supported_stages: Vec::new(),
        }
    }
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
    /// Checks whether a given stage value is supported by the controller
    fn check_stage(&mut self, stage: &str) -> BaseResult<bool> {
        if !self.supported_stages.is_empty() {
            self.supported_stages = self.get_supported_stages()?;
        }
        Ok(self.supported_stages.iter().any(|s| s == stage))
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
        match msg.chars().filter(|c| *c == '\r').count() {
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
        match self.conn_mode {
            ConnMode::Serial => self.read_serial_chunks(),
            ConnMode::Network => todo!(),
        }
    }
    /// Low-level reader for the serial connection modes.
    /// Returns bytes read.
    fn read_serial_chunks(&mut self) -> BaseResult<usize> {
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
            // Break out cases
            if read_timer_start.elapsed() > READ_TIMEOUT
                || self.read_buffer.ends_with(TERMINATOR.as_bytes())
            {
                break;
            }
        }
        // Clear the input buffer of any residual junk and return bytes read
        reader.clear(serialport::ClearBuffer::Input)?;
        Ok(total_bytes_read)
    }
    /// Used to keep the request/response paradigm in sync by draining
    /// the recv buffer of the TcpStream
    fn clear_tcp_recv_buf(&mut self) -> BaseResult<()> {
        let mut chunk_buf: [u8; READ_CHUNK_SIZE] = [0; READ_CHUNK_SIZE];
        let reader = self.net_conn.as_mut().ok_or(Error::WrongConnMode {
            expected: ConnMode::Network,
            found: ConnMode::Serial,
        })?;
        // Set in non-blocking mode and drain any remanining data from stream.
        reader.set_nonblocking(true)?;
        loop {
            match reader.read(&mut chunk_buf) {
                // Stream has been closed.
                Ok(0) => break,
                // Discard any data that is read
                Ok(_) => continue,
                // No data to read, waiting on OS to present more data.
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) => return Err(Error::Io(e)),
            }
        }
        reader.set_nonblocking(false)?;
        Ok(())
    }
    /// Low-level reader for the network connection mode.
    /// Returns bytes read.
    fn read_network_chunks(&mut self) -> BaseResult<usize> {
        let mut chunk_buf: [u8; READ_CHUNK_SIZE] = [0; READ_CHUNK_SIZE];

        // Loop to read in chunks and iteratively add to internal read buffer
        // until total timeout is reached.
        let read_timer_start = Instant::now();
        let mut total_bytes_read = 0usize;
        let reader = self.net_conn.as_mut().ok_or(Error::WrongConnMode {
            expected: ConnMode::Network,
            found: ConnMode::Serial,
        })?;
        // Set in non-blocking mode and drain any remanining data from stream.
        reader.set_nonblocking(true)?;

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
            // Break out cases
            if read_timer_start.elapsed() > READ_TIMEOUT
                || self.read_buffer.ends_with(TERMINATOR.as_bytes())
            {
                break;
            }
        }
        // return bytes read
        Ok(total_bytes_read)
    }

    // Handles the interplay between polling the device and capturing the
    // acknowledgment that most API functions will use.
    fn comms_handler(&mut self, cmd: &Command) -> BaseResult<Response> {
        // encode and send data on wire
        match self.conn_mode {
            ConnMode::Serial => {
                if let Some(ref mut handle) = self.serial_conn {
                    handle.clear(serialport::ClearBuffer::Output)?;
                    handle.write_all(cmd.payload.as_bytes())?;
                } else {
                    return Err(Error::General("Serial handle not found.".to_string()));
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
        let resp = self.comms_handler(&cmd)?;
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
}

// ======= External API =======
// Only methods that are exposed publically in Rust (not Python compatible without extension)

impl BaseController {
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
                &format!(
                    "{} {} {} {} {}",
                    "/IPS", "DHCP", "0.0.0.0", "0.0.0.0", "0.0.0.0"
                ),
            ),
            IpAddrMode::Static => Command::new(
                ModuleScope::Any,
                ModeScope::Any,
                &format!(
                    "{} {} {} {} {}",
                    "/IPS",
                    "STATIC",
                    ip_addr.to_string(),
                    mask.to_string(),
                    gateway.to_string()
                ),
            ),
        };
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0))
    }
}

// ======= PyO3 Compatible External API =======
// Contains methods that are externally accessible from Rust and Python (without extension)
// along with PRIVATE methods (Rust) that extended externally accessible Rust methods
// that are not directly compatible with Python.
#[pymethods]
impl BaseController {
    /// Returns the firmware version of the controller and updates internal value.
    pub fn get_fw_version(&mut self) -> BaseResult<String> {
        if !self.fw_vers.is_empty() {
            Ok(self.fw_vers.clone())
        } else {
            // Build Command and send to controller
            let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "/VER");
            // Extract, set, and return value. Direct indexing safe due to bounds check by the handle command
            // method.
            let mut v = self.handle_command(&cmd, Some(1), None)?;
            self.fw_vers = v[0].clone();
            Ok(v.remove(0))
        }
    }
    /// Returns firmware version information of module in given slot. Returns None if slot is empty.
    pub fn get_mod_fw_version(&mut self, slot: Slot) -> BaseResult<String> {
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, &format!("FIV {}", slot));
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Returns a list of all installed modules and updates internal module container
    pub fn get_module_list(&mut self) -> BaseResult<Vec<String>> {
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "/MODLIST");
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
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "/STAGES");
        Ok(self.handle_command(&cmd, None, None)?)
    }
    /// Returns IP configuration for the LAN interface.
    /// Response: [MODE],[IP address],[Subnet Mask],[Gateway],[MAC Address]
    pub fn get_ip_config(&mut self) -> BaseResult<Vec<String>> {
        let cmd = Command::new(ModuleScope::Any, ModeScope::Any, "/IPR");
        Ok(self.handle_command(&cmd, Some(5), None)?)
    }
    /// Private python extension method for the `set_ip_config`. Sets the IP address
    /// configuration for the controller.
    fn set_ip_config_py(
        &mut self,
        addr_mode: IpAddrMode,
        ip_addr: &str,
        mask: &str,
        gateway: &str,
    ) -> BaseResult<String> {
        self.set_ip_config(addr_mode, ip_addr.parse()?, mask.parse()?, gateway.parse()?)
    }

    /// Get baudrate setting for the USB or RS-422 interface
    pub fn get_baud_rate(&mut self, ifc: SerialInterface) -> BaseResult<u32> {
        let cmd = match ifc {
            SerialInterface::Rs422 => Command::new(ModuleScope::Any, ModeScope::Any, "/GBR RS422"),
            SerialInterface::Usb => Command::new(ModuleScope::Any, ModeScope::Any, "/GBR USB"),
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
                    &format!("/SBR RS422 {}", baud),
                ),
                SerialInterface::Usb => Command::new(
                    ModuleScope::Any,
                    ModeScope::Any,
                    &format!("/SBR USB {}", baud),
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
        let cmd = Command::new(
            ModuleScope::Any,
            ModeScope::Any,
            &format!("FU {} {}", slot, fname),
        );
        let _ = self.handle_command(&cmd, None, Some(slot))?;
        Ok(())
    }
    /// Get the fail-safe state of the CADM2 module.
    pub fn get_fail_safe_state(&mut self, slot: Slot) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Cadm]),
            ModeScope::Any,
            &format!("GFS {}", slot),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Starts moving an actuator or positioner with specified parameters in open loop mode. Supported on
    /// CADM2 modules.
    pub fn move_stage_open(
        &mut self,
        slot: Slot,
        direction: Direction,
        step_freq: u16,
        r_step_size: u8,
        n_steps: u16,
        temp: u16,
        stage: &str,
        drive_factor: f32,
    ) -> BaseResult<String> {
        // Bounds check all the input variables
        if ![
            STEP_FREQ_BOUNDS.contains(&step_freq),
            RELATIVE_ACTUATOR_STEP_SIZE_BOUND.contains(&r_step_size),
            NUM_STEPS_BOUNDS.contains(&n_steps),
            TEMP_BOUNDS.contains(&temp),
            DRIVE_FACTOR_BOUNDS.contains(&drive_factor),
        ]
        .iter()
        .all(|cond| *cond)
        {
            return Err(Error::Bound("Input parameter out of bounds.".to_string()));
        }

        // Get supported stages and see if passed stage value is supported.
        if !self.check_stage(stage)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage)));
        }

        // Create the command and send to controller
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Cadm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!(
                "MOV {} {} {} {} {} {} {} {}",
                slot, direction, step_freq, r_step_size, n_steps, temp, stage, drive_factor
            ),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Stops movement of an actuator (MOV command), disables external input mode (EXT command,
    /// breaks out of Flexdrive mode) or disables scan mode (SDC command).
    pub fn stop_stage(&mut self, slot: Slot) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Cadm]),
            ModeScope::Only(vec![
                ControllerOpMode::Basedrive,
                ControllerOpMode::Flexdrive,
            ]),
            &format!("STP {}", slot),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        self.op_mode = ControllerOpMode::Basedrive;
        Ok(v.remove(0))
    }
    /// CADM module will output a DC voltage level (to be used with a scanner piezo for example) instead of
    /// the default drive signal. `level` can be set to a value in between 0 and 1023 where zero represents
    /// ~0[V] output (-30[V] with respect to REF) and the maximum value represents ~150[V]
    /// output (+120[V] with respect to REF).
    pub fn enable_scan_mode(&mut self, slot: Slot, level: u16) -> BaseResult<String> {
        if !SCANNER_LEVEL_BOUNDS.contains(&level) {
            return Err(Error::Bound(format!(
                "Level out of range, {}-{}, got {}",
                SCANNER_LEVEL_BOUNDS.start(),
                SCANNER_LEVEL_BOUNDS.end(),
                level
            )));
        }
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Cadm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("SDC {} {}", slot, level),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Sets the CADM in external control mode (Flexdrive mode). Similar to MOV, but
    /// `step_freq` now defines the step frequency at maximum (absolute) input signal. By
    /// default, set this to 600 [Hz]. `direction` now modulates the stage movement direction
    /// with respect to the polarity of the external input signal (E.g Negative -> positive external signal voltage drives
    /// the stage in the negative direction)
    pub fn enable_ext_input_mode(
        &mut self,
        slot: Slot,
        direction: Direction,
        step_freq: u16,
        r_step_size: u8,
        temp: u16,
        stage: &str,
        drive_factor: f32,
    ) -> BaseResult<String> {
        // Bounds check all the input variables
        if ![
            STEP_FREQ_BOUNDS.contains(&step_freq),
            RELATIVE_ACTUATOR_STEP_SIZE_BOUND.contains(&r_step_size),
            TEMP_BOUNDS.contains(&temp),
            DRIVE_FACTOR_BOUNDS.contains(&drive_factor),
        ]
        .iter()
        .all(|cond| *cond)
        {
            return Err(Error::Bound("Input parameter out of bounds.".to_string()));
        }

        // Get supported stages and see if passed stage value is supported.
        if !self.check_stage(stage)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage)));
        }

        // Create the command and send to controller
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Cadm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!(
                "EXT {} {} {} {} {} {} {}",
                slot, direction, step_freq, r_step_size, temp, stage, drive_factor
            ),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        self.op_mode = ControllerOpMode::Flexdrive;
        Ok(v.remove(0))
    }
    /// Get the position of a Resistive Linear Sensor (RLS) connected to a specific channel of the RSM
    /// module. Return value is in meters.
    pub fn get_current_position(
        &mut self,
        slot: Slot,
        ch: ModuleChannel,
        stage: &str,
    ) -> BaseResult<f32> {
        // Get supported stages and see if passed stage value is supported.
        if !self.check_stage(stage)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage)));
        }
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("PGV {} {} {}", slot, ch, stage),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0).parse()?)
    }
    /// Get the position of all three channels of the RSM simultaneously. Return values are in meters
    pub fn get_current_position_all(
        &mut self,
        slot: Slot,
        stage_ch1: &str,
        stage_ch2: &str,
        stage_ch3: &str,
    ) -> BaseResult<(f32, f32, f32)> {
        // Get supported stages and see if passed stage values are supported.
        if !self.check_stage(stage_ch1)? {
            return Err(Error::DeviceError(format!(
                "Stage {} unsupported",
                stage_ch1
            )));
        }
        if !self.check_stage(stage_ch2)? {
            return Err(Error::DeviceError(format!(
                "Stage {} unsupported",
                stage_ch2
            )));
        }
        if !self.check_stage(stage_ch3)? {
            return Err(Error::DeviceError(format!(
                "Stage {} unsupported",
                stage_ch3
            )));
        }
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("PGVA {} {} {} {}", slot, stage_ch1, stage_ch2, stage_ch3),
        );
        let v = self
            .handle_command(&cmd, Some(3), Some(slot))?
            .into_iter()
            .map(|s| s.parse().map_err(|e| Error::ParseFloatError(e)))
            .collect::<BaseResult<Vec<f32>>>()?;

        Ok((v[0], v[1], v[2]))
    }
    /// Set the current position of a Resistive Linear Sensor (RLS) connected to channel `ch` of the RSM to be
    /// the negative end-stop. To be used as part of the RLS Calibration process.
    pub fn set_neg_end_stop(&mut self, slot: Slot, ch: ModuleChannel) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("MIS {} {}", slot, ch),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Set the current position of a Resistive Linear Sensor (RLS) connected to channel `ch` of the RSM to be
    /// the positive end-stop. To be used as part of the RLS Calibration process.
    pub fn set_pos_end_stop(&mut self, slot: Slot, ch: ModuleChannel) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("MIS {} {}", slot, ch),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Read the current value of the negative end-stop parameter set for a channel `ch` of an RSM.
    /// Response value in in meters.
    pub fn read_neg_end_stop(
        &mut self,
        slot: Slot,
        ch: ModuleChannel,
        stage: &str,
    ) -> BaseResult<f32> {
        // Get supported stages and see if passed stage value is supported.
        if !self.check_stage(stage)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage)));
        }
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("MIR {} {} {}", slot, ch, stage),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0).parse()?)
    }
    /// Read the current value of the positive end-stop parameter set for a channel `ch` of an RSM.
    /// Response value in in meters.
    pub fn read_pos_end_stop(
        &mut self,
        slot: Slot,
        ch: ModuleChannel,
        stage: &str,
    ) -> BaseResult<f32> {
        // Get supported stages and see if passed stage value is supported.
        if !self.check_stage(stage)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage)));
        }
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("MAR {} {} {}", slot, ch, stage),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0).parse()?)
    }
    /// Reset the current values of the negative and positive end-stop parameters set for channel `ch`
    /// of an RSM to values stored in controller NV-RAM.
    pub fn reset_end_stops(&mut self, slot: Slot, ch: ModuleChannel) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("MMR {} {}", slot, ch),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Set the duty cycle of the sensor excitation signal of the RSM for all channels. `duty` is a percentage and can
    /// be set to 0 or from 10 to 100
    pub fn set_excitation_ds(&mut self, slot: Slot, duty: u8) -> BaseResult<String> {
        if !(duty == 0 || (10..=100).contains(&duty)) {
            return Err(Error::Bound(format!(
                "Duty cycle out of range: 0, 10-100. Got {}",
                duty
            )));
        }
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("EXS {} {}", slot, duty),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Read the duty cycle of the sensor excitation signal for all channels of an RSM.
    /// Response value is a percentage.
    pub fn read_excitation_ds(&mut self, slot: Slot) -> BaseResult<u8> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("EXR {}", slot),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0).parse()?)
    }
    /// Store the current values of the following parameters of an RSM to the non-volatile memory of the
    /// controller: excitation duty cycle (EXS), negative end stop (MIS) and positive end-stop (MAS)
    pub fn save_rsm_nvram(&mut self, slot: Slot) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Only(vec![Module::Rsm]),
            ModeScope::Only(vec![ControllerOpMode::Basedrive]),
            &format!("RSS {}", slot),
        );
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
        Ok(v.remove(0))
    }
    /// Enable the internal position feedback control and start operating in Servodrive mode with up to three
    /// different stages. Initial step frequency is used adjust how fast the stages initally takes steps (the control
    /// loop will reduce this as a setpoint is approached).
    pub fn enable_servodrive(
        &mut self,
        stage_1: &str,
        init_step_freq_1: u16,
        stage_2: &str,
        init_step_freq_2: u16,
        stage_3: &str,
        init_step_freq_3: u16,
        temp: u16,
        drive_factor: f32,
    ) -> BaseResult<String> {
        // Check bounds on input params
        if ![
            DRIVE_FACTOR_BOUNDS.contains(&drive_factor),
            STEP_FREQ_BOUNDS.contains(&init_step_freq_1),
            STEP_FREQ_BOUNDS.contains(&init_step_freq_2),
            STEP_FREQ_BOUNDS.contains(&init_step_freq_3),
            TEMP_BOUNDS.contains(&temp),
        ]
        .iter()
        .all(|b| *b)
        {
            return Err(Error::Bound("Input parameter out of bounds".to_string()));
        }

        // Get supported stages and see if passed stage values are supported.
        if !self.check_stage(stage_1)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage_1)));
        }
        if !self.check_stage(stage_2)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage_2)));
        }
        if !self.check_stage(stage_3)? {
            return Err(Error::DeviceError(format!("Stage {} unsupported", stage_3)));
        }

        let cmd = Command::new(
            ModuleScope::Any,
            ModeScope::Any,
            &format!(
                "FBEN {} {} {} {} {} {} {} {}",
                stage_1,
                init_step_freq_1,
                stage_2,
                init_step_freq_2,
                stage_3,
                init_step_freq_3,
                drive_factor,
                temp
            ),
        );
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0))
    }
    /// Disable the internal position feedback control.
    pub fn disable_servodrive(&mut self) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Any,
            ModeScope::Only(vec![ControllerOpMode::Servodrive]),
            "FBXT",
        );
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0))
    }
    /// The servodrive control loop will be immediately aborted and the actuators will stop at their current location.
    pub fn servodrive_em_stop(&mut self) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Any,
            ModeScope::Only(vec![ControllerOpMode::Servodrive]),
            "FBES",
        );
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0))
    }
    /// In servodrive mode, use this command to move actuators to a set point position. For linear type actuators,
    /// setpoint values is in meters, for rotational, radians. See application notes for description of position mode.
    /// If there is no actuator/stage connected to one of the outputs, enter 0 as position set
    /// point.
    pub fn go_to_setpoint(
        &mut self,
        set_point1: f32,
        pos_mode_1: SetpointPosMode,
        set_point2: f32,
        pos_mode_2: SetpointPosMode,
        set_point3: f32,
        pos_mode_3: SetpointPosMode,
    ) -> BaseResult<String> {
        let cmd = Command::new(
            ModuleScope::Any,
            ModeScope::Only(vec![ControllerOpMode::Servodrive]),
            &format!(
                "FBCS {} {} {} {} {} {}",
                set_point1, pos_mode_1, set_point2, pos_mode_2, set_point3, pos_mode_3,
            ),
        );
        let mut v = self.handle_command(&cmd, Some(1), None)?;
        Ok(v.remove(0))
    }
    /// Returns a (comma-separated) list with status and position error information for the servodrive
    /// control loop.
    /// Response: [ENABLED] [FINISHED] [INVALID SP1] [INVALID SP2] [INVALID SP3] [POS ERROR1] [POS ERROR2] [POS ERROR3]
    /// NOTE: position error is dimensionless!
    pub fn get_servodrive_status(&mut self) -> BaseResult<(u8, u8, u8, u8, u8, i64, i64, i64)> {
        let cmd = Command::new(
            ModuleScope::Any,
            ModeScope::Only(vec![ControllerOpMode::Servodrive]),
            "FBST",
        );
        let mut v = self.handle_command(&cmd, Some(8), None)?;

        // Split the vec into it's u8 and u64 subsets
        let v_u8 = v
            .drain(..=4)
            .map(|s| s.parse().map_err(|e| Error::ParseIntError(e)))
            .collect::<BaseResult<Vec<u8>>>()?;

        let v_u16 = v
            .into_iter()
            .map(|s| s.parse().map_err(|e| Error::ParseIntError(e)))
            .collect::<BaseResult<Vec<i64>>>()?;
        Ok((
            v_u8[0], v_u8[1], v_u8[2], v_u8[3], v_u8[4], v_u16[0], v_u16[1], v_u16[2],
        ))
    }
}

/// Type-State Builder for the Controller type based on connection mode.
pub struct BaseControllerBuilder<T> {
    conn_mode: ConnMode,
    ip_addr: Option<Ipv4Addr>,
    net_conn: Option<TcpStream>,
    com_port: Option<String>,
    serial_num: Option<String>,
    baud_rate: Option<u32>,
    _state: T,
}
impl BaseControllerBuilder<Init> {
    /// Starts the type-state builder pattern
    pub fn new() -> BaseControllerBuilder<Init> {
        Self {
            com_port: None,
            conn_mode: ConnMode::Serial,
            ip_addr: None,
            net_conn: None,
            serial_num: None,
            baud_rate: None,
            _state: Init,
        }
    }
    /// Continues in the path to build the controller using serial (USB or RS-422).
    pub fn with_serial(
        self,
        com_port: &str,
        serial_num: &str,
        baud_rate: u32,
    ) -> BaseControllerBuilder<Serial> {
        BaseControllerBuilder {
            conn_mode: ConnMode::Serial,
            ip_addr: None,
            net_conn: None,
            com_port: Some(com_port.to_string()),
            serial_num: Some(serial_num.to_string()),
            baud_rate: Some(baud_rate),
            _state: Serial,
        }
    }
    /// Continies in the path to build the controller using IP.
    pub fn with_network(self, ip_addr: &str) -> BaseControllerBuilder<Network> {
        todo!()
    }
}
impl BaseControllerBuilder<Serial> {
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
    pub fn build(self) -> BaseResult<BaseController> {
        todo!("Need to determine whether the controller supports TCP or UDP...")
    }
}
/// Used to register all types that are to be accessible
/// via Python with the centralized PyModule
pub(crate) fn register_pyo3(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BaseController>()?;
    Ok(())
}
