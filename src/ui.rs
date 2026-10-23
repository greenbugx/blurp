use crate::app::App;
use crate::draft::{DraftCard, DraftStatus};
use crate::state::{Focus, Popup, Screen, Section, UiState};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Clear, HighlightSpacing, List, ListItem, Paragraph, StatefulWidget,
        Widget, Wrap,
    },
};

const BG: Color = Color::Rgb(11, 16, 15);
const TEXT: Color = Color::Rgb(216, 224, 221);
const ACCENT: Color = Color::Rgb(126, 231, 197);
const BORDER: Color = Color::Rgb(64, 78, 75);
const MUTED: Color = Color::Rgb(132, 146, 142);
const ERROR: Color = Color::Rgb(255, 128, 128);
const SELECTED_BG: Color = Color::Rgb(27, 46, 41);
const SELECTED_BG_DIM: Color = Color::Rgb(22, 32, 30);
const REVIEW: Color = Color::Rgb(232, 190, 122);
const DRAFT: Color = Color::Rgb(180, 220, 205);
const SNOOZED: Color = Color::Rgb(150, 176, 214);
const DONE: Color = Color::Rgb(120, 160, 148);

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 18;

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_ui(&mut self.state, area, buf);
    }
}

fn render_ui(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    buf.set_style(area, base_style());

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
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .areas(workspace);

    render_sidebar(state, sidebar, buf);
    render_draft_list(state, drafts, buf);
    render_preview(state, preview, buf);
    render_quick_note(state, quick_note, buf);
    render_footer(state, footer, buf);
}

fn render_sidebar(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    let focused = state.focus == Focus::Sidebar;
    let content_width = area.width.saturating_sub(5) as usize;

    let items = Section::ALL
        .iter()
        .map(|section| {
            let count = state.section_draft_count(*section);
            let count = if count > 0 {
                count.to_string()
            } else {
                String::new()
            };
            let gap = content_width.saturating_sub(section.label().len() + count.len());

            ListItem::new(Line::from(vec![
                Span::styled(section.label(), Style::default().fg(TEXT)),
                Span::raw(" ".repeat(gap)),
                Span::styled(count, Style::default().fg(MUTED)),
            ]))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(pane_block("blurp", focused))
        .highlight_symbol(highlight_symbol(focused))
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(highlight_style(focused));

    StatefulWidget::render(list, area, buf, &mut state.sections);
}

fn render_draft_list(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    let focused = state.focus == Focus::DraftList;
    let count = state.visible_draft_count();
    let title = if count > 0 {
        format!("{} · {count}", state.section.label())
    } else {
        state.section.label().to_string()
    };
    let block = pane_block(title, focused);
    let inner = block.inner(area);
    block.render(area, buf);

    if count == 0 {
        Paragraph::new(empty_list_message(state.section))
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
        return;
    }

    let content_width = inner.width.saturating_sub(3) as usize;
    let items = state
        .visible_drafts()
        .map(|draft| draft_item(draft, content_width))
        .collect::<Vec<_>>();

    let list = List::new(items)
        .highlight_symbol(highlight_symbol(focused))
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(highlight_style(focused));

    StatefulWidget::render(list, inner, buf, &mut state.draft_list);
}

fn render_preview(state: &UiState, area: Rect, buf: &mut Buffer) {
    let block = pane_block("Preview", state.focus == Focus::Preview);
    let inner = block.inner(area);
    block.render(area, buf);

    let Some(draft) = state.selected_draft() else {
        Paragraph::new("No draft selected.\n\nChoose a draft to read its message body here.")
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
        return;
    };

    let mut lines = vec![
        Line::from(Span::styled(
            draft.title.clone(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(draft_meta(draft)),
        Line::from(""),
    ];

    for body_line in draft.body.lines() {
        lines.push(Line::from(Span::styled(
            body_line.to_string(),
            Style::default().fg(TEXT),
        )));
    }

    Paragraph::new(lines)
        .wrap(Wrap { trim: false })
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
        "Model selection, storage location, and other preferences will live here.\n\n\
         Nothing is sent to a cloud service. Press Esc to return to the workspace.",
    )
    .style(Style::default().fg(MUTED))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true })
    .render(inner, buf);

    render_footer(state, footer, buf);
}

fn render_footer(state: &UiState, area: Rect, buf: &mut Buffer) {
    let [shortcuts, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    Paragraph::new(shortcut_line(state)).render(shortcuts, buf);
    Paragraph::new(footer_status(state)).render(status, buf);
}

fn shortcut_line(state: &UiState) -> Line<'static> {
    let pairs: &[(&str, &str)] = if state.popup.is_some() {
        &[("Esc", "close"), ("Enter", "confirm where shown")]
    } else {
        match state.screen {
            Screen::Compose => &[("type", "freely"), ("Esc", "back, text kept")],
            Screen::Settings => &[("Esc", "back"), ("?", "help"), ("q", "quit")],
            Screen::Home => match state.focus {
                Focus::Sidebar => &[
                    ("j/k", "section"),
                    ("Enter", "open"),
                    ("Tab", "pane"),
                    ("n", "new"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Focus::DraftList => &[
                    ("j/k", "move"),
                    ("Enter", "open"),
                    ("Tab", "pane"),
                    ("n", "new"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Focus::Preview => &[
                    ("Esc", "back"),
                    ("Tab", "pane"),
                    ("n", "new"),
                    ("?", "help"),
                    ("q", "quit"),
                ],
                Focus::QuickNote => &[("type", "note"), ("Esc", "done"), ("Tab", "pane")],
            },
        }
    };

    let mut spans = Vec::new();
    for (index, (key, action)) in pairs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(BORDER)));
        }
        spans.push(Span::styled(*key, Style::default().fg(ACCENT)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*action, Style::default().fg(MUTED)));
    }

    Line::from(spans)
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
            " Generating locally with Ollama...",
            Style::default().fg(ACCENT),
        ));
    }

    if state.screen == Screen::Compose {
        return Line::from(Span::styled(
            " local-only · your text stays in this session",
            Style::default().fg(MUTED),
        ));
    }

    Line::from(vec![
        Span::styled(" ● ", Style::default().fg(ACCENT)),
        Span::styled(
            "sample drafts in memory · nothing is saved yet",
            Style::default().fg(MUTED),
        ),
    ])
}

