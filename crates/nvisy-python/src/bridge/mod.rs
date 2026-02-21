//! Lightweight handle to a Python module loaded via PyO3.

use pyo3::prelude::*;
use nvisy_core::Error;
use crate::error::from_pyerr;

/// Lightweight handle to a Python NER module.
///
/// The bridge does **not** hold the GIL or any Python objects; it simply
/// remembers which module to `import` when a detection function is called.
/// The default module name is `"nvisy_ai"`.
#[derive(Clone)]
pub struct PythonBridge {
    /// Dotted Python module name to import (e.g., `"nvisy_ai"`).
    module_name: String,
}

impl PythonBridge {
    /// Create a new bridge that will load the given Python module.
    pub fn new(module_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
        }
    }

    /// Initialize Python and verify the module can be imported.
    pub fn init(&self) -> Result<(), Error> {
        Python::with_gil(|py| {
            py.import(&self.module_name)
                .map_err(from_pyerr)?;
            Ok(())
        })
    }

    /// Get the module name.
    pub fn module_name(&self) -> &str {
        &self.module_name
    }
}

impl Default for PythonBridge {
    fn default() -> Self {
        Self::new("nvisy_ai")
    }
}

