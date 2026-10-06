//! The Browse screen: the shell, the sidebar, a drawn page and the
//! overlays (Field, agent line/card, Peek, space menu) over it.
//!
//! [`BrowseView`] renders from the [`story`](super::story) state machine and
//! nothing else, mirroring the assistant mock: an entity view, 1.2 s timers
//! advancing the story, intents through listeners, keyed state, and the
//! `AUI_GALLERY_STEPS` harness so `--screenshot screens/browse` can capture
//! a chosen state.

use aui::browse::agent_window::{
    agent_card, agent_column, agent_line, AgentItem, AgentTranscript, CardIntent, LineIntent, LineState, Step, StepState,
};
use aui::browse::field::{
    field, field_state, intent_for, rank, select_next, select_prev, FieldCommand, FieldHistory, FieldIntent, FieldKey,
    FieldModel, FieldRowKind, FieldSources, FieldTab,
};
use aui::browse::{
    address_line, browse_footer, browse_nav_cluster, folder_row, pin_grid, split_address, tab_row, task_folder_row, Pin, TabRowData,
    TaskFace, BROWSE_SIDEBAR_WIDTH,
};
use aui::composer::{composer_state_rows, ComposerChip, ComposerChipKind};
use aui::data::{button, record_ax_label};
use aui::keys::{Cancel, Confirm, SelectNext, SelectPrev};
use aui::overlay::popover_layer;
use aui::shell::{app_shell, sidebar_header};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::input::Textarea;

use super::pages::render_page;
use super::story::{approval_data, BrowseStory, Phase, StoryLine, StoryPage, ANSWER_TEXT, ASK_TEXT, SITE_ACTIONS};

/// The screen's own key context: the bindings below live here, never in
/// the library's keymap.
pub const BROWSE_CONTEXT: &str = "BrowseScreen";

actions!(
    browse_screen,
    [
        ShowLine,
        ShowCard,
        ToggleDock,
        NewTab,
        FocusAddress,
        HideSidebar,
        SpaceOne,
        SpaceTwo,
        TakeOver,
        HandBack,
        AdvanceStep,
        ConfirmCmd,
        CloseTop
    ]
);

/// How long the steps harness waits for the first frame.
const STEPS_DELAY: std::time::Duration = std::time::Duration::from_millis(250);

/// Which space the sidebar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Space {
    /// PGI tabs, the Education footer.
    Education,
    /// Weather and HN tabs, the Personal footer.
    Personal,
}

impl Space {
    /// The footer word and the chip menu label.
    fn name(self) -> &'static str {
        match self {
            Space::Education => "Education",
            Space::Personal => "Personal",
        }
    }
}

/// One sidebar tab: title, tile letter, page and address.
struct SpaceTab {
    title: SharedString,
    letter: SharedString,
    page: StoryPage,
    url: String,
}

/// The Education tabs.
fn education_tabs() -> Vec<SpaceTab> {
    vec![
        SpaceTab {
            title: "PGI 2.0 · state scores 2023-24".into(),
            letter: "P".into(),
            page: StoryPage::Pgi,
            url: "https://pgi.udiseplus.gov.in/#/home".to_string(),
        },
        SpaceTab {
            title: "UDISE+ 2024-25 · dashboard".into(),
            letter: "U".into(),
            page: StoryPage::Udise,
            url: "https://udiseplus.gov.in/#/home".to_string(),
        },
        SpaceTab {
            title: "NAS 2024 · state report cards".into(),
            letter: "N".into(),
            page: StoryPage::Nas,
            url: "https://nas.gov.in/report-cards".to_string(),
        },
    ]
}

/// The Personal tabs.
fn personal_tabs() -> Vec<SpaceTab> {
    vec![
        SpaceTab {
            title: "Weekend weather · Bengaluru".into(),
            letter: "W".into(),
            page: StoryPage::Weather,
            url: "https://weather.example.com/bengaluru-weekend".to_string(),
        },
        SpaceTab {
            title: "Hacker News · newest".into(),
            letter: "H".into(),
            page: StoryPage::Generic("Hacker News · newest".into()),
            url: "https://news.ycombinator.com/newest".to_string(),
        },
    ]
}

/// A page's address, for tabs the story opens.
fn page_url(page: &StoryPage) -> String {
    match page {
        StoryPage::Pgi => "https://pgi.udiseplus.gov.in/#/home".to_string(),
        StoryPage::Udise => "https://udiseplus.gov.in/#/home".to_string(),
        StoryPage::Datagov => "https://www.data.gov.in/catalog/school-education-indicators".to_string(),
        StoryPage::Answer => "https://pgi.udiseplus.gov.in/#/home".to_string(),
        StoryPage::Peek => "https://pgi.udiseplus.gov.in/methodology".to_string(),
        StoryPage::Nas => "https://nas.gov.in/report-cards".to_string(),
        StoryPage::Ledger => "http://localhost:5173/".to_string(),
        StoryPage::Weather => "https://weather.example.com/bengaluru-weekend".to_string(),
        StoryPage::Dropout => "https://udiseplus.gov.in/#/dropout".to_string(),
        StoryPage::Generic(title) => format!("https://example.com/{title}"),
    }
}

/// The page chip the Ask line and the card share.
fn page_chip() -> ComposerChip {
    ComposerChip { id: "this-page".into(), kind: ComposerChipKind::File, label: "This page".into(), removable: true, thumbnail: None, detail: None }
}

/// The Ask line state: page chip plus the two site actions.
fn ask_state() -> LineState {
    LineState::Ask {
        text: "Ask about this page".into(),
        chips: vec![page_chip()],
        site_actions: SITE_ACTIONS.iter().map(|(id, label)| (SharedString::from(*id), SharedString::from(*label))).collect(),
    }
}

