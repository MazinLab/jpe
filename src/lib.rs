use crate::config::*;
use std::{
    net::AddrParseError,
    num::{ParseFloatError, ParseIntError},
    str::Utf8Error,
};

use pyo3::prelude::*;
use serialport;
use thiserror::Error;

pub mod base;
pub mod config;
mod python_ffi;

/// Errors for the base controller api
#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Serial(#[from] serialport::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Device not found.")]
    DeviceNotFound,
    #[error("{0}")]
    InvalidParams(String),
    #[error("{0}")]
    InvalidResponse(String),
    #[error("expected: {}, found: {}", expected, found)]
    WrongConnMode { expected: ConnMode, found: ConnMode },
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
#[pymodule]
#[pyo3(name = "jpe_python_ffi")]
fn py_module(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    config::register_pyo3(py, m)?;
    base::register_pyo3(py, m)?;
    python_ffi::register_pyo3(py, m)?;
    Ok(())
}
