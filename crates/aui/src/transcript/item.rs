//! Two transcript cards a provider-driven session cannot do without: the
//! mandated fallback for an item kind this build does not model, and the
//! session goal.
//!
//! Both are deliberately literal. MSP's `ItemKind` is an **open** enum and the
//! protocol requires a client that meets an unknown kind to show the kind, the
//! status and the server's own `fallbackText` rather than guess a richer card;
//! a goal's `status` and `percentComplete` are the provider's own strings and
//! numbers, so the card shows what it was handed — including a percentage over
//! 100 — and only clamps the *bar*, which cannot draw past its end.

use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::{div, prelude::*, px, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, pill, record_ax_label, PillVariant};
use crate::nav::chevron;
use crate::transcript::caps::{cap_expanded, cap_preview, Remainder};
use crate::transcript::transcript_card;

/// The body of both cards keeps the transcript's own card padding.
const BODY_PAD: f32 = scale::SP_4;
/// The header row's gap, matching every other transcript card header.
const HEAD_GAP: f32 = scale::SP_2;
/// The definition rows under a goal: label column, gap, row rhythm.
const DL_LABEL_W: f32 = 84.0;
const DL_COL_GAP: f32 = scale::SP_3;
const DL_ROW_GAP: f32 = scale::SP_2;
/// The progress bar: full width of the body, 4 px tall, its own radius.
const BAR_HEIGHT: f32 = 4.0;
const BAR_RADIUS: f32 = 2.0;
/// The bar and the number it belongs to sit on one row.
const BAR_GAP: f32 = scale::SP_3;
const BAR_NUMBER_W: f32 = 44.0;

/// Collapsed, the card shows a header plus this many preview lines.
pub const GENERIC_PREVIEW_LINES: usize = 3;
/// Expanded, the body shows at most this many lines, then `N more lines`.
/// Matches the Search and MCP body caps in `tool_card`, so no long result
/// is ever taller than ~40 lines inline.
pub const GENERIC_BODY_CAP: usize = 40;

/// What a generic item card asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericItemIntent {
    /// Toggle the body between preview and capped full text.
    Toggle,
    /// Show the whole text elsewhere (the host's pane/doc view): the
    /// `Open full text` control in the expanded fold row.
    OpenFull,
}

type IntentHandler = std::rc::Rc<dyn Fn(GenericItemIntent, &mut Window, &mut App)>;
type OpenFullHandler = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// The fallback card for an unknown item kind. Build with
/// [`generic_item_card`].
#[derive(IntoElement)]
pub struct GenericItemCard {
    id: ElementId,
    kind: SharedString,
    status: SharedString,
    text: SharedString,
    open: bool,
    on_intent: Option<IntentHandler>,
    on_open_full: Option<OpenFullHandler>,
}

/// The kind name, the item's status and the server's `fallbackText`.
///
/// This is the whole card on purpose: a client that invented a richer rendering
/// for a kind it does not model would be guessing at the provider's meaning.
/// Collapsed by default (chevron, header + [`GENERIC_PREVIEW_LINES`] preview
/// lines); expanded the body is capped at [`GENERIC_BODY_CAP`] lines with an
/// `N more lines` row and an `Open full text` control the host wires with
/// [`GenericItemCard::on_open_full`].
pub fn generic_item_card(id: impl Into<ElementId>, kind: impl Into<SharedString>, status: impl Into<SharedString>, text: impl Into<SharedString>) -> GenericItemCard {
    GenericItemCard { id: id.into(), kind: kind.into(), status: status.into(), text: text.into(), open: false, on_intent: None, on_open_full: None }
}

