use super::constants::STATUS_WIDTH;
use super::{App, DisplayMode, FormFieldState, InputMode, SubmittedInput, PALETTE_ITEMS};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MIN_INPUT_ROWS: u16 = 6;
const MAX_INPUT_ROWS: u16 = 10;

pub(super) fn draw_ui(frame: &mut Frame, app: &App) {
    let base = frame.area();
    let hide_panel = should_hide_status_panel(base.width, base.height);
    let hide_status = base.height < 12;

    let rows = if hide_status {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8)])
            .split(base)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(8)])
            .split(base)
    };

    let (status_area, main_area) = if hide_status {
        (None, rows[0])
    } else {
        (Some(rows[0]), rows[1])
    };

    if let Some(status) = status_area {
        draw_status_bar(frame, status, app);
    }

    let columns = if hide_panel {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(12)])
            .split(main_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(12), Constraint::Length(STATUS_WIDTH)])
            .split(main_area)
    };

    let (chat_area, input_area) = scrollable_panel_areas(base, app);

    draw_chat_panel(frame, chat_area, app);
    draw_input_panel(frame, input_area, app);

    if !hide_panel {
        draw_status_sidebar(frame, columns[1], app);
    }

    if app.show_palette {
        draw_palette(frame, app);
    }
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let mut line = vec![
        Span::styled(
            " AGENT-COMPOSE ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(245, 165, 36))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            if app.running { "RUNNING" } else { "IDLE" },
            Style::default().fg(if app.running {
                Color::Rgb(56, 189, 248)
            } else {
                Color::Rgb(74, 222, 128)
            }),
        ),
        Span::raw("  mode="),
        Span::styled(
            app.mode.label(),
            Style::default().fg(Color::Rgb(245, 165, 36)),
        ),
        Span::raw("  display="),
        Span::styled(
            app.display_mode.label(),
            Style::default().fg(Color::Rgb(56, 189, 248)),
        ),
        Span::raw("  model="),
        Span::styled(
            &app.model_hint,
            Style::default().fg(Color::Rgb(230, 232, 235)),
        ),
        Span::raw("  keys: Tab/Shift+Tab mode, Esc*2 interrupt, Ctrl+P palette"),
    ];
    if let Some(msg) = &app.status_message {
        line.push(Span::raw("  |  "));
        line.push(Span::styled(
            msg,
            Style::default().fg(Color::Rgb(245, 165, 36)),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(line)), area);
}

fn draw_chat_panel(frame: &mut Frame, area: Rect, app: &App) {
    let (content_area, scrollbar_area) = split_for_scrollbar(area);
    let block_width = content_area.width as usize;
    let mut lines = Vec::new();
    for turn in &app.turns {
        let user_lines = render_submitted_input_lines(&turn.submitted_input);
        append_fixed_width_input_block(&mut lines, user_lines, block_width);

        let answer_lines = render_answer_lines(&turn.answer, app.display_mode);
        append_answer_block(&mut lines, answer_lines, block_width, app.display_mode);
    }
    if app.turns.is_empty() {
        lines.push(Line::from(
            "Enter raw JSON or fill the form and press Enter.",
        ));
    }

    let chat_total_lines = lines.len();
    let chat_scroll = clamp_scroll(
        chat_total_lines,
        content_area.height as usize,
        app.chat_scroll,
    );
    frame.render_widget(
        Paragraph::new(lines.clone())
            .block(Block::default())
            .scroll((chat_scroll, 0)),
        content_area,
    );

    render_vertical_scrollbar(
        frame,
        scrollbar_area,
        chat_total_lines,
        content_area.height as usize,
        chat_scroll,
        Color::Rgb(245, 165, 36),
    );
}

