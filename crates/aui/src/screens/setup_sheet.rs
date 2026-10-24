//! The setup sheet (D5 B2, harness docs/66 §3): one modal for every
//! Install, every Claude Code sign-in and every Update — a header with the
//! provider's mark, a status line that follows the host's re-probe, a real
//! terminal (the host's own session, passed in as an element) with the
//! command typed and not run, a footnote, and Close → Done.
//!
//! [`sheet_frame`] is the frame alone (header, body, actions) so other
//! sign-ins share it: the Muse sign-in sheet is the same frame at 460 wide
//! ([`Login::sheet`](super::Login::sheet)). [`sheet_scrim`] is the 45 % black
//! layer the sheet sits on over whatever screen asked.
//!
//! Stateless: the host owns the terminal session, the watch that moves the
//! status line, and the open flag. Close, ✕ and Done all report through
//! [`SetupSheet::on_close`]; none of them cancels what runs in the terminal
//! (that is the host's promise — it keeps the session and the watch going).
//! Escape is the host's too: the terminal holds the keyboard focus, so the
//! host decides which Escape closes the sheet.

use std::rc::Rc;

use aui_icons::{icon, provider_mark, IconName, Provider};
use aui_tokens::{scale, ActiveAui, AuiStyled, Palette};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, IntoElement, SharedString, Window};
use gpui_kit::base::{h_flex, v_flex};

use crate::data::{button, icon_button, spinner, ButtonSize};

/// The setup sheet's width; the Muse sheet's is [`SIGN_IN_W`].
pub const SETUP_W: f32 = 560.0;
/// The narrower sign-in sheet (Muse).
pub const SIGN_IN_W: f32 = 460.0;
/// The sheet's radius.
const SHEET_R: f32 = 12.0;
/// The header: 44 tall, padded 0 8 0 16, gap 10, a 20 px mark.
const HEAD_H: f32 = 44.0;
const HEAD_GAP: f32 = 10.0;
const HEAD_MARK: f32 = 20.0;
/// Title 14.3 semibold.
const TITLE_TEXT: f32 = scale::FS_13;
/// Status, footnote: 13.2/18.
const CAPTION_TEXT: f32 = scale::FS_12;
const CAPTION_LH: f32 = 18.0;
/// The side padding every block shares.
const PAD_X: f32 = 16.0;
/// The terminal well: 150 tall, radius 8, 10 below the status line.
const TERM_H: f32 = 150.0;
const TERM_R: f32 = 8.0;
/// The scrim's black at 45 %.
const SCRIM_ALPHA: f32 = 0.45;

/// The status line at the top of the setup sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetupStatus {
    /// Waiting on the person or the CLI: a spinner and the line
    /// (`"Waiting for the install to finish…"`, `"Waiting for sign-in…"`).
    Waiting(SharedString),
    /// The probe sees the provider Connected: a success ✓ and the line
    /// (`"Connected · ada@example.com · Max"`). Close becomes Done.
    Connected(SharedString),
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The 45 % black layer over the whole window, the sheet centred on it.
/// A click on the scrim itself does nothing unless `on_dismiss` is given.
pub fn sheet_scrim(id: impl Into<ElementId>, sheet: impl IntoElement, on_dismiss: Option<CloseHandler>) -> AnyElement {
    let mut scrim = div()
        .id(id.into())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::black().alpha(SCRIM_ALPHA))
        // The scrim swallows the pointer so nothing under it reacts.
        .occlude();
    if let Some(dismiss) = on_dismiss {
        scrim = scrim.on_click(move |_, w, cx| dismiss(w, cx));
    }
    scrim.child(sheet).into_any_element()
}

/// The sheet frame: a header (mark, title, ✕), the caller's body and a
/// right-aligned action row. Build with [`sheet_frame`].
#[derive(IntoElement)]
pub struct SheetFrame {
    id: ElementId,
    provider: Provider,
    title: SharedString,
    width: f32,
    body: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    actions_top: f32,
    on_close: Option<CloseHandler>,
}

/// A sheet frame for `provider` titled `title`, [`SETUP_W`] wide.
pub fn sheet_frame(id: impl Into<ElementId>, provider: Provider, title: impl Into<SharedString>) -> SheetFrame {
    SheetFrame {
        id: id.into(),
        provider,
        title: title.into(),
        width: SETUP_W,
        body: Vec::new(),
        actions: Vec::new(),
        actions_top: 14.0,
        on_close: None,
    }
}

impl SheetFrame {
    /// Overrides the width ([`SIGN_IN_W`] for the Muse sheet).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Appends a body block (each brings its own padding).
    pub fn child(mut self, block: impl IntoElement) -> Self {
        self.body.push(block.into_any_element());
        self
    }

    /// The action row's buttons, primary last.
    pub fn actions(mut self, actions: Vec<AnyElement>) -> Self {
        self.actions = actions;
        self
    }

    /// The action row's top padding (14 by default; the Muse sheet's 18).
    pub fn actions_top(mut self, top: f32) -> Self {
        self.actions_top = top;
        self
    }