fn render_popup(popup: Popup, area: Rect, buf: &mut Buffer) {
    let (max_width, max_height) = match popup {
        Popup::Help => (58, 11),
        Popup::ConfirmQuit => (58, 9),
        Popup::SelectTone => (58, 7),
    };
    let popup_area = centered_rect(area, max_width, max_height);
    if popup_area.width < 20 || popup_area.height < 5 {
        return;
    }

    Clear.render(popup_area, buf);
    buf.set_style(popup_area, base_style());

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" {} ", popup.title()),
            Style::default().fg(ACCENT),
        ));
    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    Paragraph::new(format!("\n{}\n", popup.body()))
        .style(Style::default().fg(TEXT))
        .wrap(Wrap { trim: false })
        .render(inner, buf);
}

fn render_too_small(area: Rect, buf: &mut Buffer) {
    Clear.render(area, buf);
    buf.set_style(area, base_style());

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

fn draft_item(draft: &DraftCard, width: usize) -> ListItem<'static> {
    let status = draft.status.label();
    let title_width = width.saturating_sub(status.len() + 2).max(4);
    let title = truncate_chars(&draft.title, title_width);
    let gap = width
        .saturating_sub(title.chars().count() + status.len())
        .max(1);

    let title_line = Line::from(vec![
        Span::styled(
            title,
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(gap)),
        Span::styled(
            status.to_string(),
            Style::default().fg(status_color(draft.status)),
        ),
    ]);

    let meta = match &draft.recipient {
        Some(recipient) => format!("To {recipient} · {}", draft.tone.label()),
        None => draft.tone.label().to_string(),
    };
    let preview = preview_text(&draft.body, width);

    ListItem::new(vec![
        title_line,
        Line::from(Span::styled(meta, Style::default().fg(MUTED))),
        Line::from(Span::styled(preview, Style::default().fg(TEXT))),
        Line::from(""),
    ])
}