/// The screen's view: story, sidebar state and every overlay.
pub struct BrowseView {
    story: BrowseStory,
    space: Space,
    active: usize,
    address: String,
    /// A page the story or a navigation picked, instead of the active tab.
    centre_override: Option<StoryPage>,
    ka_open: bool,
    ledger_open: bool,
    task_ui_open: bool,
    sessions: bool,
    sidebar_open: bool,
    line_open: bool,
    card_open: bool,
    docked: bool,
    field_open: bool,
    field_model: FieldModel,
    field_answer: Option<SharedString>,
    peek_open: bool,
    split: bool,
    space_menu: bool,
    line_input: Entity<TextareaState>,
    card_input: Entity<TextareaState>,
    field_editor: Entity<TextareaState>,
    steps: Vec<String>,
    started: bool,
    run: u64,
    focus_root: FocusHandle,
    tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

impl BrowseView {
    /// Creates the view with the Education sidebar and no overlays.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let line_input = cx.new(|cx| composer_state_rows("Ask about this page", 1, 1, window, cx));
        let card_input = cx.new(|cx| composer_state_rows("Steer the agent…", 1, 4, window, cx));
        let field_editor = cx.new(|cx| field_state("Search tabs, history, commands — or ask", window, cx));
        // Typing in the Field's editor filters rows through `rank` live.
        let sync = cx.observe(&field_editor, |this, _, cx| {
            let query = this.field_editor.read(cx).value().to_string();
            if query != this.field_model.query.as_ref() {
                let sources = this.field_sources();
                this.field_model.query = query.clone().into();
                this.field_model.rows = rank(&query, &sources);
                this.field_model.selected = 0;
                this.field_answer = None;
                cx.notify();
            }
        });
        cx.bind_keys([
            KeyBinding::new("cmd-j", ShowLine, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-e", ShowCard, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-shift-e", ToggleDock, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-t", NewTab, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-l", FocusAddress, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-s", HideSidebar, Some(BROWSE_CONTEXT)),
            KeyBinding::new("ctrl-1", SpaceOne, Some(BROWSE_CONTEXT)),
            KeyBinding::new("ctrl-2", SpaceTwo, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-.", TakeOver, Some(BROWSE_CONTEXT)),
            KeyBinding::new("cmd-enter", ConfirmCmd, Some(BROWSE_CONTEXT)),
            KeyBinding::new("space", AdvanceStep, Some(BROWSE_CONTEXT)),
            KeyBinding::new("escape", CloseTop, Some(BROWSE_CONTEXT)),
        ]);
        let steps = std::env::var("AUI_GALLERY_STEPS")
            .unwrap_or_default()
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let mut this = Self {
            story: BrowseStory::new(),
            space: Space::Education,
            active: 0,
            address: education_tabs()[0].url.clone(),
            centre_override: None,
            ka_open: true,
            ledger_open: true,
            task_ui_open: true,
            sessions: false,
            sidebar_open: true,
            line_open: false,
            card_open: false,
            docked: false,
            field_open: false,
            field_model: FieldModel::new("", Vec::new()),
            field_answer: None,
            peek_open: false,
            split: false,
            space_menu: false,
            line_input,
            card_input,
            field_editor,
            steps,
            started: false,
            run: 0,
            focus_root: cx.focus_handle(),
            tasks: Vec::new(),
            _subs: vec![sync],
        };
        this.field_model = FieldModel::new("", rank("", &this.field_sources()));
        this
    }

    /// The tabs of the current space.
    fn space_tabs(&self) -> Vec<SpaceTab> {
        match self.space {
            Space::Education => education_tabs(),
            Space::Personal => personal_tabs(),
        }
    }

    /// What the Field ranks over: open tabs, history and commands.
    fn field_sources(&self) -> FieldSources {
        let mut sources = FieldSources::new("Claude", "Google");
        for (i, tab) in self.space_tabs().iter().enumerate() {
            let host = tab.url.split('/').nth(2).unwrap_or("").to_string();
            sources.tabs.push(FieldTab { id: format!("tab-{i}").into(), title: tab.title.clone(), host: host.into() });
        }
        sources.history.push(FieldHistory {
            title: "PGI 2.0 · state scores 2023-24".into(),
            url: "https://pgi.udiseplus.gov.in/#/home".into(),
            read: "read Tuesday".into(),
        });
        sources.history.push(FieldHistory {
            title: "UDISE+ 2024-25 · dashboard".into(),
            url: "https://udiseplus.gov.in/#/home".into(),
            read: "read Monday".into(),
        });
        for (id, name) in [("split", "Split View"), ("reading", "Reading mode"), ("new-space", "New Space…")] {
            sources.commands.push(FieldCommand { id: id.into(), name: name.into() });
        }
        sources
    }

    /// The page the centre shows: an override, the story's page while its
    /// tabs exist, else the active tab's page.
    fn centre_page(&self) -> StoryPage {
        if let Some(page) = self.centre_override.clone() {
            return page;
        }
        if !self.story.tabs().is_empty() {
            return self.story.centre().clone();
        }
        self.space_tabs().get(self.active).map(|t| t.page.clone()).unwrap_or(StoryPage::Pgi)
    }

    /// The transcript the card and the column share.
    fn transcript(&self) -> AgentTranscript {
        const TEXTS: [&str; 7] = [
            "Opened PGI 2.0 state scores (2023-24)",
            "Read UDISE+ 2024-25 PTR for 5 states",
            "Reading dropout rates, secondary",
            "Reading enrolment on data.gov.in",
            "Asking to download the spreadsheet",
            "Downloaded udise_2024_25_state_indicators.xlsx",
            "Built the comparison",
        ];
        let completed = self.story.completed();
        let mut steps: Vec<Step> = TEXTS.iter().take(completed).map(|t| Step::new(*t, StepState::Done)).collect();
        if self.story.is_advancing() && completed < 7 {
            steps.push(Step::new(TEXTS[completed], StepState::Running));
        } else if self.story.phase() == Phase::NeedsYou {
            steps.push(Step::new("Waiting on data.gov.in sign-in", StepState::Running));
        } else if self.story.phase() == Phase::Approval {
            steps.push(Step::new("Waiting on the download approval", StepState::Running));
        }
        let mut items = vec![AgentItem::User(ASK_TEXT.into()), AgentItem::Steps(steps)];
        if let Some(queued) = self.story.queued() {
            items.push(AgentItem::Queued(queued));
        }
        if self.story.answered() {
            items.push(AgentItem::Answer(ANSWER_TEXT.into()));
        }
        AgentTranscript { title: "PGI vs school indicators".into(), model: "Claude Sonnet".into(), chips: vec![page_chip()], items }
    }

    /// The library line state for the story's line.
    fn line_state(&self) -> LineState {
        match self.story.line() {
            StoryLine::Ask => ask_state(),
            StoryLine::Driving { status, face } => LineState::from_task(face, status),
            StoryLine::YouDrive => LineState::YouDrive,
            StoryLine::NeedsYou { text } => LineState::NeedsYou { text },
            StoryLine::Approval => LineState::Approval(approval_data()),
        }
    }

    // ---- story drivers: every entry point the UI, keys, timers and steps share ----

    /// Sends the ask and starts the 1.2 s timers.
    fn do_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.story.phase(), Phase::Done | Phase::Stopped) {
            self.story = BrowseStory::new();
        }
        if self.story.phase() != Phase::Idle {
            return;
        }
        self.story.send();
        self.centre_override = None;
        self.line_open = true;
        self.run += 1;
        self.kick_timer(window, cx);
        cx.notify();
    }

