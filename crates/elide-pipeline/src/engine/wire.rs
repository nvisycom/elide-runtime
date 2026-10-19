//! Reading an [`Audit`] and its artifacts back from serialized
//! form.
//!
//! These are methods on [`Engine`] for one reason: a serialized
//! report tags each entity group with its modality *name*, not its
//! concrete type, and deserialization cannot be object-safe. So
//! rebuilding one takes a name-to-type registry, and the engine is
//! that registry — it knows which modalities it handles. A free
//! `from_str` would need a global one, which would close the door
//! on modalities elide does not ship.
//!
//! Serializing needs none of this, which is why only this half
//! lives here: [`Audit`] derives [`Serialize`](serde::Serialize)
//! and nothing else.

use elide::backend::metrics::Usage;
use elide::{ArtifactSet, Error, ErrorKind, Result};
use elide_provider::{CodecParams, ComponentSelection, DocumentContext};
use elide_schema::recognition::Recognition;
use serde::Deserialize;

use super::Engine;
use crate::audit::Audit;

impl Engine {
    /// Read an [`Audit`] back from its serialized form.
    ///
    /// The counterpart to serializing one: a host persists an audit
    /// between analyze and anonymize, or ships it to a reviewer and
    /// takes it back, and this is how it returns.
    ///
    /// [`Audit`] is [`Serialize`](serde::Serialize) but not
    /// `Deserialize`, for the reason this module's docs give.
    ///
    /// # Errors
    ///
    /// Returns [`MalformedInput`](ErrorKind::MalformedInput) if the
    /// payload is not a well-formed audit, or if its report names a
    /// modality this engine has no pipeline for — rebuilding it
    /// would silently drop those entities along with any reviewer
    /// decisions on them.
    pub fn deserialize_audit<'de, D>(&self, deserializer: D) -> Result<Audit>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AuditWire::deserialize(deserializer)
            .map_err(|err| Error::new(ErrorKind::MalformedInput, err.to_string()))?;

        // The report is the one part this engine must rebuild
        // itself; everything else on an audit is plain data.
        let report = self.provider().deserialize_report(wire.report)?;

        Ok(Audit {
            report,
            context: wire.context,
            recognition: wire.recognition,
            codec: wire.codec,
            selection: wire.selection,
            usage: wire.usage,
        })
    }

    /// Rebuild an [`ArtifactSet`] persisted beside an [`Audit`],
    /// for handing back to [`re_analyze`](Self::re_analyze).
    ///
    /// The artifact-side counterpart to
    /// [`deserialize_audit`](Self::deserialize_audit), and needs
    /// this engine for the same reason: a serialized artifact set
    /// tags each group with its modality *name*, so rebuilding one
    /// takes a name-to-type registry, and the engine is it.
    ///
    /// # Errors
    ///
    /// Returns [`MalformedInput`](ErrorKind::MalformedInput) if the
    /// payload is not a well-formed artifact set.
    pub fn deserialize_artifacts<'de, D>(&self, deserializer: D) -> Result<ArtifactSet>
    where
        D: serde::Deserializer<'de>,
    {
        self.provider().deserialize_artifacts(deserializer)
    }
}

/// The wire shape of an [`Audit`], with the report left as raw
/// values.
///
/// Mirrors `Audit`'s field names so one serialized form reads back
/// through both. The report cannot be deserialized here — it needs
/// the engine's modality registry — so it is buffered and handed to
/// [`Orchestrator::deserialize_report`] afterwards.
///
/// [`Orchestrator::deserialize_report`]: elide::Orchestrator::deserialize_report
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuditWire {
    report: serde_value::Value,
    context: DocumentContext,
    // No `default`: unlike `selection`/`recognition`/`usage` below,
    // this is always serialized, so a payload omitting it is
    // malformed rather
    // than empty. Defaulting it to `RasterMode::Auto` would let a
    // document re-decode differently than analyze decoded it, and
    // the entity offsets recorded against the first decode would
    // land on different content.
    codec: CodecParams,
    // Defaulted: most requests run every recognizer they have, so
    // an audit omitting this is the common case rather than a
    // malformed one.
    #[serde(default)]
    selection: ComponentSelection,
    // Defaulted: most requests introduce no vocabulary of their
    // own, so an audit omitting it is the common case rather than
    // a malformed one.
    #[serde(default)]
    recognition: Vec<Recognition>,
    // Defaulted: a pass that called no model records nothing,
    // which is an empty tally rather than a malformed audit.
    #[serde(default)]
    usage: Vec<Usage>,
}
