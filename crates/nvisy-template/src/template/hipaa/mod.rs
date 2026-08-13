//! HIPAA §164.514 de-identification.
//!
//! §164.514(b) offers two paths to de-identification: **Safe
//! Harbor** (a fixed rule set — remove eighteen identifier
//! categories with age/date special dispatch) and **Expert
//! Determination** (a statistician-signed attestation of "very
//! small" re-identification risk). §164.514(e) then defines the
//! **Limited Data Set** — a narrower subtraction that keeps
//! dates and coarse geography for research handoffs governed by
//! a Data Use Agreement.
//!
//! This module ships all three:
//!
//! - [`HipaaDeidMethod::SafeHarbor`] — every §164.514(b)(2)
//!   identifier removed. Ages ≥ 90 [`Clamp`]ed to `"90 or older"`,
//!   dates related to an individual [`GeneralizeDate`]d to the
//!   year, the remainder [`Erase`]d.
//! - [`HipaaDeidMethod::LimitedDataSet`] — §164.514(e)(2)'s
//!   sixteen-identifier subtraction. Names, street address,
//!   contact info, IDs, biometrics, and other unique identifiers
//!   erase; dates, town/city, state, ZIP, and ages survive
//!   verbatim (the DUA is the caller's out-of-band obligation).
//! - [`HipaaDeidMethod::ExpertDetermination`] — a starting
//!   scaffold for §164.514(b)(1). Same 18-identifier label set
//!   as Safe Harbor, same age/date table dispatch, but the bulk
//!   terminal is [`Pseudonymize`] instead of [`Erase`] to
//!   preserve coreference across mentions (the analytics need
//!   that most Expert Determination processes target). Does not
//!   certify de-identification; a qualified statistician must
//!   still document that re-identification risk is "very small"
//!   under the applicable methodology.
//!
//! [`Clamp`]: nvisy_policy::redaction::TextRedaction::Clamp
//! [`Erase`]: nvisy_policy::redaction::TextRedaction::Erase
//! [`GeneralizeDate`]: nvisy_policy::redaction::TextRedaction::GeneralizeDate
//! [`HmacHash`]: nvisy_policy::redaction::TextRedaction::HmacHash
//! [`LabelGroup`]: nvisy_policy::LabelGroup
//! [`PolicyDefinition`]: nvisy_policy::PolicyDefinition
//! [`Predicated`]: nvisy_policy::RuleDispatch::Predicated
//! [`Pseudonymize`]: nvisy_policy::redaction::TextRedaction::Pseudonymize
//! [`RuleDispatch`]: nvisy_policy::RuleDispatch

use jiff::civil::Date;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Template;

mod account_numbers;
mod expert_determination;
mod limited_data_set;
mod safe_harbor;

pub use self::account_numbers::HipaaAccountNumbers;
use self::expert_determination::expert_determination_template;
use self::limited_data_set::limited_data_set_template;
use self::safe_harbor::safe_harbor_template;

/// §164.514 effective date (final Privacy Rule). Shared across
/// all three postures.
pub(super) const EFFECTIVE_DATE: Date = Date::constant(2003, 4, 14);

/// Which HIPAA §164.514 de-identification method to apply.
///
/// The tradeoff is analytic yield vs. downstream constraint:
///
/// - [`SafeHarbor`](Self::SafeHarbor) — strips all eighteen
///   identifier categories. No Data Use Agreement or statistician
///   required, but dates, coarse geography, and ages ≥ 90 all
///   disappear or collapse.
/// - [`LimitedDataSet`](Self::LimitedDataSet) — narrower
///   subtraction (§164.514(e)(2)) that keeps dates, town/city,
///   state, ZIP, and ages verbatim. Suitable for research and
///   public-health handoffs *only when* a Data Use Agreement
///   governs the recipient's use.
/// - [`ExpertDetermination`](Self::ExpertDetermination) —
///   starting scaffold for §164.514(b)(1). Same label set as
///   Safe Harbor with pseudonymization as the default terminal
///   (identity-preserving across mentions). **Does not certify
///   de-identification** — a qualified statistician must
///   document that re-identification risk is "very small" under
///   the applicable methodology before the output can be treated
///   as de-identified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HipaaDeidMethod {
    /// Safe Harbor de-identification per §164.514(b)(2).
    #[default]
    SafeHarbor,
    /// Limited Data Set per §164.514(e)(2). Requires a Data Use
    /// Agreement out-of-band.
    LimitedDataSet,
    /// Expert Determination scaffold for §164.514(b)(1). Requires
    /// a qualified statistician to document that re-identification
    /// risk is "very small" — the shipped template alone does not
    /// certify de-identification.
    ExpertDetermination,
}

