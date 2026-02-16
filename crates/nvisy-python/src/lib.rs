//! Python/PyO3 bridge for AI-powered NER and OCR detection.
//!
//! This crate embeds a CPython interpreter via PyO3 and delegates named-entity
//! recognition (NER) and OCR to a Python module (`nvisy_ai`).  It implements
//! the [`NerBackend`](nvisy_pipeline::detection::ner::NerBackend) and
//! [`OcrBackend`](nvisy_pipeline::generation::ocr::OcrBackend) traits for
//! [`PythonBridge`](bridge::PythonBridge), returning raw JSON to the pipeline.

#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod bridge;
pub mod error;
pub mod ner;
pub mod ocr;
pub mod provider;

#[doc(hidden)]
pub mod prelude;
