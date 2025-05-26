// Python extensions for existing types

use crate::base::Error;
use pyo3::exceptions::{
    PyAttributeError, PyConnectionError, PyException, PyIOError, PyOverflowError, PyUnicodeError,
    PyValueError,
};
use pyo3::prelude::*;

// Define mapping between the crate custom Error variants and Python
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
