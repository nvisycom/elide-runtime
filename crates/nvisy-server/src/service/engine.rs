//! Placeholder engine for development and testing.
//!
//! [`StubEngine`] implements [`Engine`] but rejects all requests. It is wired
//! into [`ServiceState`](super::ServiceState) at startup until a real
//! implementation is configured.

use nvisy_core::{Error, ErrorKind};
use nvisy_engine::engine::{Engine, EngineInput, EngineOutput};

/// Placeholder engine that rejects all requests.
pub struct StubEngine;

impl Engine for StubEngine {
    async fn run(&self, _input: EngineInput) -> Result<EngineOutput, Error> {
        Err(Error::new(ErrorKind::Runtime, "no engine configured"))
    }
}