fn draw_status_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(6)])
        .split(area);

    let success = app.chain.iter().filter(|e| e.status == "done").count();
    let failed = app.chain.iter().filter(|e| e.status == "failed").count();
    let retrying = app.chain.iter().filter(|e| e.status == "retrying").count();
    let queued = app
        .chain
        .iter()
        .filter(|e| e.status == "queued" || e.status == "aborted")
        .count();
    let current = app.current_task.as_deref().unwrap_or("-");
    let current_agent = app.current_agent.as_deref().unwrap_or("-");
    let form_model = app
        .form_spec
        .as_ref()
        .map(|x| x.model.as_str())
        .unwrap_or("n/a");

    let session = format!(
        "current: {current}\nagent: {current_agent}\nform: {form_model}\nsuccess: {success}\nfailed: {failed}\nretrying: {retrying}\nqueued/aborted: {queued}",
    );
    frame.render_widget(
        Paragraph::new(session)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Pipeline Status")
                    .border_style(Style::default().fg(Color::Rgb(245, 165, 36))),
            )
            .wrap(Wrap { trim: false }),
        sections[0],
    );

    let chain_items: Vec<ListItem> = app
        .chain
        .iter()
        .map(|entry| {
            let (icon, color) = chain_icon(&entry.status);
            let parent_title = if entry.children.is_empty() {
                entry.label.clone()
            } else {
                entry.task.clone()
            };

            let mut lines = vec![Line::from(Span::styled(
                if !show_detail(&entry.status, &entry.detail) {
                    format!("{icon} {parent_title}")
                } else {
                    format!("{icon} {parent_title} - {}", entry.detail)
                },
                Style::default().fg(color),
            ))];

            for child in &entry.children {
                let (child_icon, child_color) = chain_icon(&child.status);
                let child_line = if !show_detail(&child.status, &child.detail) {
                    format!("    {child_icon} - {}", child.agent)
                } else {
                    format!("    {child_icon} - {} ({})", child.agent, child.detail)
                };
                lines.push(Line::from(Span::styled(
                    child_line,
                    Style::default().fg(child_color),
                )));
            }

            ListItem::new(lines)
        })
        .collect();
    frame.render_widget(
        List::new(chain_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Agent Chain")
                .border_style(Style::default().fg(Color::Rgb(245, 165, 36))),
        ),
        sections[1],
    );
}

fn draw_input_panel(frame: &mut Frame, area: Rect, app: &App) {
    let (content_area, scrollbar_area) = split_for_scrollbar(area);
    let mode_color = if app.mode == InputMode::Form {
        Color::Rgb(168, 85, 247)
    } else {
        Color::Rgb(248, 113, 113)
    };
    let field_area = if content_area.height > 1 {
        Rect::new(
            content_area.x,
            content_area.y,
            content_area.width,
            content_area.height - 1,
        )
    } else {
        content_area
    };
    let text_area = inner_margin_rect(field_area, 2, 1, 1);
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(36, 41, 48))),
        content_area,
    );

    match app.mode {
        InputMode::Quick => {
            let quick_lines_vec = render_raw_input_lines(&app.input);
            let quick_lines = wrapped_line_count(&quick_lines_vec, text_area.width as usize);
            let quick_scroll =
                clamp_scroll(quick_lines, text_area.height as usize, app.form_scroll);
            if text_area.width > 0 && text_area.height > 0 {
                frame.render_widget(
                    Paragraph::new(quick_lines_vec)
                        .scroll((quick_scroll, 0))
                        .wrap(Wrap { trim: false }),
                    text_area,
                );
            }

            render_bottom_mode_tabs(frame, content_area, app.mode);

            render_vertical_scrollbar(
                frame,
                scrollbar_area,
                quick_lines,
                text_area.height as usize,
                quick_scroll,
                mode_color,
            );
        }
        InputMode::Form => {
            let mut lines = Vec::new();
            let mut has_required = false;
            let mut has_optional = false;

            for field in &app.form_fields {
                if field.required {
                    has_required = true;
                } else {
                    has_optional = true;
                }
            }

            if has_required {
                lines.push(Line::from(Span::styled(
                    "[required]",
                    Style::default().fg(Color::Rgb(245, 165, 36)),
                )));
                for (i, field) in app
                    .form_fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.required)
                {
                    let marker = if i == app.form_index { ">" } else { " " };
                    lines.push(Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Rgb(245, 165, 36))),
                        Span::raw(" "),
                        Span::styled(
                            format!("{} [{}]", field.name, field.kind),
                            Style::default().fg(Color::Rgb(56, 189, 248)),
                        ),
                        Span::raw(": "),
                        Span::styled(&field.value, Style::default().fg(Color::Rgb(230, 232, 235))),
                    ]));
                }
            }

            if has_optional {
                lines.push(Line::from(Span::styled(
                    "[optional]",
                    Style::default().fg(Color::Rgb(135, 146, 165)),
                )));
                for (i, field) in app
                    .form_fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| !f.required)
                {
                    let marker = if i == app.form_index { ">" } else { " " };
                    lines.push(Line::from(vec![
                        Span::styled(marker, Style::default().fg(Color::Rgb(245, 165, 36))),
                        Span::raw(" "),
                        Span::styled(
                            format!("{} [{}]", field.name, field.kind),
                            Style::default().fg(Color::Rgb(56, 189, 248)),
                        ),
                        Span::raw(": "),
                        Span::styled(&field.value, Style::default().fg(Color::Rgb(230, 232, 235))),
                    ]));
                }
            }

            if app.form_fields.is_empty() {
                lines.push(Line::from("No form fields available."));
            }
            let total_lines = wrapped_line_count(&lines, text_area.width as usize);
            let form_scroll = clamp_scroll(total_lines, text_area.height as usize, app.form_scroll);
            if text_area.width > 0 && text_area.height > 0 {
                frame.render_widget(
                    Paragraph::new(lines)
                        .scroll((form_scroll, 0))
                        .wrap(Wrap { trim: false }),
                    text_area,
                );
            }

            render_bottom_mode_tabs(frame, content_area, app.mode);

            render_vertical_scrollbar(
                frame,
                scrollbar_area,
                total_lines,
                text_area.height as usize,
                form_scroll,
                mode_color,
            );
        }
    }
}

