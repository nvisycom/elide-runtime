//! Lightweight handle to a Python module loaded via PyO3.
//!
//! Provides [`PythonBridge`] — a thin wrapper that remembers which Python
//! module to import — plus helpers for calling synchronous and asynchronous
//! Python functions from Rust async code.

mod error;

pub use error::from_pyerr;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use hipstr::HipStr;

use nvisy_core::Error;

/// Lightweight handle to a Python NER module.
///
/// The bridge does **not** hold the GIL or any Python objects; it simply
/// remembers which module to `import` when a detection function is called.
/// The default module name is `"nvisy_ai"`.
#[derive(Clone)]
pub struct PythonBridge {
    /// Dotted Python module name to import (e.g., `"nvisy_ai"`).
    module_name: HipStr<'static>,
}

impl PythonBridge {
    /// Create a new bridge that will load the given Python module.
    pub fn new(module_name: impl Into<HipStr<'static>>) -> Self {
        Self {
            module_name: module_name.into(),
        }
    }

    /// Initialize Python and verify the module can be imported.
    pub fn init(&self) -> Result<(), Error> {
        Python::with_gil(|py| {
            py.import(&*self.module_name)
                .map_err(from_pyerr)?;
            Ok(())
        })
    }

    /// Get the module name.
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// Call a **synchronous** Python method on the bridge module inside
    /// `spawn_blocking` + `Python::with_gil`.
    ///
    /// `build_kwargs` receives a GIL token and must return a [`PyDict`] of
    /// keyword arguments.  The method is invoked as
    /// `module.<method>(**, kwargs)` and the return value is deserialized
    /// into `Vec<Value>`.
    pub async fn call_sync<F>(
        &self,
        method: &str,
        build_kwargs: F,
    ) -> Result<Vec<Value>, Error>
    where
        F: FnOnce(Python<'_>) -> Result<Bound<'_, PyDict>, Error> + Send + 'static,
    {
        let module_name = self.module_name.clone();
        let method = method.to_string();

        tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| {
                let module = py.import(&*module_name).map_err(from_pyerr)?;
                let kwargs = build_kwargs(py)?;

                let result = module
                    .call_method(&method, (), Some(&kwargs))
                    .map_err(from_pyerr)?;

                pythonize::depythonize::<Vec<Value>>(&result).map_err(|e| {
                    Error::python(format!(
                        "Failed to deserialize {} result: {}",
                        method, e
                    ))
                })
            })
        })
        .await
        .map_err(|e| Error::python(format!("Task join error: {}", e)))?
    }

    /// Call an **asynchronous** (coroutine) Python method on the bridge
    /// module.
    ///
    /// Acquires the GIL, invokes `module.<method>(**kwargs)` to obtain a
    /// Python coroutine, converts it to a Rust [`Future`] via
    /// [`pyo3_async_runtimes::tokio::into_future`], and awaits it on the
    /// Tokio runtime.  The coroutine's return value is deserialized into
    /// `Vec<Value>`.
    pub async fn call_async<F>(
        &self,
        method: &str,
        build_kwargs: F,
    ) -> Result<Vec<Value>, Error>
    where
        F: FnOnce(Python<'_>) -> Result<Bound<'_, PyDict>, Error> + Send + 'static,
    {
        use std::pin::Pin;
        use std::future::Future;

        let future: Pin<Box<dyn Future<Output = PyResult<PyObject>> + Send>> =
            Python::with_gil(|py| -> Result<_, Error> {
                let module = py.import(&*self.module_name).map_err(from_pyerr)?;
                let kwargs = build_kwargs(py)?;

                let coroutine = module
                    .call_method(method, (), Some(&kwargs))
                    .map_err(from_pyerr)?;

                let fut = pyo3_async_runtimes::tokio::into_future(coroutine)
                    .map_err(from_pyerr)?;

                Ok(Box::pin(fut))
            })?;

        let py_result = future
            .await
            .map_err(from_pyerr)?;

        Python::with_gil(|py| {
            pythonize::depythonize::<Vec<Value>>(py_result.bind(py)).map_err(|e| {
                Error::python(format!(
                    "Failed to deserialize {} result: {}",
                    method, e
                ))
            })
        })
    }
}

impl Default for PythonBridge {
    fn default() -> Self {
        Self::new("nvisy_ai")
    }
}
