//! Card 34: tool call cards — one header for every tool (status glyph, verb,
//! mono target, right-aligned result and duration, chevron) and a body per
//! tool: shell output on the terminal ground, unified diff, search hits, web
//! results, browser screenshot with the click ring, sub-agent transcript and
//! generic MCP parameters.

use aui_motion::{looping, Loop};
use aui_protocol::{DiffKind, ToolBody, ToolStatus};
use aui_tokens::{scale, ActiveAui, AuiStyled, Easing, Palette, TextRole};
use gpui::{div, linear_color_stop, linear_gradient, prelude::*, px, relative, App, ElementId, IntoElement, SharedString, StyledText, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{glyph_err, glyph_ok, pill, spinner, tag, PillVariant};
use crate::icons::{icon, IconName};
use crate::transcript::ansi::{ansi_runs, ansi_spans};
use crate::transcript::transcript_card;

/// `.out{padding:10px 12px;font:11.5px/1.65 mono}`.
const OUT_PAD_Y: f32 = 10.0;
const OUT_PAD_X: f32 = 12.0;
const OUT_TEXT: f32 = 11.5;
const OUT_LH: f32 = 1.65;
/// Shell output folds after this many lines (the card shows six, then `14 more lines`).
pub const SHELL_FOLD: usize = 6;
/// `.more{gap:6px;padding:4px 10px 6px;font-size:11px;margin-top:-14px}` with an 11 px chevron and a 40 % fade.
const MORE_GAP: f32 = 6.0;
const MORE_PAD_TOP: f32 = 4.0;
const MORE_PAD_X: f32 = 10.0;
const MORE_PAD_BOTTOM: f32 = 6.0;
const MORE_OVERLAP: f32 = -14.0;
const MORE_GLYPH: f32 = 11.0;
const MORE_FADE_STOP: f32 = 0.4;
/// `.diff{font:11.5px/1.6 mono} .ln{padding:0 8px} .gutter{width:26px;padding-right:8px}`.
const DIFF_LH: f32 = 1.6;
const DIFF_PAD_X: f32 = 8.0;
const GUTTER_W: f32 = 26.0;
const GUTTER_PAD: f32 = 8.0;
/// `.hits{padding:6px 10px;font:11.5px/1.7 mono}`.
const HITS_PAD_Y: f32 = 6.0;
const HITS_PAD_X: f32 = 10.0;
const HITS_LH: f32 = 1.7;
/// `.web{padding:6px 10px;gap:6px}` rows 12 px; `.fav` 14 px radius 3; `.dom` 11 px.
const WEB_PAD_Y: f32 = 6.0;
const WEB_PAD_X: f32 = 10.0;
const WEB_GAP: f32 = 6.0;
const FAVICON: f32 = 14.0;
const FAVICON_RADIUS: f32 = 3.0;
/// `.shot{margin:8px 10px;height:70px}` with the 18 px click ring at 60 % / 40 % and the label at 10 / 8.
const SHOT_MARGIN_Y: f32 = 8.0;
const SHOT_MARGIN_X: f32 = 10.0;
const SHOT_H: f32 = 70.0;
const RING: f32 = 18.0;
const RING_STROKE: f32 = 2.0;
const RING_X: f32 = 0.6;
const RING_Y: f32 = 0.4;
const RING_PERIOD: std::time::Duration = std::time::Duration::from_millis(1600);
const SHOT_LABEL_LEFT: f32 = 10.0;
const SHOT_LABEL_BOTTOM: f32 = 8.0;
const SHOT_LABEL_PAD_Y: f32 = 2.0;
const SHOT_LABEL_PAD_X: f32 = 6.0;
/// Result pills: `height:16px;padding:0 6px`.
const RESULT_PILL_H: f32 = 16.0;
/// `.mcp` definition rows reuse the hits padding; the JSON is shown as mono text.
const SUB_PAD: f32 = 10.0;

/// What the card's fold rows and header ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCardIntent {
    /// Toggle the body.
    Toggle,
    /// Show the whole output / all hunks.
    Unfold,
    /// Open the output in the terminal pane / the diff in the review pane.
    OpenInPane,
}