fn draw_palette(frame: &mut Frame, app: &App) {
    let area = centered_rect(30, 60, frame.area());
    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = PALETTE_ITEMS
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let style = if item.command.is_none() {
                Style::default()
                    .fg(Color::Rgb(245, 165, 36))
                    .add_modifier(Modifier::BOLD)
            } else if idx == app.palette_pos {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(245, 165, 36))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(230, 230, 230))
            };

            let label_line = Line::from(Span::styled(item.label.trim(), style)).centered();
            if item.command.is_none() && idx != 0 {
                ListItem::new(vec![Line::from(""), label_line])
            } else {
                ListItem::new(label_line)
            }
        })
        .collect();

    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Command Palette")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(245, 165, 36))),
        ),
        area,
    );
}

fn should_hide_status_panel(width: u16, height: u16) -> bool {
    if width < 90 || height < 14 {
        return true;
    }
    let chat_width = width.saturating_sub(STATUS_WIDTH);
    (chat_width as f32) < (STATUS_WIDTH as f32 * 1.5)
}

pub(super) fn scrollable_panel_areas(base: Rect, app: &App) -> (Rect, Rect) {
    let hide_status = base.height < 12;
    let rows = if hide_status {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8)])
            .split(base)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(8)])
            .split(base)
    };
    let main_area = if hide_status { rows[0] } else { rows[1] };

    let hide_panel = should_hide_status_panel(base.width, base.height);
    let columns = if hide_panel {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(12)])
            .split(main_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(12), Constraint::Length(STATUS_WIDTH)])
            .split(main_area)
    };

    let left = columns[0];
    let desired = desired_input_height(app);
    let max_input_height = left.height.saturating_sub(3).max(1);
    let input_height = desired.min(max_input_height).max(1);
    let panel_gap = if left.height > input_height + 1 { 1 } else { 0 };
    let chat_height = left
        .height
        .saturating_sub(input_height)
        .saturating_sub(panel_gap);

    let chat_area = Rect::new(left.x, left.y, left.width, chat_height);
    let input_area = Rect::new(
        left.x,
        left.y.saturating_add(chat_height).saturating_add(panel_gap),
        left.width,
        input_height,
    );

    (chat_area, input_area)
}

fn desired_input_height(app: &App) -> u16 {
    let field_lines = match app.mode {
        InputMode::Quick => app.input.lines().count().max(1),
        InputMode::Form => {
            if app.form_fields.is_empty() {
                1
            } else {
                let required = app.form_fields.iter().filter(|f| f.required).count();
                let optional = app.form_fields.len().saturating_sub(required);
                let mut count = app.form_fields.len();
                if required > 0 {
                    count += 1;
                }
                if optional > 0 {
                    count += 1;
                }
                count
            }
        }
    };

    let total = (field_lines as u16).saturating_add(1);
    total.clamp(MIN_INPUT_ROWS, MAX_INPUT_ROWS)
}

