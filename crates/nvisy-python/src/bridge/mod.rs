use pyo3::prelude::*;
use nvisy_core::errors::NvisyError;
use crate::error::from_pyerr;

/// Holds a reference to the loaded Python NER module.
#[derive(Clone)]
pub struct PythonBridge {
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
    pub fn init(&self) -> Result<(), NvisyError> {
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
