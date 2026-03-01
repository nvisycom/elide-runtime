//! Conversion utilities from Python errors to [`Error`].

use nvisy_core::Error;
use pyo3::PyErr;
use pyo3::types::PyTracebackMethods;

/// Convert a [`PyErr`] into an [`Error`], preserving the Python traceback when available.
pub fn from_pyerr(err: PyErr) -> Error {
    pyo3::Python::with_gil(|py| {
        let traceback = err
            .traceback(py)
            .map(|tb| tb.format().unwrap_or_default());
        let msg = match traceback {
            Some(tb) => format!("{}\n{}", err, tb),
            None => err.to_string(),
        };
        Error::runtime(msg, "python", false)
    })
}
