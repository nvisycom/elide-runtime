//! Server lifecycle: configuration, router construction, and TCP listener.
//!
//! - [`config`] — CLI/env configuration via [`clap`].
//! - [`router`] — middleware composition and route wiring.
//! - [`listen`] — TCP bind, graceful shutdown, and post-shutdown cleanup.

pub mod config;
pub mod listen;
pub mod router;