fn split_for_scrollbar(area: Rect) -> (Rect, Rect) {
    if area.width < 2 {
        return (area, area);
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (cols[0], cols[1])
}

fn render_vertical_scrollbar(
    frame: &mut Frame,
    area: Rect,
    total_lines: usize,
    viewport_lines: usize,
    scroll: u16,
    accent: Color,
) {
    if area.width == 0 || area.height == 0 || total_lines <= viewport_lines {
        return;
    }

    let max_scroll = total_lines.saturating_sub(viewport_lines);
    let scroll_position = (scroll as usize).min(max_scroll);
    let scrollbar_content_length = max_scroll.saturating_add(1);

    let mut state = ScrollbarState::new(total_lines)
        .content_length(scrollbar_content_length)
        .position(scroll_position)
        .viewport_content_length(viewport_lines);

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(Some("│"))
        .thumb_symbol("█")
        .track_style(Style::default().fg(Color::Rgb(82, 92, 107)))
        .thumb_style(Style::default().fg(accent));

    frame.render_stateful_widget(scrollbar, area, &mut state);
}

fn render_bottom_mode_tabs(frame: &mut Frame, area: Rect, mode: InputMode) {
    if area.width < 3 || area.height == 0 {
        return;
    }

    let y = area.y.saturating_add(area.height.saturating_sub(1));
    let x = area.x.saturating_add(1);
    let width = area.width.saturating_sub(2);
    let label_area = Rect::new(x, y, width, 1);
    let muted = Style::default().fg(Color::Rgb(135, 146, 165));
    let form_active = Style::default()
        .fg(Color::Rgb(168, 85, 247))
        .add_modifier(Modifier::BOLD);
    let raw_active = Style::default()
        .fg(Color::Rgb(248, 113, 113))
        .add_modifier(Modifier::BOLD);

    let form_style = if mode == InputMode::Form {
        form_active
    } else {
        muted
    };
    let raw_style = if mode == InputMode::Quick {
        raw_active
    } else {
        muted
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Form Input", form_style),
            Span::styled(" | ", muted),
            Span::styled("Raw Input", raw_style),
        ])),
        label_area,
    );
}

fn inner_margin_rect(area: Rect, horizontal: u16, top: u16, bottom: u16) -> Rect {
    let x = area.x.saturating_add(horizontal);
    let y = area.y.saturating_add(top);
    let width = area.width.saturating_sub(horizontal.saturating_mul(2));
    let height = area.height.saturating_sub(top.saturating_add(bottom));
    Rect::new(x, y, width, height)
}

fn render_raw_input_lines(input: &str) -> Vec<Line<'static>> {
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
        spans.push(Span::styled(":".to_string(), punct_style));

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
            spans.push(Span::styled(",".to_string(), punct_style));
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

fn render_submitted_input_lines(input: &SubmittedInput) -> Vec<Line<'static>> {
    match input {
        SubmittedInput::Raw(text) => render_raw_input_lines(text),
        SubmittedInput::Form {
            fields,
            selected_index,
        } => render_form_lines(fields, Some(*selected_index)),
    }
}

fn render_form_lines(
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

fn render_answer_lines(answer: &str, display_mode: DisplayMode) -> Vec<Line<'static>> {
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

fn append_answer_block(
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

fn append_fixed_width_input_block(
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

fn wrapped_line_count(lines: &[Line<'_>], width: usize) -> usize {
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

fn clamp_scroll(total_lines: usize, viewport_lines: usize, scroll: u16) -> u16 {
    let max_scroll = total_lines.saturating_sub(viewport_lines);
    (scroll as usize).min(max_scroll) as u16
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn chain_icon(status: &str) -> (&'static str, Color) {
    match status {
        "running" => ("●", Color::Rgb(56, 189, 248)),
        "retrying" => ("↻", Color::Rgb(245, 165, 36)),
        "done" => ("✔", Color::Rgb(74, 222, 128)),
        "failed" => ("✖", Color::Rgb(248, 113, 113)),
        "skipped" => ("◌", Color::Rgb(245, 165, 36)),
        "aborted" => ("◍", Color::Rgb(148, 163, 184)),
        _ => ("○", Color::Rgb(120, 120, 120)),
    }
}

fn show_detail(status: &str, detail: &str) -> bool {
    !detail.is_empty() && status != "done"
}
