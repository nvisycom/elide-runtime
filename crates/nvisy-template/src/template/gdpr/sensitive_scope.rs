//! [`GdprSensitiveScope`]: the widening label axis for the GDPR
//! Article 9 template.

use elide_core::entity::LabelRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which sensitive-data labels the template's group covers.
///
/// Three tiers, each strictly widening the previous one so a
/// caller upgrading through the tiers never loses coverage:
///
/// - [`Article9`](Self::Article9) — the nine Article 9(1)
///   special categories only. The default.
/// - [`Article9WithReidHardening`](Self::Article9WithReidHardening) —
///   Article 9 plus `date_of_birth` and `postal_code`. The two
///   quasi-identifiers Recital 26 highlights as re-identification
///   vectors when combined with special-category data. Non-
///   binding guidance, but reflects supervisory-authority
///   expectations on pseudonymization robustness.
/// - [`Article9And10`](Self::Article9And10) — Article 9 + Recital
///   26 hardening + Article 10's criminal-justice labels
///   (`criminal_record`, `criminal_charge`, `judicial_narrative`).
///   Article 10 governs "personal data relating to criminal
///   convictions and offences", processed only under authorized
///   official-authority control or specific Union/Member-State
///   law. Callers with criminal-justice adjacency (background-
///   check services, court-records handling, HR compliance) pick
///   this tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GdprSensitiveScope {
    /// The nine Article 9(1) special categories only. The default
    /// posture for callers whose workflow only touches Article 9
    /// data.
    #[default]
    Article9,
    /// Article 9 plus `date_of_birth` and `postal_code`. Adds
    /// Recital 26's two flagged quasi-identifiers so pseudonymized
    /// output is harder to re-identify via joins against public
    /// datasets.
    Article9WithReidHardening,
    /// Article 9 + Recital 26 hardening + Article 10
    /// criminal-justice labels (`criminal_record`,
    /// `criminal_charge`, `judicial_narrative`).
    Article9And10,
}

impl GdprSensitiveScope {
    /// The label set this scope covers: Article 9(1) always,
    /// plus Recital 26 quasi-identifiers or Article 10
    /// criminal-justice labels as the tier widens.
    pub(crate) fn labels(self) -> Vec<LabelRef> {
        let mut labels: Vec<LabelRef> = GDPR_LABELS.to_vec();
        match self {
            Self::Article9 => {}
            Self::Article9WithReidHardening => {
                labels.extend(RECITAL_26_LABELS.iter().cloned());
            }
            Self::Article9And10 => {
                labels.extend(RECITAL_26_LABELS.iter().cloned());
                labels.extend(ARTICLE_10_LABELS.iter().cloned());
            }
        }
        labels
    }
}

/// Elide-builtin labels the group covers, mapped from Article
/// 9(1) categories. `pub(super)` so the erase / pseudonymize
/// posture modules can also assert on membership in tests.
pub(super) const GDPR_LABELS: &[LabelRef] = &[
    // Racial or ethnic origin
    LabelRef::from_static("ethnicity"),
    LabelRef::from_static("nationality"),
    // Political opinions
    LabelRef::from_static("political_opinion"),
    // Religious or philosophical beliefs
    LabelRef::from_static("religion"),
    // Trade-union membership
    LabelRef::from_static("trade_union_membership"),
    // Genetic data
    LabelRef::from_static("genetic_data"),
    // Biometric data (used for unique identification)
    LabelRef::from_static("fingerprint"),
    LabelRef::from_static("voiceprint"),
    LabelRef::from_static("retina_scan"),
    LabelRef::from_static("facial_geometry"),
    // Health data — specific identifiers plus the broader
    // Article 4(15) health-narrative catch-all (blood pressure,
    // appointment notes, therapy references, care plans).
    LabelRef::from_static("medical_id"),
    LabelRef::from_static("insurance_id"),
    LabelRef::from_static("prescription_id"),
    LabelRef::from_static("diagnosis"),
    LabelRef::from_static("medication"),
    LabelRef::from_static("health_narrative"),
    // Sex life and sexual orientation
    LabelRef::from_static("sex_life"),
    LabelRef::from_static("sexual_orientation"),
];

/// Quasi-identifiers Recital 26 flags as re-identification
/// vectors when combined with special-category data. Added by
/// the `Article9WithReidHardening` and `Article9And10` scopes.
const RECITAL_26_LABELS: &[LabelRef] = &[
    LabelRef::from_static("date_of_birth"),
    LabelRef::from_static("postal_code"),
];

/// Article 10 criminal-justice labels — personal data relating
/// to criminal convictions and offences. Added by the
/// `Article9And10` scope.
const ARTICLE_10_LABELS: &[LabelRef] = &[
    LabelRef::from_static("criminal_record"),
    LabelRef::from_static("criminal_charge"),
    LabelRef::from_static("judicial_narrative"),
];
