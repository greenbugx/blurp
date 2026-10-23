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

const MIN_WIDTH: u16 = 48;
const MIN_HEIGHT: u16 = 10;
const FULL_WIDTH: u16 = 80;
const FULL_HEIGHT: u16 = 18;
const COMPACT_WIDTH: u16 = 56;
const COMPACT_HEIGHT: u16 = 14;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HomeLayout {
    TooSmall,
    Focused,
    Compact,
    Full,
}

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
        Screen::Review => render_review(state, area, buf),
        Screen::Settings => render_settings(state, area, buf),
    }

    if let Some(popup) = state.popup {
        let popup_bounds = if area.height >= MIN_HEIGHT {
            Rect {
                height: area.height - 2,
                ..area
            }
        } else {
            area
        };
        render_popup(popup, state.sample_data, popup_bounds, buf);
    }
}

fn render_home(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    match home_layout(area) {
        HomeLayout::TooSmall => render_too_small(area, buf),
        HomeLayout::Focused => render_focused_home(state, area, buf),
        HomeLayout::Compact => render_compact_home(state, area, buf),
        HomeLayout::Full => render_full_home(state, area, buf),
    }
}

fn render_full_home(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    let (main, footer) = split_footer(area);
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

fn render_compact_home(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    let (main, footer) = split_footer(area);

    match state.focus {
        Focus::Sidebar => {
            let [sidebar, drafts] =
                Layout::horizontal([Constraint::Length(20), Constraint::Fill(1)]).areas(main);

            render_sidebar(state, sidebar, buf);
            render_draft_list(state, drafts, buf);
        }
        Focus::DraftList | Focus::Preview => {
            let [drafts, preview] =
                Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(main);

            render_draft_list(state, drafts, buf);
            render_preview(state, preview, buf);
        }
        Focus::QuickNote => render_quick_note(state, main, buf),
    }

    render_footer(state, footer, buf);
}

fn render_focused_home(state: &mut UiState, area: Rect, buf: &mut Buffer) {
    let (main, footer) = split_footer(area);

    match state.focus {
        Focus::Sidebar => {
            let [sidebar, drafts] =
                Layout::horizontal([Constraint::Length(20), Constraint::Fill(1)]).areas(main);

            render_sidebar(state, sidebar, buf);
            render_draft_list(state, drafts, buf);
        }
        Focus::DraftList => render_draft_list(state, main, buf),
        Focus::Preview => render_preview(state, main, buf),
        Focus::QuickNote => render_quick_note(state, main, buf),
    }

    render_footer(state, footer, buf);
}

fn split_footer(area: Rect) -> (Rect, Rect) {
    let [main, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(area);

    (main, footer)
}

fn home_layout(area: Rect) -> HomeLayout {
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        HomeLayout::TooSmall
    } else if area.width >= FULL_WIDTH && area.height >= FULL_HEIGHT {
        HomeLayout::Full
    } else if area.width >= COMPACT_WIDTH && area.height >= COMPACT_HEIGHT {
        HomeLayout::Compact
    } else {
        HomeLayout::Focused
    }
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

fn render_review(state: &UiState, area: Rect, buf: &mut Buffer) {
    if too_small(area) {
        render_too_small(area, buf);
        return;
    }

    let [body, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).areas(area);
    let block = pane_block("Review brain dump", true);
    let inner = block.inner(body);
    block.render(body, buf);

    let [hint, dump] = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(inner);
    Paragraph::new("One dump can hold several messages.\nCheck the text before submitting.\n")
        .style(Style::default().fg(MUTED))
        .wrap(Wrap { trim: true })
        .render(hint, buf);

    let lines = state
        .composer_text()
        .lines()
        .map(|line| Line::from(Span::styled(line.to_string(), Style::default().fg(TEXT))))
        .collect::<Vec<_>>();

    Paragraph::new(lines)
        .scroll((state.review_scroll, 0))
        .wrap(Wrap { trim: false })
        .render(dump, buf);

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

    Paragraph::new(shortcut_line(state, area.width)).render(shortcuts, buf);
    Paragraph::new(footer_status(state, area.width)).render(status, buf);
}

fn shortcut_line(state: &UiState, width: u16) -> Line<'static> {
    let pairs = if width < 72 {
        narrow_shortcuts(state)
    } else {
        full_shortcuts(state)
    };

    let mut spans = vec![Span::raw(" ")];
    let mut used = 1usize;

    for (index, (key, action)) in pairs.iter().enumerate() {
        let separator_width = if index == 0 { 0 } else { 3 };
        let chunk_width = separator_width + key.chars().count() + 1 + action.chars().count();

        if used + chunk_width > width as usize {
            break;
        }

        if index > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(BORDER)));
        }
        spans.push(Span::styled(*key, Style::default().fg(ACCENT)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*action, Style::default().fg(MUTED)));
        used += chunk_width;
    }

    Line::from(spans)
}

