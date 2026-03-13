use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::config_model::{MapEntry, SettingMetadata};
use crate::metadata_extractor::finalize_metadata;
use crate::parser::{
    parse_reference_config, parse_reference_keymaps, parse_reference_keymaps_str,
    parse_reference_str,
};

const BUNDLED_REFERENCE_CONF: &str = include_str!("../kitty defaults reference/kitty-full.conf");
const BUNDLED_REFERENCE_MARKDOWN: &str =
    include_str!("../kitty defaults reference/kitty_settings_reference.md");

#[derive(Debug, Clone)]
struct MarkdownSetting {
    category: String,
    default_value: Option<String>,
    format_hint: String,
}

pub fn load_reference_metadata(reference_path: Option<&Path>) -> Result<Vec<SettingMetadata>> {
    match reference_path {
        Some(path) => parse_reference_config(path),
        None => Ok(parse_bundled_reference()),
    }
}

pub fn load_reference_keymaps(reference_path: Option<&Path>) -> Result<Vec<MapEntry>> {
    match reference_path {
        Some(path) => parse_reference_keymaps(path),
        None => Ok(parse_bundled_reference_keymaps()),
    }
}

pub fn parse_bundled_reference() -> Vec<SettingMetadata> {
    let mut items = parse_reference_str(BUNDLED_REFERENCE_CONF);
    apply_markdown_reference(&mut items, BUNDLED_REFERENCE_MARKDOWN);
    finalize_metadata(items)
}

pub fn parse_bundled_reference_keymaps() -> Vec<MapEntry> {
    parse_reference_keymaps_str(BUNDLED_REFERENCE_CONF, Path::new("kitty-full.conf"))
}

fn apply_markdown_reference(items: &mut [SettingMetadata], markdown: &str) {
    let rows = parse_markdown_reference(markdown);
    for item in items {
        let Some(row) = rows.get(&item.key) else {
            continue;
        };

        if item.category == "General" && !row.category.is_empty() {
            item.category = row.category.clone();
        }

        if item.default_value.is_none() {
            item.default_value = row.default_value.clone();
        }

        let format_hint = row.format_hint.trim();
        if format_hint.is_empty() {
            continue;
        }

        if format_hint.to_ascii_lowercase().starts_with("repeatable:") {
            item.repeatable = true;
        }

        append_description(item, &format!("Possible values / format: {format_hint}"));
        item.enum_choices
            .extend(enum_choices_from_format_hint(format_hint));
    }
}

fn append_description(item: &mut SettingMetadata, note: &str) {
    if note.is_empty() || item.description.contains(note) {
        return;
    }

    if !item.description.trim().is_empty() {
        item.description.push_str("\n\n");
    }
    item.description.push_str(note);
}

fn parse_markdown_reference(markdown: &str) -> HashMap<String, MarkdownSetting> {
    let mut current_category = String::new();
    let mut rows = HashMap::new();

    for raw in markdown.lines() {
        let line = raw.trim();
        if let Some(category) = line.strip_prefix("## ") {
            current_category = category.trim().to_string();
            continue;
        }

        let Some(cols) = split_markdown_row(line) else {
            continue;
        };
        if cols.len() != 3 || is_separator_row(&cols) || cols[0].eq_ignore_ascii_case("Setting") {
            continue;
        }

        let key = clean_markdown_cell(&cols[0]);
        if key.is_empty() {
            continue;
        }

        rows.insert(
            key,
            MarkdownSetting {
                category: current_category.clone(),
                default_value: normalize_default_value(&cols[1]),
                format_hint: clean_markdown_cell(&cols[2]),
            },
        );
    }

    rows
}

fn split_markdown_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }

    let mut cols = Vec::new();
    let mut current = String::new();
    let mut escape = false;

    for ch in trimmed[1..trimmed.len() - 1].chars() {
        if escape {
            if ch != '|' && ch != '\\' {
                current.push('\\');
            }
            current.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' => escape = true,
            '|' => {
                cols.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if escape {
        current.push('\\');
    }
    cols.push(current.trim().to_string());
    Some(cols)
}

fn is_separator_row(cols: &[String]) -> bool {
    cols.iter()
        .all(|col| !col.is_empty() && col.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn clean_markdown_cell(cell: &str) -> String {
    let trimmed = cell.trim().replace("\\|", "|");
    if trimmed.len() >= 2 && trimmed.starts_with('`') && trimmed.ends_with('`') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed
    }
}

fn normalize_default_value(cell: &str) -> Option<String> {
    let cleaned = clean_markdown_cell(cell);
    if cleaned.is_empty() || cleaned == "—" {
        None
    } else {
        Some(cleaned)
    }
}

fn enum_choices_from_format_hint(hint: &str) -> Vec<String> {
    let lower = hint.to_ascii_lowercase();
    if lower.is_empty() || is_non_enum_format_hint(&lower) {
        return vec![];
    }

    lower
        .replace("one of:", "")
        .replace("can be one of:", "")
        .replace(", or ", ", ")
        .replace(" or ", ", ")
        .split(',')
        .map(str::trim)
        .map(|token| token.trim_matches('"').trim_matches('`').trim_matches('.'))
        .filter(|token| is_literal_choice(token))
        .map(ToOwned::to_owned)
        .collect()
}

fn is_non_enum_format_hint(lower: &str) -> bool {
    const BLOCKLIST: &[&str] = &[
        "repeatable:",
        "free-form",
        "number",
        "float",
        "integer",
        "size value",
        "path",
        "command",
        "string",
        "color",
        "space-separated",
        "comma-separated",
        "character set",
        "feature",
        "family",
        "shape name",
        "expression",
        "password",
        "socket spec",
        "boolean",
        "<",
        ">",
    ];

    BLOCKLIST.iter().any(|needle| lower.contains(needle))
}

fn is_literal_choice(token: &str) -> bool {
    !token.is_empty()
        && !token.contains(' ')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.' | '+' | ':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bundled_reference_without_external_file() {
        let items = parse_bundled_reference();
        assert!(items.len() >= 200);
        assert!(items.iter().any(|item| item.key == "font_size"));
        assert!(items.iter().all(|item| item.key != "include"));
    }

    #[test]
    fn adds_markdown_format_hints_to_descriptions() {
        let items = parse_bundled_reference();
        let item = items
            .iter()
            .find(|item| item.key == "select_by_word_characters")
            .expect("select_by_word_characters exists");
        assert!(item
            .description
            .contains("Possible values / format: character set string"));
    }

    #[test]
    fn marks_repeatable_settings_from_markdown_reference() {
        let items = parse_bundled_reference();
        let item = items
            .iter()
            .find(|item| item.key == "remote_control_password")
            .expect("remote_control_password exists");
        assert!(item.repeatable);
    }

    #[test]
    fn loads_bundled_reference_keymaps() {
        let keymaps = parse_bundled_reference_keymaps();
        assert!(keymaps.iter().any(|map| map.shortcut == "kitty_mod+c"));
    }

    #[test]
    fn parses_markdown_rows_with_escaped_pipes() {
        let row = split_markdown_row(
            "| `font_features` | `—` | font_features <PostScriptName\\|none> <feature...> |",
        )
        .expect("row");
        assert_eq!(row[0], "`font_features`");
        assert_eq!(row[2], "font_features <PostScriptName|none> <feature...>");
    }
}
