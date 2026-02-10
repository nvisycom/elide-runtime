#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod providers;
pub mod streams;

use nvisy_core::plugin::PluginDescriptor;
use crate::providers::s3::S3ProviderFactory;
use crate::streams::read::ObjectReadStream;
use crate::streams::write::ObjectWriteStream;

/// Create the object store plugin descriptor.
pub fn object_plugin() -> PluginDescriptor {
    PluginDescriptor::new("object")
        .with_provider(S3ProviderFactory)
        .with_source(ObjectReadStream)
        .with_target(ObjectWriteStream)
}
