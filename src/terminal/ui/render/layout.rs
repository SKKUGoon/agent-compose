use super::super::constants::STATUS_WIDTH;
use super::super::{App, InputMode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};

const MIN_INPUT_ROWS: u16 = 6;
const MAX_INPUT_ROWS: u16 = 10;

pub(super) fn should_hide_status_panel(width: u16, height: u16) -> bool {
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

pub(super) fn split_for_scrollbar(area: Rect) -> (Rect, Rect) {
    if area.width < 2 {
        return (area, area);
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);
    (cols[0], cols[1])
}

pub(super) fn inner_margin_rect(area: Rect, horizontal: u16, top: u16, bottom: u16) -> Rect {
    let x = area.x.saturating_add(horizontal);
    let y = area.y.saturating_add(top);
    let width = area.width.saturating_sub(horizontal.saturating_mul(2));
    let height = area.height.saturating_sub(top.saturating_add(bottom));
    Rect::new(x, y, width, height)
}

pub(super) fn clamp_scroll(total_lines: usize, viewport_lines: usize, scroll: u16) -> u16 {
    let max_scroll = total_lines.saturating_sub(viewport_lines);
    (scroll as usize).min(max_scroll) as u16
}

pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
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
