use pyo3::prelude::*;
pub mod base;
pub mod config;
pub mod python_ffi;

// Define the Python module that exposes Pyo3 API to python users.
#[pymodule]
#[pyo3(name = "jpe_python_ffi")]
fn py_module(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    config::register_pyo3(py, m)?;
    base::register_pyo3(py, m)?;
    python_ffi::register_pyo3(py, m)?;
    Ok(())
}
