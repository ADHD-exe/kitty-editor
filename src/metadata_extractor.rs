use crate::config_model::{SettingMetadata, ValueType};
use crate::validator::infer_value_type;

pub fn finalize_metadata(mut items: Vec<SettingMetadata>) -> Vec<SettingMetadata> {
    for item in &mut items {
        item.description = item.description.trim().to_string();
        item.examples.retain(|v| !v.trim().is_empty());
        item.enum_choices.retain(|v| !v.trim().is_empty());
        item.enum_choices.sort();
        item.enum_choices.dedup();
        apply_kitty_metadata_hints(item);

        if matches!(item.value_type, ValueType::String) {
            item.value_type = infer_value_type(
                &item.key,
                item.default_value.as_deref(),
                &item.description,
                &item.enum_choices,
            );
        }

        if !item.repeatable {
            let lower = item.description.to_lowercase();
            item.repeatable = lower.contains("can be specified multiple times")
                || lower.contains("may be specified multiple times")
                || lower.contains("repeatable");
        }
    }

    items.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.key.cmp(&b.key)));
    items
}

fn apply_kitty_metadata_hints(item: &mut SettingMetadata) {
    let hints: &[&str] = match item.key.as_str() {
        "allow_cloning" => &["ask", "no", "yes"],
        "allow_hyperlinks" => &["ask", "no", "yes"],
        "allow_remote_control" => &["no", "password", "socket", "socket-only", "yes"],
        "scrollbar" => &[
            "always",
            "hovered",
            "never",
            "scrolled",
            "scrolled-and-hovered",
        ],
        "tab_bar_align" => &["center", "left", "right"],
        "tab_bar_style" => &[
            "custom",
            "fade",
            "hidden",
            "powerline",
            "separator",
            "slant",
        ],
        "tab_powerline_style" => &["angled", "round", "slanted"],
        "tab_switch_strategy" => &["last", "left", "previous", "right"],
        _ => &[],
    };

    item.enum_choices
        .extend(hints.iter().map(|choice| (*choice).to_string()));
    item.enum_choices.sort();
    item.enum_choices.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str) -> SettingMetadata {
        SettingMetadata {
            key: key.into(),
            category: "Test".into(),
            description: String::new(),
            default_value: None,
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::String,
            order: 0,
        }
    }

    #[test]
    fn applies_kitty_enum_hints() {
        let items = finalize_metadata(vec![
            item("allow_cloning"),
            item("scrollbar"),
            item("tab_bar_style"),
        ]);
        assert_eq!(
            items[0].enum_choices,
            vec!["ask".to_string(), "no".to_string(), "yes".to_string()]
        );
        assert_eq!(
            items[1].enum_choices,
            vec![
                "always".to_string(),
                "hovered".to_string(),
                "never".to_string(),
                "scrolled".to_string(),
                "scrolled-and-hovered".to_string()
            ]
        );
        assert!(matches!(items[2].value_type, ValueType::Enum));
    }
}