/// The HIPAA §164.514 template config — method axis fused with
/// account-identifier axis. Carried directly by
/// [`crate::PolicyTemplate::HipaaDeidentification`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HipaaDeidentification {
    /// Which §164.514 method to apply. See [`HipaaDeidMethod`]
    /// for the tradeoff.
    pub method: HipaaDeidMethod,
    /// Which §(J) account-identifier labels to remove. Defaults
    /// to [`HipaaAccountNumbers::Standard`] (bank account +
    /// IBAN + payment card). Pick
    /// [`HipaaAccountNumbers::Extended`] to add crypto wallet
    /// addresses under the §(R) catch-all reading.
    #[serde(default)]
    pub accounts: HipaaAccountNumbers,
}

impl HipaaDeidentification {
    /// Build the §164.514 template for this config.
    pub(crate) fn template(self) -> Template {
        match self.method {
            HipaaDeidMethod::SafeHarbor => safe_harbor_template(self.accounts),
            HipaaDeidMethod::LimitedDataSet => limited_data_set_template(self.accounts),
            HipaaDeidMethod::ExpertDetermination => expert_determination_template(self.accounts),
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_policy::RuleDispatch;
    use nvisy_policy::redaction::TextRedaction;

    use super::limited_data_set::LDS_LABELS;
    use super::safe_harbor::{SAFE_HARBOR_LABELS, SAFE_HARBOR_TABLE_LABELS};
    use super::*;

    #[test]
    fn safe_harbor_table_labels_are_excluded_from_bulk_erase_group() {
        // Table-dispatched labels (age, dates) must not appear in
        // the bulk-erase group, else a rule reorder could silently
        // convert Clamp / GeneralizeDate into Erase.
        for label in SAFE_HARBOR_TABLE_LABELS {
            assert!(
                !SAFE_HARBOR_LABELS.contains(label),
                "table label `{}` must not appear in SAFE_HARBOR_LABELS bulk-erase set",
                label.as_str(),
            );
        }
    }

    #[test]
    fn safe_harbor_special_dispatch_rule_precedes_bulk_erase() {
        let policy = &HipaaDeidentification {
            method: HipaaDeidMethod::SafeHarbor,
            accounts: HipaaAccountNumbers::default(),
        }
        .template()
        .policy;
        assert!(
            matches!(&policy.rules[0].dispatch, RuleDispatch::Table { .. }),
            "table rule must fire first so `age` and dates reach their generalizers",
        );
        assert!(matches!(
            &policy.rules[1].dispatch,
            RuleDispatch::Predicated { .. }
        ));
    }

    #[test]
    fn safe_harbor_table_rule_dispatches_age_to_clamp_and_dates_to_generalize() {
        let RuleDispatch::Table { operators } = &HipaaDeidentification {
            method: HipaaDeidMethod::SafeHarbor,
            accounts: HipaaAccountNumbers::default(),
        }
        .template()
        .policy
        .rules[0]
            .dispatch
        else {
            panic!("first rule must be Table dispatch");
        };
        let age = operators
            .iter()
            .find(|e| e.label.as_str() == "age")
            .expect("age entry present");
        assert!(matches!(
            age.action.text,
            Some(TextRedaction::Clamp { ceiling: Some(c), .. }) if (c - 90.0).abs() < f64::EPSILON,
        ));
        for date_label in ["date_of_birth", "individual_date"] {
            let entry = operators
                .iter()
                .find(|e| e.label.as_str() == date_label)
                .unwrap_or_else(|| panic!("{date_label} entry present"));
            assert!(matches!(
                entry.action.text,
                Some(TextRedaction::GeneralizeDate { .. }),
            ));
        }
    }

    #[test]
    fn standard_accounts_cover_the_traditional_section_j_set() {
        // The Standard tier is the reg's core §(J) reading:
        // bank accounts + IBAN + payment cards. Crypto is opt-in
        // via Extended (the §(R) catch-all reading).
        let labels = HipaaAccountNumbers::Standard.labels();
        for want in ["bank_account", "iban", "payment_card"] {
            assert!(
                labels.iter().any(|l| l.as_str() == want),
                "Standard tier must include §(J) label `{want}`",
            );
        }
        assert!(
            !labels.iter().any(|l| l.as_str() == "crypto_address"),
            "Standard tier is crypto-free (Extended adds it)",
        );
    }

    #[test]
    fn extended_accounts_add_crypto_to_the_standard_set() {
        let labels = HipaaAccountNumbers::Extended.labels();
        for want in ["bank_account", "iban", "payment_card", "crypto_address"] {
            assert!(
                labels.iter().any(|l| l.as_str() == want),
                "Extended tier must include §(J)/§(R) label `{want}`",
            );
        }
    }

    #[test]
    fn account_tier_lands_in_every_hipaa_method() {
        // Whichever method the caller picks (Safe Harbor, LDS,
        // Expert Determination), the account tier must widen the
        // policy's builtin label scope so the account labels are
        // eligible for the bulk rule.
        for method in [
            HipaaDeidMethod::SafeHarbor,
            HipaaDeidMethod::LimitedDataSet,
            HipaaDeidMethod::ExpertDetermination,
        ] {
            let extended = HipaaDeidentification {
                method,
                accounts: HipaaAccountNumbers::Extended,
            }
            .template();
            for want in ["bank_account", "iban", "payment_card", "crypto_address"] {
                assert!(
                    extended
                        .policy
                        .labels
                        .builtins
                        .iter()
                        .any(|l| l.as_str() == want),
                    "{method:?} Extended must carry `{want}` in policy builtins",
                );
            }
            let standard = HipaaDeidentification {
                method,
                accounts: HipaaAccountNumbers::Standard,
            }
            .template();
            assert!(
                !standard
                    .policy
                    .labels
                    .builtins
                    .iter()
                    .any(|l| l.as_str() == "crypto_address"),
                "{method:?} Standard must not carry `crypto_address`",
            );
        }
    }

    #[test]
    fn limited_data_set_keeps_dates_ages_and_coarse_geography() {
        // The whole point of LDS is that dates, ages, and coarse
        // geography survive. If any of these ends up in the LDS
        // erase set, the template is broken.
        for survivor in [
            "age",
            "date_of_birth",
            "individual_date",
            "date_time",
            "postal_code",
        ] {
            assert!(
                !LDS_LABELS.iter().any(|l| l.as_str() == survivor),
                "LDS must not erase `{survivor}` — it's a survivor per §164.514(e)(2)",
            );
        }
    }

    #[test]
    fn every_method_ships_a_distinct_policy_identity() {
        let accounts = HipaaAccountNumbers::default();
        let sh = HipaaDeidentification {
            method: HipaaDeidMethod::SafeHarbor,
            accounts,
        }
        .template();
        let lds = HipaaDeidentification {
            method: HipaaDeidMethod::LimitedDataSet,
            accounts,
        }
        .template();
        let ed = HipaaDeidentification {
            method: HipaaDeidMethod::ExpertDetermination,
            accounts,
        }
        .template();
        // Distinct template ids and policy ids across all three
        // — audits key on these to tell the postures apart.
        for (a, b) in [(&sh, &lds), (&sh, &ed), (&lds, &ed)] {
            assert_ne!(a.id, b.id, "template ids must differ");
            assert_ne!(a.policy.id, b.policy.id, "policy ids must differ");
        }
    }

    #[test]
    fn expert_determination_pseudonymizes_the_bulk_group() {
        // The whole point of the ED scaffold vs Safe Harbor is
        // that the bulk terminal is Pseudonymize (identity-
        // preserving), not Erase.
        let RuleDispatch::Predicated { action, .. } = &HipaaDeidentification {
            method: HipaaDeidMethod::ExpertDetermination,
            accounts: HipaaAccountNumbers::default(),
        }
        .template()
        .policy
        .rules[1]
            .dispatch
        else {
            panic!("second rule must be Predicated dispatch");
        };
        assert!(matches!(action.text, Some(TextRedaction::Pseudonymize)));
    }

    #[test]
    fn template_ids_are_stable_across_calls() {
        let accounts = HipaaAccountNumbers::default();
        for method in [
            HipaaDeidMethod::SafeHarbor,
            HipaaDeidMethod::LimitedDataSet,
            HipaaDeidMethod::ExpertDetermination,
        ] {
            let a = HipaaDeidentification { method, accounts }.template();
            let b = HipaaDeidentification { method, accounts }.template();
            assert_eq!(a.id, b.id);
            assert_eq!(a.policy.id, b.policy.id);
            for (r_a, r_b) in a.policy.rules.iter().zip(b.policy.rules.iter()) {
                assert_eq!(r_a.id, r_b.id);
            }
        }
    }
}
