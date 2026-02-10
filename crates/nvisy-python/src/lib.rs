#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod actions;
pub mod bridge;
pub mod error;
pub mod ner;
pub mod provider;

use nvisy_core::plugin::PluginDescriptor;
use crate::actions::{DetectNerAction, DetectNerImageAction};
use crate::provider::AiProviderFactory;

/// Create the Python AI plugin descriptor.
pub fn python_plugin() -> PluginDescriptor {
    PluginDescriptor::new("ai")
        .with_action(DetectNerAction)
        .with_action(DetectNerImageAction)
        .with_provider(AiProviderFactory)
}
