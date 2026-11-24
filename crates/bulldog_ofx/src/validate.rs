//! Metadata, parameter-range, and clean-room identifier validation for OpenFX plugins
//! (AGENTS.md §1.1, §3.3).

use core::fmt;
use std::collections::BTreeSet;

use bulldog_core::{EffectContext, EffectDescriptor, ParamKind};

/// Mandatory organizational reverse-DNS prefix for all Bulldog Complements plugins.
pub const REQUIRED_ID_PREFIX: &str = "br.com.bulldog.complements.";

/// Case-insensitive substrings prohibited in public identifiers, display labels,
/// groupings, descriptions, and parameter names (AGENTS.md §1.1 clean-room rule).
const FORBIDDEN_COMMERCIAL_TERMS: &[&str] = &[
    "boris",
    "sapphire",
    "red giant",
    "magic bullet",
    "newblue",
    "mocha pro",
    "particle illusion",
    "pluginpac",
];

/// Descriptor validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub plugin_id: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.plugin_id, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Validates a single [`EffectDescriptor`].
pub fn validate_plugin(desc: &EffectDescriptor) -> Result<(), ValidationError> {
    let err = |msg: String| ValidationError {
        plugin_id: desc.identifier.clone(),
        message: msg,
    };

    if !desc.identifier.starts_with(REQUIRED_ID_PREFIX) {
        return Err(err(format!(
            "identifier must start with '{REQUIRED_ID_PREFIX}'"
        )));
    }
    let suffix = desc
        .identifier
        .strip_prefix(REQUIRED_ID_PREFIX)
        .unwrap_or("");
    if suffix.is_empty()
        || !suffix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
    {
        return Err(err(
            "identifier suffix must be non-empty lowercase ASCII [a-z0-9._]".to_owned(),
        ));
    }

    if desc.label.trim().is_empty() {
        return Err(err("plugin label cannot be empty".to_owned()));
    }
    if desc.grouping.trim().is_empty() {
        return Err(err("plugin grouping cannot be empty".to_owned()));
    }
    if desc.description.trim().is_empty() {
        return Err(err("plugin description cannot be empty".to_owned()));
    }

    check_clean_room(&desc.identifier, &desc.identifier)?;
    check_clean_room(&desc.identifier, &desc.label)?;
    check_clean_room(&desc.identifier, &desc.grouping)?;
    check_clean_room(&desc.identifier, &desc.description)?;

    let mut seen_params = BTreeSet::new();
    let mut has_transition_param = false;

    for param in &desc.params {
        param
            .validate()
            .map_err(|e| err(format!("parameter validation failed: {e}")))?;

        if !seen_params.insert(param.name.as_str()) {
            return Err(err(format!("duplicate parameter name '{}'", param.name)));
        }
        check_clean_room(&desc.identifier, &param.name)?;
        check_clean_room(&desc.identifier, &param.label)?;
        check_clean_room(&desc.identifier, &param.hint)?;

        if param.name == "Transition"
            && let ParamKind::Double { min, max, .. } = param.kind
            && (min - 0.0).abs() <= f64::EPSILON
            && (max - 1.0).abs() <= f64::EPSILON
        {
            has_transition_param = true;
        }
    }

    if desc.context == EffectContext::Transition && !has_transition_param {
        return Err(err(
            "Transition context plugin must declare a 'Transition' double parameter in [0.0, 1.0]"
                .to_owned(),
        ));
    }

    Ok(())
}

/// Validates a suite or cross-suite list of plugin descriptors, ensuring global ID uniqueness.
pub fn validate_suite(descriptors: &[EffectDescriptor]) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let mut seen_ids = BTreeSet::new();

    if descriptors.is_empty() {
        errors.push(ValidationError {
            plugin_id: "<suite>".to_owned(),
            message: "suite must register at least one plugin".to_owned(),
        });
    }

    for desc in descriptors {
        if !seen_ids.insert(desc.identifier.clone()) {
            errors.push(ValidationError {
                plugin_id: desc.identifier.clone(),
                message: "duplicate OpenFX plugin identifier".to_owned(),
            });
        }
        if let Err(e) = validate_plugin(desc) {
            errors.push(e);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_clean_room(plugin_id: &str, text: &str) -> Result<(), ValidationError> {
    let lower = text.to_ascii_lowercase();
    for term in FORBIDDEN_COMMERCIAL_TERMS {
        if lower.contains(term) {
            return Err(ValidationError {
                plugin_id: plugin_id.to_owned(),
                message: format!("forbidden commercial reference detected in metadata: '{text}'"),
            });
        }
    }
    Ok(())
}