type IntentHandler = std::rc::Rc<dyn Fn(ToolCardIntent, &mut Window, &mut App)>;

/// A click handler bound to one [`ToolCardIntent`], ready for `on_click`.
type IntentClick = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>;

/// A tool call card. Build with [`tool_card`].
#[derive(IntoElement)]
pub struct ToolCard {
    id: ElementId,
    verb: SharedString,
    target: SharedString,
    status: ToolStatus,
    duration_ms: Option<u64>,
    body: ToolBody,
    open: bool,
    on_intent: Option<IntentHandler>,
}

/// A card for one tool call.
pub fn tool_card(id: impl Into<ElementId>, verb: impl Into<SharedString>, target: impl Into<SharedString>, status: ToolStatus, body: ToolBody) -> ToolCard {
    ToolCard { id: id.into(), verb: verb.into(), target: target.into(), status, duration_ms: None, body, open: true, on_intent: None }
}

impl ToolCard {
    /// The duration shown in the header.
    pub fn duration_ms(mut self, ms: Option<u64>) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Whether the body is shown.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Intent handler.
    pub fn on_intent(mut self, f: impl Fn(ToolCardIntent, &mut Window, &mut App) + 'static) -> Self {
        self.on_intent = Some(std::rc::Rc::new(f));
        self
    }
}

/// `12.4 s`, `1 m 12 s`, `0.3 s`.
///
/// Rounded to tenths *before* the branch: 59 999 ms reads `1 m 00 s`, not the
/// `60.0 s` a raw comparison would print.
pub fn format_duration(ms: u64) -> String {
    let tenths = (ms + 50) / 100;
    if tenths >= 600 {
        let s = tenths / 10;
        format!("{} m {:02} s", s / 60, s % 60)
    } else {
        format!("{}.{} s", tenths / 10, tenths % 10)
    }
}

impl RenderOnce for ToolCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handler = self.on_intent.clone();
        let emit = move |intent: ToolCardIntent| -> IntentClick {
            let handler = handler.clone();
            Box::new(move |_: &gpui::ClickEvent, w: &mut Window, cx: &mut App| {
                if let Some(h) = &handler {
                    h(intent, w, cx)
                }
            })
        };

        let glyph: gpui::AnyElement = match self.status {
            ToolStatus::Pending | ToolStatus::Running => spinner((id.clone(), "spinner")).into_any_element(),
            ToolStatus::Success => glyph_ok().into_any_element(),
            ToolStatus::Error | ToolStatus::Cancelled => glyph_err().into_any_element(),
        };
        let mut card = transcript_card(id.clone(), self.open)
            .header(glyph)
            .header(div().medium().whitespace_nowrap().child(self.verb.clone()))
            .header(div().min_w(px(0.0)).truncate().mono(scale::FS_12).text_color(p.ink_2).child(self.target.clone()))
            .header(div().flex_1());

        // The right group: a result pill or count, then the duration.
        let mut right = h_flex().flex_none().gap(px(scale::SP_3)).text_role(TextRole::MonoSmall).font_weight(gpui::FontWeight::MEDIUM).text_color(p.ink_3);
        let mut show_duration = true;
        match &self.body {
            ToolBody::Shell { exit_code, live, .. } => {
                let (label, variant): (SharedString, PillVariant) = match (live, exit_code) {
                    (true, _) => ("live".into(), PillVariant::Line),
                    (false, Some(0)) => ("exit 0".into(), PillVariant::Success),
                    (false, Some(code)) => (format!("exit {code}").into(), PillVariant::Danger),
                    (false, None) => ("".into(), PillVariant::Quiet),
                };
                if !label.is_empty() {
                    right = right.child(pill(label).variant(variant).height(RESULT_PILL_H));
                }
            }
            ToolBody::Read { lines } => {
                right = right.child(format!("{lines} lines"));
                show_duration = false;
            }
            ToolBody::Edit { diff } => {
                right = right.child(tag(format!("+{}", diff.added)).color(p.success)).child(tag(format!("−{}", diff.removed)).color(p.danger));
                show_duration = false;
            }
            ToolBody::Search { hits } => {
                right = right.child(format!("{} hits", hits.len()));
                show_duration = false;
            }
            ToolBody::Web { results, hidden } => {
                right = right.child(format!("{} results", results.len() + hidden));
                show_duration = false;
            }
            ToolBody::Browser { action, .. } => {
                right = right.child(action.clone());
                show_duration = false;
            }
            ToolBody::SubAgent { .. } | ToolBody::Mcp { .. } | ToolBody::None => {}
        }
        if show_duration {
            if let Some(ms) = self.duration_ms {
                right = right.child(format_duration(ms));
            }
        }
        card = card.header(right);

        let body: Option<gpui::AnyElement> = match &self.body {
            ToolBody::Shell { output_lines, .. } => Some(shell_body(&p, &id, output_lines, emit.clone()).into_any_element()),
            ToolBody::Read { .. } | ToolBody::None => None,
            ToolBody::Edit { diff } => Some(diff_body(&p, diff, emit.clone()).into_any_element()),
            ToolBody::Search { hits } => Some(search_body(&p, hits).into_any_element()),
            ToolBody::Web { results, hidden } => Some(web_body(&p, results, *hidden).into_any_element()),
            ToolBody::Browser { caption, .. } => {
                // The click ring pulses only while the call is still in
                // flight; a finished capture is a settled placeholder and
                // must not hold the frame loop open (see `browser_body`).
                let live = matches!(self.status, ToolStatus::Pending | ToolStatus::Running);
                Some(browser_body(&p, &id, caption.as_deref(), live, window, cx).into_any_element())
            }
            ToolBody::SubAgent { turns } => Some(
                div()
                    .p(px(SUB_PAD))
                    .ui(scale::FS_12)
                    .text_color(p.ink_3)
                    .child(format!("{} turns in the delegated transcript", turns.len()))
                    .into_any_element(),
            ),
            ToolBody::Mcp { params, result_json } => Some(mcp_body(&p, params, result_json).into_any_element()),
        };
        if let Some(body) = body {
            card = card.body(body);
        } else {
            card = card.chevron(true);
        }
        let toggle = emit(ToolCardIntent::Toggle);
        card.on_toggle(move |e, w, cx| toggle(e, w, cx))
    }
}

