use super::super::{DisplayMode, FormFieldState, SubmittedInput};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub(super) fn render_raw_input_lines(input: &str) -> Vec<Line<'static>> {
    if input.trim().is_empty() {
        return vec![Line::from(vec![Span::styled(
            "{\"key\":\"value\"}",
            Style::default().fg(Color::Rgb(135, 146, 165)),
        )])];
    }

    if serde_json::from_str::<serde_json::Value>(input).is_ok() {
        return input.split('\n').map(style_json_line).collect();
    }

    input
        .split('\n')
        .map(|line| {
            Line::from(vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Rgb(230, 232, 235)),
            )])
        })
        .collect()
}

pub(super) fn render_submitted_input_lines(input: &SubmittedInput) -> Vec<Line<'static>> {
    match input {
        SubmittedInput::Raw(text) => render_raw_input_lines(text),
        SubmittedInput::Form {
            fields,
            selected_index,
        } => render_form_lines(fields, Some(*selected_index)),
    }
}

pub(super) fn render_form_lines(
    fields: &[FormFieldState],
    selected_index: Option<usize>,
) -> Vec<Line<'static>> {
    if fields.is_empty() {
        return vec![Line::from("No form fields available.")];
    }

    let mut lines = Vec::new();
    let required_count = fields.iter().filter(|f| f.required).count();
    if required_count > 0 {
        lines.push(Line::from(Span::styled(
            "[required]",
            Style::default().fg(Color::Rgb(245, 165, 36)),
        )));
        for (i, field) in fields.iter().enumerate().filter(|(_, f)| f.required) {
            lines.push(form_field_line(field, selected_index == Some(i)));
        }
    }

    if required_count < fields.len() {
        lines.push(Line::from(Span::styled(
            "[optional]",
            Style::default().fg(Color::Rgb(135, 146, 165)),
        )));
        for (i, field) in fields.iter().enumerate().filter(|(_, f)| !f.required) {
            lines.push(form_field_line(field, selected_index == Some(i)));
        }
    }

    lines
}

pub(super) fn render_answer_lines(answer: &str, display_mode: DisplayMode) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for line in answer.lines() {
        match display_mode {
            DisplayMode::PrettyYaml => lines.push(style_yaml_line(line)),
            DisplayMode::PrettyJson | DisplayMode::RawJson => {
                lines.push(style_json_output_line(line))
            }
            DisplayMode::QaCompact => lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::Rgb(226, 226, 226)),
            )])),
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

fn form_field_line(field: &FormFieldState, selected: bool) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    Line::from(vec![
        Span::styled(marker, Style::default().fg(Color::Rgb(245, 165, 36))),
        Span::raw(" "),
        Span::styled(
            format!("{} [{}]", field.name, field.kind),
            Style::default().fg(Color::Rgb(56, 189, 248)),
        ),
        Span::raw(": "),
        Span::styled(
            field.value.clone(),
            Style::default().fg(Color::Rgb(230, 232, 235)),
        ),
    ])
}

fn style_json_line(line: &str) -> Line<'static> {
    let key_style = Style::default().fg(Color::Rgb(248, 113, 113));
    let number_style = Style::default().fg(Color::Rgb(230, 232, 235));
    let value_style = Style::default().fg(Color::Rgb(74, 222, 128));
    let punct_style = Style::default().fg(Color::Rgb(135, 146, 165));

    let indent_len = line.len().saturating_sub(line.trim_start().len());
    let indent = &line[..indent_len];
    let rest = &line[indent_len..];
    let mut spans = vec![Span::raw(indent.to_string())];

    if let Some(colon_pos) = rest.find(':')
        && rest.trim_start().starts_with('"')
    {
        let key = &rest[..colon_pos];
        spans.push(Span::styled(key.to_string(), key_style));
        spans.push(Span::styled(":", punct_style));

        let after_colon = &rest[colon_pos + 1..];
        let leading_ws_len = after_colon
            .len()
            .saturating_sub(after_colon.trim_start().len());
        let leading_ws = &after_colon[..leading_ws_len];
        spans.push(Span::raw(leading_ws.to_string()));

        let trimmed = after_colon.trim_start();
        let (value_token, trailing_comma) = if let Some(prefix) = trimmed.strip_suffix(',') {
            (prefix.trim_end(), true)
        } else {
            (trimmed, false)
        };

        if !value_token.is_empty() {
            let style = if serde_json::from_str::<serde_json::Value>(value_token)
                .map(|v| v.is_number())
                .unwrap_or(false)
            {
                number_style
            } else {
                value_style
            };
            spans.push(Span::styled(value_token.to_string(), style));
        }

        if trailing_comma {
            spans.push(Span::styled(",", punct_style));
        }
        return Line::from(spans);
    }

    let trimmed = rest.trim();
    if trimmed == "{"
        || trimmed == "}"
        || trimmed == "["
        || trimmed == "]"
        || trimmed == "},"
        || trimmed == "],"
    {
        spans.push(Span::styled(trimmed.to_string(), punct_style));
        return Line::from(spans);
    }

    spans.push(Span::styled(rest.to_string(), value_style));
    Line::from(spans)
}

