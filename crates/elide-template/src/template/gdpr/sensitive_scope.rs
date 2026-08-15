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
/// - [`Article9`](Self::Article9): the nine Article 9(1)
///   special categories only. The default.
/// - [`Article9WithReidHardening`](Self::Article9WithReidHardening) -
///   Article 9 plus the quasi-identifiers that carry the most join
///   risk against public datasets: `date_of_birth`, `postal_code`,
///   and `gender` (the combination that re-identifies most of a
///   population on its own), widened with `age`, `city`,
///   `nationality`, `citizenship`, and `occupation`.
///
///   A product-defined tier, not a complete answer to Recital 26:
///   the Recital sets a reasonableness test over "all the means
///   reasonably likely to be used" to re-identify, judged against
///   the caller's own data and adversary, so no fixed label list
///   can satisfy it. Callers whose threat model reaches wider
///   extend the policy after building it.
/// - [`Article9And10`](Self::Article9And10): Article 9 + Recital
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
    /// Article 9 plus the quasi-identifier set, so pseudonymized
    /// output is harder to re-identify via joins against public
    /// datasets. A product-defined tier rather than a complete
    /// Recital 26 posture: see the type docstring.
    Article9WithReidHardening,
    /// Article 9 + Recital 26 hardening + Article 10
    /// criminal-justice labels (`criminal_record`,
    /// `criminal_charge`, `judicial_narrative`).
    Article9And10,
}

impl GdprSensitiveScope {
    /// Every shipped scope, narrowest first. The tiers strictly
    /// widen, so this order is also coverage order.
    pub const ALL: &[Self] = &[
        Self::Article9,
        Self::Article9WithReidHardening,
        Self::Article9And10,
    ];

    /// Compile-time proof that [`ALL`](Self::ALL) lists every
    /// variant. Never called.
    const fn _exhaustive(self) {
        match self {
            Self::Article9 | Self::Article9WithReidHardening | Self::Article9And10 => {}
        }
    }
}

impl GdprSensitiveScope {
    /// The label set this scope covers: Article 9(1) always,
    /// plus the re-identification quasi-identifiers or Article 10
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
    // Racial or ethnic origin. `nationality` is deliberately absent:
    // Article 9(1) covers ethnic origin, while nationality is a legal
    // status the GDPR treats as ordinary personal data. It earns its
    // place as a quasi-identifier instead: see `RECITAL_26_LABELS`.
    LabelRef::from_static("ethnicity"),
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
    // Health data: specific identifiers plus the broader
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

/// Quasi-identifiers that most often carry a re-identification
/// join risk when combined with special-category data. Added by
/// the `Article9WithReidHardening` and `Article9And10` scopes.
///
/// A product-defined set, not an enumeration from Recital 26 -
/// the Recital states a reasonableness test rather than naming
/// fields, so treat this as a floor to extend against a real
/// threat model, not a sufficient hardening set.
const RECITAL_26_LABELS: &[LabelRef] = &[
    // The Sweeney triple: ZIP + date of birth + sex re-identifies
    // the large majority of a population on its own, and is the
    // best-evidenced join vector of the set.
    LabelRef::from_static("date_of_birth"),
    LabelRef::from_static("postal_code"),
    LabelRef::from_static("gender"),
    // Coarser demographic and geographic axes. Weaker alone, but
    // they sharpen the triple and appear in most public datasets
    // an adversary would join against.
    LabelRef::from_static("age"),
    LabelRef::from_static("city"),
    // `nationality` and `citizenship` are quasi-identifiers, not
    // Article 9(1) special categories: this tier is where they
    // carry their weight, and `GDPR_LABELS` deliberately omits
    // them.
    LabelRef::from_static("nationality"),
    LabelRef::from_static("citizenship"),
    LabelRef::from_static("occupation"),
];

/// Article 10 criminal-justice labels: personal data relating
/// to criminal convictions and offences. Added by the
/// `Article9And10` scope.
const ARTICLE_10_LABELS: &[LabelRef] = &[
    LabelRef::from_static("criminal_record"),
    LabelRef::from_static("criminal_charge"),
    LabelRef::from_static("judicial_narrative"),
];
