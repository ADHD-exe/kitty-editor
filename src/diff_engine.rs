use crate::config_model::{DiffKind, DiffLine};

pub fn simple_diff(old_text: &str, new_text: &str) -> Vec<DiffLine> {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let max_len = old_lines.len().max(new_lines.len());
    let mut out = Vec::new();
    for idx in 0..max_len {
        match (old_lines.get(idx), new_lines.get(idx)) {
            (Some(a), Some(b)) if a == b => out.push(DiffLine {
                kind: DiffKind::Same,
                text: (*a).to_string(),
            }),
            (Some(a), Some(b)) => {
                out.push(DiffLine {
                    kind: DiffKind::Remove,
                    text: (*a).to_string(),
                });
                out.push(DiffLine {
                    kind: DiffKind::Add,
                    text: (*b).to_string(),
                });
            }
            (Some(a), None) => out.push(DiffLine {
                kind: DiffKind::Remove,
                text: (*a).to_string(),
            }),
            (None, Some(b)) => out.push(DiffLine {
                kind: DiffKind::Add,
                text: (*b).to_string(),
            }),
            (None, None) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_marks_changes() {
        let lines = simple_diff("a\nb", "a\nc");
        assert!(lines
            .iter()
            .any(|l| matches!(l.kind, DiffKind::Add) && l.text == "c"));
    }
}
