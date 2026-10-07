//! The PGI story's state machine.
//!
//! The screen renders from this and nothing else, so the scripted timers,
//! the Space key and the `AUI_GALLERY_STEPS` harness all drive the prototype
//! through the same transitions a click does. The unit tests below pin every
//! transition the task names.

use std::time::Duration;

use aui::browse::agent_window::ApprovalData;
use aui::browse::TaskFace;
use gpui::SharedString;

/// How often the timers (and the Space key) advance a running story.
pub const STEP_INTERVAL: Duration = Duration::from_millis(1200);

/// The exact ask text: the preset the ⌘J line sends.
pub const ASK_TEXT: &str = "compare PGI 2.0 grades with PTR, dropout and girls' enrolment for Karnataka, Tamil Nadu, Kerala, Maharashtra and Gujarat — one table, with sources";

/// The answered comparison the card shows once the table is built.
pub const ANSWER_TEXT: &str = "Karnataka and Tamil Nadu lead the five-state table on PTR and girls' enrolment. Gujarat improved 3.5 points since 2021, while dropout is highest in Maharashtra.";

/// The queued follow-up chip a steer leaves behind a running turn.
/// (Pinned by the screen's queued-chip contract; read by the tests.)
#[allow(dead_code)]
pub const QUEUED_TEXT: &str = "also add girls' GER · queued";

/// The site actions under the Ask line.
pub const SITE_ACTIONS: [(&str, &str); 2] = [("sum", "Sum it up"), ("extract-table", "Extract the table")];

/// Which drawn page a tab shows.
#[derive(Debug, Clone, PartialEq)]
pub enum StoryPage {
    /// PGI 2.0 state scores table.
    Pgi,
    /// UDISE+ PTR bars.
    Udise,
    /// UDISE+ dropout page (a saved generic page).
    Dropout,
    /// data.gov.in sign-in page.
    Datagov,
    /// The answered comparison table.
    Answer,
    /// The methodology note.
    Peek,
    /// NAS report cards.
    Nas,
    /// The local ledger page.
    Ledger,
    /// The weekend weather card.
    Weather,
    /// Any other saved page, by title.
    Generic(SharedString),
}

/// One tab of the PGI task folder.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskTab {
    /// The tab title, as the sidebar row shows it.
    pub title: SharedString,
    /// The page the tab shows.
    pub page: StoryPage,
}

impl TaskTab {
    /// A task tab with a title and a page.
    pub fn new(title: impl Into<SharedString>, page: StoryPage) -> Self {
        Self { title: title.into(), page }
    }
}

/// The tabs the story adds, in order: PGI scores, PTR, dropout, enrolment,
/// then the answered comparison. Steps 5 (approval) and 6 (download) add no
/// tab of their own.
fn story_tabs() -> Vec<TaskTab> {
    vec![
        TaskTab::new("PGI 2.0 scores · 5 states", StoryPage::Pgi),
        TaskTab::new("UDISE+ · PTR by state", StoryPage::Udise),
        TaskTab::new("UDISE+ · dropout, secondary", StoryPage::Dropout),
        TaskTab::new("data.gov.in · enrolment 2024-25", StoryPage::Datagov),
        TaskTab::new("Answer · PGI vs indicators", StoryPage::Answer),
    ]
}

/// Why a task stopped: a denied download, or the person stopping it.
/// The stopped line names the cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopCause {
    /// The download was denied on the approval card.
    Denied,
    /// The person stopped the task.
    User,
}

/// What the task is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Nothing sent yet.
    Idle,
    /// Steps advance on the timer or the Space key.
    Running,
    /// The person took the wheel; steps are held.
    Paused,
    /// data.gov.in asks for a sign-in; Done continues.
    NeedsYou,
    /// A download asks for permission; Allow continues, Deny stops.
    Approval,
    /// Denied: the face rests paused and the steps stop.
    Stopped,
    /// The comparison is built.
    Done,
}

