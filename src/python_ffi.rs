// Python extensions for existing types

use std::str::FromStr;

use crate::{
    base::{BaseController, BaseControllerBuilder, Error, Init, Network, Serial},
    config::{
        ConnMode, ControllerOpMode, Direction, IpAddrMode, Module, ModuleChannel, SerialInterface,
        SetpointPosMode, Slot,
    },
};
use pyo3::exceptions::{
    PyAttributeError, PyConnectionError, PyException, PyIOError, PyOverflowError, PyRuntimeError,
    PyUnicodeError, PyValueError,
};
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
            Error::AddrParseError(e) => PyValueError::new_err(e),
        }
    }
}

// ======= Config Type Mappings =======
// Python extensions for config spec types, mostly for trait methods
#[pymethods]
impl Slot {
    #[classmethod]
    /// Fallibly constructs this class from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Self::from_str(s).map_err(PyErr::from)
    }
    /// Returns instance (variant) One
    #[classmethod]
    fn one(_cls: &Bound<'_, PyType>) -> Self {
        Self::One
    }
    /// Returns instance (variant) Two
    #[classmethod]
    fn two(_cls: &Bound<'_, PyType>) -> Self {
        Self::Two
    }
    /// Returns instance (variant) Three
    #[classmethod]
    fn three(_cls: &Bound<'_, PyType>) -> Self {
        Self::Three
    }
    /// Returns instance (variant) Four
    #[classmethod]
    fn four(_cls: &Bound<'_, PyType>) -> Self {
        Self::Four
    }
    /// Returns instance (variant) Five
    #[classmethod]
    fn five(_cls: &Bound<'_, PyType>) -> Self {
        Self::Five
    }
    /// Returns instance (variant) Six
    #[classmethod]
    fn six(_cls: &Bound<'_, PyType>) -> Self {
        Self::Six
    }
    /// Maps instance to int
    fn to_int(&self) -> PyResult<u8> {
        Ok(u8::from(self.clone()))
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}

#[pymethods]
impl SerialInterface {
    #[classmethod]
    /// Fallibly constructs class from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Self::from_str(s).map_err(PyErr::from)
    }
    /// Returns instance (variant) Rs422
    #[classmethod]
    fn rs422(_cls: &Bound<'_, PyType>) -> Self {
        Self::Rs422
    }
    /// Returns instance (variant) Usb
    #[classmethod]
    fn usb(_cls: &Bound<'_, PyType>) -> Self {
        Self::Usb
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}