impl GenericItemCard {
    /// Whether the capped body is shown. Collapsed (the default) shows the
    /// header plus [`GENERIC_PREVIEW_LINES`] preview lines.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Intent handler: [`GenericItemIntent::Toggle`] for the header, and
    /// [`GenericItemIntent::OpenFull`] for the fold row's `Open full text`.
    pub fn on_intent(mut self, f: impl Fn(GenericItemIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }

    /// The `Open full text` hook: called with the window and app context when
    /// the expanded fold row's control is pressed, so the host can show the
    /// whole text in its own pane. Takes precedence over [`Self::on_intent`]
    /// for [`GenericItemIntent::OpenFull`]; the header toggle always reports
    /// [`GenericItemIntent::Toggle`] through `on_intent`.
    pub fn on_open_full(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_open_full = Some(std::rc::Rc::new(f));
        self
    }
}

/// The collapsed preview: at most [`GENERIC_PREVIEW_LINES`] lines and 360
/// characters in total, each long line cut at 160 characters with an
/// ellipsis. Empty text previews to nothing.
pub fn generic_preview_text(text: &str) -> String {
    cap_preview(text)
}

/// The preview as lines, for callers that lay out one row per line.
pub fn generic_preview_lines(text: &str) -> Vec<String> {
    cap_preview(text).lines().map(str::to_string).collect()
}

/// The expanded body: the first [`GENERIC_BODY_CAP`] lines and 6,000
/// characters of `text`. Never reformatted, only cut.
pub fn generic_expanded_text(text: &str) -> (String, Remainder) {
    cap_expanded(text)
}

/// The expanded body as lines, for callers that lay out one row per line.
pub fn generic_visible_lines(text: &str) -> Vec<String> {
    cap_expanded(text).0.lines().map(str::to_string).collect()
}

/// What the expanded body cut away, named by the expanded fold row as
/// `N more lines` or `M more characters`.
pub fn generic_remainder(text: &str) -> Remainder {
    cap_expanded(text).1
}

impl RenderOnce for GenericItemCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_intent.clone();
        // The frame stays expanded — collapsed means preview lines, not an
        // empty card — so the chevron is drawn by hand from the open state,
        // the `tool_group` idiom.
        let mut card = transcript_card(id.clone(), true)
            .chevron(false)
            .hover_tint(self.on_intent.is_some())
            .header(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(HEAD_GAP))
                    .child(div().flex_none().ui(scale::FS_13).semibold().text_color(p.ink).child(self.kind))
                    .child(div().flex_1().min_w(px(0.0)))
                    .child(pill(self.status).variant(PillVariant::Quiet)),
            )
            .header(chevron((id.clone(), "chevron"), self.open, p.ink_3, window, cx));
        // An item with no fallback text is a header and nothing else; an empty
        // body would draw a stray border under it.
        if !self.text.is_empty() {
            if self.open {
                let (visible, remainder) = generic_expanded_text(&self.text);
                let mut body = v_flex().w_full().p(px(BODY_PAD)).ui(scale::FS_12).text_color(p.ink_2).child(visible);
                if let Some(cut) = remainder.label() {
                    let open = handler.clone();
                    let open_full = self.on_open_full.clone();
                    let label = "Open full text";
                    record_ax_label(label);
                    body = body.child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap(px(HEAD_GAP))
                            .pt(px(scale::SP_2))
                            .child(div().flex_none().ui(scale::FS_11).text_color(p.ink_3).child(cut))
                            .child(div().flex_1())
                            .child(
                                button((id.clone(), "open-full"), label)
                                    .xs()
                                    .ghost()
                                    .accessibility_label(label)
                                    .on_click(move |_, w, cx| {
                                        if let Some(f) = &open_full {
                                            f(w, cx);
                                        } else if let Some(h) = &open {
                                            h(GenericItemIntent::OpenFull, w, cx);
                                        }
                                    }),
                            ),
                    );
                }
                card = card.body(body);
            } else {
                card = card.body(div().w_full().p(px(BODY_PAD)).ui(scale::FS_12).text_color(p.ink_2).child(generic_preview_text(&self.text)));
            }
        }
        if let Some(handler) = self.on_intent {
            card = card.on_toggle(move |_, w, cx| handler(GenericItemIntent::Toggle, w, cx));
        }
        card
    }
}

/// The session goal. Build with [`goal_card`].
#[derive(IntoElement)]
pub struct GoalCard {
    id: ElementId,
    objective: SharedString,
    status: SharedString,
    percent: Option<f32>,
    current_work: Option<SharedString>,
    next_work: Option<SharedString>,
}

/// The objective and the provider's own status string.
///
/// Add the progress with [`GoalCard::percent`] and the two work lines with
/// [`GoalCard::current_work`] / [`GoalCard::next_work`].
pub fn goal_card(id: impl Into<ElementId>, objective: impl Into<SharedString>, status: impl Into<SharedString>) -> GoalCard {
    GoalCard { id: id.into(), objective: objective.into(), status: status.into(), percent: None, current_work: None, next_work: None }
}

impl GoalCard {
    /// How far along the provider says it is, **verbatim**.
    ///
    /// The number is printed as it arrived: a provider that reports 120 % has
    /// said something about itself worth seeing. Only the bar is clamped, since
    /// it has nowhere further to fill.
    pub fn percent(mut self, percent: Option<f32>) -> Self {
        self.percent = percent;
        self
    }

    /// What the agent says it is doing now.
    pub fn current_work(mut self, work: impl Into<SharedString>) -> Self {
        self.current_work = Some(work.into());
        self
    }

    /// What it says it will do next.
    pub fn next_work(mut self, work: impl Into<SharedString>) -> Self {
        self.next_work = Some(work.into());
        self
    }
}

