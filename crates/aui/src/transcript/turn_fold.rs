//! The settled-turn fold and the live activity row: the two turn-level
//! transcript primitives.
//!
//! A finished turn folds its work — tool calls and the agent's interim
//! prose — behind one [`TurnFold`] row (`Worked for 2m 14s · read 12 files,
//! edited 3, ran 5 commands`, with an optional `+48 −12` diff chip); the
//! final answer stays outside the fold. While a turn is live, each run of
//! tool calls shows as one [`LiveActivityRow`] naming the current call.
//!
//! Hosting: rows in the transcript list are per block, so a fold that hides
//! rows is best expressed as a header row plus a flag the host uses to skip
//! the folded rows while closed (the fold never owns the blocks). When the
//! host instead renders one row per turn, it can nest the blocks as the
//! fold's children with [`TurnFold::child`] — expanded, they render in
//! order. Either way the open state lives with the host.

use aui_motion::shimmer_text;
use aui_protocol::DiffStat;
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{record_ax_label, spinner};
use crate::transcript::quiet_row::{quiet_row, quiet_status_row, status_check};
use crate::util::ClickHandler;

/// `read 12 files, edited 3, ran 5 commands`: zero counts are omitted, so a
/// turn that only ran commands reads `ran 5 commands`.
pub fn turn_fold_summary(reads: u32, edits: u32, commands: u32) -> String {
    let mut parts = Vec::new();
    if reads > 0 {
        parts.push(if reads == 1 { "read 1 file".to_string() } else { format!("read {reads} files") });
    }
    if edits > 0 {
        parts.push(format!("edited {edits}"));
    }
    if commands > 0 {
        parts.push(if commands == 1 { "ran 1 command".to_string() } else { format!("ran {commands} commands") });
    }
    parts.join(", ")
}

/// Compact elapsed for the fold title: tenths under a minute (`12.4 s`),
/// `2m 14s` past it. Rounds to tenths first, so 59 999 ms is already a
/// minute — the same rule as the tool card's duration.
pub fn turn_fold_elapsed(ms: u64) -> String {
    let tenths = (ms + 50) / 100;
    if tenths >= 600 {
        let s = tenths / 10;
        format!("{}m {:02}s", s / 60, s % 60)
    } else if tenths >= 100 {
        // Past ten seconds a tenth is noise: whole seconds read calmer.
        format!("{} s", (ms + 500) / 1000)
    } else {
        format!("{}.{} s", tenths / 10, tenths % 10)
    }
}

/// `Worked for 2m 14s · read 12 files, edited 3, ran 5 commands`; with no
/// counts the summary (and its separator) is dropped.
pub fn turn_fold_title(elapsed_ms: u64, reads: u32, edits: u32, commands: u32) -> String {
    let summary = turn_fold_summary(reads, edits, commands);
    // An unmeasured turn (no timings on the wire, a replay) says "Worked",
    // never "Worked for 0.0 s".
    let head = if elapsed_ms == 0 { "Worked".to_owned() } else { format!("Worked for {}", turn_fold_elapsed(elapsed_ms)) };
    if summary.is_empty() {
        head
    } else {
        format!("{head} · {summary}")
    }
}

/// `+4 earlier` for the live row's count of earlier calls in the run;
/// `None` when this is the run's first call and there is nothing to count.
pub fn earlier_label(earlier: usize) -> Option<String> {
    if earlier == 0 { None } else { Some(format!("+{earlier} earlier")) }
}

/// What a turn fold asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnFoldIntent {
    /// Expand or collapse the fold.
    Toggle,
    /// Open the turn's diff (the `+N −M` chip): the host shows Changes
    /// scoped to this turn, or to the session when it has no per-turn
    /// checkpoint.
    OpenDiff,
}

type IntentHandler = std::rc::Rc<dyn Fn(TurnFoldIntent, &mut Window, &mut App)>;

/// One row for a settled turn. Build with [`turn_fold`].
#[derive(IntoElement)]
pub struct TurnFold {
    id: ElementId,
    elapsed_ms: u64,
    reads: u32,
    edits: u32,
    commands: u32,
    diff_stat: Option<DiffStat>,
    open: bool,
    children: Vec<AnyElement>,
    on_intent: Option<IntentHandler>,
}

/// A settled turn's work behind one row: `elapsed_ms` of wall-clock time,
/// `reads` files read, `edits` files edited, `commands` shell commands run.
/// Collapsed (the default) the row is the title alone.
pub fn turn_fold(id: impl Into<ElementId>, elapsed_ms: u64, reads: u32, edits: u32, commands: u32) -> TurnFold {
    TurnFold { id: id.into(), elapsed_ms, reads, edits, commands, diff_stat: None, open: false, children: Vec::new(), on_intent: None }
}

