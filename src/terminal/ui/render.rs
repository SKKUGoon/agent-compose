use super::constants::STATUS_WIDTH;
use super::App;
#[path = "render/format.rs"]
mod format;
#[path = "render/layout.rs"]
mod layout;
#[path = "render/panels.rs"]
mod panels;
#[path = "render/widgets.rs"]
mod widgets;
#[path = "render/wrapping.rs"]
mod wrapping;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

pub(super) fn draw_ui(frame: &mut Frame, app: &App) {
    let base = frame.area();
    let hide_panel = layout::should_hide_status_panel(base.width, base.height);
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
        panels::draw_status_bar(frame, status, app);
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
    panels::draw_chat_panel(frame, chat_area, app);
    panels::draw_input_panel(frame, input_area, app);

    if !hide_panel {
        panels::draw_status_sidebar(frame, columns[1], app);
    }

    if app.show_palette {
        panels::draw_palette(frame, app);
    }
}

pub(super) fn scrollable_panel_areas(base: Rect, app: &App) -> (Rect, Rect) {
    layout::scrollable_panel_areas(base, app)
}
