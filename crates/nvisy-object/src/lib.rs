//! Object storage providers and streams for the nvisy pipeline.
//!
//! This crate provides an abstraction layer over cloud object stores (currently S3)
//! and exposes streaming read/write interfaces that plug into the nvisy engine.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod providers;
pub mod streams;

#[doc(hidden)]
pub mod prelude;
