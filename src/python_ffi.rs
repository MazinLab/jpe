// Python extensions for existing types

use std::fmt::Debug;
use std::str::FromStr;

use crate::{base::Error, config::Slot};
use pyo3::exceptions::{
    PyAttributeError, PyConnectionError, PyException, PyIOError, PyOverflowError, PyUnicodeError,
    PyValueError,
};
use pyo3::ffi::PyType_Slot;
use pyo3::prelude::*;
use pyo3::types::PyType;

// ======= Error Mapping =======
// Define mapping between the crate local custom Error variants and Python
// exceptions
impl From<Error> for PyErr {
    fn from(e: Error) -> Self {
        match e {
            Error::Serial(e) => PyConnectionError::new_err(e.to_string()),
            Error::Io(e) => PyIOError::new_err(e.to_string()),
            Error::DeviceNotFound => PyException::new_err("Device not found"),
            Error::InvalidParams(s) => PyValueError::new_err(s),
            Error::InvalidResponse(s) => PyValueError::new_err(s),
            Error::WrongConnMode { expected, found } => PyAttributeError::new_err(format!(
                "Wrong connection mode. Got {}, expected {}.",
                found, expected
            )),
            Error::General(s) => PyException::new_err(s),
            Error::BufOverflow { max_len, idx } => {
                PyOverflowError::new_err(format!("Buffer overflow, max: {}, idx: {}", max_len, idx))
            }
            Error::Bound(s) => PyValueError::new_err(s),
            Error::Utf8(e) => PyUnicodeError::new_err(e),
            Error::DeviceError(s) => PyException::new_err(format!("Device Error: {}", s)),
            Error::ParseIntError(e) => PyValueError::new_err(e),
            Error::ParseFloatError(e) => PyValueError::new_err(e),
        }
    }
}

// ======= Config Type Mappings =======
// Python extensions for config spec types, mostly for trait methods

#[pymethods]
impl Slot {
    #[classmethod]
    /// Fallibly constructs a Slot object from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Slot::from_str(s).map_err(PyErr::from)
    }
    /// Maps a Slot object to it's
    fn to_int(&self) -> PyResult<u8> {
        Ok(u8::from(self.clone()))
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
}
