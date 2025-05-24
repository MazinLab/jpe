// Defines types and functionality related to the base controller

#[derive(Debug, Clone, PartialEq)]
/// Reperesents the different types of Module supported by the controller
pub(crate) enum Module {
    All,
    Cadm,
    Rsm,
    Oem,
    Psm,
    Edm,
}
/// Abstract, central representation of the Controller
#[derive(Debug, Clone, PartialEq)]
pub struct BaseController {}

/// The operation modes supported by the controller
#[derive(Debug, Clone, PartialEq)]
pub enum ControllerOpMode {
    Basedrive,
    Servodrive,
    Flexdrive,
}

/// Connection mode to the controller
#[derive(Debug, Clone)]
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
    CrLfDelimited(Vec<String>),
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
    pub(crate) AllowedModule: ModuleScope,
    /// Controller operation modes that support this command
    pub(crate) AllowedMode: ModeScope,
    pub(crate) Payload: String,
    pub(crate) RespType: Response,
}
