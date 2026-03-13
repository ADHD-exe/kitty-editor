use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::config_model::{DiffKind, DiffLine, SettingMetadata, ValidationState};

pub fn render_setting_line(
    meta: &SettingMetadata,
    value: Option<&str>,
    changed: bool,
    validation: &ValidationState,
) -> Line<'static> {
    let mut spans = vec![Span::styled(
        meta.key.clone(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    if let Some(value) = value {
        spans.push(Span::raw(" = "));
        spans.extend(colorized_spans(value, Style::default().fg(Color::Yellow)));
    }
    if changed {
        spans.push(Span::styled(
            "  ●",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if matches!(validation, ValidationState::Invalid(_)) {
        spans.push(Span::styled(
            "  !",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

pub fn render_diff_line(line: &DiffLine) -> Line<'static> {
    match line.kind {
        DiffKind::Same => Line::from(colorized_prefixed_spans("  ", &line.text, Style::default())),
        DiffKind::Add => Line::from(colorized_prefixed_spans(
            "+ ",
            &line.text,
            Style::default().fg(Color::Green),
        )),
        DiffKind::Remove => Line::from(colorized_prefixed_spans(
            "- ",
            &line.text,
            Style::default().fg(Color::Red),
        )),
    }
}

pub fn render_theme_preview_line(line: &str) -> Line<'static> {
    Line::from(colorized_spans(line, Style::default()))
}

pub fn colorized_spans(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut idx = 0;

    while idx < text.len() {
        if let Some((token_len, color)) = parse_color_token(&text[idx..]) {
            push_plain_span(&mut spans, &mut plain, base_style);
            let token = &text[idx..idx + token_len];
            spans.push(Span::styled(
                token.to_string(),
                color_token_style(base_style, color),
            ));
            idx += token_len;
            continue;
        }

        let ch = text[idx..]
            .chars()
            .next()
            .expect("valid utf-8 boundary while scanning preview line");
        plain.push(ch);
        idx += ch.len_utf8();
    }

    push_plain_span(&mut spans, &mut plain, base_style);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base_style));
    }
    spans
}

pub fn validation_style(validation: &ValidationState) -> Style {
    match validation {
        ValidationState::Valid => Style::default().fg(Color::Green),
        ValidationState::Invalid(_) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ValidationState::Unknown => Style::default().fg(Color::DarkGray),
    }
}

fn parse_color_token(input: &str) -> Option<(usize, Color)> {
    parse_hex_color_token(input).or_else(|| parse_rgb_color_token(input))
}

fn colorized_prefixed_spans(prefix: &str, text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(prefix.to_string(), base_style)];
    spans.extend(colorized_spans(text, base_style));
    spans
}

fn push_plain_span(spans: &mut Vec<Span<'static>>, plain: &mut String, base_style: Style) {
    if !plain.is_empty() {
        spans.push(Span::styled(std::mem::take(plain), base_style));
    }
}

fn color_token_style(base_style: Style, color: Color) -> Style {
    base_style
        .fg(contrast_color(color))
        .bg(color)
        .add_modifier(Modifier::BOLD)
}

fn contrast_color(color: Color) -> Color {
    match color {
        Color::Rgb(red, green, blue) => {
            let luminance = (red as u32 * 299 + green as u32 * 587 + blue as u32 * 114) / 1000;
            if luminance >= 140 {
                Color::Black
            } else {
                Color::White
            }
        }
        _ => Color::White,
    }
}

fn parse_hex_color_token(input: &str) -> Option<(usize, Color)> {
    let bytes = input.as_bytes();
    if bytes.first().copied() != Some(b'#') {
        return None;
    }

    for len in [7usize, 4usize] {
        if bytes.len() < len {
            continue;
        }
        let candidate = &input[..len];
        if !candidate[1..].bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        if bytes.get(len).is_some_and(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        if let Some(color) = color_from_hex(candidate) {
            return Some((len, color));
        }
    }

    None
}

fn parse_rgb_color_token(input: &str) -> Option<(usize, Color)> {
    const RGB_LEN: usize = 12;
    if input.len() < RGB_LEN || !input.starts_with("rgb:") {
        return None;
    }

    let candidate = &input[..RGB_LEN];
    let mut parts = candidate["rgb:".len()..].split('/');
    let red = parts.next().and_then(parse_hex_byte)?;
    let green = parts.next().and_then(parse_hex_byte)?;
    let blue = parts.next().and_then(parse_hex_byte)?;
    if parts.next().is_some() {
        return None;
    }
    if input
        .as_bytes()
        .get(RGB_LEN)
        .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'/')
    {
        return None;
    }

    Some((RGB_LEN, Color::Rgb(red, green, blue)))
}

fn color_from_hex(token: &str) -> Option<Color> {
    let hex = token.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let mut chars = hex.chars();
            let red = expand_hex_nibble(chars.next()?)?;
            let green = expand_hex_nibble(chars.next()?)?;
            let blue = expand_hex_nibble(chars.next()?)?;
            Some(Color::Rgb(red, green, blue))
        }
        6 => {
            let red = parse_hex_byte(&hex[0..2])?;
            let green = parse_hex_byte(&hex[2..4])?;
            let blue = parse_hex_byte(&hex[4..6])?;
            Some(Color::Rgb(red, green, blue))
        }
        _ => None,
    }
}

fn expand_hex_nibble(ch: char) -> Option<u8> {
    let value = ch.to_digit(16)? as u8;
    Some(value * 17)
}

fn parse_hex_byte(value: &str) -> Option<u8> {
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u8::from_str_radix(value, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_hex_tokens_in_theme_preview_lines() {
        let line = render_theme_preview_line("foreground #ebdbb2");

        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[1].content, "#ebdbb2");
        assert_eq!(line.spans[1].style.bg, Some(Color::Rgb(0xeb, 0xdb, 0xb2)));
        assert_eq!(line.spans[1].style.fg, Some(Color::Black));
    }

    #[test]
    fn colors_short_hex_and_rgb_tokens() {
        let short_hex = render_theme_preview_line("cursor #abc");
        let rgb = render_theme_preview_line("selection_background rgb:11/22/33");

        assert_eq!(
            short_hex.spans[1].style.bg,
            Some(Color::Rgb(0xaa, 0xbb, 0xcc))
        );
        assert_eq!(short_hex.spans[1].style.fg, Some(Color::Black));
        assert_eq!(rgb.spans[1].style.bg, Some(Color::Rgb(0x11, 0x22, 0x33)));
        assert_eq!(rgb.spans[1].style.fg, Some(Color::White));
    }

    #[test]
    fn colors_setting_values_without_losing_base_style() {
        let spans = colorized_spans("x #ebdbb2 y", Style::default().fg(Color::Yellow));

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].style.fg, Some(Color::Yellow));
        assert_eq!(spans[1].content, "#ebdbb2");
        assert_eq!(spans[1].style.bg, Some(Color::Rgb(0xeb, 0xdb, 0xb2)));
        assert_eq!(spans[1].style.fg, Some(Color::Black));
        assert_eq!(spans[2].style.fg, Some(Color::Yellow));
    }
}
