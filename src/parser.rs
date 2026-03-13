use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;

use crate::config_model::{
    EffectiveConfig, IncludeLine, MapEntry, SettingMetadata, SettingValue, ValueType,
};
use crate::metadata_extractor::finalize_metadata;

pub fn parse_reference_config(path: &Path) -> Result<Vec<SettingMetadata>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading reference config {}", path.display()))?;
    Ok(parse_reference_str(&text))
}

pub fn parse_reference_str(text: &str) -> Vec<SettingMetadata> {
    let mut category = String::from("General");
    let mut pending_comments: Vec<String> = Vec::new();
    let mut items = Vec::new();
    let mut seen_keys = HashSet::new();
    let mut seen_setting_in_section = false;
    let mut order = 0usize;

    let section_re = Regex::new(r"^#:\s*(.+?)\s*\{\{\{$").expect("valid section regex");
    let active_setting_re =
        Regex::new(r"^([A-Za-z0-9_]+)\s+(.*)$").expect("valid active setting regex");
    let commented_setting_re =
        Regex::new(r"^#\s*([A-Za-z0-9_]+)\s*(.*)$").expect("valid commented setting regex");

    for raw in text.lines() {
        let line = raw.trim_end();
        if let Some(cap) = section_re.captures(line) {
            category = cap[1].trim().to_string();
            pending_comments.clear();
            seen_setting_in_section = false;
            continue;
        }

        if line.starts_with("#:") {
            if !seen_setting_in_section {
                pending_comments.push(line.trim_start_matches("#:").trim().to_string());
            }
            continue;
        }

        if line.trim().is_empty() {
            if !pending_comments.is_empty() {
                pending_comments.push(String::new());
            }
            continue;
        }

        let parsed = parse_setting_candidate(line, &active_setting_re, &commented_setting_re);

        let Some((key, default)) = parsed else {
            continue;
        };

        seen_setting_in_section = true;
        if is_skipped_reference_key(&key) {
            pending_comments.clear();
            continue;
        }

        if seen_keys.contains(&key) {
            pending_comments.clear();
            continue;
        }
        seen_keys.insert(key.clone());

        let (description, examples, enum_choices, repeatable) =
            parse_comment_block(&pending_comments);
        items.push(SettingMetadata {
            key,
            category: category.clone(),
            description,
            default_value: (!default.is_empty()).then_some(default),
            examples,
            enum_choices,
            repeatable,
            value_type: ValueType::String,
            order,
        });
        order += 1;
        pending_comments.clear();
    }

    merge_trailing_comment_metadata(text, &mut items);
    finalize_metadata(items)
}

fn parse_comment_block(lines: &[String]) -> (String, Vec<String>, Vec<String>, bool) {
    let mut description_lines = Vec::new();
    let mut examples = Vec::new();
    let mut enum_choices = Vec::new();
    let mut collect_enum = false;
    let mut repeatable = false;

    for line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        if lower.contains("can be one of")
            || lower.contains("one of the following")
            || lower.starts_with("choices:")
            || lower.starts_with("possible values:")
            || lower.contains("supported strategies are")
            || lower.contains("supported values are")
            || lower.contains("various values are")
        {
            collect_enum = true;
            continue;
        }

        if lower.contains("repeatable")
            || lower.contains("can be specified multiple times")
            || lower.contains("may be specified multiple times")
            || lower.contains("can be used multiple times")
        {
            repeatable = true;
        }

        if collect_enum {
            if trimmed.is_empty() {
                collect_enum = false;
                continue;
            }
            if is_enum_choice_line(trimmed) {
                enum_choices.push(trimmed.trim_matches('`').to_string());
                continue;
            }
            collect_enum = false;
        }

        if lower.starts_with("example")
            || lower.starts_with("for example")
            || lower.starts_with("e.g.")
        {
            examples.push(trimmed.to_string());
        } else if !trimmed.is_empty() {
            description_lines.push(trimmed.to_string());
        }
    }

    if enum_choices.is_empty() {
        enum_choices = infer_inline_enum_choices(&description_lines.join("\n"));
    }

    (
        description_lines.join("\n"),
        examples,
        enum_choices,
        repeatable,
    )
}

fn is_enum_choice_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && !trimmed.contains("  ")
        && trimmed.split_whitespace().count() <= 3
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | '.' | '+' | ':'))
}

