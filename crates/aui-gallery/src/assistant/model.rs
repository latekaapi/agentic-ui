//! The mock's transcript model.
//!
//! `view.rs` renders from this and nothing else, so the sample state that
//! stands in for the three design references and the state the scripted
//! backend in [`super::script`] builds while a turn runs go through exactly the
//! same code path. Every block keeps the element id the static composition used
//! before, so the parity renders are unchanged.

use aui::protocol::{ActivityState, ApprovalState, QuestionOption, Step, StepState};
use aui_tokens::AgentState;
use gpui::SharedString;

use super::view::AssistantScreen;

/// The assistant body text: 13.5 / 1.65.
pub const BODY_TEXT: f32 = 13.5;

/// One block in the transcript, with the element id it renders under.
#[derive(Debug, Clone)]
pub struct Block {
    /// Stable element id; sample blocks keep the ids the static mock used.
    pub id: SharedString,
    /// What the block is.
    pub kind: BlockKind,
}

impl Block {
    /// A block with an explicit id.
    pub fn new(id: impl Into<SharedString>, kind: BlockKind) -> Self {
        Self { id: id.into(), kind }
    }
}

/// The transcript's block types. One variant per card the mock can show.
#[derive(Debug, Clone)]
pub enum BlockKind {
    /// A user prompt bubble (card 31).
    UserTurn {
        /// What the person typed.
        text: SharedString,
    },
    /// A folded run of tool calls (card 33).
    Activity {
        /// The timeline rows.
        steps: Vec<Step>,
        /// The folded summary line.
        summary: SharedString,
        /// The ink-3 detail after the summary.
        detail: Option<SharedString>,
        /// The elapsed pill.
        elapsed: SharedString,
        /// Working, done or failed.
        state: ActivityState,
        /// Whether the timeline is unfolded.
        open: bool,
    },
    /// An answer with inline citation markers (card 55).
    Answer {
        /// The whole answer, markers written `[[1]]`.
        text: SharedString,
        /// How many whitespace-separated groups have arrived; `None` means all
        /// of them.
        revealed: Option<usize>,
        /// Whether the streaming caret is shown.
        streaming: bool,
    },
    /// A permission request (card 35).
    Approval {
        /// The tool the agent wants to use.
        tool: SharedString,
        /// The command, on the terminal ground.
        command: SharedString,
        /// Why the agent wants it.
        reason: SharedString,
        /// Pending, approved, denied or auto-allowed.
        state: ApprovalState,
    },
    /// A question the agent asks before it starts (card 36).
    Question {
        /// The question.
        prompt: SharedString,
        /// The line under it.
        subtitle: SharedString,
        /// The options, in order.
        options: Vec<QuestionOption>,
        /// Which option is ticked, while the card is still pending.
        selected: Option<usize>,
        /// Once answered the card collapses to this chip row.
        answered: Option<SharedString>,
    },
    /// The "created in chat" artifact card under an answer.
    FileCard {
        /// The spreadsheet card rather than the document one.
        sheet: bool,
    },
    /// The grouped sources card with the hover card held open (card 55).
    Sources,
}

/// The status row under the transcript.
#[derive(Debug, Clone)]
pub struct Status {
    /// The dot's state.
    pub state: AgentState,
    /// The word beside the dot ("Done", "Working…").
    pub label: SharedString,
    /// The trailing detail after the middle dot.
    pub detail: SharedString,
}

/// The whole transcript: the blocks and the status row under them.
#[derive(Debug, Clone)]
pub struct Transcript {
    /// The blocks, oldest first.
    pub blocks: Vec<Block>,
    /// The status row.
    pub status: Status,
}

/// A finished step, the shape every sample activity row has.
fn done(verb: &str, target: &str, result: Option<&str>) -> Step {
    Step { verb: verb.to_string(), target: target.to_string(), state: StepState::Done, result: result.map(str::to_string) }
}

/// A step that has not started yet.
pub fn pending(verb: &str, target: &str, result: Option<&str>) -> Step {
    Step { verb: verb.to_string(), target: target.to_string(), state: StepState::Pending, result: result.map(str::to_string) }
}

