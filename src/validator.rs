use anyhow::{bail, Result};
use regex::Regex;

use crate::config_model::{SettingMetadata, ValidationState, ValueType};

pub fn infer_value_type(
    key: &str,
    default_value: Option<&str>,
    description: &str,
    enum_choices: &[String],
) -> ValueType {
    let lower_key = key.to_lowercase();
    let lower_desc = description.to_lowercase();
    let default_value = default_value.unwrap_or_default().trim();

    if !enum_choices.is_empty() {
        return ValueType::Enum;
    }

    if is_boolean_like(default_value, &lower_desc) {
        return ValueType::Boolean;
    }

    if is_color_like(&lower_key, &lower_desc, default_value) {
        return ValueType::Color;
    }

    if is_number_with_unit_like(&lower_key, &lower_desc, default_value) {
        return ValueType::NumberWithUnit;
    }

    if is_number_like(&lower_key, &lower_desc, default_value) {
        return ValueType::Number;
    }

    ValueType::String
}

fn is_boolean_like(default_value: &str, description: &str) -> bool {
    matches!(
        default_value.to_ascii_lowercase().as_str(),
        "yes" | "no" | "true" | "false" | "y" | "n" | "on" | "off"
    ) || description.contains("yes/no")
        || description.contains("true/false")
        || contains_word(description, "boolean")
        || contains_word(description, "bool")
}

fn is_color_like(lower_key: &str, description: &str, default_value: &str) -> bool {
    lower_key.contains("color")
        || lower_key.ends_with("_foreground")
        || lower_key.ends_with("_background")
        || lower_key.ends_with("_color")
        || lower_key == "background"
        || lower_key == "foreground"
        || description.contains("hex color")
        || description.contains("rgb:")
        || default_value.starts_with('#')
        || default_value.starts_with("rgb:")
}

fn is_number_with_unit_like(lower_key: &str, description: &str, default_value: &str) -> bool {
    default_has_unit(default_value)
        || has_unit_hint(description)
        || (default_looks_numeric_sequence(default_value)
            && (lower_key.contains("width")
                || lower_key.contains("height")
                || lower_key.contains("opacity")
                || lower_key.contains("thickness")))
}

fn is_number_like(lower_key: &str, _description: &str, default_value: &str) -> bool {
    default_looks_numeric_sequence(default_value)
        || lower_key.ends_with("_size")
        || lower_key.ends_with("_duration")
        || lower_key.ends_with("_interval")
        || lower_key.ends_with("_threshold")
        || lower_key.ends_with("_opacity")
        || lower_key.ends_with("_blur")
        || lower_key.ends_with("_radius")
        || lower_key.ends_with("_fps")
        || lower_key.ends_with("_dpi")
}

pub fn validate(meta: &SettingMetadata, value: &str) -> ValidationState {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return ValidationState::Unknown;
    }

    let res = match meta.value_type {
        ValueType::String => Ok(()),
        ValueType::Number => validate_number(trimmed),
        ValueType::NumberWithUnit => validate_number_with_unit(trimmed),
        ValueType::Boolean => validate_boolean(trimmed),
        ValueType::Enum => validate_enum(trimmed, &meta.enum_choices),
        ValueType::Color => validate_color(trimmed),
    };

    match res {
        Ok(_) => ValidationState::Valid,
        Err(err) => ValidationState::Invalid(err.to_string()),
    }
}

pub fn validate_number(value: &str) -> Result<()> {
    let tokens = split_numeric_tokens(value);
    if tokens.is_empty() {
        bail!("expected number");
    }
    for token in tokens {
        token.parse::<f64>()?;
    }
    Ok(())
}

pub fn validate_number_with_unit(value: &str) -> Result<()> {
    let re = Regex::new(r"^-?(?:\d+(?:\.\d+)?|\.\d+)(?:pt|px|em|%)?$")
        .expect("valid number-with-unit regex");
    let tokens = split_numeric_tokens(value);
    if tokens.is_empty() {
        bail!("expected number with optional unit pt|px|em|%");
    }
    if tokens.into_iter().all(|token| re.is_match(token)) {
        Ok(())
    } else {
        bail!("expected numbers with optional unit pt|px|em|%")
    }
}

pub fn validate_boolean(value: &str) -> Result<()> {
    match value.to_ascii_lowercase().as_str() {
        "yes" | "no" | "true" | "false" | "0" | "1" | "y" | "n" | "on" | "off" => Ok(()),
        _ => bail!("expected yes/no true/false y/n on/off 0/1"),
    }
}

pub fn validate_enum(value: &str, choices: &[String]) -> Result<()> {
    if choices.iter().any(|choice| choice == value) {
        Ok(())
    } else {
        bail!("expected one of: {}", choices.join(", "))
    }
}

