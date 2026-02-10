use nvisy_core::errors::NvisyError;
use pyo3::PyErr;
use pyo3::types::PyTracebackMethods;

/// Convert a Python error to a NvisyError.
pub fn from_pyerr(err: PyErr) -> NvisyError {
    pyo3::Python::with_gil(|py| {
        let traceback = err
            .traceback(py)
            .map(|tb| tb.format().unwrap_or_default());
        let msg = match traceback {
            Some(tb) => format!("{}\n{}", err, tb),
            None => err.to_string(),
        };
        NvisyError::python(msg)
    })
}
