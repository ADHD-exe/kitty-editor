use std::collections::BTreeMap;

use crate::config_model::{KeymapField, MapEntry, ShortcutRow, ShortcutStatus, ShortcutView};

pub fn validate_map_entry(shortcut: &str, action: &str) -> Result<(), String> {
    let shortcut = shortcut.trim();

    if shortcut.is_empty() {
        return Err("shortcut cannot be empty".into());
    }
    if shortcut.lines().count() != 1 || action.lines().count() > 1 {
        return Err("keybindings must stay on a single line".into());
    }
    if shortcut.split_whitespace().count() != 1 {
        return Err("shortcut should be a single token".into());
    }
    if shortcut.starts_with("map ") {
        return Err("enter only the shortcut token, not the full map command".into());
    }
    Ok(())
}

pub fn update_map_field(entry: &mut MapEntry, field: KeymapField, value: String) {
    match field {
        KeymapField::Shortcut => entry.shortcut = value.trim().to_string(),
        KeymapField::Action => entry.action = value.trim().to_string(),
    }
}

pub fn map_field_value(entry: &MapEntry, field: KeymapField) -> String {
    match field {
        KeymapField::Shortcut => entry.shortcut.clone(),
        KeymapField::Action => entry.action.clone(),
    }
}

pub fn render_map(entry: &MapEntry) -> String {
    let shortcut = entry.shortcut.trim();
    let action = entry.action.trim();
    let prefix = entry.option_prefix.trim();

    let mut line = String::from("map ");
    if !prefix.is_empty() {
        line.push_str(prefix);
        line.push(' ');
    }
    line.push_str(shortcut);
    if !action.is_empty() {
        line.push(' ');
        line.push_str(action);
    }
    line
}

pub fn display_shortcut(shortcut: &str, kitty_mod: &str) -> String {
    shortcut.trim().replace("kitty_mod", kitty_mod)
}

pub fn display_action(action: &str) -> String {
    let trimmed = action.trim();
    if trimmed.is_empty() {
        String::from("<removed>")
    } else {
        trimmed.to_string()
    }
}