    /// ✕ was pressed.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for SheetFrame {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut close = icon_button((self.id.clone(), "close"), IconName::X)
            .ghost()
            .accessibility_label(format!("Close {}", self.title));
        if let Some(handler) = self.on_close.clone() {
            close = close.on_click(move |_, w, cx| handler(w, cx));
        }
        let head = h_flex()
            .w_full()
            .h(px(HEAD_H))
            .flex_none()
            .items_center()
            .gap(px(HEAD_GAP))
            .pl(px(PAD_X))
            .pr(px(8.0))
            .border_b_1()
            .border_color(p.line)
            .child(provider_mark(self.provider).size(px(HEAD_MARK)))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_px(TITLE_TEXT)
                    .semibold()
                    .text_color(p.ink)
                    .child(self.title.clone()),
            )
            .child(close);
        let mut sheet = v_flex()
            .id(self.id)
            .w(px(self.width))
            .max_w_full()
            .rounded(px(SHEET_R))
            .bg(p.surface_2)
            .shadow(p.floating_shadow())
            .overflow_hidden()
            .text_color(p.ink)
            .role(gpui::Role::Dialog)
            .aria_label(self.title)
            // A click inside the sheet is never a click on the scrim.
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(head)
            .children(self.body);
        if !self.actions.is_empty() {
            sheet = sheet.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(PAD_X))
                    .pt(px(self.actions_top))
                    .pb(px(16.0))
                    .child(div().flex_1())
                    .children(self.actions),
            );
        }
        sheet
    }
}

/// The setup sheet: status line, terminal, footnote, Close/Done. Build with
/// [`setup_sheet`].
#[derive(IntoElement)]
pub struct SetupSheet {
    id: ElementId,
    provider: Provider,
    title: SharedString,
    status: SetupStatus,
    terminal: Option<AnyElement>,
    note: Option<SharedString>,
    on_close: Option<CloseHandler>,
}

/// The setup sheet for `provider`, titled `title` (`"Install Claude Code"`,
/// `"Sign in to Claude Code"`, `"Update Codex"`).
pub fn setup_sheet(id: impl Into<ElementId>, provider: Provider, title: impl Into<SharedString>) -> SetupSheet {
    SetupSheet {
        id: id.into(),
        provider,
        title: title.into(),
        status: SetupStatus::Waiting("Waiting…".into()),
        terminal: None,
        note: None,
        on_close: None,
    }
}

impl SetupSheet {
    /// The status line.
    pub fn status(mut self, status: SetupStatus) -> Self {
        self.status = status;
        self
    }

    /// The terminal: the host's own session element. It fills the 150 px
    /// well; the host keeps the session alive across Close.
    pub fn terminal(mut self, terminal: impl IntoElement) -> Self {
        self.terminal = Some(terminal.into_any_element());
        self
    }

    /// The footnote under the terminal.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Close, ✕ or Done was pressed.
    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
}

/// The status line: a spinner or a success ✓, then the muted line.
fn status_row(id: &ElementId, status: &SetupStatus, p: &Palette) -> AnyElement {
    let (lead, text): (AnyElement, SharedString) = match status {
        SetupStatus::Waiting(text) => (spinner((id.clone(), "status-spinner")).size(px(12.0)).into_any_element(), text.clone()),
        SetupStatus::Connected(text) => (icon(IconName::Check).size(px(13.0)).color(p.success).into_any_element(), text.clone()),
    };
    h_flex()
        .id((id.clone(), "status"))
        .w_full()
        .items_center()
        .gap(px(8.0))
        .px(px(PAD_X))
        .pt(px(12.0))
        .text_px(CAPTION_TEXT)
        .line_height(px(CAPTION_LH))
        .text_color(p.ink_2)
        .role(gpui::Role::Status)
        .aria_label(text.clone())
        .child(lead)
        .child(div().flex_1().min_w(px(0.0)).truncate().child(text))
        .into_any_element()
}

impl RenderOnce for SetupSheet {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.aui().colors;
        let done = matches!(self.status, SetupStatus::Connected(_));
        let mut action = button((self.id.clone(), if done { "done" } else { "dismiss" }), if done { "Done" } else { "Close" })
            .size(ButtonSize::Row)
            .accessibility_label(if done { "Done" } else { "Close" });
        if done {
            action = action.primary();
        }
        if let Some(handler) = self.on_close.clone() {
            action = action.on_click(move |_, w, cx| handler(w, cx));
        }
        let well = div()
            .id((self.id.clone(), "terminal"))
            .mx(px(PAD_X))
            .mt(px(10.0))
            .h(px(TERM_H))
            .rounded(px(TERM_R))
            .bg(p.well)
            .shadow(p.edge())
            .overflow_hidden()
            .role(gpui::Role::Group)
            .aria_label("Terminal")
            .children(self.terminal);
        let mut frame = sheet_frame(self.id.clone(), self.provider, self.title)
            .child(status_row(&self.id, &self.status, &p))
            .child(well);
        if let Some(note) = self.note {
            frame = frame.child(
                div()
                    .px(px(PAD_X))
                    .pt(px(10.0))
                    .text_px(CAPTION_TEXT)
                    .line_height(px(CAPTION_LH))
                    .text_color(p.ink_4)
                    .child(note),
            );
        }
        if let Some(handler) = self.on_close {
            frame = frame.on_close(move |w, cx| handler(w, cx));
        }
        frame.actions(vec![action.into_any_element()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_starts_waiting_and_knows_its_measures() {
        let sheet = setup_sheet("s", Provider::Claude, "Install Claude Code");
        assert!(matches!(sheet.status, SetupStatus::Waiting(_)));
        assert_eq!(SETUP_W, 560.0);
        assert_eq!(SIGN_IN_W, 460.0);
        let frame = sheet_frame("f", Provider::Muse, "Sign in to Muse").width(SIGN_IN_W);
        assert_eq!(frame.width, SIGN_IN_W);
    }
}
