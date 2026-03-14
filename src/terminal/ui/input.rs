use super::formatting::format_answer;
use super::render;
use super::{
    App, ComposeRuntime, Event, InputMode, KeyCode, KeyEventKind, KeyModifiers, MouseEvent,
    MouseEventKind, PALETTE_ITEMS, Stdout, Terminal, UiEvent, event,
};
use ratatui::backend::CrosstermBackend;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub(super) async fn run_tui_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    runtime: ComposeRuntime,
    model: Option<String>,
    json_mode: bool,
) -> Result<(), String> {
    let (tx, mut rx): (UnboundedSender<UiEvent>, UnboundedReceiver<UiEvent>) = unbounded_channel();
    let mut app = App::new(&runtime, model.clone(), json_mode);

    loop {
        app.tick();

        while let Ok(evt) = rx.try_recv() {
            match evt {
                UiEvent::Runtime(e) => app.apply_runtime_event(e),
                UiEvent::Finished(result) => {
                    app.running = false;
                    app.run_handle = None;
                    match result {
                        Ok(v) => {
                            app.last_result = Some(v.clone());
                            app.current_task = None;
                            app.current_agent = None;
                            if let Some(last) = app.turns.last_mut() {
                                last.answer = format_answer(&v, app.structured_output, app.display_mode);
                            }
                        }
                        Err(err) => {
                            app.mark_run_failed(&err);
                            if let Some(last) = app.turns.last_mut() {
                                last.answer = format!("Error: {err}");
                            }
                            app.logs.push(format!("Run failed: {err}"));
                        }
                    }
                }
            }
        }

        terminal
            .draw(|f| render::draw_ui(f, &app))
            .map_err(|e| e.to_string())?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(50)).map_err(|e| e.to_string())? {
            match event::read().map_err(|e| e.to_string())? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    if key.code == KeyCode::Esc {
                        app.handle_esc();
                        continue;
                    }

                    if app.show_palette {
                        handle_palette_key(&mut app, key.code);
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true
                        }
                        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.show_palette = true;
                        }
                        KeyCode::Tab | KeyCode::BackTab => {
                            app.mode = match app.mode {
                                InputMode::Quick => {
                                    if app.form_spec.is_some() {
                                        InputMode::Form
                                    } else {
                                        InputMode::Quick
                                    }
                                }
                                InputMode::Form => InputMode::Quick,
                            };
                        }
                        KeyCode::Up => {
                            if app.mode == InputMode::Form {
                                app.form_index = app.form_index.saturating_sub(1);
                            }
                        }
                        KeyCode::Down => {
                            if app.mode == InputMode::Form && !app.form_fields.is_empty() {
                                app.form_index = (app.form_index + 1).min(app.form_fields.len() - 1);
                            }
                        }
                        KeyCode::PageUp => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.form_scroll = app.form_scroll.saturating_sub(3);
                            } else {
                                app.chat_scroll = app.chat_scroll.saturating_sub(3);
                            }
                        }
                        KeyCode::PageDown => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                app.form_scroll = app.form_scroll.saturating_add(3);
                            } else {
                                app.chat_scroll = app.chat_scroll.saturating_add(3);
                            }
                        }
                        KeyCode::Enter => app.submit(runtime.clone(), model.clone(), tx.clone()),
                        KeyCode::Backspace => {
                            if app.mode == InputMode::Form {
                                if let Some(field) = app.form_fields.get_mut(app.form_index) {
                                    field.value.pop();
                                }
                            } else {
                                app.input.pop();
                            }
                        }
                        KeyCode::Char(ch) => {
                            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                                if app.mode == InputMode::Form {
                                    if let Some(field) = app.form_fields.get_mut(app.form_index) {
                                        field.value.push(ch);
                                    }
                                } else {
                                    app.input.push(ch);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => handle_mouse_event(&mut app, terminal, mouse),
                _ => {}
            }
        }
    }
}

pub(super) fn first_selectable_palette_index() -> usize {
    PALETTE_ITEMS
        .iter()
        .position(|x| x.command.is_some())
        .unwrap_or(0)
}

fn handle_palette_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Esc => app.show_palette = false,
        KeyCode::Up => app.palette_pos = previous_selectable_palette_index(app.palette_pos),
        KeyCode::Down => app.palette_pos = next_selectable_palette_index(app.palette_pos),
        KeyCode::Enter => {
            if let Some(item) = PALETTE_ITEMS.get(app.palette_pos)
                && let Some(cmd) = item.command
            {
                app.execute_command(cmd);
            }
            app.show_palette = false;
        }
        _ => {}
    }
}

fn previous_selectable_palette_index(current: usize) -> usize {
    let mut idx = current.saturating_sub(1);
    loop {
        if PALETTE_ITEMS
            .get(idx)
            .and_then(|x| x.command)
            .is_some()
        {
            return idx;
        }
        if idx == 0 {
            return current;
        }
        idx = idx.saturating_sub(1);
    }
}

fn next_selectable_palette_index(current: usize) -> usize {
    let mut idx = (current + 1).min(PALETTE_ITEMS.len().saturating_sub(1));
    loop {
        if PALETTE_ITEMS
            .get(idx)
            .and_then(|x| x.command)
            .is_some()
        {
            return idx;
        }
        if idx + 1 >= PALETTE_ITEMS.len() {
            return current;
        }
        idx += 1;
    }
}

fn handle_mouse_event(
    app: &mut App,
    terminal: &Terminal<CrosstermBackend<Stdout>>,
    mouse: MouseEvent,
) {
    let size = match terminal.size() {
        Ok(rect) => rect,
        Err(_) => return,
    };

    let base = ratatui::layout::Rect::new(0, 0, size.width, size.height);
    let (chat_area, input_area) = render::scrollable_panel_areas(base, app);
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if contains(chat_area, mouse.column, mouse.row) {
                app.chat_scroll = app.chat_scroll.saturating_sub(3);
            } else if contains(input_area, mouse.column, mouse.row) {
                app.form_scroll = app.form_scroll.saturating_sub(3);
            }
        }
        MouseEventKind::ScrollDown => {
            if contains(chat_area, mouse.column, mouse.row) {
                app.chat_scroll = app.chat_scroll.saturating_add(3);
            } else if contains(input_area, mouse.column, mouse.row) {
                app.form_scroll = app.form_scroll.saturating_add(3);
            }
        }
        _ => {}
    }
}

fn contains(area: ratatui::layout::Rect, x: u16, y: u16) -> bool {
    if area.width == 0 || area.height == 0 {
        return false;
    }
    let x_end = area.x.saturating_add(area.width);
    let y_end = area.y.saturating_add(area.height);
    x >= area.x && x < x_end && y >= area.y && y < y_end
}
