//! Remote control of the JPE CPSC1 controller and associated modules.
//!
//! The `jpe` crate provides a Rust and Python implementation, via `PyO3`,
//! for controlling and administering the CPSC1 controller and the following
//! modules:
//! * RSM
//! * CADM2
//!
//! Please view [API documentation](https://www.jpe-innovations.com/wp-content/uploads/CNP_MAN02_R05_Software-User-Manual.pdf)
//! provided by JPE for more details.
//!
//! Only commands supported by the previously mentioned modules are implemented, PRs are welcome
//! for adding support for other modules!
//!
//! # Example
//! This example opens a connection to the controller using serial transport
//! and queries for the supported cryo stage SKUs.
//!
//! ```no_run
//! # fn example() -> std::io::Result<()> {
//! use jpe::BaseContextBuilder;
//!
//! // On Windows, use something like "COM1" or "COM15".
//! let mut ctx = BaseContextBuilder::new().with_serial("/dev/cu.usbserial-D30IYJT2").build()?;
//! let supported_stages = ctx.get_supported_stages()?;
//! # }
//! ```
//! # Example
//! This example opens a connection to the controller using network transport and
//! enables scan mode (E.g. for driving a piezo scanner) on the CADM2 module in slot one
//! of the controller cabinet.
//!
//! ```no_run
//! # fn example() -> std::io::Result<()> {
//! use jpe::{BaseContextBuilder, Slot};
//!
//! let mut ctx = BaseContextBuilder::new().with_network("169.254.10.10").build()?;
//! let _ = ctx.enable_scan_mode(Slot::One, 512)?;
//! # }
//! ```
use std::{
    net::AddrParseError,
    num::{ParseFloatError, ParseIntError},
    str::Utf8Error,
};

use thiserror::Error;

pub mod base;
pub mod builder;
pub(crate) mod transport;
pub use builder::BaseContextBuilder;
pub use config::{Direction, IpAddrMode, ModuleChannel, SerialInterface, SetpointPosMode, Slot};
pub mod config;

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
mod python_ffi;

/// Errors for the base controller api
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Device not found.")]
    DeviceNotFound,
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    InvalidResponse(String),
    #[error("{0}")]
    Other(String),
    #[error("max_len: {}, idx: {}", max_len, idx)]
    BufOverflow { max_len: usize, idx: usize },
    #[error("{0}")]
    Bound(String),
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    #[error("{0}")]
    DeviceError(String),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    ParseFloatError(#[from] ParseFloatError),
    #[error(transparent)]
    AddrParseError(#[from] AddrParseError),
}

pub type BaseResult<T> = std::result::Result<T, Error>;

// Define the Python module that exposes Pyo3 API to python users.
#[cfg(feature = "python")]
#[pymodule]
#[pyo3(name = "jpe_python_ffi")]
fn py_module(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    config::register_pyo3(py, m)?;
    base::register_pyo3(py, m)?;
    python_ffi::register_pyo3(py, m)?;
    Ok(())
}
