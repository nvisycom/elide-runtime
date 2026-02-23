#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod ocr;
mod synthetic;
mod transcribe;

pub use ocr::{
    GenerateOcrAction, GenerateOcrInput, GenerateOcrOutput, GenerateOcrParams,
    OcrBackend, OcrConfig, parse_ocr_entities,
};
pub use synthetic::{GenerateSyntheticAction, GenerateSyntheticInput, GenerateSyntheticParams};
pub use transcribe::{
    GenerateTranscribeAction, GenerateTranscribeInput, GenerateTranscribeOutput,
    GenerateTranscribeParams, TranscribeBackend, TranscribeConfig, parse_transcribe_entities,
};
