//! Convenience re-exports for common nvisy-pattern types.
//!
//! ```rust,ignore
//! use nvisy_pattern::prelude::*;
//! ```

pub use crate::{
    AllowList, ContextRule, DenyEntry, DenyList, DetectionSource, PatternEngine,
    PatternEngineBuilder, PatternEngineError, PatternMatch, default_engine,
};