impl TurnFold {
    /// The `+N −M` chip on the right, opening the turn's diff. `None` (the
    /// default) draws no chip.
    pub fn diff_stat(mut self, stat: Option<DiffStat>) -> Self {
        self.diff_stat = stat;
        self
    }

    /// Whether the folded work is shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// One folded block (interim prose, a tool card), in turn order.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.children.push(el.into_any_element());
        self
    }

    /// The folded blocks, in turn order. See [`Self::child`].
    pub fn children(mut self, els: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children.extend(els.into_iter().map(|el| el.into_any_element()));
        self
    }

    /// Intent handler: [`TurnFoldIntent::Toggle`] for the header, and
    /// [`TurnFoldIntent::OpenDiff`] for the diff chip.
    pub fn on_intent(mut self, f: impl Fn(TurnFoldIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for TurnFold {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let title = turn_fold_title(self.elapsed_ms, self.reads, self.edits, self.commands);
        let mut row = quiet_row(id.clone(), self.open).part(div().flex_none().child(title));
        if let Some(stat) = &self.diff_stat {
            // The `+N −M` chip, 4 after the title: the only action a quiet
            // row carries (docs/41 §4) — it opens the turn's diff.
            let open = self.on_intent.clone();
            let label = "Open diff for this turn";
            record_ax_label(label);
            row = row.part(
                h_flex()
                    .flex_none()
                    .id((id.clone(), "diff"))
                    .ml(px(-4.0))
                    .gap(px(scale::SP_2))
                    .cursor_pointer()
                    .role(gpui::Role::Button)
                    .aria_label(label)
                    .mono(scale::FS_12)
                    .on_click(move |_, w, cx| {
                        cx.stop_propagation();
                        if let Some(h) = &open {
                            h(TurnFoldIntent::OpenDiff, w, cx);
                        }
                    })
                    .child(div().text_color(p.success).child(format!("+{}", stat.added)))
                    .child(div().text_color(p.danger).child(format!("−{}", stat.removed))),
            );
        }
        if !self.children.is_empty() {
            row = row.body(fold_body(self.children));
        }
        if let Some(on_intent) = self.on_intent {
            row = row.on_toggle(move |_, w, cx| on_intent(TurnFoldIntent::Toggle, w, cx));
        }
        row
    }
}

/// The open fold's content: the turn's cards on the measure, 8 apart, no
/// indent, 8 under the row.
fn fold_body(children: Vec<AnyElement>) -> impl IntoElement {
    let mut list = v_flex().w_full().gap(px(scale::SP_3)).pt(px(scale::SP_3));
    for child in children {
        list = list.child(child);
    }
    list
}

/// One line for an in-progress run of tool calls. Build with
/// [`live_activity_row`].
#[derive(IntoElement)]
pub struct LiveActivityRow {
    id: ElementId,
    verb: SharedString,
    target: SharedString,
    earlier: usize,
    elapsed_ms: u64,
    running: bool,
    open: bool,
    children: Vec<AnyElement>,
    on_toggle: Option<ClickHandler>,
}

/// An in-progress run as one row: `verb`/`target` name the CURRENT call
/// (`Reading`, `crates/baaz/src/app.rs`), `earlier` counts the calls before
/// it in the run, `elapsed_ms` is the run's wall-clock time. Clicking
/// expands to the run's cards, given as children in run order.
pub fn live_activity_row(id: impl Into<ElementId>, verb: impl Into<SharedString>, target: impl Into<SharedString>, earlier: usize, elapsed_ms: u64) -> LiveActivityRow {
    LiveActivityRow { id: id.into(), verb: verb.into(), target: target.into(), earlier, elapsed_ms, running: true, open: false, children: Vec::new(), on_toggle: None }
}

impl LiveActivityRow {
    /// Whether a call in the run is still in flight (the default). A run
    /// whose calls have all finished shows the done glyph, not a spinner.
    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    /// Whether the run's cards are shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// One of the run's cards, in run order.
    pub fn child(mut self, el: impl IntoElement) -> Self {
        self.children.push(el.into_any_element());
        self
    }

    /// The run's cards, in run order. See [`Self::child`].
    pub fn children(mut self, els: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children.extend(els.into_iter().map(|el| el.into_any_element()));
        self
    }

    /// Header click: expands to the run's cards.
    pub fn on_toggle(mut self, f: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
}

impl RenderOnce for LiveActivityRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        // Spinner 12 while a call runs, the done check once the run settles;
        // the verb shimmers (muted → foreground → muted, 1.6 s) only while
        // it runs (docs/41 §2.4).
        let lead = if self.running {
            spinner((id.clone(), "spinner")).size(px(LIVE_GLYPH)).fluid(cx).into_any_element()
        } else {
            status_check(p.success).into_any_element()
        };
        let verb = if self.running {
            shimmer_text((id.clone(), "verb"), self.verb, cx).duration(LIVE_SHIMMER).into_any_element()
        } else {
            div().child(self.verb).into_any_element()
        };
        let mut row = quiet_status_row(id.clone(), lead)
            .part(div().flex_none().child(verb))
            .part(div().min_w(px(0.0)).truncate().mono(scale::FS_12).child(self.target));
        if let Some(earlier) = earlier_label(self.earlier) {
            row = row.part(div().flex_none().text_color(p.ink_4).child(earlier));
        }
        row = row.part(div().flex_none().mono(scale::FS_12).text_color(p.ink_4).child(turn_fold_elapsed(self.elapsed_ms)));
        if !self.children.is_empty() {
            row = row.body(fold_body(self.children));
        }
        if let Some(on_toggle) = self.on_toggle {
            row = row.on_toggle(move |e, w, cx| on_toggle(e, w, cx));
        }
        // A status row has no open state of its own; the run's cards are
        // the body, shown while the host says so.
        row.open_body(self.open)
    }
}