/// What the ⌘J line shows. Mirrors `agent_window::LineState` without the
/// input entity, so the screen maps it one to one.
#[derive(Debug, Clone, PartialEq)]
pub enum StoryLine {
    /// The idle Ask prompt.
    Ask,
    /// The agent works: status text and face.
    Driving {
        /// One status line.
        status: SharedString,
        /// The task's face.
        face: TaskFace,
    },
    /// The person drives while the agent waits.
    YouDrive,
    /// The page needs the person.
    NeedsYou {
        /// What needs the person.
        text: SharedString,
    },
    /// A download asks for permission.
    Approval,
}

/// The approval the story raises, exactly as specified.
pub fn approval_data() -> ApprovalData {
    ApprovalData {
        title: "Claude wants to download udise_2024_25_state_indicators.xlsx".into(),
        host: "data.gov.in".into(),
        line: "data.gov.in · 14 MB · to ~/Downloads/pgi-compare".into(),
        kind_pill: "Download".into(),
        hint: "Always asks before downloading".into(),
        deny: "Don't allow".into(),
        allow: "Download".into(),
        allow_always: None,
    }
}

/// The PGI story: phases, tabs, face, status word, centre page and the
/// queued steer. Step numbers are the story's own: 1 opened PGI, 2 read
/// PTR, 3 reads dropout, 4 needs the sign-in, 5 asks for the download,
/// 6 downloaded, 7 built the comparison.
#[derive(Debug, Clone)]
pub struct BrowseStory {
    phase: Phase,
    completed: usize,
    tabs: Vec<TaskTab>,
    task_open: bool,
    face: TaskFace,
    status: SharedString,
    centre: StoryPage,
    queued: Option<SharedString>,
    approval_open: bool,
    downloaded: bool,
    answered: bool,
    paused_from: Phase,
    stopped: Option<StopCause>,
}

/// The done labels the card's transcript shows for finished steps.
const DONE_TEXTS: [&str; 7] = [
    "Opened PGI 2.0 state scores (2023-24)",
    "Read UDISE+ 2024-25 PTR for 5 states",
    "Read dropout rates, secondary",
    "Read enrolment on data.gov.in",
    "Approved the spreadsheet download",
    "Downloaded udise_2024_25_state_indicators.xlsx",
    "Built the comparison",
];

/// The running label of each story step, 0-based by step index. The line
/// status and the card's running row both derive from this one table, so
/// the two can never disagree about what the agent is doing.
const RUN_TEXTS: [&str; 7] = [
    "Reading PGI 2.0 · state scores",
    "Reading UDISE+ · PTR",
    "Reading UDISE+ · dropout",
    "Reading enrolment on data.gov.in",
    "Asking to download the spreadsheet",
    "Downloading the indicators spreadsheet",
    "Building the comparison",
];

