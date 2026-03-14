use super::super::{App, InputMode, PALETTE_ITEMS};
use super::format::{
    render_answer_lines, render_form_lines, render_raw_input_lines, render_submitted_input_lines,
};
use super::layout::{centered_rect, clamp_scroll, inner_margin_rect, split_for_scrollbar};
use super::widgets::{chain_icon, render_bottom_mode_tabs, render_vertical_scrollbar, show_detail};
use super::wrapping::{append_answer_block, append_fixed_width_input_block, wrapped_line_count};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

pub(super) fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
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

pub(super) fn draw_chat_panel(frame: &mut Frame, area: Rect, app: &App) {
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

pub(super) fn draw_status_sidebar(frame: &mut Frame, area: Rect, app: &App) {
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

pub(super) fn draw_input_panel(frame: &mut Frame, area: Rect, app: &App) {
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
            let lines = render_form_lines(&app.form_fields, Some(app.form_index));
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

pub(super) fn draw_palette(frame: &mut Frame, app: &App) {
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