/// The live row's spinner and check: 12 px.
const LIVE_GLYPH: f32 = 12.0;
/// One sweep of the live verb's shimmer.
const LIVE_SHIMMER: std::time::Duration = std::time::Duration::from_millis(1600);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_row_spins_by_default_and_a_finished_run_does_not() {
        assert!(live_activity_row("r", "Running", "ls", 0, 10).running);
        assert!(!live_activity_row("r", "Ran", "ls", 0, 10).running(false).running);
    }

    #[test]
    fn summary_counts_reads_edits_and_commands() {
        assert_eq!(turn_fold_summary(12, 3, 5), "read 12 files, edited 3, ran 5 commands");
    }

    #[test]
    fn summary_singularises_each_count() {
        assert_eq!(turn_fold_summary(1, 0, 0), "read 1 file");
        assert_eq!(turn_fold_summary(0, 1, 0), "edited 1");
        assert_eq!(turn_fold_summary(0, 0, 1), "ran 1 command");
        assert_eq!(turn_fold_summary(1, 2, 1), "read 1 file, edited 2, ran 1 command");
    }

    #[test]
    fn summary_omits_zero_counts() {
        assert_eq!(turn_fold_summary(0, 0, 2), "ran 2 commands");
        assert_eq!(turn_fold_summary(0, 0, 0), "");
    }

    #[test]
    fn title_carries_elapsed_and_summary() {
        assert_eq!(turn_fold_title(134_000, 12, 3, 5), "Worked for 2m 14s · read 12 files, edited 3, ran 5 commands");
    }

    #[test]
    fn title_without_work_is_elapsed_only() {
        assert_eq!(turn_fold_title(12_400, 0, 0, 0), "Worked for 12 s");
    }

    #[test]
    fn an_unmeasured_turn_says_worked_without_a_time() {
        assert_eq!(turn_fold_title(0, 2, 0, 5), "Worked · read 2 files, ran 5 commands");
        assert_eq!(turn_fold_title(0, 0, 0, 0), "Worked");
    }

    #[test]
    fn elapsed_is_compact_past_a_minute() {
        assert_eq!(turn_fold_elapsed(0), "0.0 s");
        assert_eq!(turn_fold_elapsed(4_200), "4.2 s");
        assert_eq!(turn_fold_elapsed(12_400), "12 s");
        assert_eq!(turn_fold_elapsed(59_999), "1m 00s");
        assert_eq!(turn_fold_elapsed(134_000), "2m 14s");
    }

    #[test]
    fn earlier_names_the_count_or_nothing() {
        assert_eq!(earlier_label(0), None);
        assert_eq!(earlier_label(1).as_deref(), Some("+1 earlier"));
        assert_eq!(earlier_label(4).as_deref(), Some("+4 earlier"));
    }

    #[test]
    fn folds_start_collapsed() {
        let fold = turn_fold("t", 134_000, 12, 3, 5);
        assert!(!fold.open);
        let row = live_activity_row("r", "Reading", "crates/baaz/src/app.rs", 4, 14_000);
        assert!(!row.open);
    }

    #[test]
    fn folds_expand_through_the_builder() {
        let fold = turn_fold("t", 134_000, 12, 3, 5).open(true);
        assert!(fold.open);
        let row = live_activity_row("r", "Reading", "crates/baaz/src/app.rs", 4, 14_000).open(true);
        assert!(row.open);
    }

    #[test]
    fn the_diff_chip_is_builder_owned() {
        let fold = turn_fold("t", 134_000, 12, 3, 5).diff_stat(Some(DiffStat { added: 48, removed: 12, files: 3 }));
        assert_eq!(fold.diff_stat, Some(DiffStat { added: 48, removed: 12, files: 3 }));
        assert_eq!(turn_fold("t", 0, 0, 0, 0).diff_stat, None);
    }
}