impl BrowseStory {
    /// A fresh story: the task folder closed, face Reading, no tabs.
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            completed: 0,
            tabs: Vec::new(),
            task_open: false,
            face: TaskFace::Reading,
            status: TaskFace::Reading.status_word().into(),
            centre: StoryPage::Pgi,
            queued: None,
            approval_open: false,
            downloaded: false,
            answered: false,
            paused_from: Phase::Idle,
            stopped: None,
        }
    }

    /// The task phase.
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Completed story steps, 0–7.
    /// (Read by the state-machine tests; the screen branches on `phase`.)
    #[allow(dead_code)]
    pub fn completed(&self) -> usize {
        self.completed
    }

    /// The task folder's tabs, oldest first.
    pub fn tabs(&self) -> &[TaskTab] {
        &self.tabs
    }

    /// Whether the task folder is open.
    pub fn task_open(&self) -> bool {
        self.task_open
    }

    /// The task folder's face.
    pub fn face(&self) -> TaskFace {
        self.face
    }

    /// The task folder's status word (`typing`, `done · 5 tabs`, …).
    pub fn status(&self) -> SharedString {
        self.status.clone()
    }

    /// The page the story last opened.
    /// (Read by the state-machine tests; the screen shows `shown`.)
    #[allow(dead_code)]
    pub fn centre(&self) -> &StoryPage {
        &self.centre
    }

    /// The queued steer chip, if the person typed one mid-run.
    pub fn queued(&self) -> Option<SharedString> {
        self.queued.clone()
    }

    /// Whether the approval card waits on the person.
    /// (Read by the state-machine tests; the screen branches on `phase`.)
    #[allow(dead_code)]
    pub fn approval_open(&self) -> bool {
        self.approval_open
    }

    /// Whether the spreadsheet downloaded.
    /// (Read by the state-machine tests; the screen branches on `completed`.)
    #[allow(dead_code)]
    pub fn downloaded(&self) -> bool {
        self.downloaded
    }

    /// Whether the comparison is built.
    pub fn answered(&self) -> bool {
        self.answered
    }

    /// The timers and the Space key may advance the story.
    pub fn is_advancing(&self) -> bool {
        self.phase == Phase::Running
    }

    /// Why the task stopped, when it did.
    /// (Read by the state-machine tests; the stopped line names it.)
    #[allow(dead_code)]
    pub fn stop_cause(&self) -> Option<StopCause> {
        self.stopped
    }

    /// The 0-based index of the step currently running, if any. While
    /// NeedsYou or Approval the current step stays Running (it is not
    /// marked Done until the person answers), so the card and the line
    /// agree on what is still open.
    pub fn current_index(&self) -> Option<usize> {
        match self.phase {
            Phase::Running if self.completed < 7 => Some(self.completed),
            Phase::NeedsYou if self.completed >= 4 => Some(3),
            Phase::Approval => Some(4),
            _ => None,
        }
    }

    /// The single source of truth for the current step's running label.
    /// Both the ⌘J line status and the card transcript's running row
    /// derive from this.
    pub fn current_step(&self) -> Option<SharedString> {
        self.current_index().map(|i| SharedString::from(RUN_TEXTS[i]))
    }

    /// The done labels the transcript shows, oldest first: every finished
    /// step except the one still running.
    pub fn done_steps(&self) -> Vec<SharedString> {
        let end = self.current_index().unwrap_or(self.completed.min(7));
        DONE_TEXTS.iter().take(end).map(|s| SharedString::from(*s)).collect()
    }

    /// Send the ask: the folder opens, face Typing, first tab lands.
    pub fn send(&mut self) {
        self.task_open = true;
        self.advance_to(1);
    }

    /// Advance to step `n` (clamped to 0–7), forward only. The harness's
    /// `run:N`, the timers and the Space key all come through here. A
    /// no-op while Paused or Stopped: the held task waits for Hand back.
    pub fn advance_to(&mut self, n: usize) {
        if matches!(self.phase, Phase::Paused | Phase::Stopped) {
            return;
        }
        let target = n.min(7);
        while self.completed < target {
            self.apply(self.completed + 1);
        }
    }

    /// One story step forward; a no-op past the end or while held.
    pub fn advance_one(&mut self) {
        self.advance_to(self.completed + 1);
    }

    /// Applies step `n`, which the caller has already bounds-checked.
    fn apply(&mut self, n: usize) {
        let tabs = story_tabs();
        match n {
            1 => {
                self.phase = Phase::Running;
                self.tabs = vec![tabs[0].clone()];
                self.task_open = true;
                self.face = TaskFace::Typing;
                self.status = "typing".into();
                self.centre = StoryPage::Pgi;
            }
            2 => {
                self.tabs.push(tabs[1].clone());
                self.centre = StoryPage::Udise;
            }
            3 => {
                self.tabs.push(tabs[2].clone());
            }
            4 => {
                self.tabs.push(tabs[3].clone());
                self.phase = Phase::NeedsYou;
                self.face = TaskFace::NeedsYou;
                self.status = "needs you".into();
                self.centre = StoryPage::Datagov;
            }
            5 => {
                self.phase = Phase::Approval;
                self.approval_open = true;
            }
            6 => {
                self.approval_open = false;
                self.downloaded = true;
                self.phase = Phase::Running;
                self.face = TaskFace::Typing;
                self.status = "typing".into();
            }
            7 => {
                self.tabs.push(tabs[4].clone());
                self.answered = true;
                self.phase = Phase::Done;
                self.face = TaskFace::Done;
                self.status = "done · 5 tabs".into();
                self.centre = StoryPage::Answer;
                self.queued = None;
            }
            _ => return,
        }
        self.completed = n;
    }

    /// The sign-in page needs the person.
    pub fn needs_you(&mut self) {
        self.advance_to(4);
    }

    /// Done on the needs-you line: the download asks next.
    pub fn resolve_needs(&mut self) {
        if self.phase == Phase::NeedsYou {
            self.advance_to(5);
        }
    }

    /// Raise the approval without the earlier steps (harness shortcut).
    pub fn show_approval(&mut self) {
        if self.completed < 4 {
            self.advance_to(4);
        }
        self.advance_to(5);
    }

    /// Allow the download: the task continues to the downloaded step.
    pub fn allow(&mut self) {
        if self.phase == Phase::Approval {
            self.advance_to(6);
        }
    }

    /// Deny the download: face Paused, status stopped, steps stop.
    pub fn deny(&mut self) {
        if self.phase == Phase::Approval {
            self.approval_open = false;
            self.phase = Phase::Stopped;
            self.face = TaskFace::Paused;
            self.status = "stopped".into();
            self.stopped = Some(StopCause::Denied);
            self.queued = None;
        }
    }

    /// Build the comparison (harness shortcut for the tail). A no-op
    /// while Paused or Stopped, like every other advance.
    pub fn answer(&mut self) {
        if matches!(self.phase, Phase::Paused | Phase::Stopped) {
            return;
        }
        if self.completed < 6 {
            self.approval_open = false;
            self.downloaded = true;
            self.phase = Phase::Running;
            self.face = TaskFace::Typing;
            self.status = "typing".into();
            // The harness jumps straight to the tail: land the tabs and the
            // centre the skipped steps would have left, then build on top.
            let tabs = story_tabs();
            self.tabs = tabs[..4].to_vec();
            self.task_open = true;
            self.centre = StoryPage::Datagov;
            self.completed = 6;
        }
        self.advance_to(7);
    }

    /// Done on the answer page: the task's other tabs close.
    pub fn finish_task(&mut self) {
        if self.answered {
            let tabs = story_tabs();
            self.tabs = vec![tabs[4].clone()];
            self.status = "done · 1 tab".into();
        }
    }

    /// Stops the task outright: steps halt with the face paused.
    pub fn stop(&mut self) {
        if matches!(self.phase, Phase::Running | Phase::NeedsYou | Phase::Approval | Phase::Paused) {
            self.approval_open = false;
            self.phase = Phase::Stopped;
            self.face = TaskFace::Paused;
            self.status = "stopped".into();
            self.stopped = Some(StopCause::User);
            self.queued = None;
        }
    }

    /// A click on the page while the task runs: the person takes the wheel.
    pub fn takeover(&mut self) {
        if matches!(self.phase, Phase::Running | Phase::NeedsYou | Phase::Approval) {
            self.paused_from = self.phase;
            self.phase = Phase::Paused;
            self.face = TaskFace::Paused;
            self.status = "paused".into();
        }
    }

    /// Hand back (button or ⌘↩): the held phase resumes.
    pub fn handback(&mut self) {
        if self.phase == Phase::Paused {
            self.phase = self.paused_from;
            let (face, status) = match self.phase {
                Phase::NeedsYou | Phase::Approval => (TaskFace::NeedsYou, "needs you".into()),
                _ => (TaskFace::Typing, "typing".into()),
            };
            // A held Approval reopens its card.
            if self.phase == Phase::Approval {
                self.approval_open = true;
            }
            self.face = face;
            self.status = status;
        }
    }

    /// A steer typed mid-run waits behind the turn as the queued chip.
    pub fn queue_steer(&mut self, text: impl Into<SharedString>) {
        self.queued = Some(text.into());
    }

    /// What the ⌘J line shows for this state. The stopped line names its
    /// cause; the screen renders Done/Stopped without Take over.
    pub fn line(&self) -> StoryLine {
        match self.phase {
            Phase::Idle => StoryLine::Ask,
            Phase::Running => StoryLine::Driving { status: self.driving_status(), face: self.face },
            Phase::Paused => StoryLine::YouDrive,
            Phase::NeedsYou => StoryLine::NeedsYou { text: "data.gov.in asks you to sign in".into() },
            Phase::Approval => StoryLine::Approval,
            Phase::Stopped => StoryLine::Driving { status: self.stopped_status(), face: TaskFace::Paused },
            Phase::Done => StoryLine::Driving { status: "Done — the comparison is ready".into(), face: TaskFace::Done },
        }
    }

    /// The stopped line for the stop cause.
    fn stopped_status(&self) -> SharedString {
        match self.stopped {
            Some(StopCause::User) => "Stopped — you stopped it".into(),
            _ => "Stopped — the download was denied".into(),
        }
    }

    /// The driving line for the current step, derived from the same
    /// [`current_step`](Self::current_step) the transcript's running row
    /// uses, so the two always name the same step.
    fn driving_status(&self) -> SharedString {
        match self.current_step() {
            Some(step) => {
                let mut text = step.to_string();
                // "Reading …" reads as "Claude is reading …".
                if let Some(first) = text.get_mut(..1) {
                    first.make_ascii_lowercase();
                }
                SharedString::from(format!("Claude is {text}"))
            }
            None => "Claude is working".into(),
        }
    }
}

