use crate::contract::Draft;
use crate::draft::{DraftCard, DraftStatus, Tone, sample_drafts};
use ratatui::style::{Color, Style};
use ratatui::widgets::ListState;
use ratatui_textarea::{Input, TextArea};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Screen {
    #[default]
    Home,
    Compose,
    Review,
    Edit,
    Settings,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Focus {
    Sidebar,
    #[default]
    DraftList,
    Preview,
    QuickNote,
}

impl Focus {
    pub const ALL: [Self; 4] = [
        Self::Sidebar,
        Self::DraftList,
        Self::Preview,
        Self::QuickNote,
    ];

    pub const fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::DraftList,
            Self::DraftList => Self::Preview,
            Self::Preview => Self::QuickNote,
            Self::QuickNote => Self::Sidebar,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Sidebar => Self::QuickNote,
            Self::DraftList => Self::Sidebar,
            Self::Preview => Self::DraftList,
            Self::QuickNote => Self::Preview,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Popup {
    Help,
    ConfirmQuit,
    SelectTone,
    DiscardDump,
    DiscardEdit,
    ConfirmDelete,
    ConfirmRedraft,
}

impl Popup {
    pub const fn title(self) -> &'static str {
        match self {
            Self::Help => "Help",
            Self::ConfirmQuit => "Quit blurp?",
            Self::SelectTone => "Select tone",
            Self::DiscardDump => "Discard dump?",
            Self::DiscardEdit => "Discard changes?",
            Self::ConfirmDelete => "Delete this draft?",
            Self::ConfirmRedraft => "Replace your edits?",
        }
    }

    pub const fn body(self, sample_data: bool) -> &'static str {
        match self {
            Self::Help => {
                "n                 new brain dump\n\
                 Tab / Shift+Tab   switch pane\n\
                 j / k, up / down  move selection\n\
                 Home / End        first / last item\n\
                 Enter             open / select\n\
                 e                 edit draft\n\
                 c                 copy draft\n\
                 t                 change tone\n\
                 m                 mark done / undo\n\
                 z                 snooze / wake\n\
                 d                 delete draft\n\
                 /                 search drafts\n\
                 Esc               back\n\
                 ?                 this help\n\
                 q                 quit"
            }
            Self::ConfirmQuit => {
                if sample_data {
                    "Sample drafts reset on the next launch.\n\
                     Anything half-typed is left behind.\n\n\
                     Enter or y to quit. Esc or n to keep going."
                } else {
                    "Drafts persist on this machine.\n\
                     Anything half-typed is left behind.\n\n\
                     Enter or y to quit. Esc or n to keep going."
                }
            }
            Self::SelectTone => {
                "The draft is regenerated locally.\n\n\
                 j / k to choose. Enter to apply. Esc to close."
            }
            Self::DiscardDump => {
                "This clears the text in the composer.\n\n\
                 Enter or y to discard. Esc or n to keep it."
            }
            Self::DiscardEdit => {
                "The edited text is not saved.\n\n\
                 Enter or y to discard. Esc or n to keep editing."
            }
            Self::ConfirmDelete => {
                "This removes the draft from local storage.\n\
                 The app never sends messages for you.\n\n\
                 Enter or y to delete. Esc or n to keep it."
            }
            Self::ConfirmRedraft => {
                "This draft was edited by hand.\n\
                 Regenerating replaces the saved version.\n\n\
                 Enter or y to regenerate. Esc or n to keep your edits."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GenerationState {
    #[default]
    Idle,
    Generating,
}

impl GenerationState {
    pub const fn is_generating(self) -> bool {
        matches!(self, Self::Generating)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Section {
    #[default]
    Inbox,
    Drafts,
    Snoozed,
    Completed,
    Settings,
}

impl Section {
    pub const ALL: [Self; 5] = [
        Self::Inbox,
        Self::Drafts,
        Self::Snoozed,
        Self::Completed,
        Self::Settings,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Inbox => "Inbox",
            Self::Drafts => "Drafts",
            Self::Snoozed => "Snoozed",
            Self::Completed => "Completed",
            Self::Settings => "Settings",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or_default()
    }

    pub const fn accepts(self, status: DraftStatus) -> bool {
        matches!(
            (self, status),
            (Self::Inbox, DraftStatus::Review)
                | (Self::Drafts, DraftStatus::Draft)
                | (Self::Snoozed, DraftStatus::Snoozed)
                | (Self::Completed, DraftStatus::Completed)
        )
    }
}

#[derive(Debug)]
pub struct UiState {
    pub screen: Screen,
    pub focus: Focus,
    pub popup: Option<Popup>,

    pub section: Section,
    pub sections: ListState,
    pub drafts: Vec<DraftCard>,
    pub draft_list: ListState,
    pub sample_data: bool,

    pub composer: TextArea<'static>,
    pub quick_note: TextArea<'static>,
    pub review_scroll: u16,

    pub editor: TextArea<'static>,
    pub editing_id: Option<String>,
    pub tone_selection: usize,
    pub pending_tone: Option<Tone>,
    pub search_input: bool,
    pub search_query: String,

    pub settings_models: Option<Vec<String>>,
    pub settings_models_error: Option<String>,
    pub settings_model_list: ListState,
    pub settings_current_model: Option<String>,

    pub generation: GenerationState,
    pub error: Option<String>,
    pub notice: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        let mut sections = ListState::default();
        sections.select(Some(Section::Inbox.index()));

        let mut state = Self {
            screen: Screen::Home,
            focus: Focus::DraftList,
            popup: None,
            section: Section::Inbox,
            sections,
            drafts: Vec::new(),
            draft_list: ListState::default(),
            sample_data: false,
            composer: composer_editor(),
            quick_note: quick_note_editor(),
            review_scroll: 0,
            editor: TextArea::default(),
            editing_id: None,
            tone_selection: 0,
            pending_tone: None,
            search_input: false,
            search_query: String::new(),
            settings_models: None,
            settings_models_error: None,
            settings_model_list: ListState::default(),
            settings_current_model: None,
            generation: GenerationState::Idle,
            error: None,
            notice: None,
        };
        state.sync_draft_selection();

        state
    }
}

impl UiState {
    /// Development/demo state with clearly labeled in-memory sample drafts.
    ///
    /// This is only used by the explicit `--demo` entry point. Never use it
    /// when loading or saving real user data.
    pub fn demo() -> Self {
        let mut state = Self {
            drafts: sample_drafts(),
            sample_data: true,
            ..Self::default()
        };
        state.sync_draft_selection();

        state
    }

    pub fn with_drafts(cards: Vec<DraftCard>) -> Self {
        let mut state = Self {
            drafts: cards,
            ..Self::default()
        };
        state.sync_draft_selection();

        state
    }
}

fn composer_editor() -> TextArea<'static> {
    let mut editor = TextArea::default();
    editor.set_placeholder_text("Dump it all here. One dump can hold several messages.");
    editor.set_placeholder_style(Style::default().fg(Color::DarkGray));
    editor
}

fn quick_note_editor() -> TextArea<'static> {
    let mut editor = TextArea::default();
    editor.set_placeholder_text("Tab here to jot a thought before it escapes.");
    editor.set_placeholder_style(Style::default().fg(Color::DarkGray));
    editor
}

impl UiState {
    pub fn focus_next(&mut self) {
        self.focus = self.focus.next();
    }

    pub fn focus_previous(&mut self) {
        self.focus = self.focus.previous();
    }

    pub fn open_popup(&mut self, popup: Popup) {
        self.popup = Some(popup);
    }

    pub fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }

    pub const fn popup_is_open(&self) -> bool {
        self.popup.is_some()
    }

    pub fn select_next_section(&mut self) {
        self.select_section_offset(1);
    }

    pub fn select_previous_section(&mut self) {
        self.select_section_offset(-1);
    }

    pub fn select_first_section(&mut self) {
        self.select_section_index(0);
    }

    pub fn select_last_section(&mut self) {
        self.select_section_index(Section::ALL.len() - 1);
    }

    fn select_section_offset(&mut self, delta: isize) {
        let count = Section::ALL.len();
        let current = self.sections.selected().unwrap_or(self.section.index());
        let current = current.min(count - 1);
        let next = (current as isize + delta).rem_euclid(count as isize) as usize;

        self.select_section_index(next);
    }

    fn select_section_index(&mut self, index: usize) {
        let index = index.min(Section::ALL.len() - 1);
        self.sections.select(Some(index));
        self.section = Section::ALL[index];
        self.selector_changed();
    }

    fn selector_changed(&mut self) {
        self.draft_list.select(Some(0));
        *self.draft_list.offset_mut() = 0;
        self.sync_draft_selection();
    }

    pub fn section_draft_count(&self, section: Section) -> usize {
        self.drafts
            .iter()
            .filter(|draft| section.accepts(draft.status))
            .count()
    }

    pub fn visible_drafts(&self) -> impl Iterator<Item = &DraftCard> {
        self.drafts
            .iter()
            .filter(|draft| self.matches_filter(draft))
    }

    fn matches_filter(&self, draft: &DraftCard) -> bool {
        if self.search_query.is_empty() {
            return self.section.accepts(draft.status);
        }

        let query = self.search_query.to_lowercase();
        if draft.title.to_lowercase().contains(&query) {
            return true;
        }
        if draft.body.to_lowercase().contains(&query) {
            return true;
        }

        draft
            .recipient
            .as_deref()
            .map(|recipient| recipient.to_lowercase().contains(&query))
            .unwrap_or(false)
    }

    pub fn visible_draft_count(&self) -> usize {
        self.visible_drafts().count()
    }

    pub fn draft_at(&self, index: usize) -> Option<&DraftCard> {
        self.visible_drafts().nth(index)
    }

    pub fn selected_draft(&self) -> Option<&DraftCard> {
        let index = self.draft_list.selected()?;
        self.draft_at(index)
    }

    pub fn sync_draft_selection(&mut self) {
        let count = self.visible_draft_count();
        if count == 0 {
            self.draft_list.select(None);
        } else if self
            .draft_list
            .selected()
            .is_none_or(|index| index >= count)
        {
            self.draft_list.select(Some(0));
        }
    }

    pub fn select_next_draft(&mut self, count: usize) {
        if count == 0 {
            self.draft_list.select(None);
            return;
        }

        let next = match self.draft_list.selected() {
            Some(index) if index + 1 < count => index + 1,
            _ => 0,
        };
        self.draft_list.select(Some(next));
    }

    pub fn select_previous_draft(&mut self, count: usize) {
        if count == 0 {
            self.draft_list.select(None);
            return;
        }

        let next = match self.draft_list.selected() {
            Some(index) if index > 0 && index < count => index - 1,
            _ => count - 1,
        };
        self.draft_list.select(Some(next));
    }

    pub fn select_first_draft(&mut self) {
        if self.visible_draft_count() == 0 {
            self.draft_list.select(None);
            return;
        }

        self.draft_list.select(Some(0));
        *self.draft_list.offset_mut() = 0;
    }

    pub fn select_last_draft(&mut self) {
        let count = self.visible_draft_count();
        self.draft_list.select(count.checked_sub(1));
    }

    pub fn start_compose(&mut self) {
        self.popup = None;
        self.error = None;
        self.notice = None;
        self.review_scroll = 0;
        self.screen = Screen::Compose;
    }

    pub fn leave_compose(&mut self) {
        self.popup = None;
        self.review_scroll = 0;
        self.screen = Screen::Home;
        self.focus = Focus::DraftList;
    }

    pub fn start_review(&mut self) -> bool {
        if self.composer_text().trim().is_empty() {
            self.screen = Screen::Compose;
            self.set_error("Add a thought before reviewing.");
            return false;
        }

        self.popup = None;
        self.error = None;
        self.notice = None;
        self.review_scroll = 0;
        self.screen = Screen::Review;

        true
    }

    pub fn leave_review(&mut self) {
        self.popup = None;
        self.screen = Screen::Compose;
    }

    pub fn start_drafting(&mut self) -> bool {
        if self.generation.is_generating() {
            return false;
        }
        if self.composer_text().trim().is_empty() {
            self.set_error("Add a thought before submitting.");
            return false;
        }

        self.popup = None;
        self.review_scroll = 0;
        self.screen = Screen::Home;
        self.focus = Focus::DraftList;
        self.begin_generation();

        true
    }

    pub fn complete_drafting(&mut self, cards: Vec<DraftCard>) {
        let count = cards.len();
        let selected_id = self.selected_draft().map(|draft| draft.id.clone());
        for card in cards {
            self.drafts.insert(0, card);
        }
        self.generation = GenerationState::Idle;
        self.error = None;
        self.review_scroll = 0;
        self.composer = composer_editor();
        let noun = if count == 1 { "draft" } else { "drafts" };
        self.notice = Some(format!("{count} {noun} saved"));
        self.restore_selection(selected_id.as_deref());
    }

    fn restore_selection(&mut self, id: Option<&str>) {
        let index = id
            .and_then(|id| self.visible_drafts().position(|draft| draft.id == id))
            .or_else(|| (self.visible_draft_count() > 0).then_some(0));
        self.draft_list.select(index);
    }

    pub fn cancel_generation(&mut self) {
        self.generation = GenerationState::Idle;
        self.error = None;
        self.notice = Some("Drafting cancelled. Dump kept.".to_string());
    }

    pub fn begin_tone_select(&mut self) {
        let Some(card) = self.selected_draft() else {
            return;
        };
        self.tone_selection = Tone::ALL
            .iter()
            .position(|tone| *tone == card.tone)
            .unwrap_or(0);
        self.popup = Some(Popup::SelectTone);
    }

    pub fn cycle_tone(&mut self, delta: isize) {
        let count = Tone::ALL.len();
        let next = (self.tone_selection as isize + delta).rem_euclid(count as isize) as usize;
        self.tone_selection = next;
    }

    pub fn apply_redraft(&mut self, id: &str, draft: Draft) {
        let tone = draft.tone;
        if let Some(card) = self.drafts.iter_mut().find(|card| card.id == id) {
            card.title = draft.title;
            card.body = draft.body;
            card.tone = draft.tone;
            card.status = draft.status;
            card.edited = false;
        }
        self.generation = GenerationState::Idle;
        self.error = None;
        self.notice = Some(format!("Draft redrafted in a {} tone", tone.label()));
    }

    pub fn start_editing(&mut self) {
        let Some(card) = self.selected_draft().cloned() else {
            return;
        };
        let mut editor = TextArea::default();
        editor.insert_str(card.body.clone());
        self.editor = editor;
        self.editing_id = Some(card.id);
        self.popup = None;
        self.error = None;
        self.notice = None;
        self.screen = Screen::Edit;
    }

    pub fn editor_text(&self) -> String {
        self.editor.lines().join("\n")
    }

    pub fn input_editor(&mut self, input: Input) {
        self.editor.input(input);
    }

    pub fn edit_text_changed(&self) -> bool {
        match self
            .editing_id
            .as_deref()
            .and_then(|id| self.drafts.iter().find(|card| card.id == id))
        {
            Some(card) => self.editor_text() != card.body,
            None => false,
        }
    }

    pub fn editing_title(&self) -> Option<String> {
        let id = self.editing_id.as_deref()?;
        Some(self.drafts.iter().find(|card| card.id == id)?.title.clone())
    }

    pub fn apply_edit(&mut self) {
        let Some(id) = self.editing_id.clone() else {
            return;
        };
        let body = self.editor_text();
        if let Some(card) = self.drafts.iter_mut().find(|card| card.id == id) {
            card.body = body;
            card.edited = true;
        }
        self.leave_edit();
        self.notice = Some("Draft saved".to_string());
    }

    pub fn leave_edit(&mut self) {
        self.editing_id = None;
        self.popup = None;
        self.screen = Screen::Home;
        self.focus = Focus::Preview;
    }

    pub fn apply_status(&mut self, status: DraftStatus) {
        let index = self.draft_list.selected();
        if let Some(card) = self.selected_draft_mut() {
            card.status = status;
        }
        let count = self.visible_draft_count();
        let next = index.and_then(|index| (count > 0).then(|| index.min(count - 1)));
        self.draft_list.select(next);
    }

    pub fn remove_selected_draft(&mut self) {
        let Some(index) = self.draft_list.selected() else {
            return;
        };
        let Some(card) = self.draft_at(index) else {
            return;
        };
        let id = card.id.clone();
        self.drafts.retain(|draft| draft.id != id);

        let count = self.visible_draft_count();
        let next = (count > 0).then(|| index.min(count - 1));
        self.draft_list.select(next);
    }

    fn selected_draft_mut(&mut self) -> Option<&mut DraftCard> {
        let index = self.draft_list.selected()?;
        let id = self.draft_at(index)?.id.clone();
        self.drafts.iter_mut().find(|draft| draft.id == id)
    }

    pub fn start_search(&mut self) {
        self.search_input = true;
        self.error = None;
        self.notice = None;
    }

    pub fn close_search_input(&mut self) {
        self.search_input = false;
    }

    pub fn stop_search(&mut self) {
        self.search_input = false;
        self.search_query.clear();
        self.sync_draft_selection();
    }

    pub fn push_search(&mut self, character: char) {
        self.search_query.push(character);
        self.sync_draft_selection();
    }

    pub fn pop_search(&mut self) {
        self.search_query.pop();
        self.sync_draft_selection();
    }

    pub fn discard_compose(&mut self) {
        self.clear_composer();
        self.popup = None;
        self.error = None;
        self.review_scroll = 0;
        self.notice = Some("Dump discarded".to_string());
        self.screen = Screen::Home;
        self.focus = Focus::DraftList;
    }

    pub fn scroll_review(&mut self, delta: isize) {
        let max = self.review_line_count().saturating_sub(1);
        let max = max.min(u16::MAX as usize) as u16;
        let amount = delta.unsigned_abs().min(u16::MAX as usize) as u16;

        if delta < 0 {
            self.review_scroll = self.review_scroll.saturating_sub(amount);
        } else {
            self.review_scroll = self.review_scroll.saturating_add(amount);
        }
        self.review_scroll = self.review_scroll.min(max);
    }

    pub fn review_to_start(&mut self) {
        self.review_scroll = 0;
    }

    pub fn review_to_end(&mut self) {
        let last_line = self.review_line_count().saturating_sub(1);
        self.review_scroll = last_line.min(u16::MAX as usize) as u16;
    }

    pub fn review_line_count(&self) -> usize {
        self.composer_text().lines().count().max(1)
    }

    pub fn clear_composer(&mut self) {
        self.composer = composer_editor();
    }

    pub fn open_settings(&mut self, current_model: String) {
        self.popup = None;
        self.screen = Screen::Settings;
        self.settings_models = None;
        self.settings_models_error = None;
        self.settings_model_list = ListState::default();
        self.settings_current_model = Some(current_model);
    }

    pub const fn models_loading(&self) -> bool {
        self.settings_models.is_none() && self.settings_models_error.is_none()
    }

    pub fn apply_models(&mut self, models: Vec<String>) {
        let selected = self
            .settings_current_model
            .as_deref()
            .and_then(|current| models.iter().position(|model| model == current))
            .unwrap_or(0);
        self.settings_model_list.select(Some(selected));
        self.settings_models = Some(models);
        self.settings_models_error = None;
    }

    pub fn fail_models(&mut self, message: String) {
        self.settings_models = None;
        self.settings_models_error = Some(message);
    }

    pub fn retry_models(&mut self) {
        self.settings_models = None;
        self.settings_models_error = None;
    }

    pub fn select_next_model(&mut self) {
        self.move_model_selection(1);
    }

    pub fn select_previous_model(&mut self) {
        self.move_model_selection(-1);
    }

    fn move_model_selection(&mut self, delta: isize) {
        let Some(models) = &self.settings_models else {
            return;
        };
        let count = models.len();
        if count == 0 {
            return;
        }

        let current = self
            .settings_model_list
            .selected()
            .unwrap_or(0)
            .min(count - 1);
        let next = (current as isize + delta).rem_euclid(count as isize) as usize;
        self.settings_model_list.select(Some(next));
    }

    pub fn selected_model_name(&self) -> Option<String> {
        let index = self.settings_model_list.selected()?;
        self.settings_models.as_ref()?.get(index).cloned()
    }

    pub fn leave_settings(&mut self) {
        self.screen = Screen::Home;
        self.focus = Focus::Sidebar;
        self.sections.select(Some(Section::Settings.index()));
        self.section = Section::Settings;
        self.selector_changed();
    }

    pub fn begin_generation(&mut self) {
        self.generation = GenerationState::Generating;
        self.error = None;
        self.notice = None;
    }

    pub fn finish_generation(&mut self) {
        self.generation = GenerationState::Idle;
        self.error = None;
    }

    pub fn fail_generation(&mut self, message: impl Into<String>) {
        self.generation = GenerationState::Idle;
        self.error = Some(message.into());
        self.notice = None;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.notice = None;
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn input_composer(&mut self, input: Input) -> bool {
        self.composer.input(input)
    }

    pub fn input_quick_note(&mut self, input: Input) -> bool {
        self.quick_note.input(input)
    }

    pub fn composer_text(&self) -> String {
        self.composer.lines().join("\n")
    }

    pub fn quick_note_text(&self) -> String {
        self.quick_note.lines().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_starts_with_an_empty_workspace() {
        let state = UiState::default();

        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::DraftList);
        assert_eq!(state.section, Section::Inbox);
        assert_eq!(state.sections.selected(), Some(Section::Inbox.index()));
        assert!(state.drafts.is_empty());
        assert!(!state.sample_data);
        assert_eq!(state.draft_list.selected(), None);
        assert_eq!(state.selected_draft(), None);
        assert_eq!(state.popup, None);
        assert_eq!(state.generation, GenerationState::Idle);
        assert!(state.error.is_none());
    }

    #[test]
    fn demo_state_starts_with_sample_drafts() {
        let state = UiState::demo();

        assert!(state.sample_data);
        assert_eq!(state.drafts.len(), 7);
        assert_eq!(state.draft_list.selected(), Some(0));
        assert_eq!(
            state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );
    }

    #[test]
    fn focus_moves_in_a_cycle() {
        assert_eq!(Focus::Sidebar.next(), Focus::DraftList);
        assert_eq!(Focus::DraftList.next(), Focus::Preview);
        assert_eq!(Focus::Preview.next(), Focus::QuickNote);
        assert_eq!(Focus::QuickNote.next(), Focus::Sidebar);

        assert_eq!(Focus::Sidebar.previous(), Focus::QuickNote);
        assert_eq!(Focus::DraftList.previous(), Focus::Sidebar);
        assert_eq!(Focus::Preview.previous(), Focus::DraftList);
        assert_eq!(Focus::QuickNote.previous(), Focus::Preview);
    }

    #[test]
    fn changing_focus_preserves_unrelated_state() {
        let mut state = UiState::demo();
        state.draft_list.select(Some(2));
        *state.draft_list.offset_mut() = 1;
        state.composer.insert_str("keep this dump");
        state.quick_note.insert_str("keep this note");
        state.fail_generation("ollama is not running");

        state.focus_next();
        state.focus_next();
        state.focus_previous();

        assert_eq!(state.draft_list.selected(), Some(2));
        assert_eq!(state.draft_list.offset(), 1);
        assert_eq!(state.composer_text(), "keep this dump");
        assert_eq!(state.quick_note_text(), "keep this note");
        assert_eq!(state.error.as_deref(), Some("ollama is not running"));
    }

    #[test]
    fn popup_round_trip_preserves_workspace_state() {
        let mut state = UiState {
            section: Section::Drafts,
            focus: Focus::Preview,
            ..UiState::demo()
        };
        state.sections.select(Some(Section::Drafts.index()));
        state.draft_list.select(Some(1));
        *state.draft_list.offset_mut() = 1;
        state.composer.insert_str("unfinished thought");

        state.open_popup(Popup::Help);
        assert!(state.popup_is_open());

        assert!(state.close_popup());

        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::Preview);
        assert_eq!(state.section, Section::Drafts);
        assert_eq!(state.sections.selected(), Some(Section::Drafts.index()));
        assert_eq!(state.draft_list.selected(), Some(1));
        assert_eq!(state.draft_list.offset(), 1);
        assert_eq!(state.composer_text(), "unfinished thought");
        assert!(!state.close_popup(), "closing twice should be harmless");
    }

    #[test]
    fn sidebar_navigation_wraps_and_tracks_selection() {
        let mut state = UiState::demo();

        state.select_next_section();
        assert_eq!(state.section, Section::Drafts);
        assert_eq!(state.sections.selected(), Some(1));
        assert_eq!(state.draft_list.selected(), Some(0));

        state.select_previous_section();
        assert_eq!(state.section, Section::Inbox);
        assert_eq!(state.sections.selected(), Some(0));

        state.select_previous_section();
        assert_eq!(state.section, Section::Settings);
        assert_eq!(state.sections.selected(), Some(4));
        assert_eq!(state.draft_list.selected(), None);

        state.select_next_section();
        assert_eq!(state.section, Section::Inbox);
        assert_eq!(state.draft_list.selected(), Some(0));
    }

    #[test]
    fn sections_filter_drafts_and_reset_selection() {
        let mut state = UiState::demo();

        assert_eq!(state.visible_draft_count(), 3);
        assert_eq!(state.selected_draft().unwrap().status, DraftStatus::Review);

        state.select_next_section();
        assert_eq!(state.section, Section::Drafts);
        assert_eq!(state.visible_draft_count(), 2);
        assert_eq!(state.selected_draft().unwrap().status, DraftStatus::Draft);

        state.select_next_draft(state.visible_draft_count());
        assert_eq!(state.draft_list.selected(), Some(1));

        state.select_previous_section();
        assert_eq!(state.section, Section::Inbox);
        assert_eq!(state.draft_list.selected(), Some(0));
        assert_eq!(
            state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );
    }

    #[test]
    fn first_and_last_section_shortcuts_jump_to_edges() {
        let mut state = UiState::demo();

        state.select_last_section();
        assert_eq!(state.section, Section::Settings);
        assert_eq!(state.sections.selected(), Some(Section::Settings.index()));
        assert_eq!(state.draft_list.selected(), None);

        state.select_first_section();
        assert_eq!(state.section, Section::Inbox);
        assert_eq!(state.sections.selected(), Some(Section::Inbox.index()));
        assert_eq!(state.draft_list.selected(), Some(0));
    }

    #[test]
    fn first_and_last_draft_shortcuts_respect_the_section() {
        let mut state = UiState::demo();

        state.select_last_draft();
        assert_eq!(state.draft_list.selected(), Some(2));

        state.select_first_draft();
        assert_eq!(state.draft_list.selected(), Some(0));

        state.select_next_section();
        state.select_last_draft();
        assert_eq!(state.draft_list.selected(), Some(1));
        assert_eq!(
            state.selected_draft().map(|draft| draft.status),
            Some(DraftStatus::Draft)
        );
    }

    #[test]
    fn draft_navigation_wraps_and_handles_an_empty_list() {
        let mut state = UiState::demo();

        state.select_next_draft(0);
        assert_eq!(state.draft_list.selected(), None);

        state.select_next_draft(3);
        assert_eq!(state.draft_list.selected(), Some(0));

        state.select_next_draft(3);
        assert_eq!(state.draft_list.selected(), Some(1));

        state.select_next_draft(3);
        assert_eq!(state.draft_list.selected(), Some(2));

        state.select_next_draft(3);
        assert_eq!(state.draft_list.selected(), Some(0));

        state.select_previous_draft(3);
        assert_eq!(state.draft_list.selected(), Some(2));

        state.select_next_draft(0);
        assert_eq!(state.draft_list.selected(), None);
    }

    #[test]
    fn compose_text_survives_leaving_and_returning() {
        let mut state = UiState::default();

        state.start_compose();
        state
            .composer
            .insert_str("Tell Anya I can't make it tonight.");
        state.leave_compose();

        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::DraftList);
        assert_eq!(state.composer_text(), "Tell Anya I can't make it tonight.");

        state.start_compose();
        assert_eq!(state.screen, Screen::Compose);
        assert_eq!(state.composer_text(), "Tell Anya I can't make it tonight.");

        state.clear_composer();
        assert_eq!(state.composer_text(), "");
    }

    #[test]
    fn compose_flows_through_review_and_submit_without_losing_text() {
        let mut state = UiState::default();

        state.start_compose();
        state.composer.insert_str(
            "Tell Anya I can't make it tonight.\nAsk Dev whether the invoice was paid.",
        );

        assert!(state.start_review());
        assert_eq!(state.screen, Screen::Review);
        assert_eq!(state.review_scroll, 0);

        state.scroll_review(1);
        assert_eq!(state.review_scroll, 1);
        state.review_to_start();
        assert_eq!(state.review_scroll, 0);
        state.review_to_end();
        assert_eq!(state.review_scroll, 1);

        state.leave_review();
        assert_eq!(state.screen, Screen::Compose);
        assert!(state.composer_text().contains("Ask Dev"));

        assert!(state.start_review());
        assert!(state.start_drafting());
        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::DraftList);
        assert!(state.generation.is_generating());
        assert!(state.composer_text().contains("Ask Dev"));
    }

    #[test]
    fn empty_dumps_cannot_be_reviewed_or_submitted() {
        let mut state = UiState::default();
        state.start_compose();

        assert!(!state.start_review());
        assert_eq!(state.screen, Screen::Compose);
        assert_eq!(
            state.error.as_deref(),
            Some("Add a thought before reviewing.")
        );

        state.composer.insert_str("   \n  ");
        assert!(!state.start_review());
        assert_eq!(state.screen, Screen::Compose);
        assert_eq!(
            state.error.as_deref(),
            Some("Add a thought before reviewing.")
        );

        state.screen = Screen::Review;
        assert!(!state.start_drafting());
        assert_eq!(state.screen, Screen::Review);
        assert_eq!(
            state.error.as_deref(),
            Some("Add a thought before submitting.")
        );
        assert_eq!(state.composer_text().trim(), "");
    }

    #[test]
    fn completed_drafting_adds_cards_and_clears_the_composer() {
        let mut state = UiState::default();
        state.start_compose();
        state.composer.insert_str("tell sam i cant make it tonight");
        assert!(state.start_drafting());

        state.complete_drafting(vec![card("Cancel tonight")]);

        assert_eq!(state.generation, GenerationState::Idle);
        assert!(state.composer_text().is_empty());
        assert_eq!(state.notice.as_deref(), Some("1 draft saved"));
        assert_eq!(state.drafts.len(), 1);
        assert_eq!(state.drafts[0].title, "Cancel tonight");
        assert_eq!(state.draft_list.selected(), Some(0));
        assert!(state.section.accepts(state.drafts[0].status));
    }

    #[test]
    fn completed_drafting_counts_and_orders_multiple_cards() {
        let mut state = UiState::default();

        state.complete_drafting(vec![card("First"), card("Second")]);

        assert_eq!(state.notice.as_deref(), Some("2 drafts saved"));
        assert_eq!(state.drafts[0].title, "Second");
        assert_eq!(state.drafts[1].title, "First");
    }

    #[test]
    fn cancelling_and_failing_drafting_keep_the_dump() {
        let mut state = UiState::default();
        state.start_compose();
        state.composer.insert_str("keep me");
        assert!(state.start_drafting());

        state.cancel_generation();
        assert_eq!(state.generation, GenerationState::Idle);
        assert!(state.error.is_none());
        assert_eq!(
            state.notice.as_deref(),
            Some("Drafting cancelled. Dump kept.")
        );
        assert_eq!(state.composer_text(), "keep me");

        assert!(state.start_drafting());
        state.fail_generation("Ollama is not reachable");
        assert_eq!(state.generation, GenerationState::Idle);
        assert_eq!(state.error.as_deref(), Some("Ollama is not reachable"));
        assert!(state.notice.is_none());
        assert_eq!(state.composer_text(), "keep me");
    }

    #[test]
    fn completing_a_run_keeps_the_selected_draft_by_id() {
        let mut state = UiState::demo();
        state.select_next_draft(state.visible_draft_count());
        assert_eq!(state.selected_draft().unwrap().title, "Invoice follow-up");

        state.complete_drafting(vec![card("Newest")]);

        assert_eq!(state.drafts[0].title, "Newest");
        assert_eq!(
            state.selected_draft().unwrap().title,
            "Invoice follow-up",
            "selection must follow the id, not the position"
        );
    }

    #[test]
    fn completing_into_an_empty_workspace_selects_the_first_new_draft() {
        let mut state = UiState::default();

        state.complete_drafting(vec![card("Only")]);

        assert_eq!(state.selected_draft().unwrap().title, "Only");
    }

    #[test]
    fn editing_round_trips_through_the_editor() {
        let mut state = UiState::demo();

        state.start_editing();
        assert_eq!(state.screen, Screen::Edit);
        assert_eq!(state.editing_id.as_deref(), Some("1"));
        assert_eq!(
            state.editing_title().as_deref(),
            Some("Running late tonight")
        );
        assert_eq!(state.editor_text(), state.drafts[0].body);
        assert!(!state.edit_text_changed());

        state.editor.insert_str("extra");
        assert!(state.edit_text_changed());

        state.apply_edit();
        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::Preview);
        assert_eq!(state.editing_id, None);
        assert!(state.drafts[0].edited);
        assert!(state.drafts[0].body.ends_with("extra"));
        assert_eq!(state.notice.as_deref(), Some("Draft saved"));
    }

    #[test]
    fn search_filters_across_sections_and_resets_cleanly() {
        let mut state = UiState::demo();
        assert_eq!(state.visible_draft_count(), 3);

        state.start_search();
        state.push_search('i');
        state.push_search('n');
        state.push_search('v');
        assert!(state.search_input);

        let titles = state
            .visible_drafts()
            .map(|draft| draft.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(titles, vec!["Invoice follow-up"]);
        assert_eq!(state.draft_list.selected(), Some(0));

        state.close_search_input();
        assert!(!state.search_input);
        assert_eq!(state.visible_draft_count(), 1);

        state.stop_search();
        assert_eq!(state.search_query, "");
        assert_eq!(state.visible_draft_count(), 3);
    }

    #[test]
    fn status_updates_move_drafts_between_sections() {
        let mut state = UiState::demo();

        state.apply_status(DraftStatus::Completed);

        assert_eq!(state.drafts[0].status, DraftStatus::Completed);
        assert_eq!(state.visible_draft_count(), 2);
        assert_eq!(state.selected_draft().unwrap().status, DraftStatus::Review);
    }

    #[test]
    fn redraft_updates_one_card_in_place() {
        let mut state = UiState::demo();
        state.start_editing();
        state.apply_edit();
        assert!(state.drafts[0].edited);

        let draft = Draft {
            id: uuid::Uuid::new_v4(),
            title: "Firmer goodbye".to_string(),
            recipient: Some("Anya".to_string()),
            body: "Anya, I cannot make it tonight.".to_string(),
            tone: Tone::Firm,
            status: DraftStatus::Draft,
            missing_details: Vec::new(),
        };

        state.apply_redraft("1", draft);

        assert_eq!(state.drafts[0].title, "Firmer goodbye");
        assert_eq!(state.drafts[0].tone, Tone::Firm);
        assert!(!state.drafts[0].edited);
        assert_eq!(
            state.notice.as_deref(),
            Some("Draft redrafted in a firm tone")
        );
        assert_eq!(state.generation, GenerationState::Idle);
    }

    #[test]
    fn removing_the_selected_draft_keeps_a_neighbour_selected() {
        let mut state = UiState::demo();
        state.select_next_draft(state.visible_draft_count());
        assert_eq!(state.selected_draft().unwrap().title, "Invoice follow-up");

        state.remove_selected_draft();

        assert_eq!(state.visible_draft_count(), 2);
        assert_eq!(
            state.selected_draft().unwrap().title,
            "Monday submission",
            "the next draft takes the vacated slot"
        );
    }

    fn card(title: &str) -> DraftCard {
        DraftCard {
            id: title.to_lowercase().replace(' ', "-"),
            title: title.to_string(),
            recipient: None,
            tone: crate::draft::Tone::Casual,
            status: DraftStatus::Review,
            body: "body".to_string(),
            source_dump: None,
            edited: false,
        }
    }

    #[test]
    fn discarding_a_dump_clears_the_composer_and_returns_home() {
        let mut state = UiState::default();
        state.start_compose();
        state.composer.insert_str("A thought to discard.");

        state.discard_compose();

        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::DraftList);
        assert_eq!(state.composer_text(), "");
        assert_eq!(state.notice.as_deref(), Some("Dump discarded"));
        assert!(state.error.is_none());
    }

    #[test]
    fn starting_a_compose_clears_stale_notices_and_errors() {
        let mut state = UiState::default();
        state.set_error("previous failure");
        state.notice = Some("previous notice".to_string());

        state.start_compose();

        assert_eq!(state.screen, Screen::Compose);
        assert!(state.error.is_none());
        assert!(state.notice.is_none());
    }

    #[test]
    fn generation_and_error_state_are_independent_of_navigation() {
        let mut state = UiState::default();

        state.begin_generation();
        assert!(state.generation.is_generating());

        state.open_popup(Popup::SelectTone);
        state.close_popup();
        state.focus_next();

        assert!(state.generation.is_generating());

        state.fail_generation("model not downloaded");
        assert_eq!(state.generation, GenerationState::Idle);
        assert_eq!(state.error.as_deref(), Some("model not downloaded"));

        state.clear_error();
        assert!(state.error.is_none());
    }
}
