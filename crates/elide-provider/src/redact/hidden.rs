//! [`hidden_content_policy`]: what hidden content becomes at decode.

use elide::redaction::{Disposition, HiddenContentPolicy};

/// The hidden-content policy every document decodes under.
///
/// Not a request setting: the runtime decides what hidden content
/// becomes, and a reviewer decides what of it goes. Metadata fields
/// and collaboration items (comments, their authors, tracked
/// changes) are all kept and made entities at decode, so analyze can
/// show every one of them, marked sensitive or not, and anonymize
/// removes what the review chose. Hidden content a format cannot
/// surface item by item (scripts, macros, thumbnails, metadata no
/// field reads) is still stripped at decode, and the report lists it
/// under `stripped`.
#[must_use]
pub fn hidden_content_policy() -> HiddenContentPolicy {
    HiddenContentPolicy::default()
        .with_metadata(Disposition::FlagEverything)
        .with_collaboration(Disposition::FlagEverything)
}
