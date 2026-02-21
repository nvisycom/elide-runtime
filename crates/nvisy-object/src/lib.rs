//! Cloud object storage providers and streams for Nvisy.
//!
//! This crate wraps [`object_store`] to provide a unified
//! [`ObjectStoreClient`] with observability and streaming adapters
//! for the Nvisy pipeline.
//!
//! # Providers
//!
//! Each cloud backend has a dedicated provider that implements
//! [`Provider`](nvisy_pipeline::provider::Provider):
//!
//! - [`S3Provider`] — AWS S3, MinIO, and S3-compatible services
//! - [`AzureProvider`] — Azure Blob Storage
//! - [`GcsProvider`] — Google Cloud Storage
//!
//! # Client
//!
//! [`ObjectStoreClient`] is a thin, cloneable wrapper around
//! `Arc<dyn ObjectStore>` that adds:
//!
//! - Richer return types ([`GetResult`], [`PutResult`]) preserving metadata
//! - Conditional writes via [`put_opts`](client::ObjectStoreClient::put_opts)
//! - `head`, `copy`, and lazy `list_stream` operations
//! - `#[tracing::instrument]` on every public method
//!
//! # Streams
//!
//! [`ObjectReadStream`] and [`ObjectWriteStream`] implement
//! [`StreamSource`](nvisy_pipeline::stream::StreamSource) and
//! [`StreamTarget`](nvisy_pipeline::stream::StreamTarget) for pipeline integration.
//!
//! [`ObjectStoreClient`]: client::ObjectStoreClient
//! [`GetResult`]: client::GetResult
//! [`PutResult`]: client::PutResult
//! [`S3Provider`]: providers::S3Provider
//! [`AzureProvider`]: providers::AzureProvider
//! [`GcsProvider`]: providers::GcsProvider
//! [`ObjectReadStream`]: streams::ObjectReadStream
//! [`ObjectWriteStream`]: streams::ObjectWriteStream

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod error;

pub mod client;
pub mod providers;
pub mod streams;

#[doc(hidden)]
pub mod prelude;
