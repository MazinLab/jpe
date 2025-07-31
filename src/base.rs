// Defines types and functionality related to the base controller
use crate::config::*;

pub mod context;
pub use context::BaseContext;
pub(crate) use context::register_pyo3;

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
