use crate::draft::{DraftCard, DraftStatus, sample_drafts};
use ratatui::style::{Color, Style};
use ratatui::widgets::ListState;
use ratatui_textarea::{Input, TextArea};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Screen {
    #[default]
    Home,
    Compose,
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
}

impl Popup {
    pub const fn title(self) -> &'static str {
        match self {
            Self::Help => "Help",
            Self::ConfirmQuit => "Quit blurp?",
            Self::SelectTone => "Select tone",
        }
    }

    pub const fn body(self) -> &'static str {
        match self {
            Self::Help => {
                "n                 new brain dump\n\
                 Tab / Shift+Tab   switch pane\n\
                 j / k, up / down  move selection\n\
                 Enter             open / select\n\
                 Esc               back\n\
                 ?                 this help\n\
                 q                 quit"
            }
            Self::ConfirmQuit => {
                "Sample drafts reset on the next launch.\n\
                 Anything half-typed is left behind.\n\n\
                 Enter or y to quit. Esc or n to keep going."
            }
            Self::SelectTone => {
                "Per-draft tone changes arrive with regeneration.\n\n\
                 Esc to close."
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

    pub composer: TextArea<'static>,
    pub quick_note: TextArea<'static>,

    pub generation: GenerationState,
    pub error: Option<String>,
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
            drafts: sample_drafts(),
            draft_list: ListState::default(),
            composer: composer_editor(),
            quick_note: quick_note_editor(),
            generation: GenerationState::Idle,
            error: None,
        };
        state.sync_draft_selection();

        state
    }
}

fn composer_editor() -> TextArea<'static> {
    let mut editor = TextArea::default();
    editor.set_placeholder_text(
        "Dump it all here.\n\nTell Anya I can't make it tonight, ask Dev about the invoice...",
    );
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

    fn select_section_offset(&mut self, delta: isize) {
        let count = Section::ALL.len();
        let current = self.sections.selected().unwrap_or(self.section.index());
        let current = current.min(count - 1);
        let next = (current as isize + delta).rem_euclid(count as isize) as usize;

        self.sections.select(Some(next));
        self.section = Section::ALL[next];
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
            .filter(|draft| self.section.accepts(draft.status))
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

    pub fn start_compose(&mut self) {
        self.popup = None;
        self.screen = Screen::Compose;
    }

    pub fn leave_compose(&mut self) {
        self.screen = Screen::Home;
        self.focus = Focus::DraftList;
    }

    pub fn clear_composer(&mut self) {
        self.composer = composer_editor();
    }

    pub fn open_settings(&mut self) {
        self.popup = None;
        self.screen = Screen::Settings;
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
    }

    pub fn finish_generation(&mut self) {
        self.generation = GenerationState::Idle;
        self.error = None;
    }

    pub fn fail_generation(&mut self, message: impl Into<String>) {
        self.generation = GenerationState::Idle;
        self.error = Some(message.into());
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
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
    fn default_state_starts_on_the_workspace() {
        let state = UiState::default();

        assert_eq!(state.screen, Screen::Home);
        assert_eq!(state.focus, Focus::DraftList);
        assert_eq!(state.section, Section::Inbox);
        assert_eq!(state.sections.selected(), Some(Section::Inbox.index()));
        assert_eq!(state.drafts.len(), 7);
        assert_eq!(state.draft_list.selected(), Some(0));
        assert_eq!(
            state.selected_draft().map(|draft| draft.title.as_str()),
            Some("Running late tonight")
        );
        assert_eq!(state.popup, None);
        assert_eq!(state.generation, GenerationState::Idle);
        assert!(state.error.is_none());
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
        let mut state = UiState::default();
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
            ..UiState::default()
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
        let mut state = UiState::default();

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
        let mut state = UiState::default();

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
    fn draft_navigation_wraps_and_handles_an_empty_list() {
        let mut state = UiState::default();

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
