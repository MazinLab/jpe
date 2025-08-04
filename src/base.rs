// Defines types and functionality related to the base controller
use crate::config::*;

#[cfg(feature = "sync")]
pub mod context;
#[cfg(feature = "sync")]
pub use context::BaseContext;
#[cfg(feature = "sync")]
pub(crate) use context::register_pyo3;

#[cfg(feature = "async")]
pub mod context_async;
#[cfg(feature = "async")]
pub use context_async::BaseContextAsync;

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