#[pymethods]
impl IpAddrMode {
    #[classmethod]
    /// Fallibly constructs class from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Self::from_str(s).map_err(PyErr::from)
    }
    /// Returns instance (variant) Dhcp
    #[classmethod]
    fn dhcp(_cls: &Bound<'_, PyType>) -> Self {
        Self::Dhcp
    }
    /// Returns instance (variant) Static
    #[classmethod]
    fn stat(_cls: &Bound<'_, PyType>) -> Self {
        Self::Static
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}
#[pymethods]
impl Module {
    #[classmethod]
    /// Fallibly constructs class from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Self::from_str(s).map_err(PyErr::from)
    }
    /// Returns instance (variant) Cadm
    #[classmethod]
    fn cadm(_cls: &Bound<'_, PyType>) -> Self {
        Self::Cadm
    }
    /// Returns instance (variant) Rsm
    #[classmethod]
    fn rsm(_cls: &Bound<'_, PyType>) -> Self {
        Self::Rsm
    }
    /// Returns instance (variant) Oem
    #[classmethod]
    fn oem(_cls: &Bound<'_, PyType>) -> Self {
        Self::Oem
    }
    /// Returns instance (variant) Psm
    #[classmethod]
    fn psm(_cls: &Bound<'_, PyType>) -> Self {
        Self::Psm
    }
    /// Returns instance (variant) Edm
    #[classmethod]
    fn edm(_cls: &Bound<'_, PyType>) -> Self {
        Self::Edm
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}
#[pymethods]
impl ControllerOpMode {
    /// Returns instance (variant) Basedrive
    #[classmethod]
    fn base(_cls: &Bound<'_, PyType>) -> Self {
        Self::Basedrive
    }
    /// Returns instance (variant) Servodrive
    #[classmethod]
    fn servo(_cls: &Bound<'_, PyType>) -> Self {
        Self::Servodrive
    }
    /// Returns instance (variant) Flexdrive
    #[classmethod]
    fn flex(_cls: &Bound<'_, PyType>) -> Self {
        Self::Flexdrive
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}
#[pymethods]
impl ConnMode {
    /// Returns instance (variant) Serial
    #[classmethod]
    fn serial(_cls: &Bound<'_, PyType>) -> Self {
        Self::Serial
    }
    /// Returns instance (variant) Network
    #[classmethod]
    fn network(_cls: &Bound<'_, PyType>) -> Self {
        Self::Network
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}
#[pymethods]
impl ModuleChannel {
    #[classmethod]
    /// Fallibly constructs this class from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Self::from_str(s).map_err(PyErr::from)
    }
    /// Returns instance (variant) One
    #[classmethod]
    fn one(_cls: &Bound<'_, PyType>) -> Self {
        Self::One
    }
    /// Returns instance (variant) Two
    #[classmethod]
    fn two(_cls: &Bound<'_, PyType>) -> Self {
        Self::Two
    }
    /// Returns instance (variant) Three
    #[classmethod]
    fn three(_cls: &Bound<'_, PyType>) -> Self {
        Self::Three
    }
    /// Maps instance to int
    fn to_int(&self) -> PyResult<u8> {
        Ok(u8::from(self.clone()))
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}
#[pymethods]
impl Direction {
    #[classmethod]
    /// Fallibly constructs this class from a string.
    fn from_string(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        Self::from_str(s).map_err(PyErr::from)
    }
    /// Returns instance (variant) Positive
    #[classmethod]
    fn pos(_cls: &Bound<'_, PyType>) -> Self {
        Self::Positive
    }
    /// Returns instance (variant) Negative
    #[classmethod]
    fn neg(_cls: &Bound<'_, PyType>) -> Self {
        Self::Negative
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}
#[pymethods]
impl SetpointPosMode {
    /// Returns instance (variant) Absolute
    #[classmethod]
    fn abs(_cls: &Bound<'_, PyType>) -> Self {
        Self::Absolute
    }
    /// Returns instance (variant) Relative
    #[classmethod]
    fn rel(_cls: &Bound<'_, PyType>) -> Self {
        Self::Relative
    }
    fn __str__(&self) -> PyResult<String> {
        Ok(format!("{self}"))
    }
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("{:?}", self))
    }
}

// ======= Base Controller Builder Extensions =======
// To enable the type-state builder pattern in Python,
// need to wrap the current generic builder in individual
// types that map to a class for each state.

#[pyclass(name = "BaseControllerBuilder")]
pub struct PyBuilderInit {
    inner: Option<BaseControllerBuilder<Init>>,
}
#[pymethods]
impl PyBuilderInit {
    #[new]
    fn new() -> Self {
        Self {
            inner: Some(BaseControllerBuilder::new()),
        }
    }
    fn with_serial(
        &mut self,
        com_port: &str,
        serial_num: &str,
        baud: u32,
    ) -> PyResult<PyBaseBuilderSerial> {
        // Python does not support moving self without putting something
        // back.
        let inner = self
            .inner
            .take()
            .ok_or(PyRuntimeError::new_err("Inner already consumed"))?;

        Ok(PyBaseBuilderSerial {
            inner: Some(inner.with_serial(com_port, serial_num, baud)),
        })
    }

    fn with_network(&mut self, ip_addr: &str) -> PyResult<PyBaseBuilderNetwork> {
        // Python does not support moving self without putting something
        // back.
        let inner = self
            .inner
            .take()
            .ok_or(PyRuntimeError::new_err("Inner already consumed"))?;

        Ok(PyBaseBuilderNetwork {
            inner: Some(inner.with_network(ip_addr)),
        })
    }
}

#[pyclass(name = "BaseWithSerial")]
pub struct PyBaseBuilderSerial {
    inner: Option<BaseControllerBuilder<Serial>>,
}
#[pymethods]
impl PyBaseBuilderSerial {
    fn build(&mut self) -> PyResult<BaseController> {
        let inner = self
            .inner
            .take()
            .ok_or(PyRuntimeError::new_err("Inner already consumed"))?;
        Ok(inner.build()?)
    }
}

#[pyclass(name = "BaseWithNetwork")]
pub struct PyBaseBuilderNetwork {
    inner: Option<BaseControllerBuilder<Network>>,
}
#[pymethods]
impl PyBaseBuilderNetwork {
    fn build(&mut self) -> PyResult<BaseController> {
        let inner = self
            .inner
            .take()
            .ok_or(PyRuntimeError::new_err("Inner already consumed"))?;
        Ok(inner.build()?)
    }
}
