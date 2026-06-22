#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod anonymizer;
mod generator;
mod locale;

pub use self::anonymizer::Fake;
