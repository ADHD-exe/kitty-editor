use crate::config_model::SettingMetadata;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub index: usize,
    pub score: i64,
    #[allow(dead_code)]
    pub positions: Vec<usize>,
}

pub fn search(items: &[SettingMetadata], query: &str) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..items.len()).collect();
    }
    let mut scored: Vec<MatchResult> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            score_item(item, query).map(|mut m| {
                m.index = index;
                m
            })
        })
        .collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.index.cmp(&b.index)));
    scored.into_iter().map(|m| m.index).collect()
}

pub fn score_item(item: &SettingMetadata, query: &str) -> Option<MatchResult> {
    let haystacks = [
        format!("{}", item.key),
        format!("{} {}", item.category, item.key),
        format!("{} {} {}", item.key, item.category, item.description),
    ];
    haystacks
        .iter()
        .filter_map(|s| score_text(s, query))
        .max_by(|a, b| a.score.cmp(&b.score))
}

fn score_text(text: &str, query: &str) -> Option<MatchResult> {
    let lower = text.to_lowercase();
    let query = query.to_lowercase();
    let mut score = 0i64;
    let mut positions = Vec::new();
    let mut search_start = 0usize;

    for ch in query.chars() {
        if let Some(pos) = lower[search_start..].find(ch) {
            let absolute = search_start + pos;
            positions.push(absolute);
            score += 10;
            if absolute == 0
                || lower.as_bytes().get(absolute.wrapping_sub(1)) == Some(&b'_')
                || lower.as_bytes().get(absolute.wrapping_sub(1)) == Some(&b' ')
            {
                score += 15;
            }
            if let Some(prev) = positions.iter().rev().nth(1) {
                if absolute == *prev + 1 {
                    score += 20;
                }
            }
            search_start = absolute + 1;
        } else {
            return None;
        }
    }
    score += (100 - lower.len().min(100)) as i64;
    Some(MatchResult {
        index: 0,
        score,
        positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_model::{SettingMetadata, ValueType};

    fn item(key: &str, category: &str, description: &str) -> SettingMetadata {
        SettingMetadata {
            key: key.into(),
            category: category.into(),
            description: description.into(),
            default_value: None,
            examples: vec![],
            enum_choices: vec![],
            repeatable: false,
            value_type: ValueType::String,
            order: 0,
        }
    }

    #[test]
    fn fuzzy_prefers_direct_key_match() {
        let items = vec![
            item("font_size", "Fonts", ""),
            item("background", "Colors", ""),
        ];
        let results = search(&items, "fsize");
        assert_eq!(results[0], 0);
    }
}