impl Default for BrowseStory {
    /// A fresh story.
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_starts_running_with_one_tab() {
        let mut story = BrowseStory::new();
        story.send();
        assert_eq!(story.phase(), Phase::Running);
        assert_eq!(story.tabs().len(), 1);
        assert_eq!(story.face(), TaskFace::Typing);
        assert!(story.task_open());
    }

    #[test]
    fn run_three_lands_three_tabs_driving() {
        let mut story = BrowseStory::new();
        story.send();
        story.advance_to(3);
        assert_eq!(story.tabs().len(), 3);
        assert!(matches!(story.line(), StoryLine::Driving { .. }));
        assert_eq!(story.face(), TaskFace::Typing);
    }

    #[test]
    fn takeover_pauses_and_handback_resumes() {
        let mut story = BrowseStory::new();
        story.send();
        story.advance_to(2);
        story.takeover();
        assert_eq!(story.phase(), Phase::Paused);
        assert_eq!(story.face(), TaskFace::Paused);
        assert!(matches!(story.line(), StoryLine::YouDrive));
        story.handback();
        assert_eq!(story.phase(), Phase::Running);
    }

    #[test]
    fn needs_you_asks_for_sign_in() {
        let mut story = BrowseStory::new();
        story.send();
        story.needs_you();
        assert_eq!(story.phase(), Phase::NeedsYou);
        assert_eq!(story.face(), TaskFace::NeedsYou);
        assert!(matches!(story.line(), StoryLine::NeedsYou { .. }));
    }

