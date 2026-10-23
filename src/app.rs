use crate::event::{AppEvent, Event, EventHandler};
use crate::state::{Focus, Popup, Screen, Section, UiState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub state: UiState,
    pub events: EventHandler,
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            state: UiState::default(),
            events: EventHandler::new(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event)
                        if key_event.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        self.handle_key_events(key_event)?
                    }
                    _ => {}
                },
                Event::App(app_event) => match app_event {
                    AppEvent::Quit => self.quit(),
                },
            }
        }
        Ok(())
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c' | 'C'))
        {
            self.events.send(AppEvent::Quit);
            return Ok(());
        }

        if self.state.popup_is_open() {
            self.handle_popup_key(key_event);
            return Ok(());
        }

        match self.state.screen {
            Screen::Compose => self.handle_compose_key(key_event),
            Screen::Settings => self.handle_settings_key(key_event),
            Screen::Home if self.state.focus == Focus::QuickNote => {
                self.handle_quick_note_key(key_event);
            }
            Screen::Home => self.handle_home_key(key_event),
        }

        Ok(())
    }

    fn handle_popup_key(&mut self, key_event: KeyEvent) {
        let Some(popup) = self.state.popup else {
            return;
        };

        match popup {
            Popup::Help => match key_event.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Enter => {
                    self.state.close_popup();
                }
                KeyCode::Char('q') => {
                    self.state.close_popup();
                    self.state.open_popup(Popup::ConfirmQuit);
                }
                _ => {}
            },
            Popup::ConfirmQuit => match key_event.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => self.events.send(AppEvent::Quit),
                KeyCode::Esc
                | KeyCode::Char('n' | 'N')
                | KeyCode::Char('q' | 'Q')
                | KeyCode::Backspace => {
                    self.state.close_popup();
                }
                _ => {}
            },
            Popup::SelectTone => {
                if matches!(key_event.code, KeyCode::Esc | KeyCode::Enter) {
                    self.state.close_popup();
                }
            }
        }
    }

    fn handle_compose_key(&mut self, key_event: KeyEvent) {
        if key_event.code == KeyCode::Esc {
            self.state.leave_compose();
            return;
        }

        self.state.input_composer(key_event.into());
    }

    fn handle_settings_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.state.leave_settings(),
            KeyCode::Char('?') => self.state.open_popup(Popup::Help),
            KeyCode::Char('q') => self.state.open_popup(Popup::ConfirmQuit),
            _ => {}
        }
    }

    fn handle_quick_note_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.state.focus = Focus::DraftList,
            KeyCode::Tab => self.state.focus_next(),
            KeyCode::BackTab => self.state.focus_previous(),
            _ => {
                self.state.input_quick_note(key_event.into());
            }
        }
    }

    fn handle_home_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.state.open_popup(Popup::ConfirmQuit),
            KeyCode::Char('?') => self.state.open_popup(Popup::Help),
            KeyCode::Char('n') => self.state.start_compose(),
            KeyCode::Tab => self.state.focus_next(),
            KeyCode::BackTab => self.state.focus_previous(),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Home => self.select_edge(false),
            KeyCode::End => self.select_edge(true),
            KeyCode::Enter => self.activate_focused(),
            KeyCode::Esc => self.escape_focused(),
            _ => {}
        }
    }

    fn select_edge(&mut self, last: bool) {
        match self.state.focus {
            Focus::Sidebar => {
                if last {
                    self.state.select_last_section();
                } else {
                    self.state.select_first_section();
                }
            }
            Focus::DraftList => {
                if last {
                    self.state.select_last_draft();
                } else {
                    self.state.select_first_draft();
                }
            }
            Focus::Preview | Focus::QuickNote => {}
        }
    }

    fn move_selection(&mut self, direction: isize) {
        match self.state.focus {
            Focus::Sidebar => {
                if direction > 0 {
                    self.state.select_next_section();
                } else {
                    self.state.select_previous_section();
                }
            }
            Focus::DraftList => {
                let draft_count = self.state.visible_draft_count();
                if direction > 0 {
                    self.state.select_next_draft(draft_count);
                } else {
                    self.state.select_previous_draft(draft_count);
                }
            }
            Focus::Preview | Focus::QuickNote => {}
        }
    }

    fn activate_focused(&mut self) {
        match self.state.focus {
            Focus::Sidebar => {
                if self.state.section == Section::Settings {
                    self.state.open_settings();
                } else {
                    self.state.focus = Focus::DraftList;
                }
            }
            Focus::DraftList => {
                if self.state.selected_draft().is_some() {
                    self.state.focus = Focus::Preview;
                }
            }
            Focus::Preview | Focus::QuickNote => self.state.focus = Focus::DraftList,
        }
    }

    fn escape_focused(&mut self) {
        match self.state.focus {
            Focus::Sidebar => {}
            Focus::DraftList => self.state.focus = Focus::Sidebar,
            Focus::QuickNote | Focus::Preview => self.state.focus = Focus::DraftList,
        }
    }

    pub fn tick(&self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GenerationState;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test]
    async fn help_popup_opens_and_closes_without_losing_context() {
        let mut app = App::new();
        app.state.focus = Focus::Preview;
        app.state.draft_list.select(Some(2));
        *app.state.draft_list.offset_mut() = 1;

        app.handle_key_events(key(KeyCode::Char('?'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::Help));

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.focus, Focus::Preview);
        assert_eq!(app.state.draft_list.selected(), Some(2));
        assert_eq!(app.state.draft_list.offset(), 1);
    }

    #[tokio::test]
    async fn quit_confirmation_can_be_cancelled() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::ConfirmQuit));
        assert!(app.running);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
        assert!(app.running);
    }

    #[tokio::test]
    async fn draft_selection_moves_and_opens_the_preview() {
        let mut app = App::new();

        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Invoice follow-up")
        );

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert_eq!(app.state.focus, Focus::Preview);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.focus, Focus::DraftList);
    }

    #[tokio::test]
    async fn section_selection_changes_the_visible_drafts() {
        let mut app = App::new();
        app.state.focus = Focus::Sidebar;

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.state.section, Section::Drafts);
        assert_eq!(app.state.visible_draft_count(), 2);
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Conference follow-up")
        );

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert_eq!(app.state.focus, Focus::DraftList);
    }

    #[tokio::test]
    async fn arrow_keys_match_j_and_k() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::Down)).unwrap();
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Invoice follow-up")
        );

        app.handle_key_events(key(KeyCode::Up)).unwrap();
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );
    }

    #[tokio::test]
    async fn home_and_end_jump_to_list_edges() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::End)).unwrap();
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Monday submission")
        );

        app.handle_key_events(key(KeyCode::Home)).unwrap();
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );

        app.state.focus = Focus::Sidebar;
        app.handle_key_events(key(KeyCode::End)).unwrap();
        assert_eq!(app.state.section, Section::Settings);

        app.handle_key_events(key(KeyCode::Home)).unwrap();
        assert_eq!(app.state.section, Section::Inbox);
    }

    #[tokio::test]
    async fn escape_walks_back_toward_the_sidebar() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);
    }

    #[tokio::test]
    async fn shift_tab_cycles_focus_backwards() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::BackTab)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);

        app.handle_key_events(key(KeyCode::BackTab)).unwrap();
        assert_eq!(app.state.focus, Focus::QuickNote);
    }

    #[tokio::test]
    async fn q_from_help_opens_quit_confirmation() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::Char('?'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::Help));

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::ConfirmQuit));
        assert!(app.running);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
        assert!(app.running);
    }

    #[tokio::test]
    async fn keyboard_only_flow_reaches_every_workspace_pane() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert_eq!(app.state.focus, Focus::Preview);

        app.handle_key_events(key(KeyCode::Tab)).unwrap();
        assert_eq!(app.state.focus, Focus::QuickNote);

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(app.state.quick_note_text(), "q");

        app.handle_key_events(key(KeyCode::Tab)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.state.section, Section::Drafts);

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert_eq!(app.state.focus, Focus::DraftList);

        app.handle_key_events(key(KeyCode::Char('?'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::Help));
        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
    }

    #[tokio::test]
    async fn typing_in_compose_does_not_trigger_global_shortcuts() {
        let mut app = App::new();

        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.state.screen, Screen::Compose);

        for character in ['q', '?', 'n'] {
            app.handle_key_events(key(KeyCode::Char(character)))
                .unwrap();
        }

        assert_eq!(app.state.screen, Screen::Compose);
        assert_eq!(app.state.popup, None);
        assert_eq!(app.state.composer_text(), "q?n");

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.composer_text(), "q?n");

        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.state.screen, Screen::Compose);
        assert_eq!(app.state.composer_text(), "q?n");
    }

    #[tokio::test]
    async fn typing_in_quick_note_does_not_trigger_global_shortcuts() {
        let mut app = App::new();
        app.state.focus = Focus::QuickNote;

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        app.handle_key_events(key(KeyCode::Char('?'))).unwrap();

        assert_eq!(app.state.popup, None);
        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.quick_note_text(), "q?");
    }

    #[tokio::test]
    async fn tab_cycles_focus_and_preserves_the_editor() {
        let mut app = App::new();
        app.state.start_compose();
        app.state.composer.insert_str("draft text");
        app.state.leave_compose();

        for _ in 0..Focus::ALL.len() {
            app.handle_key_events(key(KeyCode::Tab)).unwrap();
        }

        assert_eq!(app.state.focus, Focus::DraftList);
        assert_eq!(app.state.composer_text(), "draft text");
        assert_eq!(app.state.generation, GenerationState::Idle);
    }
}
