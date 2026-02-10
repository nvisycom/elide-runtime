pub mod actions;
pub mod loaders;
pub mod patterns;

use nvisy_core::plugin::PluginDescriptor;

use crate::actions::apply_redaction::ApplyRedactionAction;
use crate::actions::classify::ClassifyAction;
use crate::actions::detect_checksum::DetectChecksumAction;
use crate::actions::detect_regex::DetectRegexAction;
use crate::actions::emit_audit::EmitAuditAction;
use crate::actions::evaluate_policy::EvaluatePolicyAction;
use crate::loaders::csv_loader::CsvLoader;
use crate::loaders::json_loader::JsonLoader;
use crate::loaders::plaintext::PlaintextLoader;

/// Create the detect plugin descriptor.
pub fn detect_plugin() -> PluginDescriptor {
    PluginDescriptor::new("detect")
        .with_action(DetectRegexAction)
        .with_action(DetectChecksumAction)
        .with_action(EvaluatePolicyAction)
        .with_action(ApplyRedactionAction)
        .with_action(ClassifyAction)
        .with_action(EmitAuditAction)
        .with_loader(PlaintextLoader)
        .with_loader(CsvLoader)
        .with_loader(JsonLoader)
}
