//! Remote control of the JPE CPSC1 controller and associated modules.
//!
//! The `jpe` crate provides a cross-platform, command-level driver for controlling and administering the [CPSC1](https://www.jpe-innovations.com/cryo-uhv-products/cryo-positioning-systems-controller/)
//! controller, which drives various positioning stages available from [jpe](https://www.jpe-innovations.com/).
//! Both Python and Rust applications are supported.
//!
//! Currently, only the following modules are supported:
//! * RSM
//! * CADM2
//!
//! Please view the [API documentation](https://www.jpe-innovations.com/wp-content/uploads/CNP_MAN02_R05_Software-User-Manual.pdf)
//! provided by JPE for more details.
//!
//! Only commands supported by the previously mentioned modules are implemented, PRs are welcome
//! for adding support for other modules!
//!
//! # Features
//! The crate has three features: `python`, `async`, and, the default, `sync`. `python` only exposes
//! the python bindings tied to the `sync` version of the API (I.E. async in Python needs to be implemented by the
//! user in Python). Using the crate from Rust with only the `python` feature enabled is not supported, `sync` and/or `async` should
//! also be enabled.
//! If Python bindings aren't needed, omitting the `python` feature will suppress any dependencies related to Python binding compliation,
//! which should minimize build headaches and reduce binary size.
//!
//!
//! # Example
//! This example opens a connection to the controller using serial transport
//! and queries for the supported positioning stage SKUs.
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
//! # Using Python
//! To compile Python bindings and install as a module in the active virtual environment, the Python package [`maturin`](https://www.maturin.rs/) should
//! be installed and used. After cloning the `jpe` repo, run the following shell command from the crate root
//! (be sure to activate the appropriate virtual env):
//!```no_run
//! maturin develop --features python
//!```
//!
//! The module should now be installed and can be used with the Python ecosystem. To help with type hints
//! and docstrings in modern IDEs, an optional wrapper module, [`jpe_python`](https://github.com/MazinLab/jpe_python),
//! can be used. Using this wrapper, the construction of the Controller context is more pythonic. If Rust builder ergonomics are
//!  desired, one can forego the convenience given by the wrapper and use the FFI directly.
//!
//! # Example using the FFI directly
//! ```python
//! from jpe_python_ffi import BaseContextBuilder, Slot
//!
//! ctx = BaseContextBuilder().with_network("169.254.10.10").build()
//! ctx.enable_scan_mode(Slot.one(), 512)
//! ```
//!
//! # Example using the `jpe_python` wrapper module.
//! Note the difference in syntax for the constructor and the enums passed as arguments.
//! ```python
//! from jpe_python import ControllerContext, ModuleChannel, Slot
//!
//! ctx = ControllerContext.with_serial("/dev/cu.usbserial-D30IYJT2")
//! ctx.set_neg_end_stop(Slot().four, ModuleChannel().one)
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
