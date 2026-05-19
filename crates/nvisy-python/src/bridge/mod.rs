//! Lightweight handle to a Python module loaded via PyO3.
//!
//! Provides [`PythonBridge`]: a thin wrapper that remembers which Python
//! module to import, plus helpers for calling synchronous and asynchronous
//! Python functions from Rust async code.

mod error;

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use hipstr::HipStr;
use nvisy_core::Error;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

pub use self::error::from_pyerr;

const TARGET: &str = "nvisy_python::bridge";

/// Lightweight handle to a Python module.
///
/// The bridge does **not** hold the GIL or any Python objects: it simply
/// remembers which module to `import` when a function is called.
/// The default module name is `"nvisy_ai"`.
#[derive(Clone)]
pub struct PythonBridge {
    /// Dotted Python module name to import (e.g. `"nvisy_ai"`).
    module_name: HipStr<'static>,
}

impl fmt::Debug for PythonBridge {
    /// Formats the bridge for debugging, showing only the module name.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PythonBridge")
            .field("module_name", &self.module_name.as_str())
            .finish()
    }
}

impl PythonBridge {
    /// Creates a new bridge that will load the given Python module.
    pub fn new(module_name: impl Into<HipStr<'static>>) -> Self {
        Self {
            module_name: module_name.into(),
        }
    }

    /// Initializes Python and verifies the module can be imported.
    ///
    /// # Errors
    ///
    /// Returns an error if the Python interpreter cannot be started or
    /// the module cannot be imported.
    #[tracing::instrument(target = TARGET, name = "bridge.init", skip(self), fields(module = %self.module_name))]
    pub fn init(&self) -> Result<(), Error> {
        Python::attach(|py| {
            py.import(&*self.module_name).map_err(from_pyerr)?;
            tracing::debug!(target: TARGET, "python module imported");
            Ok(())
        })
    }

    /// Returns the dotted Python module name.
    #[must_use]
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// Calls a **synchronous** Python method on the bridge module inside
    /// `spawn_blocking` + `Python::attach`.
    ///
    /// `build_kwargs` receives a GIL token and must return a [`PyDict`]
    /// of keyword arguments. The method is invoked as
    /// `module.<method>(**kwargs)` and the return value is deserialized
    /// into `Vec<Value>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the Python call fails or the return value
    /// cannot be deserialized.
    #[tracing::instrument(
        target = TARGET,
        name = "bridge.call_sync",
        skip(self, build_kwargs),
        fields(module = %self.module_name, method),
    )]
    pub async fn call_sync<F>(&self, method: &str, build_kwargs: F) -> Result<Vec<Value>, Error>
    where
        F: FnOnce(Python<'_>) -> Result<Bound<'_, PyDict>, Error> + Send + 'static,
    {
        let module_name = self.module_name.clone();
        let method = method.to_string();

        tracing::Span::current().record("method", &method);

        tokio::task::spawn_blocking(move || {
            Python::attach(|py| {
                let module = py.import(&*module_name).map_err(from_pyerr)?;
                let kwargs = build_kwargs(py)?;

                let result = module
                    .call_method(&method, (), Some(&kwargs))
                    .map_err(from_pyerr)?;

                pythonize::depythonize::<Vec<Value>>(&result).map_err(|e| {
                    Error::runtime(
                        format!("failed to deserialize {method} result: {e}"),
                        "python",
                        false,
                    )
                })
            })
        })
        .await
        .map_err(|e| Error::runtime(format!("blocking task panicked: {e}"), "python", false))?
    }

    /// Calls an **asynchronous** (coroutine) Python method on the bridge
    /// module.
    ///
    /// Acquires the GIL, invokes `module.<method>(**kwargs)` to obtain a
    /// Python coroutine, converts it to a Rust [`Future`] via
    /// [`pyo3_async_runtimes::tokio::into_future`], and awaits it on the
    /// Tokio runtime. The coroutine's return value is deserialized into
    /// `Vec<Value>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the Python call fails or the return value
    /// cannot be deserialized.
    #[tracing::instrument(
        target = TARGET,
        name = "bridge.call_async",
        skip(self, build_kwargs),
        fields(module = %self.module_name, method),
    )]
    pub async fn call_async<F>(&self, method: &str, build_kwargs: F) -> Result<Vec<Value>, Error>
    where
        F: FnOnce(Python<'_>) -> Result<Bound<'_, PyDict>, Error> + Send + 'static,
    {
        tracing::Span::current().record("method", method);

        let future: Pin<Box<dyn Future<Output = PyResult<Py<PyAny>>> + Send>> =
            Python::attach(|py| -> Result<_, Error> {
                let module = py.import(&*self.module_name).map_err(from_pyerr)?;
                let kwargs = build_kwargs(py)?;

                let coroutine = module
                    .call_method(method, (), Some(&kwargs))
                    .map_err(from_pyerr)?;

                let fut = pyo3_async_runtimes::tokio::into_future(coroutine).map_err(from_pyerr)?;

                Ok(Box::pin(fut))
            })?;

        let py_result = future.await.map_err(from_pyerr)?;

        Python::attach(|py| {
            pythonize::depythonize::<Vec<Value>>(py_result.bind(py)).map_err(|e| {
                Error::runtime(
                    format!("failed to deserialize {method} result: {e}"),
                    "python",
                    false,
                )
            })
        })
    }
}

impl Default for PythonBridge {
    /// Creates a bridge with the default module name `"nvisy_ai"`.
    fn default() -> Self {
        Self::new("nvisy_ai")
    }
}