fn style_json_output_line(line: &str) -> Line<'static> {
    let key_style = Style::default().fg(Color::Rgb(248, 113, 113));
    let string_style = Style::default().fg(Color::Rgb(74, 222, 128));
    let number_style = Style::default().fg(Color::Rgb(209, 154, 102));
    let bool_style = Style::default().fg(Color::Rgb(198, 120, 221));
    let punct_style = Style::default().fg(Color::Rgb(135, 146, 165));
    let default_style = Style::default().fg(Color::Rgb(226, 226, 226));

    if line.is_empty() {
        return Line::from("");
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut i = 0usize;

    while i < line.len() {
        let ch = next_char_at(line, i);

        if ch.is_whitespace() {
            let start = i;
            while i < line.len() && next_char_at(line, i).is_whitespace() {
                i += next_char_at(line, i).len_utf8();
            }
            spans.push(Span::raw(line[start..i].to_string()));
            continue;
        }

        if ch == '"' {
            let start = i;
            i += ch.len_utf8();
            let mut escaped = false;
            while i < line.len() {
                let c = next_char_at(line, i);
                i += c.len_utf8();
                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' {
                    escaped = true;
                    continue;
                }
                if c == '"' {
                    break;
                }
            }

            let token = &line[start..i.min(line.len())];
            let after = skip_json_whitespace(line, i);
            let style = if line[after..].starts_with(':') {
                key_style
            } else {
                string_style
            };
            spans.push(Span::styled(token.to_string(), style));
            continue;
        }

        if matches!(ch, '{' | '}' | '[' | ']' | ':' | ',') {
            spans.push(Span::styled(ch.to_string(), punct_style));
            i += ch.len_utf8();
            continue;
        }

        let start = i;
        while i < line.len() {
            let c = next_char_at(line, i);
            if c.is_whitespace() || matches!(c, '{' | '}' | '[' | ']' | ':' | ',' | '"') {
                break;
            }
            i += c.len_utf8();
        }

        let token = &line[start..i];
        let style = if token == "true" || token == "false" || token == "null" {
            bool_style
        } else if token.parse::<i64>().is_ok() || token.parse::<f64>().is_ok() {
            number_style
        } else {
            default_style
        };
        spans.push(Span::styled(token.to_string(), style));
    }

    Line::from(spans)
}

fn next_char_at(s: &str, idx: usize) -> char {
    s[idx..].chars().next().unwrap_or('\0')
}

fn skip_json_whitespace(s: &str, mut idx: usize) -> usize {
    while idx < s.len() {
        let c = next_char_at(s, idx);
        if !c.is_whitespace() {
            break;
        }
        idx += c.len_utf8();
    }
    idx
}

fn style_yaml_line(line: &str) -> Line<'static> {
    let key_style = Style::default().fg(Color::Rgb(97, 175, 239));
    let string_style = Style::default().fg(Color::Rgb(152, 195, 121));
    let number_style = Style::default().fg(Color::Rgb(209, 154, 102));
    let bool_style = Style::default().fg(Color::Rgb(198, 120, 221));
    let punct_style = Style::default().fg(Color::Rgb(135, 146, 165));
    let comment_style = Style::default().fg(Color::Rgb(92, 99, 112));

    if line.trim().is_empty() {
        return Line::from("");
    }

    let indent_len = line.len().saturating_sub(line.trim_start().len());
    let indent = &line[..indent_len];
    let rest = &line[indent_len..];
    let mut spans = vec![Span::raw(indent.to_string())];

    if rest.trim_start().starts_with('#') {
        spans.push(Span::styled(rest.to_string(), comment_style));
        return Line::from(spans);
    }

    let (prefix, body) = if let Some(after_dash) = rest.strip_prefix("- ") {
        (Some("- "), after_dash)
    } else {
        (None, rest)
    };

    if let Some(list_marker) = prefix {
        spans.push(Span::styled(list_marker.to_string(), punct_style));
    }

    if let Some(colon_idx) = body.find(':') {
        let key = &body[..colon_idx];
        let after_colon = &body[colon_idx + 1..];
        if !key.trim().is_empty()
            && (after_colon.is_empty()
                || after_colon.starts_with(' ')
                || after_colon.starts_with('\t'))
        {
            spans.push(Span::styled(key.to_string(), key_style));
            spans.push(Span::styled(":", punct_style));

            let leading_ws_len = after_colon
                .len()
                .saturating_sub(after_colon.trim_start().len());
            let leading_ws = &after_colon[..leading_ws_len];
            if !leading_ws.is_empty() {
                spans.push(Span::raw(leading_ws.to_string()));
            }

            let value = after_colon.trim_start();
            if !value.is_empty() {
                spans.push(Span::styled(
                    value.to_string(),
                    yaml_value_style(value, string_style, number_style, bool_style, punct_style),
                ));
            }
            return Line::from(spans);
        }
    }

    spans.push(Span::styled(
        body.to_string(),
        yaml_value_style(body, string_style, number_style, bool_style, punct_style),
    ));
    Line::from(spans)
}

fn yaml_value_style(
    value: &str,
    string_style: Style,
    number_style: Style,
    bool_style: Style,
    punct_style: Style,
) -> Style {
    let token = value.trim();
    if token.is_empty() {
        return punct_style;
    }
    if token.starts_with('#') {
        return Style::default().fg(Color::Rgb(92, 99, 112));
    }

    let no_trailing = token.trim_end_matches(',');
    if no_trailing == "true"
        || no_trailing == "false"
        || no_trailing == "null"
        || no_trailing == "~"
    {
        return bool_style;
    }

    if no_trailing.parse::<i64>().is_ok() || no_trailing.parse::<f64>().is_ok() {
        return number_style;
    }

    if no_trailing.starts_with('[')
        || no_trailing.starts_with('{')
        || no_trailing == "|"
        || no_trailing == ">"
    {
        return punct_style;
    }

    string_style
}
