//! Planning logic and types.

pub mod conversation;
pub mod feedback;
pub mod reconcile;
pub mod score;
pub mod signal;
pub mod survey;

/// Extract a JSON object from LLM text that may include markdown fences or trailing prose.
pub(crate) fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();

    // Strip markdown code fences first.
    let without_fences = if trimmed.starts_with("```") {
        trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .unwrap_or(trimmed)
            .strip_suffix("```")
            .unwrap_or(trimmed)
            .trim()
    } else {
        trimmed
    };

    // Find the outermost { ... } to ignore trailing prose.
    if let Some(start) = without_fences.find('{') {
        if let Some(end) = without_fences.rfind('}') {
            if end > start {
                return &without_fences[start..=end];
            }
        }
    }

    without_fences
}