    #[test]
    fn approval_allow_downloads() {
        let mut story = BrowseStory::new();
        story.send();
        story.show_approval();
        assert!(story.approval_open());
        story.allow();
        assert!(!story.approval_open());
        assert!(story.downloaded());
        assert_eq!(story.phase(), Phase::Running);
    }

    #[test]
    fn approval_deny_stops() {
        let mut story = BrowseStory::new();
        story.send();
        story.show_approval();
        story.deny();
        assert_eq!(story.phase(), Phase::Stopped);
        assert_eq!(story.face(), TaskFace::Paused);
        assert_eq!(story.status().as_ref(), "stopped");
    }

    #[test]
    fn answer_is_done_with_five_tabs() {
        let mut story = BrowseStory::new();
        story.send();
        story.answer();
        assert_eq!(story.phase(), Phase::Done);
        assert_eq!(story.face(), TaskFace::Done);
        assert_eq!(story.tabs().len(), 5);
        assert_eq!(story.status().as_ref(), "done · 5 tabs");
    }

    #[test]
    fn done_closes_the_other_tabs() {
        let mut story = BrowseStory::new();
        story.send();
        story.answer();
        story.finish_task();
        assert_eq!(story.tabs().len(), 1);
        assert_eq!(story.status().as_ref(), "done · 1 tab");
    }