/// `.more`: the fold row under a body.
fn fold_row(p: &Palette, ground: gpui::Hsla, overlap: bool, label: String, open_label: &'static str, emit: impl Fn(ToolCardIntent) -> Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>) -> impl IntoElement {
    let mut row = h_flex()
        .relative()
        .w_full()
        .gap(px(MORE_GAP))
        .pt(px(MORE_PAD_TOP))
        .px(px(MORE_PAD_X))
        .pb(px(MORE_PAD_BOTTOM))
        .ui(scale::FS_11)
        .text_color(p.ink_3);
    if overlap {
        row = row.mt(px(MORE_OVERLAP)).bg(linear_gradient(180.0, linear_color_stop(ground.alpha(0.0), 0.0), linear_color_stop(ground, MORE_FADE_STOP)));
    } else {
        row = row.bg(ground);
    }
    let unfold = emit(ToolCardIntent::Unfold);
    let open = emit(ToolCardIntent::OpenInPane);
    row.child(h_flex().id("unfold").gap(px(MORE_GAP)).cursor_pointer().on_click(move |e, w, cx| unfold(e, w, cx)).child(icon(IconName::ChevronDown).size(px(MORE_GLYPH))).child(label))
        .child(div().flex_1())
        .child(div().id("open").cursor_pointer().on_click(move |e, w, cx| open(e, w, cx)).child(open_label))
}