pub fn build_shortcut_rows(
    default_keymaps: &[MapEntry],
    edited_keymaps: &[MapEntry],
    kitty_mod: &str,
    view: ShortcutView,
) -> Vec<ShortcutRow> {
    let default_map = default_keymaps
        .iter()
        .map(|entry| (shortcut_identity(entry, kitty_mod), entry))
        .collect::<BTreeMap<_, _>>();

    let mut default_rows = default_map
        .iter()
        .map(|(_key, entry)| {
            shortcut_row(
                entry,
                None,
                ShortcutStatus::Default,
                String::new(),
                kitty_mod,
            )
        })
        .collect::<Vec<_>>();
    default_rows.sort_by(row_sort_key);

    let mut effective_rows = default_map
        .iter()
        .map(|(key, entry)| {
            (
                key.clone(),
                shortcut_row(
                    entry,
                    None,
                    ShortcutStatus::Default,
                    String::new(),
                    kitty_mod,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut custom_rows = Vec::new();

    for (idx, entry) in edited_keymaps.iter().enumerate() {
        let key = shortcut_identity(entry, kitty_mod);
        let default_entry = default_map.get(&key).copied();
        let (status, detail) = classify_custom_entry(entry, default_entry);
        let row = shortcut_row(entry, Some(idx), status, detail, kitty_mod);

        if status != ShortcutStatus::Default {
            custom_rows.push(row.clone());
        }
        if entry.action.trim().is_empty() {
            effective_rows.remove(&key);
        } else {
            effective_rows.insert(key, row);
        }
    }

    let mut effective_rows = effective_rows.into_values().collect::<Vec<_>>();
    effective_rows.sort_by(row_sort_key);
    custom_rows.sort_by(custom_row_sort_key);

    match view {
        ShortcutView::Custom => custom_rows,
        ShortcutView::Effective => effective_rows,
        ShortcutView::Defaults => default_rows,
    }
}

pub fn filter_shortcut_rows(rows: &[ShortcutRow], query: &str) -> Vec<ShortcutRow> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return rows.to_vec();
    }

    rows.iter()
        .filter(|row| {
            row.shortcut.to_ascii_lowercase().contains(&needle)
                || display_action(&row.action)
                    .to_ascii_lowercase()
                    .contains(&needle)
                || row.option_prefix.to_ascii_lowercase().contains(&needle)
                || row.status.label().contains(&needle)
                || row.detail.to_ascii_lowercase().contains(&needle)
                || row.mode.to_ascii_lowercase().contains(&needle)
        })
        .cloned()
        .collect()
}

pub fn shortcut_status_counts(rows: &[ShortcutRow]) -> (usize, usize, usize) {
    rows.iter()
        .fold((0, 0, 0), |(added, changed, removed), row| {
            match row.status {
                ShortcutStatus::Added => (added + 1, changed, removed),
                ShortcutStatus::Changed => (added, changed + 1, removed),
                ShortcutStatus::Removed => (added, changed, removed + 1),
                ShortcutStatus::Default => (added, changed, removed),
            }
        })
}

fn shortcut_row(
    entry: &MapEntry,
    edited_index: Option<usize>,
    status: ShortcutStatus,
    detail: String,
    kitty_mod: &str,
) -> ShortcutRow {
    ShortcutRow {
        mode: entry.mode.clone(),
        shortcut: display_shortcut(&entry.shortcut, kitty_mod),
        raw_shortcut: entry.shortcut.clone(),
        action: entry.action.clone(),
        status,
        detail,
        source_file: entry.source_file.clone(),
        line_no: entry.line_no,
        option_prefix: entry.option_prefix.clone(),
        edited_index,
    }
}

fn classify_custom_entry(
    entry: &MapEntry,
    default_entry: Option<&MapEntry>,
) -> (ShortcutStatus, String) {
    match default_entry {
        Some(default_entry) if entry.action.trim().is_empty() => (
            ShortcutStatus::Removed,
            format!("default: {}", display_action(&default_entry.action)),
        ),
        Some(default_entry) if same_mapping(entry, default_entry) => (
            ShortcutStatus::Default,
            String::from("matches built-in default"),
        ),
        Some(default_entry) => (
            ShortcutStatus::Changed,
            format!("default: {}", display_action(&default_entry.action)),
        ),
        None if entry.action.trim().is_empty() => {
            (ShortcutStatus::Removed, String::from("explicit unmap"))
        }
        None => (ShortcutStatus::Added, String::from("only in your config")),
    }
}

fn shortcut_identity(entry: &MapEntry, kitty_mod: &str) -> (String, String, String) {
    (
        entry.mode.clone(),
        normalize_option_prefix(&entry.option_prefix),
        display_shortcut(&entry.shortcut, kitty_mod),
    )
}

fn same_mapping(left: &MapEntry, right: &MapEntry) -> bool {
    left.mode == right.mode
        && left.shortcut.trim() == right.shortcut.trim()
        && left.action.trim() == right.action.trim()
        && normalize_option_prefix(&left.option_prefix)
            == normalize_option_prefix(&right.option_prefix)
}

fn row_sort_key(left: &ShortcutRow, right: &ShortcutRow) -> std::cmp::Ordering {
    left.mode
        .cmp(&right.mode)
        .then_with(|| left.option_prefix.cmp(&right.option_prefix))
        .then_with(|| left.shortcut.cmp(&right.shortcut))
        .then_with(|| left.action.cmp(&right.action))
}

fn custom_row_sort_key(left: &ShortcutRow, right: &ShortcutRow) -> std::cmp::Ordering {
    status_sort_key(left.status)
        .cmp(&status_sort_key(right.status))
        .then_with(|| row_sort_key(left, right))
}

fn status_sort_key(status: ShortcutStatus) -> usize {
    match status {
        ShortcutStatus::Changed => 0,
        ShortcutStatus::Added => 1,
        ShortcutStatus::Removed => 2,
        ShortcutStatus::Default => 3,
    }
}

fn normalize_option_prefix(prefix: &str) -> String {
    prefix.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn map_entry(mode: &str, shortcut: &str, action: &str, option_prefix: &str) -> MapEntry {
        MapEntry {
            mode: mode.into(),
            shortcut: shortcut.into(),
            action: action.into(),
            option_prefix: option_prefix.into(),
            source_file: PathBuf::from("kitty.conf"),
            line_no: 1,
        }
    }

    #[test]
    fn rejects_empty_shortcut() {
        assert!(validate_map_entry("", "new_window").is_err());
    }

    #[test]
    fn updates_action_field() {
        let mut entry = MapEntry {
            mode: "main".into(),
            shortcut: "ctrl+t".into(),
            action: "new_tab".into(),
            option_prefix: String::new(),
            source_file: PathBuf::from("kitty.conf"),
            line_no: 1,
        };
        update_map_field(&mut entry, KeymapField::Action, "new_window".into());
        assert_eq!(entry.action, "new_window");
    }

    #[test]
    fn rejects_multiline_actions() {
        assert!(validate_map_entry("ctrl+t", "new_tab\nlaunch").is_err());
    }

    #[test]
    fn allows_unmap_entries() {
        assert!(validate_map_entry("ctrl+t", "").is_ok());
    }

    #[test]
    fn renders_map_options_and_unmaps() {
        let entry = map_entry("resize", "ctrl+t", "", "--mode=resize");

        assert_eq!(render_map(&entry), "map --mode=resize ctrl+t");
    }

    #[test]
    fn custom_view_hides_explicit_mapping_that_matches_default() {
        let rows = build_shortcut_rows(
            &[map_entry("main", "kitty_mod+c", "copy_to_clipboard", "")],
            &[map_entry("main", "kitty_mod+c", "copy_to_clipboard", "")],
            "ctrl+shift",
            ShortcutView::Custom,
        );

        assert!(rows.is_empty());
    }

    #[test]
    fn effective_view_keeps_explicit_mapping_that_matches_default() {
        let rows = build_shortcut_rows(
            &[map_entry("main", "kitty_mod+c", "copy_to_clipboard", "")],
            &[map_entry("main", "kitty_mod+c", "copy_to_clipboard", "")],
            "ctrl+shift",
            ShortcutView::Effective,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, ShortcutStatus::Default);
        assert_eq!(rows[0].edited_index, Some(0));
    }

    #[test]
    fn effective_view_uses_last_custom_mapping_for_same_identity() {
        let rows = build_shortcut_rows(
            &[map_entry("main", "kitty_mod+t", "new_tab", "")],
            &[
                map_entry("main", "kitty_mod+t", "", ""),
                map_entry("main", "ctrl+shift+t", "new_window", ""),
            ],
            "ctrl+shift",
            ShortcutView::Effective,
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].shortcut, "ctrl+shift+t");
        assert_eq!(rows[0].action, "new_window");
        assert_eq!(rows[0].status, ShortcutStatus::Changed);
        assert_eq!(rows[0].edited_index, Some(1));
    }

    #[test]
    fn option_prefix_participates_in_shortcut_identity() {
        let rows = build_shortcut_rows(
            &[map_entry("main", "kitty_mod+t", "new_tab", "")],
            &[map_entry(
                "main",
                "kitty_mod+t",
                "resize_window wider",
                "--when-focus-on=var:vim",
            )],
            "ctrl+shift",
            ShortcutView::Effective,
        );

        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.action == "new_tab"));
        assert!(rows.iter().any(|row| row.action == "resize_window wider"));
    }
}
