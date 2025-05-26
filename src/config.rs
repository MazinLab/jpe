// Contains types restricting values related to the controller API spec
use crate::base::Error;
use std::{fmt::Display, str::FromStr};

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
impl FromStr for Slot {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase() {
            _ if s == "one" || s == "1" => Ok(Self::One),
            _ if s == "two" || s == "2" => Ok(Self::Two),
            _ if s == "three" || s == "3" => Ok(Self::Three),
            _ if s == "four" || s == "4" => Ok(Self::Four),
            _ if s == "five" || s == "5" => Ok(Self::Five),
            _ if s == "six" || s == "6" => Ok(Self::Six),
            _ => Err(Error::InvalidParams(format!(
                "Supported slots are 1 - 6 or One - Six, got {}",
                s
            ))),
        }
    }
}
impl Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Slot::One => "1",
            Slot::Two => "2",
            Slot::Three => "3",
            Slot::Four => "4",
            Slot::Five => "5",
            Slot::Six => "6",
        };
        write!(f, "{}", s)
    }
}
impl From<Slot> for u8 {
    fn from(slot: Slot) -> Self {
        match slot {
            Slot::One => 1,
            Slot::Two => 2,
            Slot::Three => 3,
            Slot::Four => 4,
            Slot::Five => 5,
            Slot::Six => 6,
        }
    }
}

/// Supported serial modes for the controller
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerialInterface {
    Rs422,
    Usb,
}
impl FromStr for SerialInterface {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase() {
            _ if s == "rs422" => Ok(Self::Rs422),
            _ if s == "usb" => Ok(Self::Usb),
            _ => Err(Error::InvalidParams(
                "Invalid serial mode, only RS422 or USB supported".to_string(),
            )),
        }
    }
}

/// Supported address assignment mode for the controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpAddrMode {
    Dhcp,
    Static,
}
impl FromStr for IpAddrMode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase() {
            _ if s == "dhcp" => Ok(Self::Dhcp),
            _ if s == "static" => Ok(Self::Static),
            _ => Err(Error::InvalidParams(
                "Invalid addressing mode, only DHCP or Static supported".to_string(),
            )),
        }
    }
}

/// Reperesents the different types of Module supported by the controller
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Module {
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
            _ => Err(Error::InvalidResponse(format!("Unknown module: {}", s))),
        }
    }
}
impl FromStr for Module {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // The device spec uses ASCII
        let s = s.to_ascii_lowercase();
        match s {
            _ if s.starts_with("cadm") => Ok(Self::Cadm),
            _ if s.starts_with("rsm") => Ok(Self::Rsm),
            _ if s.starts_with("oem") => Ok(Self::Oem),
            _ if s.starts_with("psm") => Ok(Self::Psm),
            _ if s.starts_with("edm") => Ok(Self::Edm),
            _ => Err(Error::InvalidResponse(format!("Unknown module: {}", s))),
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
pub struct Serial;

/// Network connection mode to the controller. Used in type-state-builder
/// pattern for controller creation
#[derive(Debug, Clone, PartialEq)]
pub struct Network;

/// Connection mode to the controller. Used internally by the controller
/// base API.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConnMode {
    Serial,
    Network,
}

/// Specific channel of a Module
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleChannel {
    One,
    Two,
    Three,
}

impl FromStr for ModuleChannel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase() {
            _ if s == "one" || s == "1" => Ok(Self::One),
            _ if s == "two" || s == "2" => Ok(Self::Two),
            _ if s == "three" || s == "3" => Ok(Self::Three),
            _ => Err(Error::InvalidParams(format!("Invalid channel: {}", s))),
        }
    }
}
impl Display for ModuleChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
        };
        write!(f, "{}", s)
    }
}
impl From<ModuleChannel> for u8 {
    fn from(m: ModuleChannel) -> Self {
        match m {
            ModuleChannel::One => 1,
            ModuleChannel::Two => 2,
            ModuleChannel::Three => 3,
        }
    }
}