pub fn validate_color(value: &str) -> Result<()> {
    let hex = Regex::new(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$").expect("valid color hex regex");
    let rgb =
        Regex::new(r"^rgb:([0-9a-fA-F]{2}/){2}[0-9a-fA-F]{2}$").expect("valid color rgb regex");
    let named = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_\-]*$").expect("valid color named regex");
    if hex.is_match(value) || rgb.is_match(value) || named.is_match(value) {
        Ok(())
    } else {
        bail!("expected kitty color format")
    }
}

fn contains_word(text: &str, needle: &str) -> bool {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|part| !part.is_empty() && part == needle)
}

fn has_unit_hint(description: &str) -> bool {
    description.contains(" in pts")
        || description.contains(" in pt")
        || description.contains(" in pixels")
        || description.contains(" in pixel")
        || description.contains("suffix of px")
        || description.contains("suffix of pt")
        || description.contains("suffix px")
        || description.contains("suffix pt")
        || description.contains("for pixels")
        || description.contains("for points")
        || description.contains("unitless number")
}

fn default_has_unit(default_value: &str) -> bool {
    split_numeric_tokens(default_value)
        .into_iter()
        .any(|token| {
            matches!(token.chars().last(), Some('%' | 'x' | 't' | 'm'))
                && validate_number_with_unit_token(token)
        })
}

fn default_looks_numeric_sequence(default_value: &str) -> bool {
    let tokens = split_numeric_tokens(default_value);
    !tokens.is_empty() && tokens.iter().all(|token| token.parse::<f64>().is_ok())
}

fn split_numeric_tokens(value: &str) -> Vec<&str> {
    value
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .filter(|token| !token.is_empty())
        .collect()
}

fn validate_number_with_unit_token(token: &str) -> bool {
    Regex::new(r"^-?(?:\d+(?:\.\d+)?|\.\d+)(?:pt|px|em|%)$")
        .expect("valid number-with-unit token regex")
        .is_match(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_model::{SettingMetadata, ValueType};

    fn meta(ty: ValueType) -> SettingMetadata {
        SettingMetadata {
            key: "x".into(),
            category: "Test".into(),
            description: String::new(),
            default_value: None,
            examples: vec![],
            enum_choices: vec!["always".into(), "never".into()],
            repeatable: false,
            value_type: ty,
            order: 0,
        }
    }

    #[test]
    fn validates_number() {
        assert!(matches!(
            validate(&meta(ValueType::Number), "1.25"),
            ValidationState::Valid
        ));
    }

    #[test]
    fn validates_number_lists() {
        assert!(matches!(
            validate(&meta(ValueType::Number), "0.001, 1, 1.5, 2"),
            ValidationState::Valid
        ));
    }

    #[test]
    fn validates_boolean() {
        assert!(matches!(
            validate(&meta(ValueType::Boolean), "yes"),
            ValidationState::Valid
        ));
        assert!(matches!(
            validate(&meta(ValueType::Boolean), "y"),
            ValidationState::Valid
        ));
        assert!(matches!(
            validate(&meta(ValueType::Boolean), "maybe"),
            ValidationState::Invalid(_)
        ));
    }

    #[test]
    fn validates_color() {
        assert!(matches!(
            validate(&meta(ValueType::Color), "#fff"),
            ValidationState::Valid
        ));
        assert!(matches!(
            validate(&meta(ValueType::Color), "rgb:ff/aa/00"),
            ValidationState::Valid
        ));
    }

    #[test]
    fn infers_boolean_from_default() {
        assert!(matches!(
            infer_value_type("confirm_os_window_close", Some("yes"), "", &[]),
            ValueType::Boolean
        ));
    }

    #[test]
    fn does_not_infer_boolean_from_numeric_zero() {
        assert!(matches!(
            infer_value_type(
                "background_blur",
                Some("0"),
                "Set to a positive value to enable background blur",
                &[]
            ),
            ValueType::Number
        ));
    }

    #[test]
    fn infers_font_family_as_string() {
        assert!(matches!(
            infer_value_type(
                "font_family",
                Some("monospace"),
                "Select the font family",
                &[]
            ),
            ValueType::String
        ));
    }

    #[test]
    fn infers_allow_cloning_as_string() {
        assert!(matches!(
            infer_value_type(
                "allow_cloning",
                Some("ask"),
                "Control whether programs running in the terminal can request new windows to be created.",
                &[]
            ),
            ValueType::String
        ));
    }

    #[test]
    fn infers_color_keys_as_color() {
        assert!(matches!(
            infer_value_type(
                "url_color",
                Some("#ec7970"),
                "URL underline color when hovering",
                &[]
            ),
            ValueType::Color
        ));
    }

    #[test]
    fn infers_unit_capable_width_as_number_with_unit() {
        assert!(matches!(
            infer_value_type(
                "window_margin_width",
                Some("0"),
                "The window margin in pts. A value with px suffix uses pixels.",
                &[]
            ),
            ValueType::NumberWithUnit
        ));
    }
}
