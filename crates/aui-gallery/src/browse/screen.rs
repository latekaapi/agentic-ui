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
    completion, field, field_state, intent_for, rank, select_next, select_prev, FieldCommand, FieldHistory, FieldIntent,
    FieldKey, FieldModel, FieldRowKind, FieldSources, FieldTab,
};
use aui::browse::{
    address_line, browse_footer, browse_nav_cluster, folder_row, pin_grid, split_address, tab_row, task_face, task_folder_row, Pin,
    TabRowData, TaskFace, BROWSE_SIDEBAR_WIDTH,
};
use aui::composer::{composer_state_rows, ComposerChip, ComposerChipKind};
use aui::data::{button, icon_button, record_ax_label};
use aui::keys::{Cancel, Confirm, SelectNext, SelectPrev, MENU_CONTEXT};
use aui_icons::IconName;
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
    /// A fresh space from `New Space…`, starting empty.
    New,
}

impl Space {
    /// The footer word and the chip menu label.
    fn name(self) -> &'static str {
        match self {
            Space::Education => "Education",
            Space::Personal => "Personal",
            Space::New => "New Space",
        }
    }

    /// A stable discriminant for the closed-tab set.
    fn tag(self) -> u8 {
        match self {
            Space::Education => 0,
            Space::Personal => 1,
            Space::New => 2,
        }
    }
}

/// The one source of truth for the shown page. Every selection — own tabs,
/// task tabs, pins, the Field, Space switches, back/forward/reload and the
/// story's own advance — sets it; the centre, the address line and the
/// single highlighted row all derive from it.
#[derive(Debug, Clone, PartialEq)]
enum Shown {
    /// One of the current space's own tabs, by index.
    Own(usize),
    /// One of the PGI task folder's tabs, by index.
    Task(usize),
    /// A navigated page (pins, history, Field Open) with no tab of its
    /// own, carrying the address that opened it.
    Page(StoryPage, String),
}

/// Which sidebar row the highlight sits on: exactly one tab row at most.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Highlight {
    /// One of the space's own tabs, by index.
    Own(usize),
    /// One of the task folder's tabs, by index.
    Task(usize),
    /// A page with no tab row (pins, history): nothing highlighted.
    None,
}

/// Where the keyboard should go on the next frame. gpui focuses through a
/// [`FocusHandle`], so opens park a request the next render performs — the
/// same pattern the assistant mock uses for its composer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusKind {
    /// The Field editor.
    Field,
    /// The ⌘J line input.
    Line,
    /// The card input.
    Card,
}

/// The quiet footer popover, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FootPop {
    /// The downloads button's quiet popover.
    Downloads,
    /// The extensions button's quiet popover.
    Extensions,
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
            url: "https://ncert.nic.in/report-cards".to_string(),
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
        StoryPage::Nas => "https://ncert.nic.in/report-cards".to_string(),
        StoryPage::Ledger => "http://localhost:5173/".to_string(),
        StoryPage::Weather => "https://weather.example.com/bengaluru-weekend".to_string(),
        StoryPage::Dropout => "https://udiseplus.gov.in/#/dropout".to_string(),
        StoryPage::Generic(title) => format!("https://example.com/{}", slugify(title)),
    }
}

/// Slugifies a saved page title for its address: lowercase, spaces become
/// `-`, `·` and other punctuation drop away.
fn slugify(title: &str) -> String {
    let dashed: String = title.to_lowercase().split_whitespace().collect::<Vec<_>>().join("-");
    let mut slug: String = dashed.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').to_string()
}

