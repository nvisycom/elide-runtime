use crate::traits::action::Action;
use crate::traits::loader::Loader;
use crate::traits::provider::ProviderFactory;
use crate::traits::stream::{StreamSource, StreamTarget};

/// Describes a plugin that bundles actions, providers, streams, and loaders.
pub struct PluginDescriptor {
    pub id: String,
    pub actions: Vec<Box<dyn Action>>,
    pub providers: Vec<Box<dyn ProviderFactory>>,
    pub sources: Vec<Box<dyn StreamSource>>,
    pub targets: Vec<Box<dyn StreamTarget>>,
    pub loaders: Vec<Box<dyn Loader>>,
}

impl PluginDescriptor {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            actions: Vec::new(),
            providers: Vec::new(),
            sources: Vec::new(),
            targets: Vec::new(),
            loaders: Vec::new(),
        }
    }

    pub fn with_action(mut self, action: impl Action) -> Self {
        self.actions.push(Box::new(action));
        self
    }

    pub fn with_provider(mut self, provider: impl ProviderFactory) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    pub fn with_source(mut self, source: impl StreamSource) -> Self {
        self.sources.push(Box::new(source));
        self
    }

    pub fn with_target(mut self, target: impl StreamTarget) -> Self {
        self.targets.push(Box::new(target));
        self
    }

    pub fn with_loader(mut self, loader: impl Loader) -> Self {
        self.loaders.push(Box::new(loader));
        self
    }
}