    #[test]
    fn line_and_current_step_agree_at_every_step() {
        for n in 1..=7 {
            let mut story = BrowseStory::new();
            story.send();
            story.advance_to(n);
            if let Some(current) = story.current_step() {
                match story.line() {
                    StoryLine::Driving { status, .. } => assert!(
                        status.to_lowercase().contains(&current.to_lowercase()),
                        "step {n}: line `{status}` names the current step `{current}`"
                    ),
                    StoryLine::NeedsYou { .. } => assert_eq!(
                        current.as_ref(),
                        "Reading enrolment on data.gov.in",
                        "step {n}: the enrolment step waits, it is not done"
                    ),
                    StoryLine::Approval => assert_eq!(
                        current.as_ref(),
                        "Asking to download the spreadsheet",
                        "step {n}: the approval step waits, it is not done"
                    ),
                    other => panic!("step {n} should drive, need or approve, got {other:?}"),
                }
            } else {
                assert_eq!(n, 7, "only the built comparison has no current step");
                assert!(matches!(story.line(), StoryLine::Driving { .. }));
            }
        }
    }

    #[test]
    fn needs_you_and_approval_keep_the_current_step_running() {
        let mut story = BrowseStory::new();
        story.send();
        story.needs_you();
        assert_eq!(story.current_step().as_deref(), Some("Reading enrolment on data.gov.in"));
        assert_eq!(story.done_steps().len(), 3, "the enrolment step is not done while it needs you");
        story.resolve_needs();
        assert_eq!(story.current_step().as_deref(), Some("Asking to download the spreadsheet"));
        assert_eq!(story.done_steps().len(), 4, "the approval step is not done while it waits");
    }

    #[test]
    fn paused_and_stopped_hold_every_advance() {
        let mut story = BrowseStory::new();
        story.send();
        story.advance_to(2);
        story.takeover();
        story.advance_to(5);
        story.answer();
        assert_eq!(story.completed(), 2, "paused steps do not advance");
        story.handback();
        story.advance_to(3);
        assert_eq!(story.completed(), 3);
        story.stop();
        story.advance_to(7);
        story.answer();
        assert_eq!(story.completed(), 3, "stopped steps do not advance");
        assert_eq!(story.phase(), Phase::Stopped);
    }

    #[test]
    fn done_and_stopped_clear_the_queued_chip() {
        let mut story = BrowseStory::new();
        story.send();
        story.queue_steer("also add girls' GER · queued");
        story.answer();
        assert!(story.queued().is_none(), "done clears the queued chip");
        let mut story = BrowseStory::new();
        story.send();
        story.queue_steer("also add girls' GER · queued");
        story.show_approval();
        story.deny();
        assert!(story.queued().is_none(), "a denied stop clears the queued chip");
    }

    #[test]
    fn stopped_names_its_cause() {
        let mut denied = BrowseStory::new();
        denied.send();
        denied.show_approval();
        denied.deny();
        assert!(matches!(denied.line(), StoryLine::Driving { status, .. } if status.as_ref() == "Stopped — the download was denied"));
        let mut user = BrowseStory::new();
        user.send();
        user.stop();
        assert!(matches!(user.line(), StoryLine::Driving { status, .. } if status.as_ref() == "Stopped — you stopped it"));
    }
}
