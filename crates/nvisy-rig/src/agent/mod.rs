//! Agent system: base agent, specialized agents, and tool-provider traits.
//!
//! All public types are re-exported here — consumer code should not reach
//! into individual agent submodules.

mod base;
mod detect;
mod extract;
mod recognize;
mod redact;

pub(crate) use base::{BaseAgent, BaseAgentBuilder, BaseAgentConfig};

pub use recognize::{NerAgent, RawEntities, RawEntity};
pub use extract::{OcrAgent, OcrOutput, OcrProvider, OcrTextRegion, RawOcrEntity};
pub use detect::{CvAgent, CvDetection, CvProvider, RawCvEntities, RawCvEntity};
pub use redact::{RawRedaction, RedactorAgent, RedactorOutput};
