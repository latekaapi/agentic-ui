//! D6's full-output and image views (docs/43 §2.9–2.10; canvas v49
//! `D6Output`, `D6Image`, `D6StatesViews`). Both are pushed views: the
//! transcript opens them on the screen the pane is showing.

use std::sync::Arc;

use aui_icons::IconName;
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{
    div, font, img, prelude::*, px, uniform_list, App, ElementId, FontWeight, Hsla, ImageSource, IntoElement, ListHorizontalSizingBehavior,
    SharedString, StyledText, TextRun, UniformListScrollHandle, Window,
};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::spinner;
use crate::transcript::{parse_ansi, AnsiSpan};

const OUT_TEXT: f32 = 12.65;
const OUT_LH: f32 = 20.0;
/// The cap the footer names.
pub const OUTPUT_LINE_CAP: usize = 10_000;

/// A colour for an ANSI span on the pane (docs/43 §2.9): green → success,
/// red → danger, yellow → warning, dim → faint; other hues keep the
/// palette's ANSI table; plain text is the foreground.
fn status_color(span: &AnsiSpan, p: &Palette) -> Hsla {
    match span.color {
        Some(1) | Some(9) => p.danger,
        Some(2) | Some(10) => p.success,
        Some(3) | Some(11) => p.warning,
        Some(i) => p.ansi16()[i as usize % 16],
        None if span.dim => p.ink_4,
        None => p.fg,
    }
}

fn output_runs(line: &str, p: &Palette) -> (String, Vec<TextRun>) {
    let spans = parse_ansi(line);
    let mono = font(scale::FONT_MONO);
    let mut bold = mono.clone();
    bold.weight = FontWeight::SEMIBOLD;
    let mut text = String::new();
    let mut runs = Vec::new();
    for s in &spans {
        let t = s.text.replace('\t', "    ");
        runs.push(TextRun { len: t.len(), font: if s.bold { bold.clone() } else { mono.clone() }, color: status_color(s, p), background_color: None, underline: None, strikethrough: None });
        text.push_str(&t);
    }
    if text.is_empty() {
        text.push(' ');
        runs = vec![TextRun { len: 1, font: mono, color: p.fg, background_color: None, underline: None, strikethrough: None }];
    }
    (text, runs)
}

/// The output view's row title: the command in mono 12.65 (ellipsis), then
/// either `exit N` in mono 12.1 (danger when non-zero, faint at 0) and the
/// time faint, or — still running — a 12 px spinner and `running · 12 s`.
pub fn output_title(command: &str, exit: Option<i32>, time: Option<SharedString>, running: Option<u64>, cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    let mut row = h_flex()
        .min_w(px(0.0))
        .gap(px(8.0))
        .items_center()
        .whitespace_nowrap()
        .child(div().min_w(px(0.0)).overflow_hidden().text_ellipsis().font_family(scale::FONT_MONO).text_px(OUT_TEXT).font_weight(FontWeight::NORMAL).text_color(p.fg).child(format!("$ {command}")));
    if let Some(secs) = running {
        row = row.child(
            h_flex()
                .flex_none()
                .gap(px(6.0))
                .items_center()
                .font_family(scale::FONT_MONO)
                .text_px(12.1)
                .text_color(p.ink_4)
                .child(spinner("output-running").size(px(12.0)).muted(cx))
                .child(format!("running · {secs} s")),
        );
    } else {
        if let Some(code) = exit {
            row = row.child(div().flex_none().font_family(scale::FONT_MONO).text_px(12.1).text_color(if code != 0 { p.danger } else { p.ink_4 }).child(format!("exit {code}")));
        }
        if let Some(t) = time {
            row = row.child(div().flex_none().font_family(scale::FONT_MONO).text_px(12.1).text_color(p.ink_4).child(t));
        }
    }
    row
}

/// The full output on the Well (docs/43 §2.9): mono 12.65 / 20, padding
/// 12 / 16 / 40, no wrap, a virtual list. `capped` adds the 28 px footer
/// "N lines · capped at 10 000". While `follow` is set the view keeps the
/// newest line in view until the reader scrolls up.
#[derive(IntoElement)]
pub struct OutputView {
    id: ElementId,
    lines: Arc<Vec<String>>,
    capped: Option<usize>,
    follow: bool,
    scroll: Option<UniformListScrollHandle>,
}

/// An output view over `lines`.
pub fn output_view(id: impl Into<ElementId>, lines: Arc<Vec<String>>) -> OutputView {
    OutputView { id: id.into(), lines, capped: None, follow: false, scroll: None }
}

impl OutputView {
    /// The output was capped at [`OUTPUT_LINE_CAP`]; `total` is how many
    /// lines it had.
    pub fn capped(mut self, total: Option<usize>) -> Self {
        self.capped = total;
        self
    }