fn infer_inline_enum_choices(description: &str) -> Vec<String> {
    let lower = description.to_lowercase();
    let mut out = Vec::new();
    for marker in ["can be one of", "choices are", "possible values are"] {
        if let Some(idx) = lower.find(marker) {
            let rest = &description[idx + marker.len()..];
            let head = rest
                .lines()
                .next()
                .unwrap_or_default()
                .trim_matches(':')
                .trim();
            for token in head.split(|c| [',', '/', '|'].contains(&c)) {
                let candidate = token.trim().trim_matches('`').trim_matches('.');
                if is_enum_choice_line(candidate) {
                    out.push(candidate.to_string());
                }
            }
        }
    }

    let prose_re = Regex::new(r"\b(?:default of|value of)\s+([A-Za-z0-9_-]+)\b")
        .expect("valid prose enum regex");
    let prose_choices = prose_re
        .captures_iter(description)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .filter(|candidate| is_enum_choice_line(candidate))
        .map(|candidate| candidate.to_string())
        .collect::<Vec<_>>();
    if prose_choices.len() >= 2 {
        out.extend(prose_choices);
    }

    out.sort();
    out.dedup();
    out
}

fn parse_setting_candidate(
    line: &str,
    active_setting_re: &Regex,
    commented_setting_re: &Regex,
) -> Option<(String, String)> {
    let trimmed = line.trim();
    if let Some(cap) = active_setting_re.captures(trimmed) {
        return Some((cap[1].to_string(), cap[2].trim().to_string()));
    }
    if let Some(cap) = commented_setting_re.captures(trimmed) {
        let key = cap[1].to_string();
        if key.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_') {
            None
        } else {
            Some((key, cap[2].trim().to_string()))
        }
    } else {
        None
    }
}

fn is_skipped_reference_key(key: &str) -> bool {
    matches!(key, "map" | "include" | "mouse_map")
}

fn merge_trailing_comment_metadata(text: &str, items: &mut [SettingMetadata]) {
    let by_key = items
        .iter()
        .enumerate()
        .map(|(idx, item)| (item.key.clone(), idx))
        .collect::<HashMap<_, _>>();
    let section_re = Regex::new(r"^#:\s*(.+?)\s*\{\{\{$").expect("valid section regex");
    let active_setting_re =
        Regex::new(r"^([A-Za-z0-9_]+)\s+(.*)$").expect("valid active setting regex");
    let commented_setting_re =
        Regex::new(r"^#\s*([A-Za-z0-9_]+)\s*(.*)$").expect("valid commented setting regex");
    let mut active_key: Option<String> = None;
    let mut trailing_comments = Vec::new();

    let flush = |active_key: &mut Option<String>,
                 trailing_comments: &mut Vec<String>,
                 items: &mut [SettingMetadata]| {
        let Some(key) = active_key.take() else {
            trailing_comments.clear();
            return;
        };
        let Some(idx) = by_key.get(&key).copied() else {
            trailing_comments.clear();
            return;
        };
        if trailing_comments.is_empty() {
            return;
        }
        let (description, examples, enum_choices, repeatable) =
            parse_comment_block(trailing_comments);
        merge_metadata_parts(
            &mut items[idx],
            &description,
            examples,
            enum_choices,
            repeatable,
        );
        trailing_comments.clear();
    };

    for raw in text.lines() {
        let line = raw.trim_end();
        if section_re.is_match(line) {
            flush(&mut active_key, &mut trailing_comments, items);
            continue;
        }

        if let Some((key, _default)) =
            parse_setting_candidate(line, &active_setting_re, &commented_setting_re)
        {
            flush(&mut active_key, &mut trailing_comments, items);
            if !is_skipped_reference_key(&key) && by_key.contains_key(&key) {
                active_key = Some(key);
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("#:") {
            if active_key.is_some() {
                trailing_comments.push(rest.trim().to_string());
            }
            continue;
        }

        if line.trim().is_empty() {
            if active_key.is_some() && !trailing_comments.is_empty() {
                trailing_comments.push(String::new());
            }
        }
    }

    flush(&mut active_key, &mut trailing_comments, items);
}

fn merge_metadata_parts(
    item: &mut SettingMetadata,
    description: &str,
    examples: Vec<String>,
    enum_choices: Vec<String>,
    repeatable: bool,
) {
    let description = description.trim();
    if !description.is_empty() && !item.description.contains(description) {
        if !item.description.trim().is_empty() {
            item.description.push_str("\n\n");
        }
        item.description.push_str(description);
    }
    item.examples.extend(examples);
    item.enum_choices.extend(enum_choices);
    item.repeatable |= repeatable;
}

pub fn parse_current_config(main_file: &Path) -> Result<EffectiveConfig> {
    let mut values: HashMap<String, Vec<SettingValue>> = HashMap::new();
    let mut keymaps = Vec::new();
    let mut includes = Vec::new();
    let mut visited = HashSet::new();
    let leading_block = extract_leading_block(main_file)?;
    load_config_recursive(
        main_file,
        &mut visited,
        &mut values,
        &mut keymaps,
        &mut includes,
    )?;
    Ok(EffectiveConfig {
        values,
        keymaps,
        includes,
        leading_block,
        main_file: main_file.to_path_buf(),
    })
}

pub fn parse_reference_keymaps(path: &Path) -> Result<Vec<MapEntry>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading current config {}", path.display()))?;
    Ok(parse_reference_keymaps_str(&text, path))
}

pub fn parse_reference_keymaps_str(text: &str, source_file: &Path) -> Vec<MapEntry> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, raw)| parse_map_entry(raw, source_file, idx + 1, true))
        .collect()
}