    /// Restarts the timer loop while the story advances.
    fn kick_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.story.is_advancing() {
            return;
        }
        let run = self.run;
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            loop {
                cx.background_executor().timer(super::story::STEP_INTERVAL).await;
                let cont = this
                    .update_in(cx, |this, _, cx| {
                        if this.run != run || !this.story.is_advancing() {
                            return false;
                        }
                        this.story.advance_one();
                        this.centre_override = None;
                        cx.notify();
                        this.story.is_advancing()
                    })
                    .unwrap_or(false);
                if !cont {
                    return;
                }
            }
        });
        self.tasks.push(task);
    }

    /// One step forward from the Space key or the `run` harness: the same
    /// transition the timers make.
    fn advance_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.story.is_advancing() {
            return;
        }
        self.story.advance_one();
        self.centre_override = None;
        self.kick_timer(window, cx);
        cx.notify();
    }

    /// A click on the page while the task runs: the person takes the wheel.
    fn page_takeover(&mut self, cx: &mut Context<Self>) {
        if self.story.is_advancing() {
            self.story.takeover();
            self.line_open = true;
            cx.notify();
        }
    }

    /// Stops the task outright (the needs-you line's Stop).
    fn stop_task(&mut self, cx: &mut Context<Self>) {
        self.story.stop();
        cx.notify();
    }

    /// Every agent-line intent, in one place, so clicks, keys and steps
    /// reach the same transitions.
    fn on_line_intent(&mut self, intent: LineIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            LineIntent::Send(text) => {
                if !text.trim().is_empty() {
                    self.do_send(window, cx);
                }
            }
            LineIntent::SiteAction(_) => self.do_send(window, cx),
            LineIntent::TakeOver => self.page_takeover(cx),
            LineIntent::HandBack => {
                self.story.handback();
                self.kick_timer(window, cx);
                cx.notify();
            }
            LineIntent::Done => {
                self.story.resolve_needs();
                self.kick_timer(window, cx);
                cx.notify();
            }
            LineIntent::Stop => self.stop_task(cx),
            LineIntent::Allow => {
                self.story.allow();
                self.kick_timer(window, cx);
                cx.notify();
            }
            LineIntent::Deny => self.story.deny(),
            LineIntent::Expand => {
                self.card_open = true;
                cx.notify();
            }
            LineIntent::Plus | LineIntent::ChipRemove(_) | LineIntent::AllowAlways => {}
        }
        cx.notify();
    }

    /// Every card/column intent, in one place.
    fn on_card_intent(&mut self, intent: CardIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            CardIntent::Send(text) | CardIntent::Queue(text) => {
                if self.story.is_advancing() {
                    self.story.queue_steer(format!("{text} · queued"));
                } else {
                    self.do_send(window, cx);
                }
                self.card_input.update(cx, |state, cx| state.set_value("", window, cx));
                cx.notify();
            }
            CardIntent::Dock => {
                self.docked = true;
                self.card_open = true;
                cx.notify();
            }
            CardIntent::Undock => {
                self.docked = false;
                cx.notify();
            }
            CardIntent::Close => {
                self.card_open = false;
                cx.notify();
            }
            CardIntent::Plus | CardIntent::ChipRemove(_) | CardIntent::Model | CardIntent::Choose(_) => {}
        }
    }

    /// Navigates the active tab to a URL's drawn page.
    fn navigate(&mut self, url: &str, page: StoryPage, cx: &mut Context<Self>) {
        self.address = url.to_string();
        self.centre_override = Some(page);
        self.field_open = false;
        self.field_answer = None;
        cx.notify();
    }

    /// Every Field intent, in one place. Open(tab) selects it, Open(url)
    /// navigates there, Task opens the chat line pre-filled, Ask answers
    /// inline under the input.
    fn on_field_intent(&mut self, intent: FieldIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            FieldIntent::Open(row) | FieldIntent::Background(row) => match row.kind {
                FieldRowKind::Tab { id } => {
                    let index = id.as_ref().strip_prefix("tab-").and_then(|n| n.parse::<usize>().ok());
                    if let Some(i) = index {
                        self.select_space_tab(i, cx);
                    }
                    self.field_open = false;
                    cx.notify();
                }
                FieldRowKind::Open { url } | FieldRowKind::History { url } | FieldRowKind::Clipboard { url } => {
                    let url = url.to_string();
                    let page = StoryPage::Generic(url.clone().into());
                    self.navigate(&url, page, cx);
                }
                FieldRowKind::Ask => {
                    let q = self.field_model.query.clone();
                    self.field_answer = Some(format!("Quick answer: {q} — open a row below for the source.").into());
                    cx.notify();
                }
                FieldRowKind::Search => {
                    let q = self.field_model.query.clone();
                    let page = StoryPage::Generic(format!("Search · {q}").into());
                    self.navigate(&format!("https://www.google.com/search?q={q}"), page, cx);
                }
                FieldRowKind::Task => {
                    let q = self.field_model.query.clone().to_string();
                    self.field_editor.update(cx, |state, cx| state.set_value("", window, cx));
                    self.field_open = false;
                    self.line_open = true;
                    self.line_input.update(cx, |state, cx| state.set_value(q.as_str(), window, cx));
                    cx.notify();
                }
                FieldRowKind::Command { id } => {
                    self.field_open = false;
                    match id.as_ref() {
                        "split" => self.split = true,
                        "new-space" => self.space_menu = true,
                        _ => {}
                    }
                    cx.notify();
                }
            },
            FieldIntent::SearchExact(q) => {
                let page = StoryPage::Generic(format!("Search · {q}").into());
                self.navigate(&format!("https://www.google.com/search?q={q}"), page, cx);
            }
            FieldIntent::Complete => {
                if let Some(full) = completion(&self.field_model.query, &self.field_model.rows) {
                    self.field_editor.update(cx, |state, cx| state.set_value(full.as_str(), window, cx));
                }
            }
            FieldIntent::Close => {
                self.field_open = false;
                cx.notify();
            }
        }
    }

    /// Selects a space tab: page, address and intent-free state follow.
    fn select_space_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let tabs = self.space_tabs();
        if let Some(tab) = tabs.get(index) {
            self.active = index;
            self.address = tab.url.clone();
            self.centre_override = None;
        }
        cx.notify();
    }

    /// Switches space: tabs, address and footer follow; the story rides along.
    fn switch_space(&mut self, space: Space, cx: &mut Context<Self>) {
        self.space = space;
        self.active = 0;
        self.space_menu = false;
        if let Some(tab) = self.space_tabs().first() {
            self.address = tab.url.clone();
        }
        self.centre_override = None;
        cx.notify();
    }

    /// Opens the Field: empty for a new tab, or with the address selected.
    fn open_field(&mut self, preset: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let query = preset.unwrap_or_default();
        self.field_editor.update(cx, |state, cx| state.set_value(query.as_str(), window, cx));
        let sources = self.field_sources();
        self.field_model = FieldModel::new(query.as_str(), rank(query.as_str(), &sources));
        self.field_answer = None;
        self.field_open = true;
        cx.notify();
    }

    /// Closes the topmost overlay: menu, Field, Peek, card, line, split.
    fn close_topmost(&mut self, cx: &mut Context<Self>) {
        if self.space_menu {
            self.space_menu = false;
        } else if self.field_open {
            self.field_open = false;
        } else if self.peek_open {
            self.peek_open = false;
        } else if self.card_open {
            self.card_open = false;
        } else if self.line_open {
            self.line_open = false;
        } else if self.split {
            self.split = false;
        } else {
            return;
        }
        cx.notify();
    }

    // ---- rendering ----

    /// The `+ New tab ⌘T` row in the nav-row style.
    fn new_tab_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        record_ax_label("New tab");
        h_flex()
            .id("browse-new-tab")
            .h(px(30.0))
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .mx(px(8.0))
            .rounded(px(scale::R_SM))
            .ui(scale::FS_13)
            .text_color(p.ink_2)
            .cursor_pointer()
            .role(Role::Button)
            .aria_label("New tab")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_field(None, window, cx)))
            .child(div().flex_none().w(px(20.0)).flex().items_center().justify_center().child("+"))
            .child(div().flex_1().min_w(px(0.0)).truncate().child("+ New tab"))
            .child(div().ui(scale::FS_11).text_color(p.ink_3).child("⌘T"))
            .into_any_element()
    }

    /// The static Sessions | Browse switch; Browse is on, Sessions only
    /// flips the visual state.
    fn mode_switch(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        let (on_bg, off_bg) = (p.surface_3, gpui::transparent_black());
        let seg = |label: &str, on: bool| {
            record_ax_label(label);
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(scale::R_SM))
                .bg(if on { on_bg } else { off_bg })
                .ui(scale::FS_12)
                .medium()
                .text_color(if on { p.ink } else { p.ink_3 })
                .cursor_pointer()
                .role(Role::Button)
                .aria_label(label)
                .child(label.to_string())
        };
        h_flex()
            .id("browse-mode")
            .h(px(28.0))
            .flex_none()
            .items_center()
            .gap(px(2.0))
            .mx(px(8.0))
            .my(px(4.0))
            .p(px(2.0))
            .rounded(px(scale::R_SM))
            .bg(p.surface_2)
            .child(seg("Sessions", self.sessions).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.sessions = true;
                cx.notify();
            })))
            .child(seg("Browse", !self.sessions).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.sessions = false;
                cx.notify();
            })))
            .into_any_element()
    }

    /// The sidebar column: switch, address, new tab, pins, tabs, folders,
    /// task folders and the footer.
    /// The sidebar column (see below).
    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        let tabs = self.space_tabs();
        let mut col = v_flex().size_full().overflow_y_scroll().py(px(4.0)).child(self.mode_switch(cx));
        col = col.child(
            address_line("browse-address", split_address(&self.address))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let address = this.address.clone();
                    this.open_field(Some(address), window, cx);
                })),
        );
        col = col.child(self.new_tab_row(cx));
        col = col.child(
            pin_grid(
                "browse-pins",
                vec![
                    Pin::new("u", "U", "UDISE+").position(1),
                    Pin::new("p", "P", "PGI 2.0").position(2),
                    Pin::new("d", "D", "data.gov.in").position(3),
                    Pin::new("m", "M", "education.gov.in").position(4),
                ],
            )
            .on_select(cx.listener(|this, id: &SharedString, _, cx| {
                let (url, page) = match id.as_ref() {
                    "u" => ("https://udiseplus.gov.in/#/home", StoryPage::Udise),
                    "p" => ("https://pgi.udiseplus.gov.in/#/home", StoryPage::Pgi),
                    "d" => ("https://www.data.gov.in/catalog/school-education-indicators", StoryPage::Datagov),
                    _ => ("https://education.gov.in/", StoryPage::Generic("education.gov.in".into())),
                };
                this.navigate(url, page, cx);
            })),
        );
        let mut position = 0usize;
        for (i, tab) in tabs.iter().enumerate() {
            position += 1;
            let active = i == self.active && self.story.tabs().is_empty() && self.centre_override.is_none();
            let data = TabRowData::new(tab.title.clone(), tab.letter.clone()).position(position);
            let data = if active { data.active() } else { data };
            col = col.child(
                tab_row(SharedString::from(format!("browse-tab-{i}")), data).on_select(cx.listener(move |this, _: &SharedString, _, cx| {
                    this.select_space_tab(i, cx);
                })),
            );
        }
        col = col.child(folder_row("browse-folder-ka", "Karnataka review", self.ka_open, 0).on_toggle(cx.listener(
            |this, _: &SharedString, _, cx| {
                this.ka_open = !this.ka_open;
                cx.notify();
            },
        )));
        if self.ka_open {
            for (i, title) in ["KSEEB · SSLC results 2025", "Samagra Shiksha · Karnataka"].iter().enumerate() {
                position += 1;
                let letter = if i == 0 { "K" } else { "S" };
                let title = SharedString::from(*title);
                let shown = title.clone();
                col = col.child(
                    tab_row(
                        SharedString::from(format!("browse-tab-ka-{i}")),
                        TabRowData::new(title, letter).depth(1).position(position),
                    )
                    .on_select(cx.listener(move |this, _: &SharedString, _, cx| {
                        this.navigate(&page_url(&StoryPage::Generic(shown.clone())), StoryPage::Generic(shown.clone()), cx);
                    })),
                );
            }
        }
        col = col.child(div().flex_none().h(px(1.0)).mx(px(8.0)).my(px(4.0)).bg(p.line));
        // The PGI task folder: closed with face Reading before Send.
        let task_open = self.story.task_open() && self.task_ui_open;
        col = col.child(
            task_folder_row("browse-task-pgi", "PGI vs school indicators · 5 states", self.story.face(), self.story.status(), task_open)
                .on_toggle(cx.listener(|this, _: &SharedString, _, cx| {
                    this.task_ui_open = !this.task_ui_open;
                    cx.notify();
                }))
                .on_open_chat(cx.listener(|this, _: &SharedString, _, cx| {
                    this.line_open = true;
                    cx.notify();
                })),
        );
        if task_open {
            for (i, tab) in self.story.tabs().iter().enumerate() {
                position += 1;
                let page = tab.page.clone();
                let title = tab.title.clone();
                col = col.child(
                    tab_row(
                        SharedString::from(format!("browse-task-tab-{i}")),
                        TabRowData::new(title, if i == 4 { "A" } else { "P" }).depth(1).position(position),
                    )
                    .on_select(cx.listener(move |this, _: &SharedString, _, cx| {
                        this.address = page_url(&page);
                        this.centre_override = Some(page.clone());
                        cx.notify();
                    })),
                );
            }
        }
        col = col.child(
            task_folder_row(
                "browse-task-ledger",
                "Fix ledger chart",
                TaskFace::Done,
                SharedString::from(format!("{} · 1 tab", TaskFace::Done.status_word())),
                self.ledger_open,
            )
            .on_toggle(cx.listener(|this, _: &SharedString, _, cx| {
                this.ledger_open = !this.ledger_open;
                cx.notify();
            }))
            .on_open_chat(cx.listener(|this, _: &SharedString, _, cx| {
                this.line_open = true;
                cx.notify();
            })),
        );
        if self.ledger_open {
            position += 1;
            col = col.child(
                tab_row(
                    "browse-tab-ledger",
                    TabRowData::new("localhost:5173 · Ledger", "L").depth(1).position(position).port(5173),
                )
                .on_select(cx.listener(|this, _: &SharedString, _, cx| {
                    this.navigate("http://localhost:5173/", StoryPage::Ledger, cx);
                })),
            );
        }
        col.child(div().flex_1()).child(browse_footer("browse-footer", self.space.name()).on_space(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.space_menu = !this.space_menu;
                cx.notify();
            },
        )))
    }

    /// The centre: the drawn page (plus the split page), or the 28 px
    /// strip with the page title when the sidebar hides.
    fn render_centre(&self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.centre_page();
        let title: SharedString = match &page {
            StoryPage::Generic(t) => t.clone(),
            StoryPage::Pgi => "PGI 2.0 · state scores 2023-24".into(),
            StoryPage::Udise => "UDISE+ 2024-25 · dashboard".into(),
            StoryPage::Answer => "Answer · PGI vs indicators".into(),
            StoryPage::Peek => "Methodology note".into(),
            _ => "Browse".into(),
        };
        let method = cx.listener(|this, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>| {
            // Plain click navigates here; ⇧ peeks, ⌥ splits.
            let shift = window.modifiers().shift;
            let alt = window.modifiers().alt;
            cx.stop_propagation();
            if shift {
                this.peek_open = true;
            } else if alt {
                this.split = true;
            } else {
                this.address = page_url(&StoryPage::Peek);
                this.centre_override = Some(StoryPage::Peek);
            }
            cx.notify();
        });
        let ask_more = cx.listener(|this, _: &ClickEvent, _, cx: &mut Context<Self>| {
            cx.stop_propagation();
            this.line_open = true;
            cx.notify();
        });
        let done = cx.listener(|this, _: &ClickEvent, _, cx: &mut Context<Self>| {
            cx.stop_propagation();
            this.story.finish_task();
            cx.notify();
        });
        let body = render_page(&page, method, ask_more, done);
        let mut centre = v_flex().size_full();
        if !self.sidebar_open {
            let p = cx.aui().colors;
            centre = centre.child(
                div()
                    .flex_none()
                    .h(px(28.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_b_1()
                    .border_color(p.line)
                    .ui(scale::FS_12)
                    .medium()
                    .text_color(p.ink_2)
                    .child(title.clone()),
            );
        }
        let page_cell = div()
            .id("browse-page")
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .overflow_y_scroll()
            .cursor_pointer()
            .role(Role::Button)
            .aria_label(format!("Page, {title}"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.page_takeover(cx)))
            .child(body);
        if self.split {
            centre.child(h_flex().flex_1().min_h(px(0.0)).w_full().child(div().flex_1().min_w(px(0.0)).child(page_cell)).child(
                div().flex_1().min_w(px(0.0)).border_l_1().border_color(cx.aui().colors.line).child(render_page(
                    &StoryPage::Peek,
                    cx.listener(|_, _: &ClickEvent, _, _| {}),
                    cx.listener(|_, _: &ClickEvent, _, _| {}),
                    cx.listener(|_, _: &ClickEvent, _, _| {}),
                )),
            ))
        } else {
            centre.child(page_cell)
        }
        .into_any_element()
    }

    /// The Field overlay: the input row over the page, with the inline
    /// answer card under it once Ask answers.
    fn render_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = self.field_model.clone();
        let editor = self.field_editor.clone();
        let card = v_flex()
            .w(px(640.0))
            .max_w(relative(0.9))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.stop_propagation()))
            .child(
                div()
                    .key_context(aui::keys::MENU_CONTEXT)
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| {
                        this.field_model.selected = select_next(this.field_model.selected, this.field_model.rows.len());
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| {
                        this.field_model.selected = select_prev(this.field_model.selected, this.field_model.rows.len());
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                        if let Some(intent) = intent_for(&FieldKey::Enter, &this.field_model) {
                            this.on_field_intent(intent, window, cx);
                        }
                    }))
                    .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                        this.field_open = false;
                        cx.notify();
                    }))
                    .child(
                        field("browse-field", &model)
                            .query_slot(Textarea::new(&editor).appearance(false).bordered(false))
                            .on_hover(cx.listener(|this, index: &usize, _, cx| {
                                this.field_model.selected = *index;
                                cx.notify();
                            }))
                            .on_intent(cx.listener(|this, intent: &FieldIntent, window, cx| {
                                this.on_field_intent(intent.clone(), window, cx);
                            })),
                    ),
            );
        let card = match self.field_answer.clone() {
            Some(answer) => card.child(
                div()
                    .mx(px(14.0))
                    .mb(px(12.0))
                    .p(px(12.0))
                    .rounded(px(scale::R_SM))
                    .bg(cx.aui().colors.surface_2)
                    .ui(scale::FS_13)
                    .text_color(cx.aui().colors.ink)
                    .child(answer),
            ),
            None => card,
        };
        popover_layer(
            div()
                .id("browse-field-layer")
                .absolute()
                .inset_0()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.field_open = false;
                    cx.notify();
                }))
                .child(h_flex().absolute().inset_0().justify_center().items_start().pt(px(72.0)).child(card)),
        )
        .into_any_element()
    }

    /// The ⌘J line over the page's bottom-left.
    fn render_line(&self, cx: &mut Context<Self>) -> AnyElement {
        if matches!(self.story.line(), StoryLine::YouDrive) {
            // A status row, not a control: announce it so it reads in the tree.
            record_ax_label("You're driving");
        }
        let state = self.line_state();
        let input = self.line_input.clone();
        popover_layer(
            div()
                .id("browse-line-layer")
                .absolute()
                .left(px(20.0))
                .bottom(px(72.0))
                .child(agent_line("browse-line", &state, &input).on_intent(cx.listener(
                    |this, intent: &LineIntent, window, cx| this.on_line_intent(intent.clone(), window, cx),
                ))),
        )
        .into_any_element()
    }

    /// The ⌘E card over the page's bottom-right (20/72 px).
    fn render_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let transcript = self.transcript();
        let input = self.card_input.clone();
        popover_layer(
            div()
                .id("browse-card-layer")
                .absolute()
                .right(px(20.0))
                .bottom(px(72.0))
                .child(agent_card("browse-card", &transcript, &input).on_intent(cx.listener(
                    |this, intent: &CardIntent, window, cx| this.on_card_intent(intent.clone(), window, cx),
                ))),
        )
        .into_any_element()
    }

    /// The docked column in the right pane.
    fn render_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let transcript = self.transcript();
        let input = self.card_input.clone();
        agent_column("browse-column", &transcript, &input)
            .on_intent(cx.listener(|this, intent: &CardIntent, window, cx| this.on_card_intent(intent.clone(), window, cx)))
            .into_any_element()
    }

    /// Peek: the methodology note over the page's right edge.
    fn render_peek(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        popover_layer(
            div().id("browse-peek-layer").absolute().top(px(20.0)).right(px(20.0)).bottom(px(20.0)).w(px(480.0)).child(
                v_flex()
                    .size_full()
                    .rounded(px(scale::R_LG))
                    .border_1()
                    .border_color(p.line_strong)
                    .bg(p.overlay)
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .w_full()
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(12.0))
                            .h(px(40.0))
                            .border_b_1()
                            .border_color(p.line)
                            .child(div().flex_1().ui(scale::FS_13).semibold().text_color(p.ink).child("Methodology note"))
                            .child(
                                button("browse-peek-close", "Close").on_click(cx.listener(
                                    |this, _: &ClickEvent, _, cx| {
                                        this.peek_open = false;
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(div().flex_1().min_h(px(0.0)).w_full().overflow_y_scroll().child(render_page(
                        &StoryPage::Peek,
                        cx.listener(|_, _: &ClickEvent, _, _| {}),
                        cx.listener(|_, _: &ClickEvent, _, _| {}),
                        cx.listener(|_, _: &ClickEvent, _, _| {}),
                    ))),
            ),
        )
        .into_any_element()
    }

    /// The Space chip menu: Education, Personal, New Space….
    fn render_space_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        let mut menu = v_flex()
            .w(px(220.0))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line_strong)
            .bg(p.overlay)
            .p(px(4.0));
        for (id, label) in [("edu", "Education"), ("per", "Personal"), ("new", "New Space…")] {
            let label = SharedString::from(label);
            let name = label.clone();
            menu = menu.child(
                button(SharedString::from(format!("browse-space-{id}-{label}")), label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        match name.as_ref() {
                            "Education" => this.switch_space(Space::Education, cx),
                            "Personal" => this.switch_space(Space::Personal, cx),
                            _ => {
                                this.space_menu = false;
                                cx.notify();
                            }
                        }
                    })),
            );
        }
        popover_layer(div().id("browse-space-layer").absolute().left(px(12.0)).bottom(px(56.0)).child(menu)).into_any_element()
    }

    // ---- the AUI_GALLERY_STEPS harness ----

    /// Applies one `;`-separated step through the same handlers a click or
    /// a keystroke reaches — never straight into the model.
    fn apply_step(&mut self, step: &str, window: &mut Window, cx: &mut Context<Self>) {
        let dispatch = |action: Box<dyn Action>, window: &mut Window, cx: &mut App| window.dispatch_action(action, cx);
        match step.split_once(':') {
            Some(("run", n)) => {
                let n: usize = n.parse().unwrap_or(0);
                self.story.advance_to(n);
                self.centre_override = None;
                cx.notify();
            }
            Some(("space", n)) => {
                if n.trim() == "2" {
                    self.switch_space(Space::Personal, cx);
                } else {
                    self.switch_space(Space::Education, cx);
                }
            }
            Some(("field", text)) => self.open_field(Some(text.to_string()), window, cx),
            _ => match step {
                "ask" => {
                    self.line_input.update(cx, |state, cx| state.set_value(ASK_TEXT, window, cx));
                    self.line_open = true;
                    cx.notify();
                }
                "send" => {
                    self.line_input.update(cx, |state, cx| state.set_value(ASK_TEXT, window, cx));
                    self.do_send(window, cx);
                }
                "takeover" => self.page_takeover(cx),
                "handback" => {
                    self.story.handback();
                    cx.notify();
                }
                "needs-you" => {
                    self.story.needs_you();
                    self.line_open = true;
                    cx.notify();
                }
                "done" => {
                    if self.story.phase() == Phase::NeedsYou {
                        self.story.resolve_needs();
                    } else {
                        self.story.finish_task();
                    }
                    cx.notify();
                }
                "approval" => {
                    self.story.show_approval();
                    self.line_open = true;
                    cx.notify();
                }
                "allow" => {
                    self.story.allow();
                    cx.notify();
                }
                "deny" => {
                    self.story.deny();
                    cx.notify();
                }
                "answer" => {
                    self.story.answer();
                    self.centre_override = None;
                    cx.notify();
                }
                "field" => self.open_field(None, window, cx),
                "card" => {
                    self.card_open = true;
                    self.docked = false;
                    cx.notify();
                }
                "dock" => {
                    self.card_open = true;
                    self.docked = true;
                    cx.notify();
                }
                "undock" => {
                    self.docked = false;
                    cx.notify();
                }
                "peek" => {
                    self.peek_open = true;
                    cx.notify();
                }
                "split" => {
                    self.split = true;
                    cx.notify();
                }
                "hide-sidebar" => {
                    self.sidebar_open = false;
                    cx.notify();
                }
                "close" => self.close_topmost(cx),
                "cmd-j" => dispatch(Box::new(ShowLine), window, cx),
                "cmd-e" => dispatch(Box::new(ShowCard), window, cx),
                "cmd-shift-e" => dispatch(Box::new(ToggleDock), window, cx),
                "cmd-t" => dispatch(Box::new(NewTab), window, cx),
                "cmd-l" => dispatch(Box::new(FocusAddress), window, cx),
                "cmd-s" => dispatch(Box::new(HideSidebar), window, cx),
                "ctrl-1" => dispatch(Box::new(SpaceOne), window, cx),
                "ctrl-2" => dispatch(Box::new(SpaceTwo), window, cx),
                "cmd-." => dispatch(Box::new(TakeOver), window, cx),
                "cmd-enter" => dispatch(Box::new(ConfirmCmd), window, cx),
                "space" => dispatch(Box::new(AdvanceStep), window, cx),
                "esc" => dispatch(Box::new(CloseTop), window, cx),
                "shot" | "" => {}
                other => eprintln!("AUI_GALLERY_STEPS: unknown step `{other}`"),
            },
        }
    }

    /// Starts the harness after the first frame.
    fn start_steps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let steps = std::mem::take(&mut self.steps);
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            cx.background_executor().timer(STEPS_DELAY).await;
            for step in steps {
                if this.update_in(cx, |this, window, cx| this.apply_step(&step, window, cx)).is_err() {
                    return;
                }
            }
        });
        self.tasks.push(task);
    }
}

