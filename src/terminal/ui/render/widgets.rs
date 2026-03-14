use super::super::InputMode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

pub(super) fn render_vertical_scrollbar(
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

pub(super) fn render_bottom_mode_tabs(frame: &mut Frame, area: Rect, mode: InputMode) {
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

pub(super) fn chain_icon(status: &str) -> (&'static str, Color) {
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

pub(super) fn show_detail(status: &str, detail: &str) -> bool {
    !detail.is_empty() && status != "done"
}
