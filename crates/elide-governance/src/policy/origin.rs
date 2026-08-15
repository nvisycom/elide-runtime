//! [`TemplateOrigin`]: which shipped template a policy was built from.

use hipstr::HipStr;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};

/// The template a [`PolicyDefinition`] was built from.
///
/// Records **provenance, not fidelity**. Templates are plain data
/// and callers are expected to mutate the returned policy before
/// submitting it (swapping an operator, widening a group), so this
/// says "built from `hipaa_deid_safe_harbor` v1.0.0" and nothing
/// about whether the policy still matches what that template
/// ships. A reviewer who needs that rebuilds the template and
/// diffs.
///
/// `None` on a policy means hand-authored, not "unknown template".
///
/// Both fields are needed: a policy built from v1 of a template
/// can differ materially from one built from v2 (a widened label
/// set, a changed operator), and an audit that records only the id
/// cannot tell the two apart.
///
/// [`PolicyDefinition`]: super::PolicyDefinition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateOrigin {
    /// The template's machine key: `"hipaa_deid_safe_harbor"`,
    /// `"pci_dss_pan_hmac_sha256"`. Stable across version bumps.
    #[schemars(with = "String")]
    pub id: HipStr<'static>,
    /// The template's own semver version, distinct from the
    /// crate's release version. A shipped template bumps this when
    /// its labelset or operator dispatch changes.
    #[schemars(with = "String")]
    pub version: Version,
}

impl TemplateOrigin {
    /// An origin naming `id` at `version`.
    pub fn new(id: impl Into<HipStr<'static>>, version: Version) -> Self {
        Self {
            id: id.into(),
            version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let origin = TemplateOrigin::new("hipaa_deid_safe_harbor", Version::new(1, 0, 0));
        let json = serde_json::to_string(&origin).expect("serialize");
        let back: TemplateOrigin = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(origin, back);
    }

    #[test]
    fn versions_distinguish_origins_sharing_an_id() {
        // The reason `version` is carried at all: two policies
        // built from different revisions of one template must not
        // compare equal.
        let v1 = TemplateOrigin::new("hipaa_deid_safe_harbor", Version::new(1, 0, 0));
        let v2 = TemplateOrigin::new("hipaa_deid_safe_harbor", Version::new(2, 0, 0));
        assert_ne!(v1, v2);
    }
}
