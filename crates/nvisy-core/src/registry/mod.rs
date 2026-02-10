use std::collections::HashMap;

use crate::datatypes::blob::Blob;
use crate::errors::NvisyError;
use crate::plugin::PluginDescriptor;
use crate::traits::action::Action;
use crate::traits::loader::Loader;
use crate::traits::provider::ProviderFactory;
use crate::traits::stream::{StreamSource, StreamTarget};

/// Registry of all actions, providers, streams, and loaders.
///
/// Items are keyed by "plugin_id/item_id" (e.g. "detect/detect-regex").
pub struct Registry {
    actions: HashMap<String, Box<dyn Action>>,
    providers: HashMap<String, Box<dyn ProviderFactory>>,
    sources: HashMap<String, Box<dyn StreamSource>>,
    targets: HashMap<String, Box<dyn StreamTarget>>,
    loaders: Vec<Box<dyn Loader>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            providers: HashMap::new(),
            sources: HashMap::new(),
            targets: HashMap::new(),
            loaders: Vec::new(),
        }
    }

    /// Load a plugin, registering all its items under "plugin_id/item_id" keys.
    pub fn load(&mut self, plugin: PluginDescriptor) -> Result<(), NvisyError> {
        let prefix = &plugin.id;

        for action in plugin.actions {
            let key = format!("{}/{}", prefix, action.id());
            if self.actions.contains_key(&key) {
                return Err(NvisyError::validation(
                    format!("Duplicate action: {}", key),
                    "registry",
                ));
            }
            self.actions.insert(key, action);
        }

        for provider in plugin.providers {
            let key = format!("{}/{}", prefix, provider.id());
            if self.providers.contains_key(&key) {
                return Err(NvisyError::validation(
                    format!("Duplicate provider: {}", key),
                    "registry",
                ));
            }
            self.providers.insert(key, provider);
        }

        for source in plugin.sources {
            let key = format!("{}/{}", prefix, source.id());
            if self.sources.contains_key(&key) {
                return Err(NvisyError::validation(
                    format!("Duplicate source: {}", key),
                    "registry",
                ));
            }
            self.sources.insert(key, source);
        }

        for target in plugin.targets {
            let key = format!("{}/{}", prefix, target.id());
            if self.targets.contains_key(&key) {
                return Err(NvisyError::validation(
                    format!("Duplicate target: {}", key),
                    "registry",
                ));
            }
            self.targets.insert(key, target);
        }

        for loader in plugin.loaders {
            self.loaders.push(loader);
        }

        Ok(())
    }

    pub fn get_action(&self, key: &str) -> Option<&dyn Action> {
        self.actions.get(key).map(|a| a.as_ref())
    }

    pub fn get_provider(&self, key: &str) -> Option<&dyn ProviderFactory> {
        self.providers.get(key).map(|p| p.as_ref())
    }

    pub fn get_source(&self, key: &str) -> Option<&dyn StreamSource> {
        self.sources.get(key).map(|s| s.as_ref())
    }

    pub fn get_target(&self, key: &str) -> Option<&dyn StreamTarget> {
        self.targets.get(key).map(|t| t.as_ref())
    }

    /// Find a loader that matches a blob's extension or content type.
    pub fn find_loader_for_blob(&self, blob: &Blob) -> Option<&dyn Loader> {
        let ext = blob.extension();
        let ct = blob.content_type();

        for loader in &self.loaders {
            if let Some(ext) = ext {
                if loader.extensions().contains(&ext) {
                    return Some(loader.as_ref());
                }
            }
            if let Some(ct) = ct {
                if loader.content_types().contains(&ct) {
                    return Some(loader.as_ref());
                }
            }
        }
        None
    }

    pub fn action_keys(&self) -> Vec<&str> {
        self.actions.keys().map(|s| s.as_str()).collect()
    }

    pub fn provider_keys(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    pub fn source_keys(&self) -> Vec<&str> {
        self.sources.keys().map(|s| s.as_str()).collect()
    }

    pub fn target_keys(&self) -> Vec<&str> {
        self.targets.keys().map(|s| s.as_str()).collect()
    }

    pub fn loader_ids(&self) -> Vec<&str> {
        self.loaders.iter().map(|l| l.id()).collect()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