fn load_config_recursive(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    values: &mut HashMap<String, Vec<SettingValue>>,
    keymaps: &mut Vec<MapEntry>,
    includes: &mut Vec<IncludeLine>,
) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(());
    }

    let text = fs::read_to_string(path)
        .with_context(|| format!("reading current config {}", path.display()))?;
    let setting_re = Regex::new(r"^([A-Za-z0-9_]+)\s+(.*)$").expect("valid setting regex");

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("include ") {
            let include_path = parse_include_target(rest);
            let resolved = resolve_include(path, include_path);
            includes.push(IncludeLine {
                raw: raw.to_string(),
                path: include_path.to_string(),
                source_file: path.to_path_buf(),
                line_no,
            });
            if resolved.exists() {
                load_config_recursive(&resolved, visited, values, keymaps, includes)?;
            }
            continue;
        }
        if let Some(entry) = parse_map_entry(raw, path, line_no, false) {
            keymaps.push(entry);
            continue;
        }
        if let Some(cap) = setting_re.captures(line) {
            let key = cap[1].to_string();
            let value = cap[2].trim().to_string();
            values.entry(key.clone()).or_default().push(SettingValue {
                key,
                value,
                source_file: path.to_path_buf(),
                line_no,
            });
        }
    }
    Ok(())
}

pub fn extract_leading_block(path: &Path) -> Result<String> {
    let text = fs::read_to_string(path)?;
    let setting_re = Regex::new(r"^([A-Za-z0-9_]+)\s+").expect("valid leading-block regex");
    let mut block = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("include ") {
            block.push(raw.to_string());
            continue;
        }
        if line.starts_with("map ") || setting_re.is_match(line) {
            break;
        }
        block.push(raw.to_string());
    }

    let mut out = block.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

pub fn resolve_include(base_file: &Path, include_path: &str) -> PathBuf {
    let include_path = parse_include_target(include_path);
    let expanded = shellexpand::tilde(include_path).to_string();
    let raw = PathBuf::from(expanded);
    if raw.is_absolute() {
        raw
    } else {
        base_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(raw)
    }
}

fn parse_include_target(include_path: &str) -> &str {
    include_path.trim().trim_matches('"')
}

fn parse_map_entry(
    raw: &str,
    source_file: &Path,
    line_no: usize,
    allow_commented: bool,
) -> Option<MapEntry> {
    let mut trimmed = raw.trim_start();
    if allow_commented {
        trimmed = trimmed.strip_prefix('#')?.trim_start();
    }
    let body = trimmed.strip_prefix("map ")?.trim();
    let (mode, shortcut, action, option_prefix) = parse_map_body(body)?;
    Some(MapEntry {
        mode,
        shortcut,
        action,
        option_prefix,
        source_file: source_file.to_path_buf(),
        line_no,
    })
}