    /// Follow the tail (the command is still running).
    pub fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    /// The host's scroll handle.
    pub fn track_scroll(mut self, handle: &UniformListScrollHandle) -> Self {
        self.scroll = Some(handle.clone());
        self
    }
}

#[derive(Clone, Copy, Default)]
struct Tail {
    seen: usize,
}

impl RenderOnce for OutputView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let id = self.id.clone();
        let handle = match self.scroll {
            Some(h) => h,
            None => window.use_keyed_state((id.clone(), "scroll"), cx, |_, _| UniformListScrollHandle::default()).read(cx).clone(),
        };
        let total = self.lines.len();
        if self.follow && total > 0 {
            let tail = window.use_keyed_state((id.clone(), "tail"), cx, |_, _| Tail::default());
            let seen = tail.read(cx).seen;
            // New lines arrived: follow them only if the reader was at the
            // end (scrolling up stops the follow until they come back down).
            if total != seen {
                if seen == 0 || handle.is_scrolled_to_end().unwrap_or(true) {
                    handle.scroll_to_bottom();
                }
                tail.update(cx, |t, _| t.seen = total);
            }
        }
        let widest = self.lines.iter().enumerate().max_by_key(|(_, l)| l.len()).map(|(i, _)| i);
        let lines = self.lines.clone();
        let list = uniform_list((id.clone(), "lines"), total, move |range, _window, cx| {
            let p = cx.aui().colors;
            range
                .map(|i| {
                    let (text, runs) = output_runs(&lines[i], &p);
                    div().h(px(OUT_LH)).px(px(16.0)).whitespace_nowrap().child(StyledText::new(text).with_runs(runs))
                })
                .collect()
        })
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .with_width_from_item(widest)
        .track_scroll(&handle)
        .flex_1()
        .min_h(px(0.0));
        v_flex()
            .id(id)
            .role(gpui::Role::Document)
            .aria_label("Full output")
            .size_full()
            .bg(p.well)
            .font_family(scale::FONT_MONO)
            .text_px(OUT_TEXT)
            .line_height(px(OUT_LH))
            .child(div().flex_1().min_h(px(0.0)).pt(px(12.0)).pb(px(if self.capped.is_some() { 12.0 } else { 40.0 })).flex().flex_col().child(list))
            .when_some(self.capped, |d, n| {
                d.child(
                    h_flex()
                        .h(px(28.0))
                        .flex_none()
                        .px(px(16.0))
                        .items_center()
                        .border_t_1()
                        .border_color(p.line)
                        .font_family(scale::FONT_UI)
                        .text_px(12.1)
                        .text_color(p.ink_4)
                        .child(format!("{} lines · capped at {}", group_thousands(n), group_thousands(OUTPUT_LINE_CAP))),
                )
            })
    }
}

/// `10000` → `10 000` (the canvas's thin grouping).
pub fn group_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Fetching the full output: a centred spinner and one line.
pub fn output_fetching(cx: &App) -> gpui::Div {
    let p = cx.aui().colors;
    v_flex()
        .size_full()
        .bg(p.well)
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .font_family(scale::FONT_UI)
        .text_px(13.2)
        .text_color(p.fg_muted)
        .child(spinner("output-fetching").size(px(12.0)).muted(cx))
        .child("Fetching the full output…")
}

// ─────────────────────────────────────────────────────────────── image

