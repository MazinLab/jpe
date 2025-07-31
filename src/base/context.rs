// Defines types and functionality related to the base controller
use super::*;
use crate::{BaseResult, Error, transport::*};
use pyo3::prelude::*;
use std::{net::Ipv4Addr, str::FromStr};

/// Abstract, central representation of the Controller.
#[derive(Debug)]
#[pyclass]
pub struct BaseContext {
    /// Mode used to connect to the controller
    op_mode: ControllerOpMode,
    /// Firmware version of controller
    fw_vers: String,
    /// Type-erased connection. Using dynamic dispatch due to PyO3 not
    /// supporting generic types.
    conn: Box<dyn Transport>,
    /// Serial connection handle (if using serial)
    /// Internal representation of the installed modules
    modules: [Module; 6],
    supported_stages: Vec<String>,
}
// ======= Internal API =======
impl BaseContext {
    pub(crate) fn new(conn: Box<dyn Transport>) -> Self {
        // Initialize modules vec with installed modules.
        Self {
            op_mode: ControllerOpMode::Basedrive,
            fw_vers: "".to_string(),
            conn,
            modules: [Module::Empty; 6],
            supported_stages: Vec::new(),
        }
    }
    /// Checks whether a command is valid given the current operation mode of the controller
    /// and given slot.
    fn check_command(&self, cmd: &Command, slot: Option<Slot>) -> BaseResult<()> {
        if !match &cmd.allowed_mode {
            ModeScope::Any => true,
            ModeScope::Only(modes) => modes.contains(&self.op_mode),
        } {
            return Err(Error::InvalidParams(format!(
                "Unsupported command: '{}', in mode: '{}'",
                &cmd, self.op_mode
            )));
        }
        if !match (&cmd.allowed_mod, &slot) {
            (ModuleScope::Any, _) => true,
            (ModuleScope::Only(mods), Some(slot)) => match slot {
                Slot::One => mods.contains(&self.modules[0]),
                Slot::Two => mods.contains(&self.modules[1]),
                Slot::Three => mods.contains(&self.modules[2]),
                Slot::Four => mods.contains(&self.modules[3]),
                Slot::Five => mods.contains(&self.modules[4]),
                Slot::Six => mods.contains(&self.modules[5]),
            },
            // This is a non-expected path, but should return true if it is used.
            (ModuleScope::Only(_), None) => true,
        } {
            // SAFETY: The number of slots is mapped to the size the const array.
            // Indexing here should be safe.
            return Err(Error::InvalidParams(format!(
                "Unsupported command: '{}', for module: '{}'",
                &cmd,
                self.modules
                    [u8::from(slot.expect("Slot always present in false case.")) as usize - 1]
            )));
        }
        Ok(())
    }
    /// Checks whether a given stage is supported by the controller
    fn check_stage(&mut self, stage: &str) -> BaseResult<bool> {
        if self.supported_stages.is_empty() {
            self.supported_stages = self.get_supported_stages()?;
        }
        Ok(self.supported_stages.iter().any(|s| s == stage))
    }

    /// Handler to abstract the boilerplate used in most command methods. The length bounds check allows
    /// for the use of safe direct indexing into the resulting return value deeper in the call stack.
    fn handle_command(
        &mut self,
        cmd: &Command,
        n_resp_vals: Option<usize>,
        slot: Option<Slot>,
    ) -> BaseResult<Vec<String>> {
        // Check to verify if command is valid
        self.check_command(cmd, slot)?;

        let resp = self.conn.transact(&cmd)?;
        match resp {
            Frame::Error(s) => Err(Error::DeviceError(s)),
            Frame::CrDelimited(v) | Frame::CommaDelimited(v) => {
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
                // None implies length can be variable, return as-is.
                } else {
                    return Ok(v);
                }
            }
        }
    }
}

// ======= External API =======
// Only methods that are exposed publically in Rust (not Python compatible without extension)

