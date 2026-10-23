use crate::app::App;
use crate::state::{Focus, Popup, Screen, Section, UiState};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Paragraph, StatefulWidget, Widget, Wrap},
};

const ACCENT: Color = Color::Rgb(126, 231, 197);
const BORDER: Color = Color::Rgb(70, 84, 82);
const MUTED: Color = Color::Rgb(138, 150, 147);
const ERROR: Color = Color::Rgb(255, 128, 128);
const SELECTED: Color = Color::Rgb(180, 255, 232);

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 18;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_ui(&mut self.state, area, buf);
    }
}

fn render_ui(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    match state.screen {
        Screen::Home => render_home(state, area, buf),
        Screen::Compose => render_compose(state, area, buf),
        Screen::Settings => render_settings(state, area, buf),
    }

    if let Some(popup) = state.popup {
        render_popup(popup, area, buf);
    }
}

fn render_home(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    if too_small(area) {
        render_too_small(area, buf);
        return;
    }

    let [main, footer] = Layout::vertical([Constraint::Min(6), Constraint::Length(2)]).areas(area);
    let [workspace, quick_note] =
        Layout::vertical([Constraint::Min(4), Constraint::Length(3)]).areas(main);
    let [sidebar, drafts, preview] = Layout::horizontal([
        Constraint::Length(20),
        Constraint::Min(24),
        Constraint::Min(28),
    ])
    .areas(workspace);

    render_sidebar(state, sidebar, buf);
    render_draft_list(state, drafts, buf);
    render_preview(state, preview, buf);
    render_quick_note(state, quick_note, buf);
    render_footer(state, footer, buf);
}

fn render_sidebar(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    let items = Section::ALL
        .iter()
        .map(|section| ListItem::new(format!(" {} ", section.label())))
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(pane_block("Sections", state.focus == Focus::Sidebar))
        .highlight_style(highlight_style(state.focus == Focus::Sidebar))
        .highlight_symbol("› ");

    StatefulWidget::render(list, area, buf, &mut state.sections);
}

fn render_draft_list(state: &UiState, area: Rect, buf: &mut Buffer) {
    let block = pane_block(state.section.label(), state.focus == Focus::DraftList);
    let inner = block.inner(area);
    block.render(area, buf);

    let empty = match state.section {
        Section::Settings => "Press Enter to open settings.",
        _ => "Press n to dump a thought.\n\nDrafts will appear here after you process one.",
    };

    Paragraph::new(format!("Nothing here yet.\n\n{empty}"))
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .render(inner, buf);
}

fn render_preview(state: &UiState, area: Rect, buf: &mut Buffer) {
    let block = pane_block("Preview", state.focus == Focus::Preview);
    let inner = block.inner(area);
    block.render(area, buf);

    let body = match state.drafts.selected() {
        Some(index) => format!(
            "Draft #{}\n\nMessage details will render here once drafts are persisted.",
            index + 1
        ),
        None => "No draft selected.\n\nSelect a draft to read its message body here.".to_string(),
    };

    Paragraph::new(body)
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .render(inner, buf);
}

fn render_quick_note(state: &UiState, area: Rect, buf: &mut Buffer) {
    let block = pane_block("Quick note", state.focus == Focus::QuickNote);
    let inner = block.inner(area);
    block.render(area, buf);

    (&state.quick_note).render(inner, buf);
}

fn render_compose(state: &UiState, area: Rect, buf: &mut Buffer) {
    if too_small(area) {
        render_too_small(area, buf);
        return;
    }

    let [editor, footer] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(area);
    let block = pane_block("New brain dump", true);
    let inner = block.inner(editor);
    block.render(editor, buf);

    (&state.composer).render(inner, buf);
    render_footer(state, footer, buf);
}

fn render_settings(state: &UiState, area: Rect, buf: &mut Buffer) {
    if too_small(area) {
        render_too_small(area, buf);
        return;
    }

    let [body, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(area);
    let block = pane_block("Settings", false);
    let inner = block.inner(body);
    block.render(body, buf);

    Paragraph::new(
        "Settings\n\n\
         Model selection, storage location, and other preferences will live here.\n\n\
         Nothing is sent to a cloud service. Press Esc to return to the workspace.",
    )
    .style(Style::default().fg(MUTED))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true })
    .render(inner, buf);

    render_footer(state, footer, buf);
}