fn shell_body(p: &Palette, _id: &ElementId, lines: &[String], emit: impl Fn(ToolCardIntent) -> Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)> + Clone + 'static) -> impl IntoElement {
    let shown = lines.iter().take(SHELL_FOLD);
    let mut out = v_flex().w_full().py(px(OUT_PAD_Y)).px(px(OUT_PAD_X)).mono(OUT_TEXT).line_height(relative(OUT_LH)).text_color(p.term_fg).whitespace_nowrap();
    for line in shown {
        let (text, runs) = ansi_runs(&ansi_spans(line), p, scale::FONT_MONO);
        // An empty line still takes a line box.
        let text = if text.is_empty() { " ".to_string() } else { text };
        let runs = if runs.is_empty() || runs.iter().map(|r| r.len).sum::<usize>() != text.len() {
            vec![gpui::TextRun { len: text.len(), font: gpui::font(scale::FONT_MONO), color: p.term_fg, background_color: None, underline: None, strikethrough: None }]
        } else {
            runs
        };
        out = out.child(div().w_full().overflow_hidden().child(StyledText::new(text).with_runs(runs)));
    }
    let mut body = v_flex().w_full().bg(p.term_bg).child(out);
    if lines.len() > SHELL_FOLD {
        body = body.child(fold_row(p, p.term_bg, true, format!("{} more lines", lines.len() - SHELL_FOLD), "open in terminal", emit));
    }
    body
}

fn diff_body(p: &Palette, diff: &aui_protocol::Diff, emit: impl Fn(ToolCardIntent) -> Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)> + Clone + 'static) -> impl IntoElement {
    let mut body = v_flex().w_full().bg(p.surface_1).mono(OUT_TEXT).line_height(relative(DIFF_LH)).text_color(p.ink);
    if let Some(hunk) = diff.hunks.first() {
        for line in &hunk.lines {
            let (bg, gutter_ink, no) = match line.kind {
                DiffKind::Add => (Some(p.diff_add), p.success, line.new_no),
                DiffKind::Del => (Some(p.diff_del), p.danger, line.old_no),
                DiffKind::Context => (None, p.ink_4, line.new_no.or(line.old_no)),
            };
            let mut row = h_flex().w_full().items_start().px(px(DIFF_PAD_X));
            if let Some(bg) = bg {
                row = row.bg(bg);
            }
            body = body.child(
                row.child(div().flex_none().w(px(GUTTER_W)).pr(px(GUTTER_PAD)).flex().justify_end().text_color(gutter_ink).child(no.map(|n| n.to_string()).unwrap_or_default()))
                    .child(div().flex_1().min_w(px(0.0)).child(line.text.clone())),
            );
        }
    }
    let more_hunks = diff.hunks.len().saturating_sub(1);
    if more_hunks > 0 {
        body = body.child(fold_row(p, p.surface_1, false, format!("{more_hunks} more hunks"), "open in Diff", emit));
    }
    body
}

fn search_body(p: &Palette, hits: &[aui_protocol::SearchHit]) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(HITS_PAD_Y)).px(px(HITS_PAD_X)).mono(OUT_TEXT).line_height(relative(HITS_LH)).text_color(p.ink);
    for hit in hits {
        // `path:line  snippet` as one wrapping line: path ink-3, match accent-ink 500.
        let head = format!("{}:{}  ", hit.path, hit.line);
        let text = format!("{head}{}", hit.snippet);
        let mut strong = gpui::font(scale::FONT_MONO);
        strong.weight = gpui::FontWeight::MEDIUM;
        let runs = vec![
            gpui::TextRun { len: head.len(), font: gpui::font(scale::FONT_MONO), color: p.ink_3, background_color: None, underline: None, strikethrough: None },
            gpui::TextRun { len: hit.snippet.len(), font: strong, color: p.accent_ink, background_color: None, underline: None, strikethrough: None },
        ];
        body = body.child(div().w_full().child(StyledText::new(text).with_runs(runs)));
    }
    body
}

fn web_body(p: &Palette, results: &[aui_protocol::WebResult], hidden: usize) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(WEB_PAD_Y)).px(px(WEB_PAD_X)).gap(px(WEB_GAP)).ui(scale::FS_12).text_color(p.ink);
    for r in results {
        body = body.child(
            h_flex()
                .w_full()
                .gap(px(scale::SP_3))
                .child(div().flex_none().size(px(FAVICON)).rounded(px(FAVICON_RADIUS)).bg(p.surface_3))
                .child(div().min_w(px(0.0)).truncate().child(r.title.clone()))
                .child(div().ui(scale::FS_11).text_color(p.ink_3).child(r.domain.clone())),
        );
    }
    if hidden > 0 {
        body = body.child(div().ui(scale::FS_11).text_color(p.ink_3).child(format!("+{hidden} more")));
    }
    body
}