impl RenderOnce for GoalCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let header = h_flex()
            .w_full()
            .items_center()
            .gap(px(HEAD_GAP))
            .child(div().flex_1().min_w(px(0.0)).truncate().ui(scale::FS_13).semibold().text_color(p.ink).child(self.objective))
            .child(pill(self.status).variant(PillVariant::Line));

        let mut body = v_flex().w_full().p(px(BODY_PAD)).gap(px(DL_ROW_GAP));
        if let Some(percent) = self.percent {
            let fraction = (percent / 100.0).clamp(0.0, 1.0);
            body = body.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(BAR_GAP))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .h(px(BAR_HEIGHT))
                            .rounded(px(BAR_RADIUS))
                            .bg(p.surface_3)
                            .overflow_hidden()
                            .child(div().h_full().w(gpui::relative(fraction)).rounded(px(BAR_RADIUS)).bg(p.accent)),
                    )
                    .child(div().flex_none().w(px(BAR_NUMBER_W)).flex().justify_end().mono(scale::FS_11).text_color(p.ink_3).child(format!("{percent:.0}%"))),
            );
        }
        for (label, value) in [("Doing now", self.current_work), ("Next", self.next_work)] {
            let Some(value) = value else { continue };
            body = body.child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(DL_COL_GAP))
                    .ui(scale::FS_12)
                    .child(div().flex_none().w(px(DL_LABEL_W)).text_color(p.ink_3).child(label))
                    .child(div().flex_1().min_w(px(0.0)).text_color(p.ink_2).child(value)),
            );
        }

        transcript_card(id, true).chevron(false).hover_tint(false).header(header).body(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long_text(lines: usize) -> String {
        (1..=lines).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn collapsed_by_default() {
        let card = generic_item_card("g", "Artifact", "completed", "some text");
        assert!(!card.open, "the noisy fallback must start collapsed");
    }

    #[test]
    fn preview_shows_at_most_three_lines() {
        assert_eq!(generic_preview_lines(""), Vec::<String>::new());
        assert_eq!(generic_preview_lines("one"), vec!["one".to_string()]);
        assert_eq!(
            generic_preview_lines("a\nb\nc"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            generic_preview_lines(&long_text(121)),
            vec!["line 1".to_string(), "line 2".to_string(), "line 3".to_string()]
        );
    }

    #[test]
    fn preview_binds_characters_not_just_lines() {
        // Three 1,000-char lines: the total stays within the preview cap.
        let three = (0..3).map(|_| "q".repeat(1_000)).collect::<Vec<_>>().join("\n");
        let preview = generic_preview_text(&three);
        assert!(
            preview.chars().count() <= 360,
            "three huge preview lines total {}, over the 360 cap",
            preview.chars().count()
        );
        // A single long line is cut with an ellipsis, not shown whole.
        let single = "q".repeat(1_000);
        let lines = generic_preview_lines(&single);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with('…'), "the cut line must mark its cut");
        assert!(lines[0].chars().count() <= 161);
    }

    #[test]
    fn expanded_body_caps_at_forty_lines() {
        let text = long_text(121);
        assert_eq!(generic_visible_lines(&text).len(), GENERIC_BODY_CAP);
        assert_eq!(generic_visible_lines(&text)[0], "line 1");
        assert_eq!(generic_remainder(&text).lines, 121 - GENERIC_BODY_CAP);
        assert_eq!(generic_remainder(&text).chars, 0);
    }

    #[test]
    fn expanded_single_line_body_is_cut_and_reports_the_rest() {
        let text = "x".repeat(26_000);
        let (visible, remainder) = generic_expanded_text(&text);
        assert!(
            visible.chars().count() <= 6_000,
            "26k single-line body shows {}, over the 6,000 cap",
            visible.chars().count()
        );
        assert_eq!(visible, format!("{}…", "x".repeat(160)));
        assert_eq!(remainder.lines, 0);
        assert_eq!(remainder.chars, 26_000 - 161);
        assert_eq!(remainder.label().as_deref(), Some("25,839 more characters"));
    }

    #[test]
    fn short_text_hides_nothing() {
        let text = long_text(40);
        assert_eq!(generic_visible_lines(&text).len(), 40);
        assert!(generic_remainder(&text).is_empty());
        assert!(generic_remainder("").is_empty());
    }

    #[test]
    fn expand_state_is_builder_owned() {
        let card = generic_item_card("g", "Artifact", "completed", "text").open(true);
        assert!(card.open);
    }

    /// Painted body bounds, recorded in prepaint.
    struct BodyHost {
        text: String,
        seen: std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>,
    }

    impl gpui::Render for BodyHost {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let seen = self.seen.clone();
            gpui::div().child(
                gpui::div()
                    .on_children_prepainted(move |bounds, _, _| {
                        *seen.borrow_mut() = bounds;
                    })
                    .child(
                        super::generic_item_card(
                            "render-26k",
                            "Artifact",
                            "completed",
                            self.text.clone(),
                        )
                        .open(true),
                    ),
            )
        }
    }

    /// A 26k-char single-line body paints bounded: the char cap holds no
    /// matter how few newlines the text holds. Full text would wrap past
    /// ~3,500 px in an 800 px window; the capped body stays far below.
    #[gpui::test]
    fn expanded_26k_body_paints_bounded_height(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| crate::init(crate::tokens::ThemeKind::Dark, cx));
        let seen =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let text = "x".repeat(26_000);
        let _handle = cx.open_window(gpui::size(gpui::px(800.0), gpui::px(600.0)), |_, _| BodyHost {
            text,
            seen: seen.clone(),
        });
        cx.run_until_parked();
        let painted = seen.borrow();
        assert!(!painted.is_empty(), "the body must prepaint inside the window");
        let height: f32 =
            painted.iter().map(|b| f32::from(b.size.height)).fold(0.0, f32::max);
        assert!(
            height < 2500.0,
            "26k single-line body painted {height}px tall, over the bound"
        );
    }
}