/// The task-folder tab letter, derived from the tab's host: U for
/// udiseplus, D for data.gov.in, A for the Answer, P for PGI scores.
fn task_letter(page: &StoryPage) -> &'static str {
    match page {
        StoryPage::Udise | StoryPage::Dropout => "U",
        StoryPage::Datagov => "D",
        StoryPage::Answer => "A",
        _ => "P",
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
    /// The one source of truth for the shown page (H1).
    shown: Shown,
    /// Back/forward stacks of shown pages.
    back: Vec<Shown>,
    forward: Vec<Shown>,
    ka_open: bool,
    ledger_open: bool,
    task_ui_open: bool,
    sessions: bool,
    sidebar_open: bool,
    line_open: bool,
    /// After Done, the line re-opens as the Ask prompt.
    ask_after_done: bool,
    card_open: bool,
    docked: bool,
    field_open: bool,
    field_model: FieldModel,
    field_answer: Option<SharedString>,
    /// Rows Background-opened without showing, oldest first.
    background_opened: Vec<String>,
    peek_open: bool,
    /// The page peek shows (a clicked link's page, else the note).
    peek_page: StoryPage,
    split: bool,
    /// The page Split shows beside the current one.
    split_page: StoryPage,
    /// A simple reader layout of the current page.
    reader: bool,
    space_menu: bool,
    /// The quiet footer popover, if any.
    foot: Option<FootPop>,
    /// Own tabs closed with their row ×, as `(space tag, title)`.
    closed: Vec<(u8, SharedString)>,
    line_input: Entity<TextareaState>,
    card_input: Entity<TextareaState>,
    field_editor: Entity<TextareaState>,
    steps: Vec<String>,
    started: bool,
    /// The timer generation: spawns and stops bump it, and a loop exits
    /// once its generation is stale, so exactly one loop ever acts.
    timer_gen: u64,
    /// Whether a timer loop is currently running.
    timer_live: bool,
    /// How many timer loops have spawned (a test hook for the single loop).
    timer_spawns: u64,
    /// A parked focus request the next render performs.
    want_focus: Option<FocusKind>,
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
            shown: Shown::Own(0),
            back: Vec::new(),
            forward: Vec::new(),
            ka_open: true,
            ledger_open: true,
            task_ui_open: true,
            sessions: false,
            sidebar_open: true,
            line_open: false,
            ask_after_done: false,
            card_open: false,
            docked: false,
            field_open: false,
            field_model: FieldModel::new("", Vec::new()),
            field_answer: None,
            background_opened: Vec::new(),
            peek_open: false,
            peek_page: StoryPage::Peek,
            split: false,
            split_page: StoryPage::Peek,
            reader: false,
            space_menu: false,
            foot: None,
            closed: Vec::new(),
            line_input,
            card_input,
            field_editor,
            steps,
            started: false,
            timer_gen: 0,
            timer_live: false,
            timer_spawns: 0,
            want_focus: None,
            focus_root: cx.focus_handle(),
            tasks: Vec::new(),
            _subs: vec![sync],
        };
        this.field_model = FieldModel::new("", rank("", &this.field_sources()));
        this
    }

    /// The tabs of the current space, minus rows closed with their ×.
    /// The New space starts empty.
    fn space_tabs(&self) -> Vec<SpaceTab> {
        let all = match self.space {
            Space::Education => education_tabs(),
            Space::Personal => personal_tabs(),
            Space::New => Vec::new(),
        };
        all.into_iter().filter(|tab| !self.closed.iter().any(|(tag, title)| *tag == self.space.tag() && title == &tab.title)).collect()
    }

    /// What the Field ranks over: open tabs, history and commands.
    fn field_sources(&self) -> FieldSources {
        let mut sources = FieldSources::new("Claude", "Google");
        for (i, tab) in self.space_tabs().iter().enumerate() {
            let host = tab.url.split('/').nth(2).unwrap_or("").to_string();
            sources.tabs.push(FieldTab { id: format!("tab-{i}").into(), title: tab.title.clone(), host: host.into() });
        }
        sources.history.push(FieldHistory {
            title: "pgi.udiseplus.gov.in".into(),
            url: "https://pgi.udiseplus.gov.in/#/home".into(),
            read: "read Tuesday".into(),
        });
        sources.history.push(FieldHistory {
            title: "udiseplus.gov.in".into(),
            url: "https://udiseplus.gov.in/#/home".into(),
            read: "read Monday".into(),
        });
        for (id, name) in [("split", "Split View"), ("reading", "Reading mode"), ("new-space", "New Space…")] {
            sources.commands.push(FieldCommand { id: id.into(), name: name.into() });
        }
        sources
    }

    /// The page the centre shows: whatever `shown` says.
    fn centre_page(&self) -> StoryPage {
        match &self.shown {
            Shown::Own(i) => self.space_tabs().get(*i).map(|t| t.page.clone()).unwrap_or(StoryPage::Pgi),
            Shown::Task(i) => self.story.tabs().get(*i).map(|t| t.page.clone()).unwrap_or(StoryPage::Pgi),
            Shown::Page(page, _) => page.clone(),
        }
    }

    /// The address line for the shown page.
    fn shown_address(&self) -> String {
        match &self.shown {
            Shown::Own(i) => self.space_tabs().get(*i).map(|t| t.url.clone()).unwrap_or_else(|| page_url(&StoryPage::Pgi)),
            Shown::Task(i) => {
                self.story.tabs().get(*i).map(|t| page_url(&t.page)).unwrap_or_else(|| page_url(&StoryPage::Pgi))
            }
            Shown::Page(_, url) => url.clone(),
        }
    }

    /// The single highlighted sidebar row: the own tab shown, or the task
    /// tab shown — never both, never none while a tab shows.
    fn highlighted(&self) -> Highlight {
        match &self.shown {
            Shown::Own(i) => Highlight::Own(*i),
            Shown::Task(i) => Highlight::Task(*i),
            Shown::Page(page, _) => {
                if let Some(i) = self.space_tabs().iter().position(|t| &t.page == page) {
                    Highlight::Own(i)
                } else if let Some(i) = self.story.tabs().iter().position(|t| &t.page == page) {
                    Highlight::Task(i)
                } else {
                    Highlight::None
                }
            }
        }
    }

    /// Sets the shown page through back/forward history.
    fn set_shown(&mut self, shown: Shown) {
        if self.shown != shown {
            self.back.push(self.shown.clone());
            self.forward.clear();
            self.shown = shown;
        }
    }

    /// Follows the story's advance: when the person is looking at a task
    /// tab, the shown tab tracks the story's newest tab; on their own tab
    /// the task folder updates but the page stays (M5).
    fn story_advanced(&mut self) {
        if matches!(self.shown, Shown::Task(_)) && !self.story.tabs().is_empty() {
            self.shown = Shown::Task(self.story.tabs().len() - 1);
        }
    }

    /// The transcript the card and the column share, derived from the
    /// story's one [`current_step`](BrowseStory::current_step): the steps
    /// before it are Done, it runs, and NeedsYou/Approval never mark the
    /// waiting step Done early.
    fn transcript(&self) -> AgentTranscript {
        let mut steps: Vec<Step> =
            self.story.done_steps().iter().map(|text| Step::new(text.clone(), StepState::Done)).collect();
        if let Some(current) = self.story.current_step() {
            steps.push(Step::new(current, StepState::Running));
        }
        if self.story.phase() == Phase::NeedsYou {
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

    /// The library line state for the story's line. After Done, an opened
    /// line is the Ask prompt again: a fresh story may start.
    fn line_state(&self) -> LineState {
        if self.story.phase() == Phase::Done && self.ask_after_done {
            return ask_state();
        }
        match self.story.line() {
            StoryLine::Ask => ask_state(),
            StoryLine::Driving { status, face } => LineState::from_task(face, status),
            StoryLine::YouDrive => LineState::YouDrive,
            StoryLine::NeedsYou { text } => LineState::NeedsYou { text },
            StoryLine::Approval => LineState::Approval(approval_data()),
        }
    }

    // ---- story drivers: every entry point the UI, keys, timers and steps share ----

    /// Sends the ask and starts the 1.2 s timer.
    fn do_send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.story.phase(), Phase::Done | Phase::Stopped) {
            self.story = BrowseStory::new();
        }
        if self.story.phase() != Phase::Idle {
            return;
        }
        self.story.send();
        self.shown = Shown::Task(0);
        self.back.clear();
        self.forward.clear();
        self.ask_after_done = false;
        self.line_open = true;
        self.want_focus = Some(FocusKind::Line);
        self.kick_timer(window, cx);
        cx.notify();
    }

    /// (Re)starts the single timer loop while the story advances. A live
    /// loop keeps ticking across (re)starts, so repeated HandBacks never
    /// pile loops up; a new loop spawns only when none runs. Stopping bumps
    /// the generation so a superseded loop exits without acting.
    fn kick_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.story.is_advancing() {
            return;
        }
        if self.timer_live {
            return;
        }
        self.timer_gen += 1;
        let gen = self.timer_gen;
        self.timer_live = true;
        self.timer_spawns += 1;
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            loop {
                cx.background_executor().timer(super::story::STEP_INTERVAL).await;
                let cont = this
                    .update_in(cx, |this, _, cx| {
                        if this.timer_gen != gen {
                            // Superseded by a stop: exit without acting.
                            return false;
                        }
                        if !this.story.is_advancing() {
                            this.timer_live = false;
                            return false;
                        }
                        this.story.advance_one();
                        this.story_advanced();
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

    /// Stops the timer: the live loop exits on its next tick. The harness
    /// calls this after its steps so `run:N` never captures a live timer.
    fn stop_timer(&mut self) {
        self.timer_gen += 1;
        self.timer_live = false;
    }

    /// One step forward from the Space key or the `run` harness: the same
    /// transition the timers make.
    fn advance_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.story.is_advancing() {
            return;
        }
        self.story.advance_one();
        self.story_advanced();
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

    /// Steers text into the task: while Running/Paused it queues behind
    /// the turn (the queued chip) and clears the input; while
    /// NeedsYou/Approval the text stays in the input, never silently
    /// dropped; otherwise (Idle/Done/Stopped) a non-blank send starts.
    fn steer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        if matches!(self.story.phase(), Phase::Running | Phase::Paused) {
            self.story.queue_steer(format!("{text} · queued"));
            self.card_input.update(cx, |state, cx| state.set_value("", window, cx));
        } else if matches!(self.story.phase(), Phase::NeedsYou | Phase::Approval) {
            // Kept in the input: the person answers first.
        } else {
            self.do_send(window, cx);
        }
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
            LineIntent::Deny => {
                self.story.deny();
                cx.notify();
            }
            LineIntent::Expand => {
                self.card_open = true;
                self.want_focus = Some(FocusKind::Card);
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
                self.steer(&text, window, cx);
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

    /// Navigates to a URL's drawn page: the shown page changes, so the
    /// centre, the address line and the highlight follow together.
    fn navigate(&mut self, url: &str, page: StoryPage, cx: &mut Context<Self>) {
        self.set_shown(Shown::Page(page, url.to_string()));
        self.field_open = false;
        self.field_answer = None;
        cx.notify();
    }

    /// A page link's gesture: ⇧ peeks the clicked page, ⌥ splits it beside
    /// the current one, a plain click navigates there.
    fn on_page_link(&mut self, target: StoryPage, window: &mut Window, cx: &mut Context<Self>) {
        let shift = window.modifiers().shift;
        let alt = window.modifiers().alt;
        cx.stop_propagation();
        if shift {
            self.peek_page = target;
            self.peek_open = true;
        } else if alt {
            self.split_page = target;
            self.split = true;
        } else {
            let url = page_url(&target);
            self.set_shown(Shown::Page(target, url));
            self.field_open = false;
            self.field_answer = None;
        }
        cx.notify();
    }

    /// Every Field intent, in one place. Open(tab) selects it, Open(url)
    /// navigates there, Task opens the chat line pre-filled, Ask answers
    /// inline under the input.
    fn on_field_intent(&mut self, intent: FieldIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            FieldIntent::Open(row) => match row.kind {
                FieldRowKind::Tab { id } => {
                    let index = id.as_ref().strip_prefix("tab-").and_then(|n| n.parse::<usize>().ok());
                    if let Some(i) = index {
                        self.select_space_tab(i, cx);
                    }
                    self.field_open = false;
                    cx.notify();
                }
                FieldRowKind::Open { url } | FieldRowKind::Clipboard { url } => {
                    let page = StoryPage::Generic(url.clone().into());
                    self.navigate(&url, page, cx);
                }
                FieldRowKind::History { url } => {
                    let url = url.to_string();
                    let title = url.split('/').nth(2).unwrap_or(&url).to_string();
                    let page = StoryPage::Generic(title.into());
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
                    self.ask_after_done = self.story.phase() == Phase::Done;
                    self.line_input.update(cx, |state, cx| state.set_value(q.as_str(), window, cx));
                    self.want_focus = Some(FocusKind::Line);
                    cx.notify();
                }
                FieldRowKind::Command { id } => {
                    self.field_open = false;
                    match id.as_ref() {
                        "split" => self.split = true,
                        "reading" => self.reader = !self.reader,
                        "new-space" => self.switch_space(Space::New, cx),
                        _ => {}
                    }
                    cx.notify();
                }
            },
            FieldIntent::Background(row) => {
                // Background opens without showing: the row is remembered,
                // the shown page never moves.
                let url = match &row.kind {
                    FieldRowKind::Tab { id } => {
                        let index = id.as_ref().strip_prefix("tab-").and_then(|n| n.parse::<usize>().ok());
                        index.and_then(|i| self.space_tabs().get(i).map(|t| t.url.clone()))
                    }
                    FieldRowKind::Open { url } | FieldRowKind::Clipboard { url } => Some(url.to_string()),
                    FieldRowKind::History { url } => Some(url.to_string()),
                    FieldRowKind::Ask | FieldRowKind::Search | FieldRowKind::Task | FieldRowKind::Command { .. } => None,
                };
                if let Some(url) = url {
                    if !self.background_opened.contains(&url) {
                        self.background_opened.push(url);
                    }
                }
                self.field_open = false;
                cx.notify();
            }
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

    /// Selects a space tab: the shown page changes, so the centre, the
    /// address line and the highlight follow together.
    fn select_space_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.space_tabs().get(index).is_some() {
            self.set_shown(Shown::Own(index));
        }
        cx.notify();
    }

    /// Closes an own tab row: the tab leaves the sidebar, and closing the
    /// shown tab shows its neighbour.
    fn close_space_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let tabs = self.space_tabs();
        let Some(tab) = tabs.get(index) else {
            return;
        };
        self.closed.push((self.space.tag(), tab.title.clone()));
        if self.shown == Shown::Own(index) {
            let rest = tabs.len().saturating_sub(1);
            self.shown = Shown::Own(index.min(rest.saturating_sub(1)));
        } else if let Shown::Own(i) = self.shown {
            if i > index {
                self.shown = Shown::Own(i - 1);
            }
        }
        cx.notify();
    }

    /// Switches space: tabs, address and footer follow; the story rides along.
    fn switch_space(&mut self, space: Space, cx: &mut Context<Self>) {
        self.space = space;
        self.shown = Shown::Own(0);
        self.back.clear();
        self.forward.clear();
        self.space_menu = false;
        cx.notify();
    }

    /// Goes back one shown page, keeping the forward stack.
    fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(prev) = self.back.pop() {
            self.forward.push(self.shown.clone());
            self.shown = prev;
            cx.notify();
        }
    }

    /// Goes forward after back.
    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(next) = self.forward.pop() {
            self.back.push(self.shown.clone());
            self.shown = next;
            cx.notify();
        }
    }

    /// Opens the Field and parks a focus request for its editor.
    fn open_field(&mut self, preset: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let query = preset.unwrap_or_default();
        self.field_editor.update(cx, |state, cx| state.set_value(query.as_str(), window, cx));
        let sources = self.field_sources();
        self.field_model = FieldModel::new(query.as_str(), rank(query.as_str(), &sources));
        self.field_answer = None;
        self.field_open = true;
        self.want_focus = Some(FocusKind::Field);
        cx.notify();
    }

    /// Whether a text input holds the focus: the Field editor, the ⌘J line
    /// input or the card input.
    fn text_focused(&self, window: &Window, cx: &App) -> bool {
        let focused = window.focused(cx);
        [self.field_editor.read(cx).focus_handle(cx), self.line_input.read(cx).focus_handle(cx), self.card_input.read(cx).focus_handle(cx)]
            .iter()
            .any(|handle| Some(handle) == focused.as_ref())
    }

    /// Closes the topmost overlay in paint order: footer popover, line,
    /// card, peek, Field, menu, split. Esc on the approval card denies
    /// (library behaviour): the line stays, and the next Esc closes it.
    fn close_topmost(&mut self, cx: &mut Context<Self>) {
        if self.foot.is_some() {
            self.foot = None;
        } else if self.line_open && self.story.phase() == Phase::Approval {
            self.story.deny();
        } else if self.line_open {
            self.line_open = false;
        } else if self.card_open {
            self.card_open = false;
        } else if self.peek_open {
            self.peek_open = false;
        } else if self.field_open {
            self.field_open = false;
        } else if self.space_menu {
            self.space_menu = false;
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
                .id(SharedString::from(format!("browse-mode-{label}")))
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
        let highlight = self.highlighted();
        let mut col = v_flex().size_full().id("browse-sidebar-scroll").overflow_y_scroll().py(px(4.0)).child(self.mode_switch(cx));
        col = col.child(
            address_line("browse-address", split_address(&self.shown_address()))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let address = this.shown_address();
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
            let active = highlight == Highlight::Own(i);
            let data = TabRowData::new(tab.title.clone(), tab.letter.clone()).position(position);
            let data = if active { data.active() } else { data };
            col = col.child(
                tab_row(SharedString::from(format!("browse-tab-{i}")), data)
                    .on_select(cx.listener(move |this, _: &SharedString, _, cx| {
                        this.select_space_tab(i, cx);
                    }))
                    .on_close(cx.listener(move |this, _: &SharedString, _, cx| {
                        this.close_space_tab(i, cx);
                    })),
            );
        }
        // The Karnataka folder belongs to Education: Personal shows only
        // its own tabs (the task folders follow every space).
        let show_ka = self.space == Space::Education;
        if show_ka {
            col = col.child(folder_row("browse-folder-ka", "Karnataka review", self.ka_open, 0).on_toggle(cx.listener(
                |this, _: &SharedString, _, cx| {
                    this.ka_open = !this.ka_open;
                    cx.notify();
                },
            )));
        }
        if self.ka_open && show_ka {
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
                let letter = SharedString::from(task_letter(&page));
                let active = highlight == Highlight::Task(i);
                let data = TabRowData::new(title, letter).depth(1).position(position);
                let data = if active { data.active() } else { data };
                col = col.child(
                    tab_row(SharedString::from(format!("browse-task-tab-{i}")), data).on_select(cx.listener(
                        move |this, _: &SharedString, _, cx| {
                            this.set_shown(Shown::Task(i));
                            cx.notify();
                        },
                    )),
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
        col.child(div().flex_1())
            .child(
                browse_footer("browse-footer", self.space.name())
                    .on_space(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.space_menu = !this.space_menu;
                        cx.notify();
                    }))
                    .on_downloads(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.foot = if this.foot == Some(FootPop::Downloads) { None } else { Some(FootPop::Downloads) };
                        cx.notify();
                    }))
                    .on_extensions(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.foot = if this.foot == Some(FootPop::Extensions) { None } else { Some(FootPop::Extensions) };
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// The quiet footer popover: downloads or extensions, one calm line.
    fn render_foot(&self, cx: &mut Context<Self>) -> AnyElement {
        let pop = self.foot.expect("the footer popover renders only when set");
        let (id, text) = match pop {
            FootPop::Downloads => ("browse-foot-downloads", "No downloads yet"),
            FootPop::Extensions => ("browse-foot-extensions", "Extensions: uBlock Origin Lite · Manage"),
        };
        record_ax_label(text);
        let p = cx.aui().colors;
        popover_layer(
            div()
                .id("browse-foot-layer")
                .absolute()
                .left(px(12.0))
                .bottom(px(56.0))
                .child(
                    div()
                        .id(SharedString::from(id))
                        .px(px(12.0))
                        .py(px(8.0))
                        .rounded(px(scale::R_SM))
                        .border_1()
                        .border_color(p.line)
                        .bg(p.overlay)
                        .ui(scale::FS_12)
                        .text_color(p.ink_2)
                        .role(Role::Status)
                        .aria_label(text)
                        .child(text.to_string()),
                ),
        )
        .into_any_element()
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
        let link = cx.aui().colors.accent;
        let on_link = cx.listener(|this, target: &StoryPage, window: &mut Window, cx: &mut Context<Self>| {
            this.on_page_link(target.clone(), window, cx);
        });
        let ask_more = cx.listener(|this, _: &ClickEvent, _, cx: &mut Context<Self>| {
            cx.stop_propagation();
            this.ask_after_done = this.story.phase() == Phase::Done;
            this.line_open = true;
            this.want_focus = Some(FocusKind::Line);
            cx.notify();
        });
        let done = cx.listener(|this, _: &ClickEvent, _, cx: &mut Context<Self>| {
            cx.stop_propagation();
            this.story.finish_task();
            cx.notify();
        });
        let body = render_page(&page, link, on_link, ask_more, done);
        let mut centre = v_flex().size_full();
        if !self.sidebar_open {
            record_ax_label(&title);
            let p = cx.aui().colors;
            centre = centre.child(
                div()
                    .id("browse-collapsed-strip")
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
                    .role(Role::Label)
                    .aria_label(format!("Page, {title}"))
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
        // Reading mode: the same page in a narrow centred column.
        let page_cell = if self.reader {
            record_ax_label("Reader mode");
            div()
                .id("browse-reader")
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .overflow_y_scroll()
                .flex()
                .justify_center()
                .role(Role::Document)
                .aria_label("Reader mode")
                .child(div().w(px(640.0)).max_w(relative(0.8)).flex_none().child(page_cell))
                .into_any_element()
        } else {
            page_cell.into_any_element()
        };
        if self.split {
            let link = cx.aui().colors.accent;
            let line = cx.aui().colors.line;
            let right_link =
                cx.listener(|this, target: &StoryPage, window: &mut Window, cx: &mut Context<Self>| {
                    this.on_page_link(target.clone(), window, cx);
                });
            let noop_ask = cx.listener(|_, _: &ClickEvent, _, _| {});
            let noop_done = cx.listener(|_, _: &ClickEvent, _, _| {});
            centre
                .child(
                    h_flex().flex_1().min_h(px(0.0)).w_full().child(self.split_pane("left", page_cell, cx)).child(
                        div().flex_none().w(px(1.0)).bg(line),
                    ).child(self.split_pane(
                        "right",
                        render_page(&self.split_page, link, right_link, noop_ask, noop_done),
                        cx,
                    )),
                )
                .into_any_element()
        } else {
            centre.child(page_cell).into_any_element()
        }
    }

    /// One split pane: full height, its page, a × in its top-right corner.
    fn split_pane(&self, side: &str, page: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let label = format!("Close {side} split");
        record_ax_label(&label);
        let close_label = label.clone();
        v_flex()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                h_flex()
                    .w_full()
                    .flex_none()
                    .items_center()
                    .child(div().flex_1())
                    .child(
                        icon_button(SharedString::from(format!("browse-split-close-{side}")), IconName::X)
                            .accessibility_label(close_label)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.split = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("browse-split-scroll-{side}")))
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .overflow_y_scroll()
                    .child(page),
            )
            .into_any_element()
    }

    /// The Field overlay: the input row over the page, with the inline
    /// answer card under it once Ask answers. The menu key context lives on
    /// the layer itself — the ancestor of the focused editor — so Select,
    /// Confirm and Cancel arrive while typing.
    fn render_field(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = self.field_model.clone();
        let editor = self.field_editor.clone();
        let card = v_flex()
            .id("browse-field-card")
            .w(px(640.0))
            .max_w(relative(0.9))
            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.stop_propagation()))
            .child({
                let hover = cx.listener(|this, index: &usize, _, cx: &mut Context<Self>| {
                    this.field_model.selected = *index;
                    cx.notify();
                });
                let intent = cx.listener(|this, intent: &FieldIntent, window, cx| {
                    this.on_field_intent(intent.clone(), window, cx);
                });
                field("browse-field", &model)
                    .query_slot(Textarea::new(&editor).appearance(false).bordered(false))
                    .on_hover(move |index: usize, window, cx| hover(&index, window, cx))
                    .on_intent(move |next: FieldIntent, window, cx| intent(&next, window, cx))
            });
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
                .key_context(MENU_CONTEXT)
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
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.field_open = false;
                    cx.notify();
                }))
                .child(h_flex().absolute().inset_0().justify_center().items_start().pt(px(72.0)).child(card)),
        )
        .into_any_element()
    }

    /// The ⌘J line, centred over the page 22 px above its foot.
    fn render_line(&self, cx: &mut Context<Self>) -> AnyElement {
        if matches!(self.story.line(), StoryLine::YouDrive) {
            // A status row, not a control: announce it so it reads in the tree.
            record_ax_label("You're driving");
        }
        // Terminal states never offer Take over: Done and Stopped draw
        // their own quiet rows instead of the library's driving row.
        if self.story.phase() == Phase::Done && !self.ask_after_done {
            return self.render_done_line(cx);
        }
        if self.story.phase() == Phase::Stopped {
            return self.render_stopped_line(cx);
        }
        let state = self.line_state();
        let input = self.line_input.clone();
        let handler = cx.listener(|this, intent: &LineIntent, window, cx| this.on_line_intent(intent.clone(), window, cx));
        popover_layer(self.centered_layer("browse-line-layer", self.page_left(), agent_line("browse-line", &state, &input).on_intent(
            move |next: LineIntent, window, cx| handler(&next, window, cx),
        )))
        .into_any_element()
    }

    /// Done: the ✓ face, the ready text, a secondary Open answer and ×.
    fn render_done_line(&self, cx: &mut Context<Self>) -> AnyElement {
        record_ax_label("Done — the comparison is ready");
        let p = cx.aui().colors;
        let row = h_flex()
            .id("browse-line")
            .w_full()
            .min_w(px(560.0))
            .max_w(px(640.0))
            .h(px(48.0))
            .items_center()
            .gap(px(10.0))
            .px(px(14.0))
            .rounded(px(scale::R_XL))
            .border_1()
            .border_color(p.line)
            .bg(p.overlay)
            .child(task_face(TaskFace::Done))
            .child(div().flex_1().min_w(px(0.0)).ui(scale::FS_13).text_color(p.ink).child("Done — the comparison is ready"))
            .child(button("browse-done-open", "Open answer").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                let answer = this.story.tabs().iter().position(|t| t.page == StoryPage::Answer);
                match answer {
                    Some(i) => this.set_shown(Shown::Task(i)),
                    None => this.set_shown(Shown::Page(StoryPage::Answer, page_url(&StoryPage::Answer))),
                }
                cx.notify();
            })))
            .child(
                icon_button("browse-done-close", IconName::X)
                    .accessibility_label("Close")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.line_open = false;
                        cx.notify();
                    })),
            );
        popover_layer(self.centered_layer("browse-line-layer", self.page_left(), row)).into_any_element()
    }

    /// Stopped: the paused face, the cause text and ×. No Take over.
    fn render_stopped_line(&self, cx: &mut Context<Self>) -> AnyElement {
        let text: SharedString = match self.story.line() {
            StoryLine::Driving { status, .. } => status,
            _ => "Stopped".into(),
        };
        record_ax_label(&text);
        let p = cx.aui().colors;
        let label = text.to_string();
        let row = h_flex()
            .id("browse-line")
            .w_full()
            .min_w(px(560.0))
            .max_w(px(640.0))
            .h(px(48.0))
            .items_center()
            .gap(px(10.0))
            .px(px(14.0))
            .rounded(px(scale::R_XL))
            .border_1()
            .border_color(p.line)
            .bg(p.overlay)
            .child(task_face(TaskFace::Paused))
            .child(div().flex_1().min_w(px(0.0)).ui(scale::FS_13).text_color(p.ink).child(label))
            .child(
                icon_button("browse-stopped-close", IconName::X)
                    .accessibility_label("Close")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.line_open = false;
                        cx.notify();
                    })),
            );
        popover_layer(self.centered_layer("browse-line-layer", self.page_left(), row)).into_any_element()
    }

    /// The page's left edge: the sidebar width while it shows, else zero,
    /// so centred overlays sit over the page rather than the window.
    fn page_left(&self) -> f32 {
        if self.sidebar_open { BROWSE_SIDEBAR_WIDTH } else { 0.0 }
    }

    /// A bottom-centred overlay layer over the page rect.
    fn centered_layer(&self, id: &str, left: f32, child: impl IntoElement) -> impl IntoElement {
        div()
            .id(SharedString::from(id))
            .absolute()
            .left(px(left))
            .right(px(0.0))
            .bottom(px(22.0))
            .flex()
            .flex_row()
            .justify_center()
            .child(child)
    }

    /// The ⌘E card, centred over the page 22 px above its foot.
    fn render_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let transcript = self.transcript();
        let input = self.card_input.clone();
        let handler = cx.listener(|this, intent: &CardIntent, window, cx| this.on_card_intent(intent.clone(), window, cx));
        popover_layer(self.centered_layer("browse-card-layer", self.page_left(), agent_card("browse-card", &transcript, &input).on_intent(
            move |next: CardIntent, window, cx| handler(&next, window, cx),
        )))
        .into_any_element()
    }

    /// The docked column in the right pane.
    fn render_column(&self, cx: &mut Context<Self>) -> AnyElement {
        let transcript = self.transcript();
        let input = self.card_input.clone();
        let handler = cx.listener(|this, intent: &CardIntent, window, cx| this.on_card_intent(intent.clone(), window, cx));
        agent_column("browse-column", &transcript, &input)
            .on_intent(move |next: CardIntent, window, cx| handler(&next, window, cx))
            .into_any_element()
    }

    /// Peek: a frameless rounded card centred over the page (72 % × 82 %),
    /// the page beneath dimmed, a round × floating inside its top-right
    /// corner and a quiet caption at its foot.
    fn render_peek(&self, cx: &mut Context<Self>) -> AnyElement {
        record_ax_label("education.gov.in · Peek · Esc closes");
        let p = cx.aui().colors;
        let link = p.accent;
        let peek_link = cx.listener(|this, target: &StoryPage, window: &mut Window, cx: &mut Context<Self>| {
            this.on_page_link(target.clone(), window, cx);
        });
        let noop = cx.listener(|_, _: &ClickEvent, _, _| {});
        let noop2 = cx.listener(|_, _: &ClickEvent, _, _| {});
        popover_layer(
            div()
                .id("browse-peek-layer")
                .absolute()
                .inset_0()
                .child(div().absolute().inset_0().bg(gpui::black()).opacity(0.35))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            v_flex()
                                .w(relative(0.72))
                                .h(relative(0.82))
                                .rounded(px(12.0))
                                .bg(gpui::white())
                                .shadow(p.shadow(3))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .flex_none()
                                        .w_full()
                                        .h(px(0.0))
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .child(
                                            div().mt(px(8.0)).mr(px(8.0)).rounded_full().bg(gpui::black()).opacity(0.55).child(
                                                icon_button("browse-peek-close", IconName::X)
                                                    .accessibility_label("Close peek")
                                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                                        this.peek_open = false;
                                                        cx.notify();
                                                    })),
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("browse-peek-scroll")
                                        .flex_1()
                                        .min_h(px(0.0))
                                        .w_full()
                                        .overflow_y_scroll()
                                        .child(render_page(&self.peek_page, link, peek_link, noop, noop2)),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .w_full()
                                        .py(px(6.0))
                                        .flex()
                                        .flex_row()
                                        .justify_center()
                                        .ui(scale::FS_11)
                                        .text_color(p.ink_3)
                                        .child("education.gov.in · Peek · Esc closes"),
                                ),
                        ),
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
                            _ => this.switch_space(Space::New, cx),
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
                self.story_advanced();
                cx.notify();
            }
            Some(("own", n)) => {
                if let Ok(i) = n.trim().parse::<usize>() {
                    self.select_space_tab(i, cx);
                }
            }
            Some(("task", n)) => {
                if let Ok(i) = n.trim().parse::<usize>() {
                    if self.story.tabs().get(i).is_some() {
                        self.set_shown(Shown::Task(i));
                        cx.notify();
                    }
                }
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
                    self.ask_after_done = self.story.phase() == Phase::Done;
                    self.line_open = true;
                    self.want_focus = Some(FocusKind::Line);
                    cx.notify();
                }
                "send" => {
                    self.line_input.update(cx, |state, cx| state.set_value(ASK_TEXT, window, cx));
                    self.do_send(window, cx);
                }
                "takeover" => self.page_takeover(cx),
                "handback" => {
                    self.story.handback();
                    self.kick_timer(window, cx);
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
                        self.kick_timer(window, cx);
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
                    self.kick_timer(window, cx);
                    cx.notify();
                }
                "deny" => {
                    self.story.deny();
                    cx.notify();
                }
                "answer" => {
                    self.story.answer();
                    self.story_advanced();
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

    /// Starts the harness after the first frame. When the steps finish,
    /// the timer stops, so a scripted `run:N` never leaves a live loop
    /// racing the capture.
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
            let _ = this.update_in(cx, |this, _, cx| {
                this.stop_timer();
                cx.notify();
            });
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
        // Parked focus requests: the Field editor, the ⌘J line input and
        // the card input take the keyboard as they open.
        if let Some(kind) = self.want_focus.take() {
            let handle = match kind {
                FocusKind::Field => self.field_editor.read(cx).focus_handle(cx),
                FocusKind::Line => self.line_input.read(cx).focus_handle(cx),
                FocusKind::Card => self.card_input.read(cx).focus_handle(cx),
            };
            window.focus(&handle, cx);
        }
        // Hidden means gone: with the sidebar closed the shell gets an
        // empty pane (no rail), so the page takes the width.
        let sidebar = if self.sidebar_open { self.render_sidebar(cx) } else { div().size_full().into_any_element() };
        let centre = self.render_centre(cx);
        let right_open = self.docked && self.card_open;
        let right = self.docked.then(|| self.render_column(cx));
        // SidebarHeader paints the lights but owns no trailing slot for the
        // nav cluster, so the cluster sits beside it in the header cell.
        let can_back = !self.back.is_empty();
        let can_forward = !self.forward.is_empty();
        let header_sidebar = h_flex()
            .w_full()
            .h_full()
            .items_center()
            .child(div().flex_none().child(sidebar_header("browse-hd-side").traffic_lights(true).collapsed(true)))
            .child(div().flex_1())
            .child(
                browse_nav_cluster("browse-nav")
                    .can_go_back(can_back)
                    .can_go_forward(can_forward)
                    .on_back(cx.listener(|this, _: &ClickEvent, _, cx| this.go_back(cx)))
                    .on_forward(cx.listener(|this, _: &ClickEvent, _, cx| this.go_forward(cx)))
                    .on_reload(cx.listener(|_, _: &ClickEvent, _, cx| {
                        cx.notify();
                    })),
            );
        let shell = app_shell("browse-shell")
            .sidebar_width(px(BROWSE_SIDEBAR_WIDTH))
            .sidebar_open(self.sidebar_open)
            .hide_rail(true)
            .traffic_lights(true)
            .header_sidebar(header_sidebar)
            .header_centre(div())
            .sidebar(sidebar)
            .centre(centre)
            .right_width(px(420.0))
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
                // After Done the line re-opens as the Ask prompt: a fresh
                // story may start.
                if this.story.phase() == Phase::Done && !this.line_open {
                    this.ask_after_done = true;
                }
                this.line_open = !this.line_open;
                if this.line_open {
                    this.want_focus = Some(FocusKind::Line);
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowCard, window, cx| {
                this.card_open = true;
                this.docked = false;
                this.want_focus = Some(FocusKind::Card);
                let _ = window;
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
                let address = this.shown_address();
                this.open_field(Some(address), window, cx);
            }))
            .on_action(cx.listener(|this, _: &HideSidebar, _, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SpaceOne, _, cx| this.switch_space(Space::Education, cx)))
            .on_action(cx.listener(|this, _: &SpaceTwo, _, cx| this.switch_space(Space::Personal, cx)))
            .on_action(cx.listener(|this, _: &TakeOver, _, cx| this.page_takeover(cx)))
            .on_action(cx.listener(|this, _: &AdvanceStep, window, cx| {
                // Space steps the story only while it runs with no text
                // input focused; typed spaces always reach the input.
                if !this.story.is_advancing() || this.text_focused(window, cx) {
                    cx.propagate();
                } else {
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
                    this.steer(&text, window, cx);
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
            .children(self.foot.is_some().then(|| self.render_foot(cx)))
    }
}

/// One screen in window state, keyed for the gallery entry.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state(SharedString::from("browse-screen"), cx, BrowseView::new);
    div().size_full().child(view).into_any_element()
}

#[cfg(test)]
mod tests {
    // NOTE: `use super::build`, not a glob: the parent's `use gpui::*`
    // imports gpui's `test` macro, and a glob would pull it into this
    // scope where the `#[gpui::test]` expansion's inner `#[test]`
    // re-resolves to the macro instead of the builtin, recursing forever.
    use super::build;
    use super::{AdvanceStep, BrowseView, FocusKind, Highlight, Phase, Shown, StoryPage};
    use super::{BrowseStory, StoryLine};
    use aui::browse::agent_window::{AgentItem, StepState};
    use aui::keys::SelectNext;
    use aui::transcript::{arm_ax_probe, take_ax_labels};
    use gpui::{Entity, Focusable, IntoElement, TestAppContext, Window};

    struct Host;

    impl gpui::Render for Host {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            build(window, cx)
        }
    }

    /// A bare view with the theme initialised, for state-level tests.
    fn bare_view(cx: &mut TestAppContext) -> (Entity<BrowseView>, &mut gpui::VisualTestContext) {
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        cx.add_window_view(|window, cx| BrowseView::new(window, cx))
    }

    /// The scripted takeover leaves the "You're driving" line with its
    /// Hand back button: the screen builds with both labels.
    #[gpui::test]
    fn takeover_leaves_you_driving_with_hand_back(cx: &mut TestAppContext) {
        std::env::set_var("AUI_GALLERY_STEPS", "ask;send;run:3;takeover");
        cx.update(|cx| aui::init(aui_tokens::ThemeKind::Dark, cx));
        arm_ax_probe(true);
        take_ax_labels();
        // Shadow the context with the window's, per `add_window_view`'s
        // docs: the returned context borrows the old one, so keeping both
        // alive does not compile.
        let (_host, cx) = cx.add_window_view(|_, _| Host);
        // The harness applies steps after its first-frame delay; advance
        // the simulated clock past it, park the step task, then land a
        // frame so the takeover renders.
        cx.executor().advance_clock(std::time::Duration::from_millis(600));
        cx.run_until_parked();
        cx.update(|window, app| {
            window.simulate_next_frame(app);
        });
        let labels = take_ax_labels();
        arm_ax_probe(false);
        std::env::remove_var("AUI_GALLERY_STEPS");
        assert!(labels.iter().any(|l| l.contains("You're driving")), "takeover shows You're driving, got {labels:?}");
        assert!(labels.iter().any(|l| l.contains("Hand back")), "takeover shows Hand back, got {labels:?}");
    }

    /// H1: after `send;run:3`, selecting own tab 1 shows the UDISE+
    /// dashboard, highlights row 1 and addresses its URL.
    #[gpui::test]
    fn own_tab_select_shows_dashboard_with_highlight_and_address(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.do_send(window, cx);
                v.story.advance_to(3);
                v.story_advanced();
                v.stop_timer();
                v.select_space_tab(1, cx);
            });
        });
        let (centre, address, highlight, shown) =
            cx.update(|_, app| view.update(app, |v, _| (v.centre_page(), v.shown_address(), v.highlighted(), v.shown.clone())));
        assert_eq!(centre, StoryPage::Udise, "own tab 1 shows the UDISE+ dashboard");
        assert_eq!(highlight, Highlight::Own(1), "only row 1 is highlighted");
        assert_eq!(address, "https://udiseplus.gov.in/#/home", "the address follows the shown tab");
        assert_eq!(shown, Shown::Own(1));
    }

    /// H2: hidden means the sidebar is gone and the 28 px strip stays.
    #[gpui::test]
    fn hidden_sidebar_removes_sidebar_keeps_strip(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        arm_ax_probe(true);
        take_ax_labels();
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.sidebar_open = false;
                cx.notify();
            });
            window.simulate_next_frame(app);
        });
        let labels = take_ax_labels();
        arm_ax_probe(false);
        assert!(!labels.iter().any(|l| l.contains("New tab")), "no sidebar rows while hidden, got {labels:?}");
        assert!(
            labels.iter().any(|l| l.contains("PGI 2.0 · state scores 2023-24")),
            "the strip names the shown page, got {labels:?}"
        );
    }

    /// H3: after `field`, the Field editor is focused and one `down`
    /// moves the selection.
    #[gpui::test]
    fn field_editor_takes_focus_and_down_moves_selection(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        cx.update(|window, app| {
            view.update(app, |v, cx| v.open_field(None, window, cx));
            window.simulate_next_frame(app);
        });
        let editor = cx.update(|_, app| view.update(app, |v, cx| v.field_editor.read(cx).focus_handle(cx)));
        let focused = cx.update(|window, app| window.focused(app));
        assert_eq!(focused, Some(editor), "the Field editor holds the focus");
        let before = cx.update(|_, app| view.update(app, |v, _| v.field_model.selected));
        cx.dispatch_action(SelectNext);
        cx.run_until_parked();
        let after = cx.update(|_, app| view.update(app, |v, _| v.field_model.selected));
        let rows = cx.update(|_, app| view.update(app, |v, _| v.field_model.rows.len()));
        assert_eq!(after, (before + 1) % rows.max(1), "one down moves the selection");
    }

    /// H4: with the line open and focused, typing a space does not
    /// advance the story.
    #[gpui::test]
    fn space_with_focused_line_does_not_advance(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.do_send(window, cx);
                v.stop_timer();
                v.line_open = true;
                v.want_focus = Some(FocusKind::Line);
            });
            window.simulate_next_frame(app);
        });
        let line = cx.update(|_, app| view.update(app, |v, cx| v.line_input.read(cx).focus_handle(cx)));
        let focused = cx.update(|window, app| window.focused(app));
        assert_eq!(focused, Some(line), "the line input holds the focus");
        let before = cx.update(|_, app| view.update(app, |v, _| v.story.completed()));
        cx.dispatch_action(AdvanceStep);
        cx.run_until_parked();
        let after = cx.update(|_, app| view.update(app, |v, _| v.story.completed()));
        assert_eq!(after, before, "a typed space never steps the story");
    }

    /// M1: three HandBacks keep exactly one live loop.
    #[gpui::test]
    fn three_handbacks_keep_a_single_timer(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.do_send(window, cx);
                assert_eq!(v.timer_spawns, 1, "send starts the one loop");
                v.page_takeover(cx);
                for _ in 0..3 {
                    v.story.handback();
                    v.kick_timer(window, cx);
                }
                assert_eq!(v.timer_spawns, 1, "re-kicks reuse the live loop");
                assert!(v.timer_live, "the loop is still live");
                v.stop_timer();
                assert!(!v.timer_live, "the harness leaves no live timer");
            });
        });
    }

    /// M2: at each `completed` the line status and the card's running step
    /// name the same step.
    #[gpui::test]
    fn line_and_card_share_the_current_step(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        for n in 1..=7 {
            let line_current = cx.update(|_, app| {
                view.update(app, |v, _| {
                    v.story = BrowseStory::new();
                    v.story.send();
                    v.story.advance_to(n);
                    let current = v.story.current_step();
                    let transcript = v.transcript();
                    let running = transcript
                        .items
                        .iter()
                        .filter_map(|item| match item {
                            AgentItem::Steps(steps) => Some(steps),
                            _ => None,
                        })
                        .flat_map(|steps| steps.iter())
                        .filter(|step| step.state == StepState::Running)
                        .map(|step| step.text.clone())
                        .collect::<Vec<_>>();
                    (v.story.line(), current, running)
                })
            });
            let (line, current, running) = line_current;
            match (line, current) {
                (_, None) => assert_eq!(n, 7, "only the built comparison has no current step"),
                (StoryLine::Driving { status, .. }, Some(current)) => {
                    assert!(
                        status.to_lowercase().contains(&current.to_lowercase()),
                        "step {n}: line `{status}` names `{current}`"
                    );
                    assert!(running.iter().any(|text| text == &current), "step {n}: the card runs `{current}`");
                }
                (_, Some(current)) => {
                    assert!(running.iter().any(|text| text == &current), "step {n}: the card runs `{current}`");
                }
            }
        }
    }

    /// M3: Done and Stopped lines never offer Take over.
    #[gpui::test]
    fn terminal_lines_offer_no_take_over(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        arm_ax_probe(true);
        take_ax_labels();
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.story.send();
                v.story.answer();
                v.stop_timer();
                v.line_open = true;
                cx.notify();
            });
            window.simulate_next_frame(app);
        });
        let labels = take_ax_labels();
        assert!(labels.iter().any(|l| l.contains("Done — the comparison is ready")), "done reads ready, got {labels:?}");
        assert!(labels.iter().any(|l| l.contains("Open answer")), "done offers Open answer, got {labels:?}");
        assert!(!labels.iter().any(|l| l.contains("Take over")), "a finished task has no Take over, got {labels:?}");
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.story = BrowseStory::new();
                v.story.send();
                v.story.show_approval();
                v.story.deny();
                v.line_open = true;
                cx.notify();
            });
            window.simulate_next_frame(app);
        });
        let labels = take_ax_labels();
        arm_ax_probe(false);
        assert!(
            labels.iter().any(|l| l.contains("Stopped — the download was denied")),
            "denial names its cause, got {labels:?}"
        );
        assert!(!labels.iter().any(|l| l.contains("Take over")), "a stopped task has no Take over, got {labels:?}");
    }

    /// M3/L3: card steers queue while Running/Paused, stay in the input
    /// while NeedsYou/Approval, and never queue from Idle.
    #[gpui::test]
    fn card_steers_queue_keep_or_start(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        // Idle + text starts instead of queueing (⌘↩ must not queue).
        cx.update(|window, app| {
            view.update(app, |v, cx| v.steer("hello", window, cx));
        });
        let phase = cx.update(|_, app| view.update(app, |v, _| v.story.phase()));
        assert_eq!(phase, Phase::Running, "idle steer sends");
        assert!(cx.update(|_, app| view.update(app, |v, _| v.story.queued().is_none())), "a fresh send queues nothing");
        // Running + text queues and clears the card input.
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.card_input.update(cx, |state, cx| state.set_value("also add GER", window, cx));
                v.steer("also add GER", window, cx);
            });
        });
        let (queued, cleared) = cx.update(|_, app| {
            view.update(app, |v, cx| (v.story.queued().is_some(), v.card_input.read(cx).value().is_empty()))
        });
        assert!(queued, "running steers queue behind the turn");
        assert!(cleared, "a queued steer clears the input");
        // NeedsYou + text stays in the input, never silently dropped, and
        // the earlier queued chip is left exactly as it was.
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.story.needs_you();
                v.card_input.update(cx, |state, cx| state.set_value("wait for me", window, cx));
                v.steer("wait for me", window, cx);
            });
        });
        let (kept, queued) = cx.update(|_, app| {
            view.update(app, |v, cx| (v.card_input.read(cx).value().to_string(), v.story.queued()))
        });
        assert_eq!(kept.as_str(), "wait for me", "needs-you steers stay in the input");
        assert!(
            matches!(&queued, Some(text) if text.to_string().contains("also add GER")),
            "needs-you steers queue nothing new, got {queued:?}"
        );
    }

    /// M4: with peek + line open, Esc closes the line first; Esc on the
    /// approval denies, and the next Esc closes the line.
    #[gpui::test]
    fn esc_closes_the_line_before_peek(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        cx.update(|_, app| {
            view.update(app, |v, cx| {
                v.line_open = true;
                v.peek_open = true;
                v.close_topmost(cx);
                assert!(!v.line_open, "Esc closes the line first");
                assert!(v.peek_open, "peek stays until the next Esc");
                v.line_open = true;
                v.story.send();
                v.story.show_approval();
                v.close_topmost(cx);
                assert_eq!(v.story.phase(), Phase::Stopped, "Esc on the approval denies");
                assert!(v.line_open, "the line stays so the next Esc can close it");
                v.close_topmost(cx);
                assert!(!v.line_open, "the next Esc closes the line");
            });
        });
    }

    /// M5: the story's advance tracks a task tab but never yanks an own tab.
    #[gpui::test]
    fn advance_keeps_the_persons_own_tab(cx: &mut TestAppContext) {
        let (view, cx) = bare_view(cx);
        cx.update(|window, app| {
            view.update(app, |v, cx| {
                v.do_send(window, cx);
                v.stop_timer();
                v.select_space_tab(1, cx);
                v.story.advance_to(3);
                v.story_advanced();
                assert_eq!(v.shown, Shown::Own(1), "an own tab stays while the folder grows");
                assert_eq!(v.centre_page(), StoryPage::Udise);
                v.set_shown(Shown::Task(0));
                v.story.advance_to(4);
                v.story_advanced();
                assert_eq!(v.shown, Shown::Task(3), "a task tab tracks the story");
            });
        });
    }

    /// L6: saved-page addresses slugify.
    #[gpui::test]
    fn generic_page_url_slugifies(cx: &mut TestAppContext) {
        let _ = cx;
        assert_eq!(
            super::page_url(&StoryPage::Generic("Hacker News · newest".into())),
            "https://example.com/hacker-news-newest"
        );
    }
}

