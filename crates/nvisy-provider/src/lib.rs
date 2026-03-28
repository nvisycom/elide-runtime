#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! External service providers: HTTP client, OCR, LLM agents, STT, TTS.

pub mod agent;
pub mod audio;
pub mod error;
pub mod http;
pub mod llm;
pub mod ocr;