fn render_footer(state: &UiState, area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::from(Span::styled(
            footer_shortcuts(state),
            Style::default().fg(MUTED),
        )),
        footer_status(state),
    ];

    Paragraph::new(lines).render(area, buf);
}

fn footer_shortcuts(state: &UiState) -> &'static str {
    if state.popup.is_some() {
        return " Esc close popup · Enter confirms where shown";
    }

    match state.screen {
        Screen::Compose => " Type freely · Esc back (your text is kept)",
        Screen::Settings => " Esc back",
        Screen::Home => match state.focus {
            Focus::Sidebar => " j/k move · Enter select · Tab pane · n new · ? help · q quit",
            Focus::DraftList => " j/k move · Enter open · Tab pane · n new · ? help · q quit",
            Focus::Preview => " Esc back · Tab pane · n new · ? help · q quit",
            Focus::QuickNote => " Type a note · Esc done · Tab pane",
        },
    }
}

fn footer_status(state: &UiState) -> Line<'static> {
    if let Some(error) = &state.error {
        return Line::from(vec![
            Span::styled(" ! ", Style::default().fg(ERROR)),
            Span::styled(error.clone(), Style::default().fg(ERROR)),
        ]);
    }

    if state.generation.is_generating() {
        return Line::from(Span::styled(
            " Generating locally with Ollama…",
            Style::default().fg(ACCENT),
        ));
    }

    Line::from(Span::styled(
        " local-only · no cloud · your words stay here",
        Style::default().fg(MUTED),
    ))
}

fn render_popup(popup: Popup, area: Rect, buf: &mut Buffer) {
    let popup_area = centered_rect(area, 66, 16);
    if popup_area.width < 20 || popup_area.height < 5 {
        return;
    }

    Clear.render(popup_area, buf);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(format!(" {} ", popup.title()));
    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    Paragraph::new(format!("\n{}\n", popup.body()))
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: false })
        .render(inner, buf);
}

fn render_too_small(area: Rect, buf: &mut Buffer) {
    Clear.render(area, buf);

    let [_, middle, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(7),
        Constraint::Fill(1),
    ])
    .areas(area);

    Paragraph::new(format!(
        "Terminal too small\n\n\
         blurp needs at least {MIN_WIDTH}×{MIN_HEIGHT} cells.\n\
         Current: {}×{}.\n\n\
         Resize the window to continue.",
        area.width, area.height
    ))
    .style(Style::default().fg(MUTED))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true })
    .render(middle, buf);
}

fn pane_block(title: &'static str, focused: bool) -> Block<'static> {
    let color = if focused { ACCENT } else { BORDER };

    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(color),
        ))
}

fn highlight_style(focused: bool) -> Style {
    let color = if focused { SELECTED } else { Color::Gray };

    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn centered_rect(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.saturating_sub(4).min(max_width);
    let height = area.height.saturating_sub(2).min(max_height);

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_is_safe_across_sizes_screens_and_popups() {
        let sizes = [(1, 1), (20, 5), (79, 17), (80, 18), (120, 40)];
        let screens = [Screen::Home, Screen::Compose, Screen::Settings];
        let popups = [
            None,
            Some(Popup::Help),
            Some(Popup::ConfirmQuit),
            Some(Popup::SelectTone),
        ];

        for (width, height) in sizes {
            for screen in screens {
                for popup in popups {
                    let mut state = UiState {
                        screen,
                        popup,
                        ..UiState::default()
                    };

                    let area = Rect::new(0, 0, width, height);
                    let mut buf = Buffer::empty(area);
                    render_ui(&mut state, area, &mut buf);
                }
            }
        }
    }

    #[test]
    fn rendering_each_focused_pane_is_safe() {
        for focus in Focus::ALL {
            let mut state = UiState {
                focus,
                ..UiState::default()
            };
            state.drafts.select(Some(1));
            state.error = Some("a readable error".to_string());
            state.generation = crate::state::GenerationState::Generating;

            let area = Rect::new(0, 0, 100, 30);
            let mut buf = Buffer::empty(area);
            render_ui(&mut state, area, &mut buf);
        }
    }
}