fn draft_meta(draft: &DraftCard) -> Vec<Span<'static>> {
    let prefix = match &draft.recipient {
        Some(recipient) => format!("To {recipient} · {} · ", draft.tone.label()),
        None => format!("{} · ", draft.tone.label()),
    };

    vec![
        Span::styled(prefix, Style::default().fg(MUTED)),
        Span::styled(
            draft.status.label(),
            Style::default().fg(status_color(draft.status)),
        ),
    ]
}

fn empty_list_message(section: Section) -> &'static str {
    match section {
        Section::Inbox => "Nothing needs a look right now.\n\nPress n to dump a thought.",
        Section::Drafts => "No drafts in progress.\n\nPress n to start from a brain dump.",
        Section::Snoozed => "Nothing snoozed.\n\nDrafts you set aside wait here.",
        Section::Completed => "Nothing completed yet.\n\nFinished drafts land here.",
        Section::Settings => "Press Enter on Settings in the sidebar.",
    }
}

fn preview_text(body: &str, width: usize) -> String {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&flat, width.max(4))
}

fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }

    let mut shortened = text.chars().take(max.saturating_sub(1)).collect::<String>();
    if let Some(index) = shortened.rfind(' ') {
        shortened.truncate(index);
    }
    shortened.push('…');
    shortened
}

fn status_color(status: DraftStatus) -> Color {
    match status {
        DraftStatus::Review => REVIEW,
        DraftStatus::Draft => DRAFT,
        DraftStatus::Snoozed => SNOOZED,
        DraftStatus::Completed => DONE,
    }
}

fn pane_block(title: impl Into<String>, focused: bool) -> Block<'static> {
    let color = if focused { ACCENT } else { BORDER };

    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            format!(" {} ", title.into()),
            Style::default().fg(color),
        ))
}

fn highlight_symbol(focused: bool) -> Line<'static> {
    let color = if focused { ACCENT } else { MUTED };

    Line::from(Span::styled("▌ ", Style::default().fg(color)))
}

fn highlight_style(focused: bool) -> Style {
    let background = if focused {
        SELECTED_BG
    } else {
        SELECTED_BG_DIM
    };

    Style::default().bg(background).add_modifier(Modifier::BOLD)
}

fn base_style() -> Style {
    Style::default().bg(BG).fg(TEXT)
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
            state.draft_list.select(Some(1));
            state.error = Some("a readable error".to_string());
            state.generation = crate::state::GenerationState::Generating;

            let area = Rect::new(0, 0, 100, 30);
            let mut buf = Buffer::empty(area);
            render_ui(&mut state, area, &mut buf);
        }
    }

    #[test]
    fn workspace_shows_the_selected_draft_in_the_preview() {
        let mut state = UiState::default();
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);

        render_ui(&mut state, area, &mut buf);
        let text = buffer_text(&buf);

        assert!(text.contains("blurp"));
        assert!(text.contains("Inbox · 3"));
        assert!(text.contains("Running late tonight"));
        assert!(text.contains("can't make it tonight"));
        assert!(text.contains("review"));
        assert!(text.contains("sample drafts in memory"));
    }

    #[test]
    fn empty_sections_render_a_calm_message() {
        let mut state = UiState::default();
        state
            .drafts
            .retain(|draft| draft.status != DraftStatus::Snoozed);
        state.sections.select(Some(Section::Snoozed.index()));
        state.section = Section::Snoozed;
        state.sync_draft_selection();

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        render_ui(&mut state, area, &mut buf);

        assert!(buffer_text(&buf).contains("Nothing snoozed"));
        assert_eq!(state.draft_list.selected(), None);
    }

    #[test]
    fn selected_row_highlight_covers_all_item_lines() {
        let mut state = UiState::default();
        state.draft_list.select(Some(1));

        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        render_ui(&mut state, area, &mut buf);

        let y = (0..area.height)
            .find(|&y| {
                (21..70)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .contains("Invoice follow-up")
            })
            .expect("selected title should render");

        for offset in 0..4 {
            assert_eq!(
                buf[(30, y + offset)].bg,
                SELECTED_BG,
                "highlight should cover line {offset}"
            );
        }
    }

    fn buffer_text(buf: &Buffer) -> String {
        let mut text = String::new();

        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf[(x, y)].symbol());
            }
            text.push('\n');
        }

        text
    }
}
