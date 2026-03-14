use super::super::DisplayMode;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn append_answer_block(
    lines: &mut Vec<Line<'static>>,
    answer_lines: Vec<Line<'static>>,
    row_width: usize,
    display_mode: DisplayMode,
) {
    let inner_width = row_width.saturating_sub(4).max(1);
    lines.push(pad_line(Line::from(""), None, row_width));
    for line in answer_lines {
        let continuation_indent = if display_mode == DisplayMode::PrettyYaml {
            yaml_continuation_indent(&line)
        } else {
            0
        };
        for wrapped in
            wrap_line_to_width_with_hanging_indent(line, inner_width, continuation_indent)
        {
            lines.push(pad_line(wrapped, None, row_width));
        }
    }
    lines.push(pad_line(Line::from(""), None, row_width));
}

pub(super) fn append_fixed_width_input_block(
    lines: &mut Vec<Line<'static>>,
    block_lines: Vec<Line<'static>>,
    row_width: usize,
) {
    let bg = Color::Rgb(36, 41, 48);
    let side_margin = if row_width >= 4 { 2 } else { row_width / 2 };
    let inner_width = row_width
        .saturating_sub(side_margin.saturating_mul(2))
        .max(1);

    let mut wrapped_lines = Vec::new();
    for line in block_lines {
        wrapped_lines.extend(wrap_line_to_width(line, inner_width));
    }

    lines.push(Line::from(vec![Span::styled(
        " ".repeat(row_width.max(1)),
        Style::default().bg(bg),
    )]));

    for line in wrapped_lines {
        lines.push(pad_input_line(line, row_width, side_margin, bg));
    }

    lines.push(Line::from(vec![Span::styled(
        " ".repeat(row_width.max(1)),
        Style::default().bg(bg),
    )]));
}

pub(super) fn wrapped_line_count(lines: &[Line<'_>], width: usize) -> usize {
    if lines.is_empty() {
        return 0;
    }
    if width == 0 {
        return lines.len();
    }

    lines
        .iter()
        .map(|line| {
            let len: usize = line
                .spans
                .iter()
                .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                .sum();
            len.max(1).div_ceil(width)
        })
        .sum()
}

fn yaml_continuation_indent(line: &Line<'_>) -> usize {
    let plain = line_plain_text(line);
    let indent = plain.len().saturating_sub(plain.trim_start().len());
    let rest = &plain[indent..];
    let (list_offset, body) = if let Some(after_dash) = rest.strip_prefix("- ") {
        (2usize, after_dash)
    } else {
        (0usize, rest)
    };

    if let Some(colon_idx) = body.find(':') {
        let after_colon = &body[colon_idx + 1..];
        if after_colon.starts_with(' ') {
            return indent + list_offset + colon_idx + 2;
        }
    }

    indent + list_offset
}

fn line_plain_text(line: &Line<'_>) -> String {
    let mut out = String::new();
    for span in &line.spans {
        out.push_str(span.content.as_ref());
    }
    out
}

fn pad_input_line(
    line: Line<'static>,
    row_width: usize,
    side_margin: usize,
    background: Color,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len() + 3);
    spans.push(Span::styled(
        " ".repeat(side_margin),
        Style::default().bg(background),
    ));

    let mut current_width = side_margin;
    for span in line.spans {
        let content = span.content.to_string();
        current_width += UnicodeWidthStr::width(content.as_str());
        spans.push(Span::styled(content, span.style.bg(background)));
    }

    let right_margin_and_fill = row_width.saturating_sub(current_width);
    spans.push(Span::styled(
        " ".repeat(right_margin_and_fill),
        Style::default().bg(background),
    ));

    Line::from(spans)
}

fn pad_line(line: Line<'static>, background: Option<Color>, row_width: usize) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len() + 2);
    let side_style = match background {
        Some(bg) => Style::default().bg(bg),
        None => Style::default(),
    };
    spans.push(Span::styled("  ", side_style));
    for span in line.spans {
        let content = span.content.to_string();
        let style = match background {
            Some(bg) => span.style.bg(bg),
            None => span.style,
        };
        spans.push(Span::styled(content, style));
    }
    spans.push(Span::styled("  ", side_style));

    if let Some(bg) = background {
        let current_width: usize = spans
            .iter()
            .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
            .sum();
        if row_width > current_width {
            spans.push(Span::styled(
                " ".repeat(row_width - current_width),
                Style::default().bg(bg),
            ));
        }
    }

    Line::from(spans)
}

fn wrap_line_to_width(line: Line<'static>, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![line];
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_len = 0usize;
    let mut saw_content = false;

    for span in line.spans {
        let style = span.style;
        let chars: Vec<char> = span.content.chars().collect();
        if !chars.is_empty() {
            saw_content = true;
        }
        for ch in chars {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if current_len > 0 && current_len + ch_width > width {
                out.push(Line::from(std::mem::take(&mut current)));
                current_len = 0;
            }
            current.push(Span::styled(ch.to_string(), style));
            current_len += ch_width;
            if current_len >= width {
                out.push(Line::from(std::mem::take(&mut current)));
                current_len = 0;
            }
        }
    }

    if !current.is_empty() {
        out.push(Line::from(current));
    } else if !saw_content {
        out.push(Line::from(""));
    }

    if out.is_empty() {
        out.push(Line::from(""));
    }
    out
}

fn wrap_line_to_width_with_hanging_indent(
    line: Line<'static>,
    width: usize,
    hanging_indent: usize,
) -> Vec<Line<'static>> {
    if hanging_indent == 0 {
        return wrap_line_to_width(line, width);
    }
    if width == 0 {
        return vec![line];
    }

    let mut out: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_len = 0usize;
    let mut saw_content = false;
    let indent_width = hanging_indent.min(width.saturating_sub(1));

    for span in line.spans {
        let style = span.style;
        let chars: Vec<char> = span.content.chars().collect();
        if !chars.is_empty() {
            saw_content = true;
        }
        for ch in chars {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);

            if current_len == 0 && !out.is_empty() && indent_width > 0 {
                current.push(Span::raw(" ".repeat(indent_width)));
                current_len = indent_width;
            }

            if current_len > 0 && current_len + ch_width > width {
                out.push(Line::from(std::mem::take(&mut current)));
                current_len = 0;
                if indent_width > 0 {
                    current.push(Span::raw(" ".repeat(indent_width)));
                    current_len = indent_width;
                }
            }

            current.push(Span::styled(ch.to_string(), style));
            current_len += ch_width;
            if current_len >= width {
                out.push(Line::from(std::mem::take(&mut current)));
                current_len = 0;
            }
        }
    }

    if !current.is_empty() {
        out.push(Line::from(current));
    } else if !saw_content {
        out.push(Line::from(""));
    }

    if out.is_empty() {
        out.push(Line::from(""));
    }
    out
}