impl Transcript {
    /// The sample state one assistant screen opens in — the exact composition
    /// the three design references show.
    pub fn sample(screen: AssistantScreen) -> Self {
        let (prompt, answer, detail) = match screen {
            AssistantScreen::Main => (
                "Tighten section 2. Eligibility must reflect the current procurement rules and the 2024 verification order.",
                "Under the current rules a bidder needs a valid registration and three years of comparable placements[[1]]. The 2024 order moved certificate verification to onboarding and requires originals[[2]]. The brief sets the cohort at 240 teachers across 38 institutions[[3]]. I folded all three into section 2 and left the rest of the draft untouched.",
                "3 sources cited",
            ),
            AssistantScreen::Sources => (
                "What does a bidder need to qualify, and where does that come from?",
                "Three requirements, each from a different source. A valid registration with the Directorate and at least three years of comparable placements in the preceding five[[1]]. Original certificates verified at onboarding rather than at bid time[[2]]. And for this RFP specifically, capacity for 240 teachers across 38 institutions[[3]]. Nothing in the Education Code adds to this; it defers to the procurement rules.",
                "hovering citation 1 \u{b7} source open on the right",
            ),
            AssistantScreen::Sheet => (
                "Score the four vendors against the Annex A matrix. Weight experience 45, coverage 30, price 25.",
                "Northlight leads on the weighted score, mostly on experience. Civic Talent is cheapest but thin on experience, which the matrix penalises hardest. The weights sit in row 7 so you can change them and the totals follow.",
                "E2 selected in the sheet",
            ),
        };
        let activity = match screen {
            AssistantScreen::Sheet => BlockKind::Activity {
                steps: vec![done("Read", "Annex A", None), done("Built", "the scoring matrix", None), done("Wrote", "vendor-scoring.xlsx", None)],
                summary: "Read Annex A".into(),
                detail: Some("\u{b7} built the matrix \u{b7} wrote vendor-scoring.xlsx".into()),
                elapsed: "14 s".into(),
                state: ActivityState::Done,
                open: false,
            },
            _ => BlockKind::Activity {
                steps: vec![
                    done("Searched", "Education Code", Some("4 passages")),
                    done("Searched", "Procurement Rules 2019", Some("5 passages")),
                    done("Searched", "GO 2024-18", Some("2 passages")),
                ],
                summary: "Searched 3 knowledge sets".into(),
                detail: Some("\u{b7} read 2 regulations \u{b7} 11 passages".into()),
                elapsed: "6 s".into(),
                state: ActivityState::Done,
                open: false,
            },
        };
        let tail = match screen {
            AssistantScreen::Sources => Block::new("assistant-sources", BlockKind::Sources),
            _ => Block::new("assistant-file", BlockKind::FileCard { sheet: screen == AssistantScreen::Sheet }),
        };
        Self {
            blocks: vec![
                Block::new("assistant-user-1", BlockKind::UserTurn { text: prompt.into() }),
                Block::new("assistant-activity", activity),
                Block::new("assistant-answer", BlockKind::Answer { text: answer.into(), revealed: None, streaming: false }),
                tail,
            ],
            status: Status { state: AgentState::Done, label: "Done".into(), detail: detail.into() },
        }
    }

    /// The index of the last block that is still streaming, if any.
    pub fn streaming_answer(&self) -> Option<usize> {
        self.blocks
            .iter()
            .rposition(|b| matches!(&b.kind, BlockKind::Answer { streaming: true, .. }))
    }

    /// The index of the newest approval card that is still pending.
    pub fn pending_approval(&self) -> Option<usize> {
        self.blocks
            .iter()
            .rposition(|b| matches!(&b.kind, BlockKind::Approval { state: ApprovalState::Pending, .. }))
    }

    /// The index of the newest question card that has not been answered.
    pub fn pending_question(&self) -> Option<usize> {
        self.blocks
            .iter()
            .rposition(|b| matches!(&b.kind, BlockKind::Question { answered: None, .. }))
    }
}

/// The whitespace-separated groups of an answer, so a partially streamed
/// answer is the first `n` of them joined back together. Citation markers ride
/// along inside their group, exactly as `CitedAnswer` expects them.
pub fn word_groups(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// The prefix of `text` that has arrived, given the reveal count.
pub fn revealed_text(text: &SharedString, revealed: Option<usize>) -> SharedString {
    match revealed {
        None => text.clone(),
        Some(n) => {
            let groups = word_groups(text);
            let n = n.min(groups.len());
            SharedString::from(groups[..n].join(" "))
        }
    }
}