/// The phase the click ring rests at once the capture is done, and the one
/// reduced motion holds it at: the ring fully expanded and nearly faded out.
const RING_REST: f32 = 1.0;

fn browser_body(p: &Palette, id: &ElementId, caption: Option<&str>, live: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // `.shot i` pulses on the ease-out curve every 1.6 s — but only while the
    // call is live. `looping` requests a frame on every render for as long as
    // it is mounted, so a settled browser card left pulsing would keep the
    // window redrawing for as long as it is on screen; a finished capture
    // holds the resting phase instead, exactly as reduced motion does.
    let phase = if live {
        looping((id.clone(), "click-ring"), Loop::eased(RING_PERIOD, Easing::OUT).resting(RING_REST), window, cx)
    } else {
        let _ = (window, cx);
        RING_REST
    };
    let ring = RING * (0.6 + 0.9 * phase);
    div().w_full().bg(p.surface_1).child(
        div()
            .relative()
            .my(px(SHOT_MARGIN_Y))
            .mx(px(SHOT_MARGIN_X))
            .h(px(SHOT_H))
            .rounded(px(scale::R_SM))
            .border_1()
            .border_color(p.line)
            .bg(linear_gradient(135.0, linear_color_stop(p.surface_2, 0.0), linear_color_stop(p.surface_3, 1.0)))
            .child(
                div()
                    .absolute()
                    .left(relative(RING_X))
                    .top(relative(RING_Y))
                    .ml(px(-(ring - RING) / 2.0))
                    .mt(px(-(ring - RING) / 2.0))
                    .size(px(ring))
                    .rounded_full()
                    .border(px(RING_STROKE))
                    .border_color(p.accent)
                    .opacity(1.0 - 0.7 * phase),
            )
            .children(caption.map(|caption| {
                div()
                    .absolute()
                    .left(px(SHOT_LABEL_LEFT))
                    .bottom(px(SHOT_LABEL_BOTTOM))
                    .py(px(SHOT_LABEL_PAD_Y))
                    .px(px(SHOT_LABEL_PAD_X))
                    .rounded(px(scale::R_XS))
                    .border_1()
                    .border_color(p.line)
                    .bg(p.surface_1)
                    .mono(scale::FS_11)
                    .line_height(relative(scale::LH_UI))
                    .text_color(p.ink_2)
                    .child(caption.to_string())
            })),
    )
}

fn mcp_body(p: &Palette, params: &[(String, String)], result_json: &str) -> impl IntoElement {
    let mut body = v_flex().w_full().py(px(HITS_PAD_Y)).px(px(HITS_PAD_X)).mono(OUT_TEXT).line_height(relative(HITS_LH)).text_color(p.ink);
    for (k, v) in params {
        body = body.child(h_flex().gap(px(scale::SP_3)).child(div().text_color(p.ink_3).child(k.clone())).child(div().child(v.clone())));
    }
    body.child(div().text_color(p.ink_2).child(result_json.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_under_a_minute_print_tenths_of_a_second() {
        assert_eq!(format_duration(0), "0.0 s");
        assert_eq!(format_duration(300), "0.3 s");
        assert_eq!(format_duration(999), "1.0 s");
        assert_eq!(format_duration(1_000), "1.0 s");
        assert_eq!(format_duration(12_400), "12.4 s");
        assert_eq!(format_duration(59_000), "59.0 s");
    }

    #[test]
    fn a_minute_and_over_prints_minutes_and_padded_seconds() {
        // Rounding decides the branch, so 59 999 ms is already a minute.
        assert_eq!(format_duration(59_999), "1 m 00 s");
        assert_eq!(format_duration(60_000), "1 m 00 s");
        assert_eq!(format_duration(61_000), "1 m 01 s");
        assert_eq!(format_duration(72_000), "1 m 12 s");
        assert_eq!(format_duration(3_600_000), "60 m 00 s");
    }

    #[test]
    fn the_seconds_branch_never_prints_sixty_seconds() {
        for ms in 59_900..=60_100 {
            let out = format_duration(ms);
            assert!(!out.starts_with("60.") && !out.starts_with("60 s"), "{ms} ms printed `{out}`");
        }
    }
}
