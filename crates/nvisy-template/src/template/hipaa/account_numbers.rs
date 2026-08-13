//! §164.514(b)(2)(i)(J) account-identifier scope for the HIPAA
//! templates.

use elide_core::entity::LabelRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which account-identifier labels §164.514(b)(2)(i)(J) covers.
///
/// The regulation names "account numbers" without enumerating
/// them; HHS OCR's 2012 guidance doesn't narrow it either. In
/// practice, covered entities and de-ID vendors treat bank
/// accounts, IBANs, and payment cards as the core §(J) set —
/// [`Standard`](Self::Standard). Cryptocurrency wallet addresses
/// are the newer case: they identify a specific holder's account
/// by function, and the §(R) catch-all pulls in "any other unique
/// identifying number, characteristic, or code" tied to an
/// individual, which arguably captures them. Deployments where
/// crypto addresses can appear in patient-facing artifacts pick
/// [`Extended`](Self::Extended); deployments where they can't
/// stay on `Standard`.
///
/// The compliant posture for anything account-shaped is
/// "remove"; the split is only about how broadly this template
/// pre-declares the account label scope. A caller who wants
/// broader coverage combines this template's policy with a
/// separate PII scope on the same request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HipaaAccountNumbers {
    /// `bank_account`, `iban`, `payment_card`. The core §(J) set.
    #[default]
    Standard,
    /// Standard plus `crypto_address`. Pulls wallet addresses in
    /// under the §(R) catch-all reading.
    Extended,
}

const STANDARD_ACCOUNT_LABELS: &[LabelRef] = &[
    LabelRef::from_static("bank_account"),
    LabelRef::from_static("iban"),
    LabelRef::from_static("payment_card"),
];

const EXTENDED_ACCOUNT_LABELS: &[LabelRef] = &[
    LabelRef::from_static("bank_account"),
    LabelRef::from_static("iban"),
    LabelRef::from_static("payment_card"),
    LabelRef::from_static("crypto_address"),
];

impl HipaaAccountNumbers {
    /// The account-identifier labels this tier covers. Just the
    /// account delta — each posture's own base label set fuses
    /// this in via its own `labels()` helper.
    pub(crate) fn labels(self) -> &'static [LabelRef] {
        match self {
            Self::Standard => STANDARD_ACCOUNT_LABELS,
            Self::Extended => EXTENDED_ACCOUNT_LABELS,
        }
    }
}