impl BaseContext {
    /// Sets the IP configuration for the LAN interface
    pub fn set_ip_config(
        &mut self,
        addr_mode: IpAddrMode,
        ip_addr: impl AsRef<str>,
        mask: impl AsRef<str>,
        gateway: impl AsRef<str>,
    ) -> BaseResult<String> {
        let ip_addr: Ipv4Addr = ip_addr.as_ref().parse()?;
        let mask: Ipv4Addr = mask.as_ref().parse()?;
        let gateway: Ipv4Addr = gateway.as_ref().parse()?;

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
                &format!("{} {} {} {} {}", "/IPS", "STATIC", ip_addr, mask, gateway),
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
impl BaseContext {
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
            .map(|mod_str| Module::from_str(mod_str))
            .collect::<BaseResult<Vec<Module>>>()?
            .iter()
            .enumerate()
            .for_each(|(idx, new_mod)| self.modules[idx] = new_mod.clone());
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
        self.set_ip_config(addr_mode, ip_addr, mask, gateway)
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
        self.op_mode = ControllerOpMode::Basedrive;
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
            ModeScope::Only(vec![ControllerOpMode::Flexdrive]),
            &format!(
                "EXT {} {} {} {} {} {} {}",
                slot, direction, step_freq, r_step_size, temp, stage, drive_factor
            ),
        );
        self.op_mode = ControllerOpMode::Flexdrive;
        let mut v = self.handle_command(&cmd, Some(1), Some(slot))?;
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
            &format!("MAS {} {}", slot, ch),
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

        self.op_mode = ControllerOpMode::Servodrive;
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
        self.op_mode = ControllerOpMode::Basedrive;
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
        self.op_mode = ControllerOpMode::Basedrive;
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

        let v_i64 = v
            .into_iter()
            .map(|s| s.parse().map_err(|e| Error::ParseIntError(e)))
            .collect::<BaseResult<Vec<i64>>>()?;
        Ok((
            v_u8[0], v_u8[1], v_u8[2], v_u8[3], v_u8[4], v_i64[0], v_i64[1], v_i64[2],
        ))
    }
}

/// Used to register all types that are to be accessible
/// via Python with the centralized PyModule
pub(crate) fn register_pyo3(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BaseContext>()?;
    Ok(())
}
// ========== Tests ==========
// NOTE: These are integration tests, but PyO3 currently has a bug (at least in 0.25.0) that
// does not allow for using integration tests as a separate crate (recommended). The workarounds
// listed in the issue and in the FAQ do not work for MacOS (aarch64-apple-darwin).
#[cfg(test)]
#[cfg(target_os = "linux")]
mod test {
    use super::*;
    use serial2::SerialPort;
    use std::{
        sync::mpsc::{Sender, channel},
        thread::JoinHandle,
    };

    /// Mock controller used to test serial reads/writes from the API
    /// in a concurrent fashion.
    struct MockSerialResponder {
        thread_handle: Option<JoinHandle<()>>,
    }
    impl MockSerialResponder {
        fn new(port: SerialPort, query: &[u8], resp: &[u8]) -> (Self, Sender<()>) {
            let q = query.to_vec();
            let r = resp.to_vec();
            let (tx_port, rx_port) = channel::<()>();

            let handle = std::thread::spawn(move || {
                let mut buf = [0u8; 512];
                loop {
                    // Look for the stop signal
                    if rx_port.try_recv().is_ok() {
                        break;
                    }
                    match port.read(&mut buf) {
                        Ok(n_bytes_read) => {
                            // Check to see if the bytes read equals the given
                            // query
                            if &buf[..n_bytes_read] == q.as_slice() {
                                let _ =
                                    port.write_all(r.as_slice()).expect("Failed on mock write.");
                            }
                            break;
                        }
                        Err(_) => break,
                    }
                }
            });
            (
                Self {
                    thread_handle: Some(handle),
                },
                tx_port,
            )
        }
    }
    impl Drop for MockSerialResponder {
        fn drop(&mut self) {
            // Clean up thread
            let _ = self
                .thread_handle
                .take()
                .expect("Thread should always spawn if constructed")
                .join();
        }
    }

