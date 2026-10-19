#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! What a deployment decides once, before any request arrives.
//!
//! A [`Deployment`] is the whole of it: which backends exist and
//! how to reach them, which of them a caller may select, and the
//! codec and enrichment knobs that apply to every document. It is
//! plain serde, so a host either deserializes one — from TOML, or
//! any format of its own — or builds one in code with
//! [`Deployment::builder`]. Both produce the same value.
//!
//! Three ways in, all validated: [`from_toml_file`] for a path,
//! [`from_toml`] for a document already in hand, and
//! [`from_toml_resolved`] when `${VAR}` should come from somewhere
//! other than the environment — a vault, a secret store, a test.
//!
//! [`from_toml`]: Deployment::from_toml
//! [`from_toml_file`]: Deployment::from_toml_file
//! [`from_toml_resolved`]: Deployment::from_toml_resolved
//!
//! This is deliberately *not* request schema. Everything here is
//! owned by whoever operates the deployment: it carries API keys
//! and base URLs, and a request never sends any of it. What a
//! request may say is in `elide-schema` (the policies and
//! vocabulary) and `elide-provider` (the per-request context).
//!
//! ## What a host writes
//!
//! ```
//! use elide_deployment::Deployment;
//!
//! // From a path; `${VAR}` is expanded from the environment.
//! // let deployment = Deployment::from_toml_file("/etc/elide/deployment.toml")?;
//!
//! // Or from a document in hand:
//! let deployment = Deployment::from_toml(r#"
//!     [[recognizers.ner]]
//!     name = "medical-ner"
//!     tags = ["medical"]
//!     kind = "bento"
//!     base_url = "https://ner.internal"
//!     model = "clinical"
//! "#).expect("a valid deployment");
//!
//! assert_eq!(deployment.recognizers().ner.len(), 1);
//! ```
//!
//! The same thing in code, lowering into the identical value:
//!
//! ```
//! use elide_deployment::Deployment;
//! use elide_deployment::component::NerBackend;
//!
//! let deployment = Deployment::builder()
//!     .ner("medical-ner", NerBackend::bento("https://ner.internal", "clinical"))
//!     .tags(["medical"])
//!     .finish()
//!     .build()?;
//! # Ok::<_, elide_deployment::Error>(())
//! ```

mod builder;
mod deployment;
mod error;
mod feature;

pub mod component;

pub use self::builder::{ComponentEntry, DeploymentBuilder};
pub use self::deployment::Deployment;
pub use self::error::{Error, ErrorKind, Result};
pub use self::feature::{Lingua, RateLimit};
