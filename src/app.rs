use crate::contract::{Draft, DraftBatch, DraftRequest};
use crate::db::Store;
use crate::draft::{DraftCard, DraftStatus, Tone};
use crate::event::{AppEvent, Event, EventHandler};
use crate::ollama::{MODEL_ENV, OllamaClient, OllamaConfig, OllamaError};
use crate::state::{Focus, Popup, Screen, Section, UiState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub state: UiState,
    pub events: EventHandler,
    pub client_config: OllamaConfig,
    pub store: Option<Store>,
    drafting: Option<JoinHandle<()>>,
    redrafting: Option<JoinHandle<()>>,
    models_task: Option<JoinHandle<()>>,
    generation_id: u64,
}

impl App {
    fn base(state: UiState) -> Self {
        Self {
            running: true,
            state,
            events: EventHandler::new(),
            client_config: OllamaConfig::default(),
            store: None,
            drafting: None,
            redrafting: None,
            models_task: None,
            generation_id: 0,
        }
    }

    pub fn new(store: Store) -> color_eyre::Result<Self> {
        let cards: Vec<DraftCard> = store
            .drafts()?
            .into_iter()
            .map(|stored| {
                let mut card = DraftCard::from(&stored.draft);
                card.source_dump = Some(stored.dump);
                card.edited = stored.edited;
                card
            })
            .collect();
        let mut app = Self::base(UiState::with_drafts(cards));
        if let Some(saved) = store.model_setting()?
            && std::env::var(MODEL_ENV).is_err()
        {
            app.client_config.model = saved;
        }
        app.store = Some(store);

        Ok(app)
    }