    fn setup_mock_and_base(
        query: &[u8],
        resp: &[u8],
    ) -> BaseResult<(MockSerialResponder, BaseContext, Sender<()>)> {
        let (pty_port_a, pty_port_b) = SerialPort::pair()?;

        let controller = BaseContext {
            conn_mode: ConnMode::Serial,
            op_mode: ControllerOpMode::Basedrive,
            fw_vers: "".to_string(),
            ip_addr: None,
            net_conn: None,
            com_port: Some("fake".to_string()),
            serial_conn: Some(pty_port_a),
            baud_rate: Some(9600),
            read_buffer: vec![0; READ_BUF_SIZE],
            modules: [Module::Empty; 6],
            supported_stages: Vec::new(),
        };

        let (mock_device, stop_ch) = MockSerialResponder::new(pty_port_b, query, resp);

        Ok((mock_device, controller, stop_ch))
    }
    #[test]
    fn test_build_mock_state() {
        let from_api = b"/VER\r\n";
        let from_mock_device = b"v8.0.20220221/";
        let ret = setup_mock_and_base(from_api, from_mock_device);
        ret.unwrap();
    }
    #[test]
    fn test_base_controller_bad_terminator() {
        let from_api = b"/VER\r\n";
        let from_mock_device = b"v8.0.20220221/";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };
        // Send data to mock device and read the response.
        let res = controller.get_fw_version();
        assert!(res.is_err());

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_invalid_slot() {
        let from_api = b"FIV 1\r\n";
        let from_mock_device = b"";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller.get_mod_fw_version(Slot::Three);
        assert!(res.is_err());

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_invalid_mode() {
        let from_api = b"MOV 1 1 600 100 0 293 CLA2601 1.2\r\n";
        let from_mock_device = b"";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        controller.supported_stages = vec!["CLA2601".into()];
        controller.op_mode = ControllerOpMode::Servodrive;
        controller.modules[0] = Module::Cadm;
        // Send data to mock device and read the response.
        let res = controller.move_stage_open(
            Slot::One,
            Direction::Positive,
            600,
            100,
            0,
            293,
            "CLA2601",
            1.2,
        );

        assert!(res.is_err());

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_invalid_module() {
        let from_api = b"MOV 1 1 600 100 0 293 CLA2601 1.2\r\n";
        let from_mock_device = b"";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        controller.supported_stages = vec!["CLA2601".into()];
        controller.op_mode = ControllerOpMode::Basedrive;
        controller.modules[0] = Module::Rsm;
        // Send data to mock device and read the response.
        let res = controller.move_stage_open(
            Slot::One,
            Direction::Positive,
            600,
            100,
            0,
            293,
            "CLA2601",
            1.2,
        );

        assert!(res.is_err());

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_fw_version() {
        let from_api = b"/VER\r\n";
        let from_mock_device = b"v8.0.20220221\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };
        // Send data to mock device and read the response.
        let _ = controller
            .get_fw_version()
            .expect("Valid mock response given.");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_modlist_comma() {
        let from_api = b"/MODLIST\r\n";
        let from_mock_device = b"CADM2,CADM2,RSM,OEM2,-,EDM\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .get_module_list()
            .expect("Valid mock response given.");
        assert_eq!(res.len(), 6);
        assert_eq!(res[0].as_str(), "CADM2");
        assert_eq!(res[1].as_str(), "CADM2");
        assert_eq!(res[2].as_str(), "RSM");
        assert_eq!(res[3].as_str(), "OEM2");
        assert_eq!(res[4].as_str(), "-");
        assert_eq!(res[5].as_str(), "EDM");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_modlist_carriage_return() {
        let from_api = b"/MODLIST\r\n";
        let from_mock_device = b"CADM2\rCADM2\rRSM\rOEM2\r-\rEDM\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .get_module_list()
            .expect("Valid mock response given.");
        assert_eq!(res.len(), 6);
        assert_eq!(res[0].as_str(), "CADM2");
        assert_eq!(res[1].as_str(), "CADM2");
        assert_eq!(res[2].as_str(), "RSM");
        assert_eq!(res[3].as_str(), "OEM2");
        assert_eq!(res[4].as_str(), "-");
        assert_eq!(res[5].as_str(), "EDM");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_mod_fw_version() {
        let from_api = b"FIV 1\r\n";
        let from_mock_device = b"CADM2.7.3.20210802\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .get_mod_fw_version(Slot::One)
            .expect("Valid mock response given.");
        assert_eq!(res, "CADM2.7.3.20210802");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_fw_update() {
        let from_api = b"FU 5 Cadm2Firmware.bin\r\n";
        let from_mock_device = b"Firmware update complete.\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller.start_mod_fw_update("Cadm2Firmware.bin", Slot::Five);
        assert!(res.is_ok());

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_get_baud_rate() {
        let from_api = b"/GBR RS422\r\n";
        let from_mock_device = b"9600\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .get_baud_rate(SerialInterface::Rs422)
            .expect("Valid mock response given.");
        assert_eq!(res, 9600);

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_set_baud_rate() {
        let from_api = b"/SBR USB 9600\r\n";
        let from_mock_device = b"Restarting ...\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .set_baud_rate(SerialInterface::Usb, 9600)
            .expect("Valid mock response given.");
        assert_eq!(res, "Restarting ...");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_get_ip_settings() {
        let from_api = b"/IPR\r\n";
        let from_mock_device =
            b"DHCP,192.168.15.62,255.255.255.0,192.168.15.125,40:2e:71:90:e4:8a\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .get_ip_config()
            .expect("Valid mock response given.");
        assert_eq!(res.len(), 5);
        assert_eq!(res[0], "DHCP");
        assert_eq!(res[1], "192.168.15.62");
        assert_eq!(res[2], "255.255.255.0");
        assert_eq!(res[3], "192.168.15.125");
        assert_eq!(res[4], "40:2e:71:90:e4:8a");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_set_ip_settings() {
        let from_api = b"/IPS STATIC 192.168.1.10 255.255.255.0 192.168.1.1\r\n";
        let from_mock_device = b"Restarting ...\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .set_ip_config(
                IpAddrMode::Static,
                "192.168.1.10",
                "255.255.255.0",
                "192.168.1.1",
            )
            .expect("Valid mock response given.");
        assert_eq!(res, "Restarting ...");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }

    #[test]
    fn test_base_controller_get_stages() {
        let from_api = b"/STAGES\r\n";
        let from_mock_device = b"CLA2201,CLA2201-COE,CLA2201MK1,CLA2201MK1-COE,CLA2601\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        // Send data to mock device and read the response.
        let res = controller
            .get_supported_stages()
            .expect("Valid mock response given.");
        assert_eq!(res.len(), 5);
        assert!(res.contains(&"CLA2201".to_string()));
        assert!(res.contains(&"CLA2201-COE".to_string()));
        assert!(res.contains(&"CLA2201MK1".to_string()));
        assert!(res.contains(&"CLA2201MK1-COE".to_string()));
        assert!(res.contains(&"CLA2601".to_string()));

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_cadm2_mov() {
        let from_api = b"MOV 1 1 600 100 0 293 CLA2601 1.2\r\n";
        let from_mock_device = b"";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        controller.supported_stages = vec!["CLA2601".into()];
        controller.op_mode = ControllerOpMode::Basedrive;
        controller.modules[0] = Module::Cadm;
        // Send data to mock device and read the response.
        let res = controller.move_stage_open(
            Slot::One,
            Direction::Positive,
            600,
            100,
            0,
            293,
            "CLA2601",
            1.2,
        );

        assert!(res.is_err());

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
    #[test]
    fn test_base_controller_cadm2_stop() {
        let from_api = b"STP 4\r\n";
        let from_mock_device = b"Stopping the stage.\r\n";

        // Build the mock device and base controller type
        let Ok((mut _mock, mut controller, stop_ch)) =
            setup_mock_and_base(from_api, from_mock_device)
        else {
            panic!("bad test construction!")
        };

        controller.supported_stages = vec!["CLA2601".into()];
        controller.op_mode = ControllerOpMode::Basedrive;
        controller.modules[3] = Module::Cadm;
        // Send data to mock device and read the response.
        let res = controller
            .stop_stage(Slot::Four)
            .expect("Valid response given.");

        assert_eq!(res, "Stopping the stage.");

        // Make sure reader thread is cleaned up.
        let _ = stop_ch.send(());
    }
}