impl Render for BrowseView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.started {
            self.started = true;
            window.focus(&self.focus_root, cx);
            if !self.steps.is_empty() {
                self.start_steps(window, cx);
            }
        }
        let sidebar = self.render_sidebar(cx);
        let centre = self.render_centre(cx);
        let right_open = self.docked && self.card_open;
        let right = self.docked.then(|| self.render_column(cx));
        // SidebarHeader paints the lights but owns no trailing slot for the
        // nav cluster, so the cluster sits beside it in the header cell.
        let header_sidebar = h_flex()
            .w_full()
            .h_full()
            .items_center()
            .child(div().flex_none().child(sidebar_header("browse-hd-side").traffic_lights(true).collapsed(true)))
            .child(div().flex_1())
            .child(browse_nav_cluster("browse-nav").can_go_back(true).on_back(cx.listener(|this, _: &ClickEvent, _, cx| {
                let tabs = this.space_tabs();
                if this.active > 0 {
                    let next = this.active - 1;
                    let url = tabs[next].url.clone();
                    this.active = next;
                    this.address = url;
                    this.centre_override = None;
                    cx.notify();
                }
            })).on_forward(cx.listener(|this, _: &ClickEvent, _, cx| {
                let tabs = this.space_tabs();
                if this.active + 1 < tabs.len() {
                    let next = this.active + 1;
                    let url = tabs[next].url.clone();
                    this.active = next;
                    this.address = url;
                    this.centre_override = None;
                    cx.notify();
                }
            })).on_reload(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.centre_override = None;
                cx.notify();
            })));
        let shell = app_shell("browse-shell")
            .sidebar_width(px(BROWSE_SIDEBAR_WIDTH))
            .traffic_lights(true)
            .header_sidebar(header_sidebar)
            .header_centre(div())
            .sidebar(sidebar)
            .centre(centre)
            .right_open(right_open);
        let shell = match right {
            Some(right) => shell.right(right),
            None => shell,
        };
        div()
            .size_full()
            .relative()
            .key_context(BROWSE_CONTEXT)
            .track_focus(&self.focus_root)
            .on_action(cx.listener(|this, _: &ShowLine, _, cx| {
                this.line_open = !this.line_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowCard, _, cx| {
                this.card_open = true;
                this.docked = false;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleDock, _, cx| {
                if this.card_open && this.docked {
                    this.docked = false;
                } else {
                    this.card_open = true;
                    this.docked = true;
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| this.open_field(None, window, cx)))
            .on_action(cx.listener(|this, _: &FocusAddress, window, cx| {
                let address = this.address.clone();
                this.open_field(Some(address), window, cx);
            }))
            .on_action(cx.listener(|this, _: &HideSidebar, _, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SpaceOne, _, cx| this.switch_space(Space::Education, cx)))
            .on_action(cx.listener(|this, _: &SpaceTwo, _, cx| this.switch_space(Space::Personal, cx)))
            .on_action(cx.listener(|this, _: &TakeOver, _, cx| this.page_takeover(cx)))
            .on_action(cx.listener(|this, _: &HandBack, window, cx| {
                this.story.handback();
                this.kick_timer(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &AdvanceStep, window, cx| {
                if this.story.is_advancing() && !this.field_open && !this.card_open {
                    this.advance_step(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ConfirmCmd, window, cx| {
                if this.story.phase() == Phase::Approval {
                    this.story.allow();
                    this.kick_timer(window, cx);
                } else if this.story.phase() == Phase::Paused {
                    this.story.handback();
                    this.kick_timer(window, cx);
                } else if this.card_open {
                    let text = this.card_input.read(cx).value().trim().to_string();
                    if !text.is_empty() {
                        this.story.queue_steer(format!("{text} · queued"));
                        this.card_input.update(cx, |state, cx| state.set_value("", window, cx));
                    }
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CloseTop, _, cx| this.close_topmost(cx)))
            .child(shell)
            .children(self.space_menu.then(|| self.render_space_menu(cx)))
            .children(self.field_open.then(|| self.render_field(cx)))
            .children(self.peek_open.then(|| self.render_peek(cx)))
            .children((self.card_open && !self.docked).then(|| self.render_card(cx)))
            .children(self.line_open.then(|| self.render_line(cx)))
    }
}

/// One screen in window state, keyed for the gallery entry.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state(SharedString::from("browse-screen"), cx, move |window, cx| BrowseView::new(window, cx));
    div().size_full().child(view).into_any_element()
}

#[cfg(test)]
mod tests {
    // NOTE: `use super::build`, not a glob: the parent's `use gpui::*`
    // imports gpui's `test` macro, and a glob would pull it into this
    // scope where the `#[gpui::test]` expansion's inner `#[test]`
    // re-resolves to the macro instead of the builtin, recursing forever.
    use super::build;
    use aui::transcript::{arm_ax_probe, take_ax_labels};
    use gpui::{IntoElement, TestAppContext, Window};

    struct Host;

    impl gpui::Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            build(window, cx)
        }
    }

    /// The scripted takeover leaves the "You're driving" line with its
    /// Hand back button: the screen builds with both labels.
    #[gpui::test]
    fn takeover_leaves_you_driving_with_hand_back(cx: &mut TestAppContext) {
        std::env::set_var("AUI_GALLERY_STEPS", "ask;send;run:3;takeover");
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        arm_ax_probe(true);
        take_ax_labels();
        let (_host, mut vcx) = cx.add_window_view(|_, _| Host);
        // The harness applies steps after its first-frame delay; advance
        // the simulated clock past it, park the step task, then land a
        // frame so the takeover renders.
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        cx.run_until_parked();
        vcx.update(|window, app| {
            window.simulate_next_frame(app);
        });
        let labels = take_ax_labels();
        arm_ax_probe(false);
        std::env::remove_var("AUI_GALLERY_STEPS");
        assert!(labels.iter().any(|l| l.contains("You're driving")), "takeover shows You're driving, got {labels:?}");
        assert!(labels.iter().any(|l| l.contains("Hand back")), "takeover shows Hand back, got {labels:?}");
    }
}