    pub fn demo() -> Self {
        Self::base(UiState::demo())
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
                    AppEvent::DraftResult { id, result } => self.on_draft_result(id, result),
                    AppEvent::RedraftResult {
                        id,
                        draft_id,
                        result,
                    } => self.on_redraft_result(id, draft_id, result),
                    AppEvent::ModelsResult { result } => self.on_models_result(result),
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

        if self.state.search_input {
            self.handle_search_key(key_event);
            return Ok(());
        }

        match self.state.screen {
            Screen::Compose => self.handle_compose_key(key_event),
            Screen::Review => self.handle_review_key(key_event),
            Screen::Edit => self.handle_edit_key(key_event),
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
            Popup::SelectTone => match key_event.code {
                KeyCode::Esc => {
                    self.state.close_popup();
                }
                KeyCode::Enter => {
                    let tone = Tone::ALL[self.state.tone_selection];
                    self.state.close_popup();
                    self.confirm_redraft(tone);
                }
                KeyCode::Down | KeyCode::Char('j') => self.state.cycle_tone(1),
                KeyCode::Up | KeyCode::Char('k') => self.state.cycle_tone(-1),
                _ => {}
            },
            Popup::ConfirmRedraft => match key_event.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    self.state.close_popup();
                    if let Some(tone) = self.state.pending_tone.take() {
                        self.start_redrafting(tone);
                    }
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') | KeyCode::Backspace => {
                    self.state.pending_tone = None;
                    self.state.close_popup();
                }
                _ => {}
            },
            Popup::DiscardEdit => match key_event.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    self.state.close_popup();
                    self.state.leave_edit();
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') | KeyCode::Backspace => {
                    self.state.close_popup();
                }
                _ => {}
            },
            Popup::ConfirmDelete => match key_event.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    self.state.close_popup();
                    self.delete_selected();
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') | KeyCode::Backspace => {
                    self.state.close_popup();
                }
                _ => {}
            },
            Popup::DiscardDump => match key_event.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    self.state.close_popup();
                    self.state.discard_compose();
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') | KeyCode::Backspace => {
                    self.state.close_popup();
                }
                _ => {}
            },
        }
    }

    fn handle_compose_key(&mut self, key_event: KeyEvent) {
        if key_event.code == KeyCode::Esc {
            self.state.leave_compose();
            return;
        }

        if is_review_shortcut(key_event) {
            self.state.start_review();
            return;
        }

        self.state.clear_error();
        self.state.input_composer(key_event.into());
    }

    fn handle_review_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.state.leave_compose(),
            KeyCode::Enter => self.start_drafting(),
            KeyCode::Char('e') => self.state.leave_review(),
            KeyCode::Char('x') => self.state.open_popup(Popup::DiscardDump),
            KeyCode::Char('?') => self.state.open_popup(Popup::Help),
            KeyCode::Char('q') => self.state.open_popup(Popup::ConfirmQuit),
            KeyCode::Down | KeyCode::Char('j') => self.state.scroll_review(1),
            KeyCode::Up | KeyCode::Char('k') => self.state.scroll_review(-1),
            KeyCode::PageDown => self.state.scroll_review(5),
            KeyCode::PageUp => self.state.scroll_review(-5),
            KeyCode::Home => self.state.review_to_start(),
            KeyCode::End => self.state.review_to_end(),
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.state.leave_settings(),
            KeyCode::Char('?') => self.state.open_popup(Popup::Help),
            KeyCode::Char('q') => self.state.open_popup(Popup::ConfirmQuit),
            KeyCode::Char('r') => {
                self.state.retry_models();
                self.fetch_models();
            }
            KeyCode::Enter => self.choose_selected_model(),
            KeyCode::Down | KeyCode::Char('j') => self.state.select_next_model(),
            KeyCode::Up | KeyCode::Char('k') => self.state.select_previous_model(),
            _ => {}
        }
    }

    fn fetch_models(&mut self) {
        if self.models_task.is_some() {
            return;
        }

        let client = match OllamaClient::new(self.client_config.clone()) {
            Ok(client) => client,
            Err(error) => {
                self.state.fail_models(error.to_string());
                return;
            }
        };

        let sender = self.events.sender();
        self.models_task = Some(tokio::spawn(async move {
            let result = client.models().await;
            let _ = sender.send(Event::App(AppEvent::ModelsResult { result }));
        }));
    }

    fn on_models_result(&mut self, result: Result<Vec<String>, OllamaError>) {
        self.models_task = None;

        match result {
            Ok(models) => self.state.apply_models(models),
            Err(error) => self.state.fail_models(error.to_string()),
        }
    }

    fn choose_selected_model(&mut self) {
        let Some(name) = self.state.selected_model_name() else {
            return;
        };
        if name == self.client_config.model {
            return;
        }

        if let Some(store) = &self.store
            && let Err(error) = store.set_model_setting(&name)
        {
            self.state.set_error(error.to_string());
            return;
        }

        self.client_config.model = name.clone();
        self.state.settings_current_model = Some(name.clone());
        if let Some(models) = self.state.settings_models.clone() {
            self.state.apply_models(models);
        }
        self.state.error = None;
        self.state.notice = Some(format!("Model switched to {name}"));
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
            KeyCode::Char('/') => self.state.start_search(),
            KeyCode::Char('e') if self.draft_actions_ready() => self.state.start_editing(),
            KeyCode::Char('c') if self.draft_actions_ready() => self.copy_selected(),
            KeyCode::Char('t') if self.draft_actions_ready() => self.state.begin_tone_select(),
            KeyCode::Char('m') if self.draft_actions_ready() => self.toggle_complete(),
            KeyCode::Char('z') if self.draft_actions_ready() => self.toggle_snooze(),
            KeyCode::Char('d') if self.draft_actions_ready() => {
                self.state.open_popup(Popup::ConfirmDelete);
            }
            KeyCode::Char('n') if self.state.generation.is_generating() => {}
            KeyCode::Char('n') => self.state.start_compose(),
            KeyCode::Tab => self.state.focus_next(),
            KeyCode::BackTab => self.state.focus_previous(),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Home => self.select_edge(false),
            KeyCode::End => self.select_edge(true),
            KeyCode::Enter => self.activate_focused(),
            KeyCode::Esc if self.state.generation.is_generating() => self.cancel_drafting(),
            KeyCode::Esc if !self.state.search_query.is_empty() => self.state.stop_search(),
            KeyCode::Esc => self.escape_focused(),
            _ => {}
        }
    }

    fn draft_actions_ready(&self) -> bool {
        matches!(self.state.focus, Focus::DraftList | Focus::Preview)
            && !self.state.generation.is_generating()
    }

    fn handle_edit_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                if self.state.edit_text_changed() {
                    self.state.open_popup(Popup::DiscardEdit);
                } else {
                    self.state.leave_edit();
                }
            }
            KeyCode::Char('s') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.save_edit();
            }
            _ => {
                self.state.clear_error();
                self.state.input_editor(key_event.into());
            }
        }
    }

    fn handle_search_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => self.state.stop_search(),
            KeyCode::Enter => self.state.close_search_input(),
            KeyCode::Backspace => self.state.pop_search(),
            KeyCode::Char(character)
                if !key_event.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                self.state.push_search(character);
            }
            _ => {}
        }
    }

    fn save_edit(&mut self) {
        let Some(id) = self.state.editing_id.clone() else {
            return;
        };
        let body = self.state.editor_text();

        if let Some(store) = &self.store {
            let result = Uuid::parse_str(&id)
                .map_err(|_| format!("stored draft '{id}' has an invalid id"))
                .and_then(|uuid| {
                    store
                        .update_draft_body(uuid, &body)
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = result {
                self.state.set_error(error);
                return;
            }
        }

        self.state.apply_edit();
    }

    fn copy_selected(&mut self) {
        let Some(card) = self.state.selected_draft() else {
            return;
        };
        let body = card.body.clone();
        self.report_copy(copy_to_clipboard(&body));
    }

    fn report_copy(&mut self, result: Result<(), arboard::Error>) {
        match result {
            Ok(()) => {
                self.state.error = None;
                self.state.notice = Some("Draft copied to the clipboard".to_string());
            }
            Err(error) => self.state.set_error(format!("Could not copy: {error}")),
        }
    }

    fn toggle_complete(&mut self) {
        let Some(card) = self.state.selected_draft() else {
            return;
        };
        if card.status == DraftStatus::Completed {
            self.set_status(DraftStatus::Draft, "Moved back to Drafts.");
        } else {
            self.set_status(
                DraftStatus::Completed,
                "Marked complete. The app never sends it for you.",
            );
        }
    }

    fn toggle_snooze(&mut self) {
        let Some(card) = self.state.selected_draft() else {
            return;
        };
        if card.status == DraftStatus::Snoozed {
            self.set_status(DraftStatus::Review, "Draft moved to Inbox.");
        } else {
            self.set_status(DraftStatus::Snoozed, "Draft moved to Snoozed.");
        }
    }

    fn set_status(&mut self, status: DraftStatus, notice: &str) {
        let Some(card) = self.state.selected_draft() else {
            return;
        };
        if card.status == status {
            return;
        }
        let id = card.id.clone();

        if let Some(store) = &self.store {
            let result = Uuid::parse_str(&id)
                .map_err(|_| format!("stored draft '{id}' has an invalid id"))
                .and_then(|uuid| {
                    store
                        .update_draft_status(uuid, status)
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = result {
                self.state.set_error(error);
                return;
            }
        }

        self.state.apply_status(status);
        self.state.error = None;
        self.state.notice = Some(notice.to_string());
    }

    fn delete_selected(&mut self) {
        let Some(card) = self.state.selected_draft() else {
            return;
        };
        let id = card.id.clone();

        if let Some(store) = &self.store {
            let result = Uuid::parse_str(&id)
                .map_err(|_| format!("stored draft '{id}' has an invalid id"))
                .and_then(|uuid| store.delete_draft(uuid).map_err(|error| error.to_string()));
            if let Err(error) = result {
                self.state.set_error(error);
                return;
            }
        }

        self.state.remove_selected_draft();
        self.state.error = None;
        self.state.notice = Some("Draft deleted".to_string());
    }

    fn confirm_redraft(&mut self, tone: Tone) {
        let edited = self
            .state
            .selected_draft()
            .map(|card| card.edited)
            .unwrap_or(false);

        if edited {
            self.state.pending_tone = Some(tone);
            self.state.open_popup(Popup::ConfirmRedraft);
        } else {
            self.start_redrafting(tone);
        }
    }

    fn start_redrafting(&mut self, tone: Tone) {
        if self.state.generation.is_generating() {
            return;
        }
        let Some(card) = self.state.selected_draft() else {
            return;
        };
        let Some(dump) = card.source_dump.clone() else {
            self.state
                .set_error("This draft has no original dump to regenerate from.");
            return;
        };
        let draft_id = card.id.clone();
        let body = card.body.clone();

        let client = match OllamaClient::new(self.client_config.clone()) {
            Ok(client) => client,
            Err(error) => {
                self.state.fail_generation(error.to_string());
                return;
            }
        };

        self.generation_id += 1;
        let generation = self.generation_id;
        self.state.begin_generation();
        let sender = self.events.sender();
        self.redrafting = Some(tokio::spawn(async move {
            let result = client.redraft(&dump, &body, tone).await;
            let _ = sender.send(Event::App(AppEvent::RedraftResult {
                id: generation,
                draft_id,
                result,
            }));
        }));
    }

    fn on_redraft_result(&mut self, id: u64, draft_id: String, result: Result<Draft, OllamaError>) {
        if id != self.generation_id {
            return;
        }
        self.redrafting = None;

        match result {
            Ok(mut draft) => {
                let recipient = self
                    .state
                    .drafts
                    .iter()
                    .find(|card| card.id == draft_id)
                    .map(|card| card.recipient.clone());
                let Some(recipient) = recipient else {
                    return;
                };
                draft.recipient = recipient;

                if let Some(store) = &self.store {
                    let result = Uuid::parse_str(&draft_id)
                        .map_err(|_| format!("stored draft '{draft_id}' has an invalid id"))
                        .and_then(|uuid| {
                            draft.id = uuid;
                            store
                                .update_draft(&draft)
                                .map_err(|error| error.to_string())
                        });
                    if let Err(error) = result {
                        self.state.fail_generation(error.to_string());
                        return;
                    }
                }

                self.state.apply_redraft(&draft_id, draft);
            }
            Err(error) => self.state.fail_generation(error.to_string()),
        }
    }

    fn start_drafting(&mut self) {
        if !self.state.start_drafting() {
            return;
        }

        let request = match DraftRequest::new(self.state.composer_text(), None) {
            Ok(request) => request,
            Err(error) => {
                self.state.fail_generation(error.to_string());
                return;
            }
        };
        let client = match OllamaClient::new(self.client_config.clone()) {
            Ok(client) => client,
            Err(error) => {
                self.state.fail_generation(error.to_string());
                return;
            }
        };

        self.generation_id += 1;
        let id = self.generation_id;
        let sender = self.events.sender();
        self.drafting = Some(tokio::spawn(async move {
            let result = client.draft(&request).await;
            let _ = sender.send(Event::App(AppEvent::DraftResult { id, result }));
        }));
    }

    fn on_draft_result(&mut self, id: u64, result: Result<DraftBatch, OllamaError>) {
        if id != self.generation_id {
            return;
        }
        self.drafting = None;

        match result {
            Ok(batch) => {
                if let Some(store) = &self.store
                    && let Err(error) = store.save_batch(&batch)
                {
                    self.state.fail_generation(error.to_string());
                    return;
                }
                let cards: Vec<DraftCard> = batch.drafts.iter().map(DraftCard::from).collect();
                self.state.complete_drafting(cards);
            }
            Err(error) => self.state.fail_generation(error.to_string()),
        }
    }

    fn cancel_drafting(&mut self) {
        if let Some(handle) = self.drafting.take() {
            handle.abort();
        }
        self.generation_id += 1;
        self.state.cancel_generation();
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
                    let model = self.client_config.model.clone();
                    self.state.open_settings(model);
                    self.fetch_models();
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

fn is_review_shortcut(key_event: KeyEvent) -> bool {
    key_event.code == KeyCode::F(2)
        || (key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('s' | 'S') | KeyCode::Enter))
}

fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.set_text(text.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GenerationState;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[tokio::test]
    async fn app_starts_empty_without_the_demo_flag() {
        let app = App::new(Store::open_in_memory().unwrap()).unwrap();

        assert_eq!(app.state.screen, Screen::Home);
        assert!(!app.state.sample_data);
        assert!(app.state.drafts.is_empty());
        assert_eq!(app.state.draft_list.selected(), None);
        assert_eq!(app.state.selected_draft(), None);
    }

    #[tokio::test]
    async fn startup_loads_persisted_drafts_from_the_store() {
        let store = Store::open_in_memory().unwrap();
        let request = DraftRequest::new("tell sam i cant make it tonight", None).unwrap();
        let batch = crate::contract::parse_draft_response(&request, CASUAL_REPLY).unwrap();
        store.save_batch(&batch).unwrap();

        let app = App::new(store).unwrap();

        assert_eq!(app.state.drafts.len(), 1);
        assert_eq!(app.state.drafts[0].title, "Cancel tonight");
        assert_eq!(app.state.draft_list.selected(), Some(0));
        assert_eq!(app.state.section, Section::Inbox);
        assert!(!app.state.sample_data);
    }

    #[tokio::test]
    async fn completed_drafting_saves_the_batch_before_showing_it() {
        let mut app = App::new(Store::open_in_memory().unwrap()).unwrap();
        app.client_config = OllamaConfig::resolve(
            Some("http://127.0.0.1:1".to_string()),
            Some("test-model".to_string()),
            None,
        );
        start_drafting(&mut app);
        let id = app.generation_id;
        let request = DraftRequest::new("tell sam i cant make it tonight", None).unwrap();
        let batch = crate::contract::parse_draft_response(&request, CASUAL_REPLY).unwrap();

        app.on_draft_result(id, Ok(batch));

        let stored = app.store.as_ref().unwrap().drafts().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].draft.title, "Cancel tonight");
        assert_eq!(stored[0].draft.status, crate::draft::DraftStatus::Review);
        assert_eq!(app.state.drafts[0].title, "Cancel tonight");
        assert_eq!(app.state.notice.as_deref(), Some("1 draft saved"));
    }

    #[tokio::test]
    async fn editing_saves_through_the_store_and_marks_the_draft() {
        let mut app = stored_app();

        app.handle_key_events(key(KeyCode::Char('e'))).unwrap();
        assert_eq!(app.state.screen, Screen::Edit);

        app.state.editor.insert_str(" Handwritten.");
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();

        assert_eq!(app.state.screen, Screen::Home);
        assert!(app.state.drafts[0].body.ends_with("Handwritten."));
        assert!(app.state.drafts[0].edited);

        let stored = app.store.as_ref().unwrap().drafts().unwrap();
        assert!(stored[0].edited);
        assert_eq!(stored[0].draft.body, app.state.drafts[0].body);
        assert_eq!(app.state.notice.as_deref(), Some("Draft saved"));
    }

    #[tokio::test]
    async fn esc_from_an_edited_draft_asks_before_discarding() {
        let mut app = stored_app();

        app.handle_key_events(key(KeyCode::Char('e'))).unwrap();
        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.popup, None);

        app.handle_key_events(key(KeyCode::Char('e'))).unwrap();
        app.state.editor.insert_str(" typed");
        app.handle_key_events(key(KeyCode::Esc)).unwrap();

        assert_eq!(app.state.popup, Some(Popup::DiscardEdit));
        assert_eq!(app.state.screen, Screen::Edit);

        app.handle_key_events(key(KeyCode::Enter)).unwrap();

        assert_eq!(app.state.screen, Screen::Home);
        assert!(!app.state.drafts[0].body.ends_with(" typed"));
        assert_eq!(app.state.editing_id, None);
    }

    #[tokio::test]
    async fn status_changes_persist_through_the_store() {
        let mut app = stored_app();

        app.handle_key_events(key(KeyCode::Char('m'))).unwrap();

        assert_eq!(app.state.drafts[0].status, DraftStatus::Completed);
        assert_eq!(
            app.store.as_ref().unwrap().drafts().unwrap()[0]
                .draft
                .status,
            DraftStatus::Completed
        );
        assert_eq!(
            app.state.notice.as_deref(),
            Some("Marked complete. The app never sends it for you.")
        );

        app.state.section = Section::Completed;
        app.state.sync_draft_selection();
        app.handle_key_events(key(KeyCode::Char('m'))).unwrap();

        assert_eq!(app.state.drafts[0].status, DraftStatus::Draft);
    }

    #[tokio::test]
    async fn deleting_asks_before_removing_from_the_store() {
        let mut app = stored_app();

        app.handle_key_events(key(KeyCode::Char('d'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::ConfirmDelete));
        assert_eq!(app.state.drafts.len(), 1);

        app.handle_key_events(key(KeyCode::Enter)).unwrap();

        assert_eq!(app.state.drafts.len(), 0);
        assert!(app.store.as_ref().unwrap().drafts().unwrap().is_empty());
        assert_eq!(app.state.notice.as_deref(), Some("Draft deleted"));
    }

    #[tokio::test]
    async fn a_tone_change_redrafts_one_draft_through_the_store() {
        let mut app = stored_app();

        app.handle_key_events(key(KeyCode::Char('t'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::SelectTone));
        assert_eq!(app.state.tone_selection, 0);

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.state.tone_selection, 1);

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert!(app.state.generation.is_generating());

        let id = app.generation_id;
        let draft_id = app.state.drafts[0].id.clone();
        let request =
            DraftRequest::new("tell sam i cant make it tonight", Some(Tone::Warm)).unwrap();
        let batch = crate::contract::parse_draft_response(&request, WARM_REPLY).unwrap();

        app.on_redraft_result(id, draft_id, Ok(batch.drafts.into_iter().next().unwrap()));

        assert_eq!(app.state.generation, GenerationState::Idle);
        assert_eq!(app.state.drafts[0].tone, Tone::Warm);
        assert_eq!(
            app.state.drafts[0].recipient.as_deref(),
            Some("Sam"),
            "the recipient survives regeneration"
        );
        assert_eq!(
            app.state.notice.as_deref(),
            Some("Draft redrafted in a warm tone")
        );

        let stored = app.store.as_ref().unwrap().drafts().unwrap();
        assert_eq!(stored[0].draft.tone, Tone::Warm);
    }

    #[tokio::test]
    async fn an_edited_draft_asks_before_regenerating() {
        let mut app = stored_app();

        app.handle_key_events(key(KeyCode::Char('e'))).unwrap();
        app.state.editor.insert_str(" typed");
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();
        assert!(app.state.drafts[0].edited);

        app.handle_key_events(key(KeyCode::Char('t'))).unwrap();
        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert_eq!(app.state.popup, Some(Popup::ConfirmRedraft));
        assert!(!app.state.generation.is_generating());

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert!(app.state.generation.is_generating());
    }

    #[tokio::test]
    async fn search_filters_and_navigates_results() {
        let mut app = drafting_app();

        app.handle_key_events(key(KeyCode::Char('/'))).unwrap();
        assert!(app.state.search_input);

        for character in "invoice".chars() {
            app.handle_key_events(key(KeyCode::Char(character)))
                .unwrap();
        }
        assert_eq!(app.state.search_query, "invoice");
        assert_eq!(app.state.visible_draft_count(), 1);
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Invoice follow-up")
        );

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert!(!app.state.search_input);
        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.state.draft_list.selected(), Some(0));

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.search_query, "");
        assert_eq!(app.state.visible_draft_count(), 3);
    }

    #[tokio::test]
    async fn the_settings_screen_lists_and_switches_models() {
        let reply = serde_json::json!({
            "models": [
                {"name": "gemma4-long:latest"},
                {"name": "gemma4:e4b"}
            ]
        })
        .to_string();
        let (base_url, _) = crate::ollama::harness::serve_once(
            crate::ollama::harness::http_response(200, "OK", &reply),
        )
        .await;
        let mut app = stored_app();
        app.client_config =
            OllamaConfig::resolve(Some(base_url), Some("gemma4:e4b".to_string()), None);

        app.state.section = Section::Settings;
        app.state.focus = Focus::Sidebar;
        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert_eq!(app.state.screen, Screen::Settings);

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            app.models_task.take().unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
        match app.events.try_next_app().expect("models event") {
            AppEvent::ModelsResult { result } => app.on_models_result(result),
            other => panic!("unexpected event {other:?}"),
        }
        assert_eq!(
            app.state.settings_models.as_deref(),
            Some(&["gemma4-long:latest".to_string(), "gemma4:e4b".to_string()] as &[_])
        );
        assert_eq!(app.state.settings_model_list.selected(), Some(1));

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        app.handle_key_events(key(KeyCode::Enter)).unwrap();

        assert_eq!(app.client_config.model, "gemma4-long:latest");
        assert_eq!(
            app.store
                .as_ref()
                .unwrap()
                .model_setting()
                .unwrap()
                .as_deref(),
            Some("gemma4-long:latest")
        );
        assert_eq!(
            app.state.notice.as_deref(),
            Some("Model switched to gemma4-long:latest")
        );
    }

    #[tokio::test]
    async fn a_saved_model_setting_is_used_when_no_env_override_exists() {
        let store = Store::open_in_memory().unwrap();
        store.set_model_setting("gemma4-long:latest").unwrap();

        let app = App::new(store).unwrap();

        if std::env::var(MODEL_ENV).is_err() {
            assert_eq!(app.client_config.model, "gemma4-long:latest");
        }
    }

    #[tokio::test]
    async fn a_failed_copy_reports_a_useful_error() {
        let mut app = drafting_app();

        app.report_copy(Err(arboard::Error::Unknown {
            description: "no clipboard provider".to_string(),
        }));

        assert!(
            app.state
                .error
                .as_deref()
                .unwrap()
                .contains("Could not copy")
        );
        assert!(app.state.notice.is_none());

        app.report_copy(Ok(()));
        assert_eq!(
            app.state.notice.as_deref(),
            Some("Draft copied to the clipboard")
        );
        assert!(app.state.error.is_none());
    }

    #[tokio::test]
    async fn quitting_works_while_an_operation_is_running() {
        let mut app = drafting_app();
        start_drafting(&mut app);
        assert!(app.state.generation.is_generating());

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::ConfirmQuit));
        assert!(app.running);

        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        match app.events.try_next_app().expect("quit event") {
            AppEvent::Quit => app.quit(),
            other => panic!("unexpected event {other:?}"),
        }
        assert!(!app.running);
    }

    #[tokio::test]
    #[ignore]
    async fn clipboard_round_trip_works_on_this_machine() {
        let mut clipboard = arboard::Clipboard::new().expect("no clipboard provider");
        clipboard
            .set_text("blurp clipboard check".to_string())
            .unwrap();
        assert_eq!(clipboard.get_text().unwrap(), "blurp clipboard check");
    }

    #[tokio::test]
    async fn demo_app_starts_with_labeled_sample_drafts() {
        let app = App::demo();

        assert!(app.state.sample_data);
        assert_eq!(app.state.drafts.len(), 7);
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );
    }

    #[tokio::test]
    async fn help_popup_opens_and_closes_without_losing_context() {
        let mut app = App::demo();
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
        let mut app = App::demo();

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::ConfirmQuit));
        assert!(app.running);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
        assert!(app.running);
    }

    #[tokio::test]
    async fn draft_selection_moves_and_opens_the_preview() {
        let mut app = App::demo();

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
        let mut app = App::demo();
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
        let mut app = App::demo();

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
        let mut app = App::demo();

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
        let mut app = App::demo();

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);
    }

    #[tokio::test]
    async fn shift_tab_cycles_focus_backwards() {
        let mut app = App::demo();

        app.handle_key_events(key(KeyCode::BackTab)).unwrap();
        assert_eq!(app.state.focus, Focus::Sidebar);

        app.handle_key_events(key(KeyCode::BackTab)).unwrap();
        assert_eq!(app.state.focus, Focus::QuickNote);
    }

    #[tokio::test]
    async fn q_from_help_opens_quit_confirmation() {
        let mut app = App::demo();

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
        let mut app = App::demo();

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
        let mut app = App::demo();

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

    const CASUAL_REPLY: &str = r#"{"drafts":[{"id":"1","title":"Cancel tonight","recipient":"Sam","body":"Hey Sam, I can't make it tonight. Does Thursday work instead?","tone":"casual","missingDetails":[],"status":"review"}]}"#;

    const WARM_REPLY: &str = r#"{"drafts":[{"id":"1","title":"Cancel tonight","recipient":"Sam","body":"Hey Sam, I can't make it tonight. Does Thursday work instead?","tone":"casual","missingDetails":[],"status":"draft"}]}"#;

    fn drafting_app() -> App {
        let mut app = App::demo();
        app.client_config = OllamaConfig::resolve(
            Some("http://127.0.0.1:1".to_string()),
            Some("test-model".to_string()),
            None,
        );
        app
    }

    fn stored_app() -> App {
        let store = Store::open_in_memory().unwrap();
        let request = DraftRequest::new("tell sam i cant make it tonight", None).unwrap();
        let batch = crate::contract::parse_draft_response(&request, CASUAL_REPLY).unwrap();
        store.save_batch(&batch).unwrap();

        let mut app = App::new(store).unwrap();
        app.client_config = OllamaConfig::resolve(
            Some("http://127.0.0.1:1".to_string()),
            Some("test-model".to_string()),
            None,
        );
        app
    }

    fn start_drafting(app: &mut App) {
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        app.state
            .composer
            .insert_str("Tell Anya I can't make it tonight.");
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();
        app.handle_key_events(key(KeyCode::Enter)).unwrap();
    }

    #[tokio::test]
    async fn submitting_a_review_starts_drafting_and_keeps_the_dump() {
        let mut app = drafting_app();

        start_drafting(&mut app);

        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.focus, Focus::DraftList);
        assert!(app.state.generation.is_generating());
        assert!(app.state.error.is_none());
        assert_eq!(
            app.state.composer_text(),
            "Tell Anya I can't make it tonight."
        );
    }

    #[tokio::test]
    async fn duplicate_submits_are_blocked_while_drafting() {
        let mut app = drafting_app();
        start_drafting(&mut app);
        let first_id = app.generation_id;
        app.drafting = None;

        app.start_drafting();
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();

        assert!(app.state.generation.is_generating());
        assert_eq!(app.generation_id, first_id);
        assert!(app.drafting.is_none(), "blocked submit must not spawn");
        assert_eq!(app.state.screen, Screen::Home);
    }

    #[tokio::test]
    async fn failed_drafting_reports_an_error_and_allows_a_retry() {
        let mut app = drafting_app();
        start_drafting(&mut app);
        let id = app.generation_id;

        app.on_draft_result(
            id,
            Err(OllamaError::NotRunning {
                url: "http://localhost:11434".to_string(),
            }),
        );

        assert_eq!(app.state.generation, GenerationState::Idle);
        assert!(app.state.error.as_deref().unwrap().contains("ollama serve"));
        assert_eq!(
            app.state.composer_text(),
            "Tell Anya I can't make it tonight."
        );
        assert!(app.drafting.is_none());

        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        assert_eq!(app.state.screen, Screen::Compose);
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();
        app.handle_key_events(key(KeyCode::Enter)).unwrap();
        assert!(app.state.generation.is_generating(), "retry must work");
    }

    #[tokio::test]
    async fn completed_drafting_adds_cards_and_clears_the_composer() {
        let mut app = drafting_app();
        start_drafting(&mut app);
        let id = app.generation_id;
        let request = DraftRequest::new("tell sam i cant make it tonight", None).unwrap();
        let batch = crate::contract::parse_draft_response(&request, CASUAL_REPLY).unwrap();

        app.on_draft_result(id, Ok(batch));

        assert_eq!(app.state.generation, GenerationState::Idle);
        assert!(app.state.composer_text().is_empty());
        assert_eq!(app.state.notice.as_deref(), Some("1 draft saved"));
        assert_eq!(app.state.drafts.len(), 8);
        assert_eq!(app.state.drafts[0].title, "Cancel tonight");
        assert_eq!(app.state.section, Section::Inbox);
        assert_eq!(app.state.draft_list.selected(), Some(1));
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight"),
            "selection follows the id across the refresh"
        );
    }

    #[tokio::test]
    async fn esc_cancels_drafting_and_ignores_stale_results() {
        let mut app = drafting_app();
        start_drafting(&mut app);
        let stale_id = app.generation_id;

        app.handle_key_events(key(KeyCode::Esc)).unwrap();

        assert_eq!(app.state.generation, GenerationState::Idle);
        assert!(app.drafting.is_none());
        assert_eq!(
            app.state.notice.as_deref(),
            Some("Drafting cancelled. Dump kept.")
        );
        assert_eq!(
            app.state.composer_text(),
            "Tell Anya I can't make it tonight."
        );

        let request = DraftRequest::new("tell sam i cant make it tonight", None).unwrap();
        let batch = crate::contract::parse_draft_response(&request, CASUAL_REPLY).unwrap();
        app.on_draft_result(stale_id, Ok(batch));

        assert_eq!(
            app.state.drafts[0].title, "Running late tonight",
            "stale result must be ignored"
        );
        assert_eq!(app.state.drafts.len(), 7);
        assert_eq!(
            app.state.notice.as_deref(),
            Some("Drafting cancelled. Dump kept.")
        );
    }

    #[tokio::test]
    async fn navigation_and_help_stay_responsive_while_drafting() {
        let mut app = drafting_app();
        start_drafting(&mut app);

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(
            app.state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Invoice follow-up")
        );

        app.handle_key_events(key(KeyCode::Char('?'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::Help));

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
        assert!(app.state.generation.is_generating());
        assert_eq!(app.state.screen, Screen::Home);
    }

    #[tokio::test]
    async fn drafting_completes_through_the_event_loop() {
        let reply =
            serde_json::json!({"message": {"role": "assistant", "content": CASUAL_REPLY}, "done": true})
                .to_string();
        let (base_url, received) = crate::ollama::harness::serve_once(
            crate::ollama::harness::http_response(200, "OK", &reply),
        )
        .await;

        let mut app = App::demo();
        app.client_config =
            OllamaConfig::resolve(Some(base_url), Some("test-model".to_string()), None);
        start_drafting(&mut app);
        assert!(app.state.generation.is_generating());

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            app.drafting.take().unwrap(),
        )
        .await
        .unwrap()
        .unwrap();

        match app.events.try_next_app().expect("draft result event") {
            AppEvent::DraftResult { id, result } => app.on_draft_result(id, result),
            other => panic!("unexpected app event: {other:?}"),
        }

        assert_eq!(app.state.generation, GenerationState::Idle);
        assert_eq!(app.state.drafts.len(), 8);
        assert_eq!(app.state.drafts[0].title, "Cancel tonight");
        assert_eq!(app.state.notice.as_deref(), Some("1 draft saved"));

        let sent = received.await.unwrap();
        assert!(sent.contains("\"test-model\""));
    }

    #[tokio::test]
    async fn f2_also_opens_the_review_step() {
        let mut app = App::demo();
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        app.state.composer.insert_str("A thought.");

        app.handle_key_events(key(KeyCode::F(2))).unwrap();

        assert_eq!(app.state.screen, Screen::Review);
    }

    #[tokio::test]
    async fn empty_compose_blocks_review_with_an_error() {
        let mut app = App::demo();
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();

        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();

        assert_eq!(app.state.screen, Screen::Compose);
        assert_eq!(
            app.state.error.as_deref(),
            Some("Add a thought before reviewing.")
        );

        app.handle_key_events(key(KeyCode::Char('a'))).unwrap();
        assert!(app.state.error.is_none());
        assert_eq!(app.state.composer_text(), "a");
    }

    #[tokio::test]
    async fn escape_from_review_returns_to_workspace_with_text() {
        let mut app = App::demo();
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        app.state.composer.insert_str("Keep this dump.");
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();
        assert_eq!(app.state.screen, Screen::Review);

        app.handle_key_events(key(KeyCode::Esc)).unwrap();

        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.composer_text(), "Keep this dump.");
    }

    #[tokio::test]
    async fn discard_from_review_requires_confirmation() {
        let mut app = App::demo();
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        app.state.composer.insert_str("Discard this dump.");
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();

        app.handle_key_events(key(KeyCode::Char('x'))).unwrap();
        assert_eq!(app.state.popup, Some(Popup::DiscardDump));

        app.handle_key_events(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.state.popup, None);
        assert_eq!(app.state.screen, Screen::Review);
        assert_eq!(app.state.composer_text(), "Discard this dump.");

        app.handle_key_events(key(KeyCode::Char('x'))).unwrap();
        app.handle_key_events(key(KeyCode::Enter)).unwrap();

        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.composer_text(), "");
        assert_eq!(app.state.notice.as_deref(), Some("Dump discarded"));
    }

    #[tokio::test]
    async fn review_scrolling_is_bounded() {
        let mut app = App::demo();
        app.handle_key_events(key(KeyCode::Char('n'))).unwrap();
        app.state.composer.insert_str("one\ntwo\nthree");
        app.handle_key_events(ctrl(KeyCode::Char('s'))).unwrap();

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.state.review_scroll, 1);

        app.handle_key_events(key(KeyCode::PageDown)).unwrap();
        assert_eq!(app.state.review_scroll, 2);

        app.handle_key_events(key(KeyCode::Char('j'))).unwrap();
        assert_eq!(app.state.review_scroll, 2);

        app.handle_key_events(key(KeyCode::Home)).unwrap();
        assert_eq!(app.state.review_scroll, 0);

        app.handle_key_events(key(KeyCode::End)).unwrap();
        assert_eq!(app.state.review_scroll, 2);
    }

    #[tokio::test]
    async fn typing_in_quick_note_does_not_trigger_global_shortcuts() {
        let mut app = App::demo();
        app.state.focus = Focus::QuickNote;

        app.handle_key_events(key(KeyCode::Char('q'))).unwrap();
        app.handle_key_events(key(KeyCode::Char('?'))).unwrap();

        assert_eq!(app.state.popup, None);
        assert_eq!(app.state.screen, Screen::Home);
        assert_eq!(app.state.quick_note_text(), "q?");
    }

    #[tokio::test]
    async fn tab_cycles_focus_and_preserves_the_editor() {
        let mut app = App::demo();
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