fn parse_map_body(body: &str) -> Option<(String, String, String, String)> {
    let mut tokens = body.split_whitespace();
    let mut mode = String::from("main");
    let mut option_tokens = Vec::new();

    let shortcut = loop {
        let token = tokens.next()?;
        if token.starts_with("--") {
            if let Some(value) = token.strip_prefix("--mode=") {
                mode = if value.is_empty() {
                    String::from("main")
                } else {
                    value.to_string()
                };
            }
            option_tokens.push(token.to_string());
            continue;
        }
        break token.to_string();
    };

    let action = tokens.collect::<Vec<_>>().join(" ");
    Some((mode, shortcut, action, option_tokens.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reference_sections_from_commented_defaults() {
        let text = "#: Fonts {{{\n#: Font size in points\n# font_size 11.0\n";
        let items = parse_reference_str(text);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, "Fonts");
        assert_eq!(items[0].key, "font_size");
        assert_eq!(items[0].default_value.as_deref(), Some("11.0"));
    }

    #[test]
    fn captures_enum_choices() {
        let text = "#: Cursor {{{\n#: can be one of:\n#: block\n#: beam\n# cursor_shape block\n";
        let items = parse_reference_str(text);
        assert_eq!(
            items[0].enum_choices,
            vec!["beam".to_string(), "block".to_string()]
        );
    }

    #[test]
    fn captures_trailing_enum_choices() {
        let text = "#: Tabs {{{\n# tab_bar_style fade\n#: The tab bar style, can be one of:\n#: fade\n#: slant\n#: hidden\n";
        let items = parse_reference_str(text);
        assert_eq!(
            items[0].enum_choices,
            vec![
                "custom".to_string(),
                "fade".to_string(),
                "hidden".to_string(),
                "powerline".to_string(),
                "separator".to_string(),
                "slant".to_string()
            ]
        );
    }

    #[test]
    fn captures_trailing_value_lists() {
        let text = "#: Advanced {{{\n# allow_remote_control no\n#: The meaning of the various values are:\n#: password\n#: socket-only\n#: socket\n#: no\n#: yes\n";
        let items = parse_reference_str(text);
        assert_eq!(
            items[0].enum_choices,
            vec![
                "no".to_string(),
                "password".to_string(),
                "socket".to_string(),
                "socket-only".to_string(),
                "yes".to_string()
            ]
        );
    }

    #[test]
    fn captures_prose_enum_values() {
        let text = "#: Tabs {{{\n# tab_switch_strategy previous\n#: The default of previous will switch to the last used tab. A value of left will switch left. A value of right will switch right. A value of last will switch last.\n";
        let items = parse_reference_str(text);
        assert_eq!(
            items[0].enum_choices,
            vec![
                "last".to_string(),
                "left".to_string(),
                "previous".to_string(),
                "right".to_string()
            ]
        );
    }

    #[test]
    fn action_alias_does_not_inherit_previous_setting_comment() {
        let text = "\
#: Keyboard shortcuts {{{
# clear_all_shortcuts no
#: Remove all shortcut definitions up to this point.
#
# action_alias
#: Define action aliases to avoid repeating the same options.
";
        let items = parse_reference_str(text);
        let action_alias = items
            .iter()
            .find(|item| item.key == "action_alias")
            .expect("action_alias metadata");
        assert!(action_alias.description.contains("Define action aliases"));
        assert!(!action_alias
            .description
            .contains("Remove all shortcut definitions"));
    }

    #[test]
    fn parses_current_keymaps_with_modes_and_unmaps() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("kitty.conf");
        fs::write(&config, "map --mode=resize ctrl+t\nmap ctrl+n new_window\n")
            .expect("write config");

        let current = parse_current_config(&config).expect("parse current config");
        assert_eq!(current.keymaps.len(), 2);
        assert_eq!(current.keymaps[0].mode, "resize");
        assert_eq!(current.keymaps[0].shortcut, "ctrl+t");
        assert!(current.keymaps[0].action.is_empty());
        assert_eq!(current.keymaps[0].option_prefix, "--mode=resize");
        assert_eq!(current.keymaps[1].mode, "main");
        assert_eq!(current.keymaps[1].shortcut, "ctrl+n");
        assert_eq!(current.keymaps[1].action, "new_window");
    }

    #[test]
    fn parses_reference_keymaps_from_commented_defaults() {
        let keymaps =
            parse_reference_keymaps_str("# map kitty_mod+c copy\n", Path::new("kitty.conf"));
        assert_eq!(keymaps.len(), 1);
        assert_eq!(keymaps[0].mode, "main");
        assert_eq!(keymaps[0].shortcut, "kitty_mod+c");
        assert_eq!(keymaps[0].action, "copy");
    }

    #[test]
    fn resolves_quoted_include_paths() {
        let resolved = resolve_include(
            Path::new("/tmp/kitty/kitty.conf"),
            "\"themes/gruvbox.conf\"",
        );
        assert_eq!(resolved, PathBuf::from("/tmp/kitty/themes/gruvbox.conf"));
    }
}
