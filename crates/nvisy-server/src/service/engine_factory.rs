use nvisy_core::registry::Registry;
use nvisy_core::errors::NvisyError;

/// Create a registry with all standard plugins loaded.
pub fn create_registry() -> Result<Registry, NvisyError> {
    let mut registry = Registry::new();
    registry.load(nvisy_detect::detect_plugin())?;
    registry.load(nvisy_object::object_plugin())?;
    registry.load(nvisy_python::python_plugin())?;
    tracing::info!(
        actions = ?registry.action_keys(),
        providers = ?registry.provider_keys(),
        "Registry initialized"
    );
    Ok(registry)
}
