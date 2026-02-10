//! Python/PyO3 bridge for AI-powered NER detection.
//!
//! This crate embeds a CPython interpreter via PyO3 and delegates named-entity
//! recognition (NER) to a Python module (`nvisy_ai`).  It exposes pipeline
//! [`Action`](nvisy_core::traits::action::Action) implementations as well as a
//! [`ProviderFactory`](nvisy_core::traits::provider::ProviderFactory) for the
//! `"ai"` provider.

#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod actions;
pub mod bridge;
pub mod error;
pub mod ner;
pub mod provider;

#[doc(hidden)]
pub mod prelude;