fn full_shortcuts(state: &UiState) -> &'static [(&'static str, &'static str)] {
    if let Some(popup) = state.popup {
        return match popup {
            Popup::Help => &[("Esc", "close"), ("q", "quit")],
            Popup::ConfirmQuit => &[("Enter", "quit"), ("Esc", "cancel")],
            Popup::SelectTone => &[("Esc", "close")],
            Popup::DiscardDump => &[("Enter", "discard"), ("Esc", "keep")],
        };
    }

    match state.screen {
        Screen::Compose => &[
            ("type", "freely"),
            ("Ctrl+S", "review"),
            ("Esc", "workspace"),
        ],
        Screen::Review => &[
            ("Enter", "submit"),
            ("e", "edit"),
            ("x", "discard"),
            ("j/k", "scroll"),
            ("Esc", "workspace"),
            ("?", "help"),
        ],
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
}

fn narrow_shortcuts(state: &UiState) -> &'static [(&'static str, &'static str)] {
    if let Some(popup) = state.popup {
        return match popup {
            Popup::Help => &[("Esc", "close"), ("q", "quit")],
            Popup::ConfirmQuit => &[("Enter", "quit"), ("Esc", "cancel")],
            Popup::SelectTone => &[("Esc", "close")],
            Popup::DiscardDump => &[("Enter", "discard"), ("Esc", "keep")],
        };
    }

    match state.screen {
        Screen::Compose => &[("Ctrl+S", "review"), ("Esc", "workspace")],
        Screen::Review => &[
            ("Enter", "submit"),
            ("e", "edit"),
            ("x", "discard"),
            ("Esc", "back"),
        ],
        Screen::Settings => &[("Esc", "back"), ("q", "quit")],
        Screen::Home => match state.focus {
            Focus::Sidebar => &[
                ("j/k", "section"),
                ("Enter", "open"),
                ("Tab", "pane"),
                ("q", "quit"),
            ],
            Focus::DraftList => &[
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
}

fn footer_status(state: &UiState, width: u16) -> Line<'static> {
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

    if let Some(notice) = &state.notice {
        return Line::from(vec![
            Span::styled(" ✓ ", Style::default().fg(ACCENT)),
            Span::styled(notice.clone(), Style::default().fg(MUTED)),
        ]);
    }

    if matches!(state.screen, Screen::Compose | Screen::Review) {
        return Line::from(Span::styled(
            " local-only · your text stays in this session",
            Style::default().fg(MUTED),
        ));
    }

    let sample_status = if width < 72 {
        "sample drafts · not saved yet"
    } else {
        "sample drafts in memory · nothing is saved yet"
    };

    if state.sample_data {
        return Line::from(vec![
            Span::styled(" ● ", Style::default().fg(ACCENT)),
            Span::styled(sample_status, Style::default().fg(MUTED)),
        ]);
    }

    Line::from(vec![
        Span::styled(" ● ", Style::default().fg(MUTED)),
        Span::styled("local-only · nothing saved yet", Style::default().fg(MUTED)),
    ])
}

fn render_popup(popup: Popup, sample_data: bool, area: Rect, buf: &mut Buffer) {
    let (max_width, max_height) = match popup {
        Popup::Help => (58, 12),
        Popup::ConfirmQuit => (58, 9),
        Popup::SelectTone => (58, 7),
        Popup::DiscardDump => (58, 9),
    };
    let popup_area = centered_rect(area, max_width, max_height);
    if popup_area.width < 20 || popup_area.height < 5 {
        return;
    }

    let clear_area = if popup_area.y == area.y || popup_area.bottom() >= area.bottom() {
        Rect {
            x: area.x,
            width: area.width,
            y: popup_area.y,
            height: popup_area.height,
        }
    } else {
        popup_area
    };
    Clear.render(clear_area, buf);
    buf.set_style(clear_area, base_style());

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" {} ", popup.title()),
            Style::default().fg(ACCENT),
        ));
    let inner = block.inner(popup_area);
    block.render(popup_area, buf);

    Paragraph::new(format!("\n{}\n", popup.body(sample_data)))
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
    let height = area.height.min(max_height);

    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn too_small(area: Rect) -> bool {
    home_layout(area) == HomeLayout::TooSmall
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_is_safe_across_sizes_screens_and_popups() {
        let sizes = [
            (1, 1),
            (20, 5),
            (47, 9),
            (48, 10),
            (55, 13),
            (56, 14),
            (70, 20),
            (79, 17),
            (79, 40),
            (80, 17),
            (80, 18),
            (120, 40),
        ];
        let screens = [
            Screen::Home,
            Screen::Compose,
            Screen::Review,
            Screen::Settings,
        ];
        let popups = [
            None,
            Some(Popup::Help),
            Some(Popup::ConfirmQuit),
            Some(Popup::SelectTone),
            Some(Popup::DiscardDump),
        ];

        for (width, height) in sizes {
            for screen in screens {
                for popup in popups {
                    let mut state = UiState {
                        screen,
                        popup,
                        ..UiState::demo()
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
            for (width, height) in [(50, 16), (70, 20), (100, 30)] {
                let mut state = UiState {
                    focus,
                    ..UiState::demo()
                };
                state.draft_list.select(Some(1));
                state.error = Some("a readable error".to_string());
                state.generation = crate::state::GenerationState::Generating;

                let area = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(area);
                render_ui(&mut state, area, &mut buf);
            }
        }
    }

    #[test]
    fn workspace_shows_the_selected_draft_in_the_preview() {
        let mut state = UiState::demo();
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
    fn empty_workspace_shows_a_calm_start_and_no_sample_label() {
        let mut state = UiState::default();
        let text = render_text(&mut state, Rect::new(0, 0, 120, 40));

        assert!(text.contains("Nothing needs a look right now"));
        assert!(text.contains("No draft selected."));
        assert!(text.contains("local-only · nothing saved yet"));
        assert!(!text.contains("sample drafts"));
    }

    #[test]
    fn quit_prompt_mentions_sample_data_only_in_demo_mode() {
        let mut demo = UiState::demo();
        demo.open_popup(Popup::ConfirmQuit);
        let demo_text = render_text(&mut demo, Rect::new(0, 0, 80, 18));
        assert!(demo_text.contains("Sample drafts reset on the next launch."));

        let mut empty = UiState::default();
        empty.open_popup(Popup::ConfirmQuit);
        let empty_text = render_text(&mut empty, Rect::new(0, 0, 80, 18));
        assert!(empty_text.contains("Nothing is saved between launches yet."));
        assert!(!empty_text.contains("Sample drafts reset"));
    }

    #[test]
    fn empty_sections_render_a_calm_message() {
        let mut state = UiState::demo();
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
        let mut state = UiState::demo();
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

    #[test]
    fn layout_breakpoints_are_explicit() {
        assert_eq!(home_layout(Rect::new(0, 0, 80, 18)), HomeLayout::Full);
        assert_eq!(home_layout(Rect::new(0, 0, 79, 18)), HomeLayout::Compact);
        assert_eq!(home_layout(Rect::new(0, 0, 56, 14)), HomeLayout::Compact);
        assert_eq!(home_layout(Rect::new(0, 0, 55, 14)), HomeLayout::Focused);
        assert_eq!(home_layout(Rect::new(0, 0, 48, 10)), HomeLayout::Focused);
        assert_eq!(home_layout(Rect::new(0, 0, 47, 10)), HomeLayout::TooSmall);
        assert_eq!(home_layout(Rect::new(0, 0, 120, 13)), HomeLayout::Focused);
    }

    #[test]
    fn full_layout_keeps_all_three_panes_and_the_quick_note() {
        let mut state = UiState::demo();
        let text = render_text(&mut state, Rect::new(0, 0, 80, 18));

        assert!(text.contains("blurp"));
        assert!(text.contains("Inbox · 3"));
        assert!(text.contains("Preview"));
        assert!(text.contains("Quick note"));
    }

    #[test]
    fn compact_layout_collapses_the_pane_that_is_not_focused() {
        let mut state = UiState::demo();
        let text = render_text(&mut state, Rect::new(0, 0, 70, 20));

        assert!(text.contains("Inbox · 3"));
        assert!(text.contains("Preview"));
        assert!(!text.contains("Quick note"));
        assert!(!text.contains("blurp"));

        state.focus = Focus::Sidebar;
        let text = render_text(&mut state, Rect::new(0, 0, 70, 20));

        assert!(text.contains("blurp"));
        assert!(!text.contains("Preview"));
    }

    #[test]
    fn narrow_layout_shows_only_the_focused_pane() {
        let mut state = UiState::demo();
        let text = render_text(&mut state, Rect::new(0, 0, 50, 16));

        assert!(text.contains("Inbox · 3"));
        assert!(!text.contains("Preview"));
        assert!(!text.contains("Quick note"));

        state.focus = Focus::Preview;
        let text = render_text(&mut state, Rect::new(0, 0, 50, 16));

        assert!(text.contains("Preview"));
        assert!(text.contains("can't make it tonight"));
    }

    #[test]
    fn tiny_layout_explains_the_minimum_size() {
        let mut state = UiState::demo();
        let text = render_text(&mut state, Rect::new(0, 0, 47, 9));

        assert!(text.contains("Terminal too small"));
        assert!(text.contains("48×10"));
    }

    #[test]
    fn footer_shortcuts_fit_the_available_width() {
        for width in [48, 56, 70, 72, 80, 120] {
            for focus in Focus::ALL {
                for popup in [
                    None,
                    Some(Popup::Help),
                    Some(Popup::ConfirmQuit),
                    Some(Popup::SelectTone),
                    Some(Popup::DiscardDump),
                ] {
                    let state = UiState {
                        focus,
                        popup,
                        ..UiState::demo()
                    };
                    let line = shortcut_line(&state, width);
                    let status = footer_status(&state, width);

                    assert!(
                        line.width() <= width as usize,
                        "shortcuts overflow at width {width} for {focus:?}"
                    );
                    assert!(
                        status.width() <= width as usize,
                        "status overflows at width {width} for {focus:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn footer_shortcuts_follow_the_active_popup() {
        let mut state = UiState::demo();

        state.open_popup(Popup::Help);
        let text = line_text(shortcut_line(&state, 100));
        assert!(text.contains("Esc close"));
        assert!(text.contains("q quit"));

        state.open_popup(Popup::ConfirmQuit);
        let text = line_text(shortcut_line(&state, 100));
        assert!(text.contains("Enter quit"));
        assert!(text.contains("Esc cancel"));

        state.open_popup(Popup::DiscardDump);
        let text = line_text(shortcut_line(&state, 100));
        assert!(text.contains("Enter discard"));
        assert!(text.contains("Esc keep"));
    }

    #[test]
    fn compose_and_review_show_their_actions() {
        let mut state = UiState::demo();
        state.start_compose();
        state
            .composer
            .insert_str("Tell Anya I can't make it tonight.");

        let compose = render_text(&mut state, Rect::new(0, 0, 80, 18));
        assert!(compose.contains("New brain dump"));
        assert!(compose.contains("Ctrl+S review"));
        assert!(compose.contains("Tell Anya I can't make it tonight."));

        state.start_review();
        let review = render_text(&mut state, Rect::new(0, 0, 80, 18));
        assert!(review.contains("Review brain dump"));
        assert!(review.contains("Tell Anya I can't make it tonight."));
        assert!(review.contains("Enter submit"));
        assert!(review.contains("e edit"));
        assert!(review.contains("x discard"));
        assert!(review.contains("Esc workspace"));
    }

    #[test]
    fn review_scroll_moves_the_rendered_dump() {
        let mut state = UiState::demo();
        state.start_compose();
        state
            .composer
            .insert_str("first intention\nsecond intention\nthird intention");
        state.start_review();

        let area = Rect::new(0, 0, 80, 18);
        let initial = render_text(&mut state, area);
        assert!(initial.contains("first intention"));

        state.scroll_review(2);
        let scrolled = render_text(&mut state, area);
        assert!(!scrolled.contains("first intention"));
        assert!(scrolled.contains("third intention"));
    }

    #[test]
    fn popup_footer_stays_visible_at_minimum_size() {
        let mut state = UiState {
            popup: Some(Popup::Help),
            ..UiState::demo()
        };

        let text = render_text(&mut state, Rect::new(0, 0, 48, 10));

        assert!(text.contains("Esc close"));
        assert!(text.contains("q quit"));
        assert!(
            text.lines()
                .next()
                .expect("first line")
                .trim_start()
                .starts_with("╭ Help")
        );
    }

    #[test]
    fn notices_and_review_status_fit_the_footer() {
        for width in [48, 56, 72, 80, 120] {
            let width_u16 = width as u16;
            let mut state = UiState {
                notice: Some("Dump ready · local drafting not connected".to_string()),
                ..UiState::demo()
            };
            assert!(footer_status(&state, width_u16).width() <= width);

            state.notice = Some("Dump discarded".to_string());
            assert!(footer_status(&state, width_u16).width() <= width);

            state.notice = None;
            state.screen = Screen::Review;
            assert!(footer_status(&state, width_u16).width() <= width);
            assert!(line_text(shortcut_line(&state, width_u16)).contains("submit"));

            let empty = UiState::default();
            assert!(footer_status(&empty, width_u16).width() <= width);
        }
    }

    fn render_text(state: &mut UiState, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        render_ui(state, area, &mut buf);
        buffer_text(&buf)
    }

    fn line_text(line: Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
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