/// The image view (docs/43 §2.10): padding 16, the picture fitted (or at
/// 100 % when `actual`), r 6 with the edge, and under it 10 px the facts in
/// mono 12.1 faint (`1440 × 900 · PNG · 312 KB`). Click toggles fit ↔ 100 %
/// (the host flips `actual`); at 100 % the view scrolls both ways.
pub fn image_view(
    id: impl Into<ElementId>,
    source: impl Into<ImageSource>,
    facts: impl Into<SharedString>,
    size: Option<(f32, f32)>,
    actual: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let p = cx.aui().colors;
    let id: ElementId = id.into();
    let facts: SharedString = facts.into();
    // The box the picture fits in, measured last frame (gpui has no
    // intrinsic `max-height: 100% - 40px` for an image in a flex column).
    let measured = window.use_keyed_state((id.clone(), "box"), cx, |_, _| None::<gpui::Size<gpui::Pixels>>);
    let fitted = match (size, *measured.read(cx)) {
        (Some((w, h)), Some(b)) if w > 0.0 && h > 0.0 => {
            let (bw, bh) = ((f32::from(b.width) - 32.0).max(1.0), (f32::from(b.height) - 32.0 - 40.0).max(1.0));
            let k = (bw / w).min(bh / h).min(1.0);
            Some((w * k, h * k))
        }
        _ => None,
    };
    let picture = img(source.into()).rounded(px(6.0));
    let picture = match (actual, size, fitted) {
        (true, Some((w, h)), _) => picture.w(px(w)).h(px(h)),
        (false, _, Some((w, h))) => picture.w(px(w)).h(px(h)),
        _ => picture.size(px(1.0)),
    };
    let framed = div().rounded(px(6.0)).shadow(p.edge()).overflow_hidden().flex_none().child(picture);
    let measure = {
        let measured = measured.clone();
        gpui::canvas(
            move |bounds, _, cx| {
                if *measured.read(cx) != Some(bounds.size) {
                    measured.update(cx, |m, cx| {
                        *m = Some(bounds.size);
                        cx.notify();
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    };
    let body = if actual {
        // At 100 %: drag to pan (gpui has no zoom cursors; the open hand says "drag").
        let scroll = window.use_keyed_state((id.clone(), "pan-scroll"), cx, |_, _| gpui::ScrollHandle::new()).read(cx).clone();
        let drag = window.use_keyed_state((id.clone(), "pan-drag"), cx, |_, _| None::<gpui::Point<gpui::Pixels>>);
        // Whether this press moved the picture: its release is then a pan,
        // not the host's click (live 2026-10-19: a drag toggled back to fit).
        let moved = window.use_keyed_state((id.clone(), "pan-moved"), cx, |_, _| false);
        let (s_down, d_down, m_down) = (scroll.clone(), drag.clone(), moved.clone());
        let (s_move, d_move, m_move) = (scroll.clone(), drag.clone(), moved.clone());
        let (d_up, m_up) = (drag.clone(), moved.clone());
        div()
            .id((id.clone(), "pan"))
            .size_full()
            .overflow_scroll()
            .track_scroll(&scroll)
            .p(px(16.0))
            .cursor(gpui::CursorStyle::OpenHand)
            .on_mouse_down(gpui::MouseButton::Left, move |e, _, cx| {
                let _ = &s_down;
                d_down.update(cx, |d, _| *d = Some(e.position));
                m_down.update(cx, |m, _| *m = false);
            })
            .on_mouse_move(move |e, window, cx| {
                let Some(last) = *d_move.read(cx) else { return };
                if !e.dragging() {
                    d_move.update(cx, |d, _| *d = None);
                    return;
                }
                let delta = e.position - last;
                if f32::from(delta.x).abs() + f32::from(delta.y).abs() > 0.0 {
                    m_move.update(cx, |m, _| *m = true);
                }
                let off = s_move.offset();
                s_move.set_offset(gpui::point(off.x + delta.x, off.y + delta.y));
                d_move.update(cx, |d, _| *d = Some(e.position));
                window.refresh();
            })
            .on_mouse_up(gpui::MouseButton::Left, move |_, _, cx| {
                d_up.update(cx, |d, _| *d = None);
                if m_up.update(cx, |m, _| std::mem::take(m)) {
                    // A pan: the host's click (fit ↔ 100 %) never sees it.
                    cx.stop_propagation();
                }
            })
            .child(framed)
            .into_any_element()
    } else {
        v_flex()
            .size_full()
            .p(px(16.0))
            .gap(px(10.0))
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(framed)
            .child(div().flex_none().font_family(scale::FONT_MONO).text_px(12.1).text_color(p.ink_4).child(facts.clone()))
            .into_any_element()
    };
    div().id(id).relative().size_full().role(gpui::Role::Image).aria_label(facts).child(measure).child(body)
}

/// Image not available: the image glyph, "Image not available", "<name>
/// could not be read."
pub fn image_missing(id: impl Into<ElementId>, name: &str, cx: &App) -> super::pane::StateBlock {
    let p = cx.aui().colors;
    super::pane::state_block(id, IconName::Image, "Image not available").line_with(
        div().child(div().font_family(scale::FONT_MONO).text_color(p.fg_muted).child(name.to_string())).child("could not be read."),
    )
}

/// A file kind and size for the image facts line: `PNG · 312 KB`.
pub fn image_facts(width: u32, height: u32, kind: &str, bytes: u64) -> String {
    let size = if bytes >= 1024 * 1024 { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) } else { format!("{} KB", bytes.div_ceil(1024)) };
    format!("{width} × {height} · {} · {size}", kind.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_group_with_a_space() {
        assert_eq!(group_thousands(1204), "1 204");
        assert_eq!(group_thousands(10_000), "10 000");
        assert_eq!(group_thousands(999), "999");
    }

    #[test]
    fn image_facts_read_like_the_canvas() {
        assert_eq!(image_facts(1440, 900, "png", 312 * 1024), "1440 × 900 · PNG · 312 KB");
        assert_eq!(image_facts(10, 10, "jpg", 3 * 1024 * 1024), "10 × 10 · JPG · 3.0 MB");
    }
}
