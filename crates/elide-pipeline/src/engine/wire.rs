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

use elide::entity::audit::AuditHash;
use elide::{ArtifactSet, Error, ErrorKind, Report, Result};
use elide_provider::{CodecParams, ComponentSelection, DocumentContext};
use elide_schema::recognition::Recognition;
use serde::Deserialize;

use super::Engine;
use crate::audit::Audit;

impl Engine {
    /// Read an [`Audit`] back from its serialized form, and check it
    /// against `expected`, the [digest](Audit::digest) it had when it
    /// was written.
    ///
    /// The counterpart to serializing one: a host persists an audit
    /// between analyze and anonymize, or ships it to a reviewer and
    /// takes it back, and this is how it returns. The digest is what
    /// says it returned as it left: kept where the carrier cannot
    /// change it, it catches an entity dropped or rewritten, a trail
    /// cut back, or the request's settings swapped.
    ///
    /// [`Audit`] is [`Serialize`](serde::Serialize) but not
    /// `Deserialize`, for the reason this module's docs give.
    ///
    /// # Errors
    ///
    /// Returns [`MalformedInput`](ErrorKind::MalformedInput) if the
    /// payload is not a well-formed audit, if its report names a
    /// modality this engine has no pipeline for — rebuilding it
    /// would silently drop those entities along with any reviewer
    /// decisions on them — or if an entity's trail does not verify;
    /// [`Integrity`](ErrorKind::Integrity) if its digest is not
    /// `expected`.
    pub fn deserialize_audit<'de, D>(&self, expected: &AuditHash, deserializer: D) -> Result<Audit>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = AuditWire::deserialize(deserializer)
            .map_err(|err| Error::new(ErrorKind::MalformedInput, err.to_string()))?;

        // The report is the one part this engine must rebuild
        // itself; everything else on an audit is plain data. Its own
        // digest is not checked here: the audit's covers it, with
        // the rest of the audit beside it.
        let report: Report = self.provider().restorer().restore_unchecked(wire.report)?;

        let audit = Audit {
            report,
            context: wire.context,
            recognition: wire.recognition,
            codec: wire.codec,
            selection: wire.selection,
        };
        if audit.digest() != *expected {
            return Err(Error::new(
                ErrorKind::Integrity,
                "the audit does not match its digest: it was changed after it was written",
            ));
        }
        Ok(audit)
    }

    /// Rebuild an [`ArtifactSet`] persisted beside an [`Audit`],
    /// for handing back to [`re_analyze`](Self::re_analyze), and
    /// check it against `expected`, the digest it had when it was
    /// written.
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
    /// payload is not a well-formed artifact set;
    /// [`Integrity`](ErrorKind::Integrity) if its digest is not
    /// `expected`.
    pub fn deserialize_artifacts<'de, D>(
        &self,
        expected: &AuditHash,
        deserializer: D,
    ) -> Result<ArtifactSet>
    where
        D: serde::Deserializer<'de>,
    {
        self.provider().restorer().restore(expected, deserializer)
    }
}

/// The wire shape of an [`Audit`], with the report left as raw
/// values.
///
/// Mirrors `Audit`'s field names so one serialized form reads back
/// through both. The report cannot be deserialized here — it needs
/// the engine's modality registry — so it is buffered and handed to
/// a [`Restorer`] afterwards.
///
/// [`Restorer`]: elide::Restorer
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuditWire {
    report: serde_value::Value,
    context: DocumentContext,
    // No `default`: unlike `selection`/`recognition` below,
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
}
