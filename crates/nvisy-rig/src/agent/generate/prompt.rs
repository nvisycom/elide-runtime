//! Generation-specific prompt construction.

use super::GenRequest;

/// Builds user prompts for text generation from a batch of requests.
pub(crate) struct GenPromptBuilder;

impl GenPromptBuilder {
    /// Build the user prompt from a batch of generation requests.
    pub fn build(requests: &[GenRequest]) -> String {
        let mut prompt = String::from(
            "Generate realistic synthetic replacement values for each entity below. \
             Return a JSON object with an \"entities\" array.\n\n",
        );

        for (i, req) in requests.iter().enumerate() {
            prompt.push_str(&format!(
                "{}. entity_type={}, original_value=\"{}\"",
                i + 1,
                req.entity_type,
                req.original_value,
            ));
            if let Some(ref ctx) = req.context {
                prompt.push_str(&format!(", context=\"{ctx}\""));
            }
            if let Some(ref locale) = req.locale {
                prompt.push_str(&format!(", locale=\"{locale}\""));
            }
            prompt.push('\n');
        }

        prompt
    }
}

/// Default system prompt for text generation.
pub(super) const GEN_SYSTEM_PROMPT: &str = "\
You are a data synthesis system that generates realistic, format-matching \
synthetic values to replace real PII/sensitive data. \
For each entity provided, generate a plausible fake replacement that: \
1) Matches the format and structure of the original (e.g. email → email, phone → phone). \
2) Is contextually appropriate (e.g. a person name from the same cultural context). \
3) Is clearly different from the original value. \
4) Maintains consistency — if the same original_value appears multiple times, \
   produce the same synthetic_value each time. \
Return results as a JSON object with an \"entities\" key containing an array of objects with keys: \
entity_type, original_value, synthetic_value. \
Examples: \
PersonName \"John Smith\" → \"Michael Davis\"; \
EmailAddress \"john@example.com\" → \"sarah.jones@mail.org\"; \
PhoneNumber \"+1-555-123-4567\" → \"+1-555-987-6543\"; \
IpAddress \"192.168.1.100\" → \"10.0.42.7\".";
