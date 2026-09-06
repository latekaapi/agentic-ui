//! The backend contract: what the pane needs from "a page", whether that page
//! is the scripted mock ([`crate::fake`]) or a real WKWebView
//! (`crate::wry_backend`).
//!
//! The pane never talks to a browser directly. It pushes commands down
//! ([`WebBackend::navigate`], [`WebBackend::eval`], …) and pulls
//! [`WebEvent`]s back up on a timer ([`WebBackend::poll_events`]), so the
//! gpui side is the same code for the mock and for the real thing.
//!
//! # Where the element metadata lives
//!
//! [`WebEvent::Annotation`] carries only the `aui` [`Annotation`] the side
//! panel renders — the number, the selector and the note. The rest of what an
//! annotation is sent with (the bounding box, the trimmed `outerHTML`, the
//! source file) is bulky, is not part of the panel's data, and for the real
//! backend arrives on the same IPC message. So the backend keeps it in a **side
//! table** and the pane asks for it by annotation number through
//! [`WebBackend::element_info`]. That keeps [`WebEvent`] small and lets a
//! backend that knows nothing about source maps simply return `None`.

use aui::workbench::Annotation;
use gpui::SharedString;

/// One element of the page, as the annotator sees it.
///
/// The pane uses [`ElementInfo::origin`] and [`ElementInfo::size`] to draw the
/// hover outline and to drop the pin, and the rest to label the note popover
/// and to describe the element to the agent.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementInfo {
    /// The CSS path of the element (`div.card.starter`).
    pub selector: SharedString,
    /// The tag shown in the outline's label (`p`, `h1`, `div`).
    pub label: SharedString,
    /// The element's top-left corner, in page coordinates.
    pub origin: (f32, f32),
    /// The element's box, in page pixels.
    pub size: (f32, f32),
    /// Where the element comes from when a dev server is mapped
    /// (`pricing.tsx:42`); empty when it is not.
    pub source: SharedString,
    /// The element's `outerHTML`, trimmed to [`OUTER_HTML_LIMIT`] bytes.
    pub outer_html: SharedString,
}

/// How much `outerHTML` an annotation carries. Two kilobytes is enough for the
/// element and its immediate children and small enough to cross the IPC bridge
/// on every click without a copy anyone notices.
pub const OUTER_HTML_LIMIT: usize = 2048;

impl ElementInfo {
    /// The outline's label: `p · 392 × 34`.
    pub fn outline_label(&self) -> SharedString {
        SharedString::from(format!("{} · {} × {}", self.label, self.size.0.round(), self.size.1.round()))
    }

    /// The note popover's path line: `div.card.starter · 150 × 118 · pricing.tsx:42`.
    pub fn path(&self) -> SharedString {
        let box_ = format!("{} · {} × {}", self.selector, self.size.0.round(), self.size.1.round());
        if self.source.is_empty() {
            SharedString::from(box_)
        } else {
            SharedString::from(format!("{box_} · {}", self.source))
        }
    }

    /// Whether `point` (page coordinates) is inside the element's box.
    pub fn contains(&self, point: (f32, f32)) -> bool {
        point.0 >= self.origin.0
            && point.1 >= self.origin.1
            && point.0 <= self.origin.0 + self.size.0
            && point.1 <= self.origin.1 + self.size.1
    }
}

/// Something the page told the pane about itself.
#[derive(Debug, Clone, PartialEq)]
pub enum WebEvent {
    /// The document title changed.
    Title(String),
    /// The page navigated; the pane shows this in the URL field.
    Url(String),
    /// Loading started (`true`) or finished (`false`).
    Loading(bool),
    /// The person pinned an element while annotate mode was on. The metadata
    /// that goes with it is fetched with [`WebBackend::element_info`].
    Annotation(Annotation),
    /// An encoded PNG of the page.
    Screenshot(Vec<u8>),
}

/// A page the [`crate::view::WebviewState`] can drive.
///
/// Every method is a command; nothing returns a result, because a real webview
/// answers asynchronously. Answers come back through [`Self::poll_events`],
/// which the pane drains on a timer.
pub trait WebBackend {
    /// Loads `url`.
    fn navigate(&mut self, url: &str);

    /// Goes back one entry in history, if there is one.
    fn back(&mut self);

    /// Goes forward one entry, if there is one.
    fn forward(&mut self);

    /// Reloads the current page.
    fn reload(&mut self);

    /// Runs `js` in the page.
    fn eval(&mut self, js: &str);

    /// Turns annotate mode on or off. Off clears the hovered element.
    fn set_annotate(&mut self, on: bool);

    /// Takes everything the page has said since the last call.
    fn poll_events(&mut self) -> Vec<WebEvent>;

    /// Whether [`Self::back`] would go anywhere. Defaults to `false`.
    fn can_go_back(&self) -> bool {
        false
    }

    /// Whether [`Self::forward`] would go anywhere. Defaults to `false`.
    fn can_go_forward(&self) -> bool {
        false
    }

    /// Asks for a screenshot; the answer arrives as [`WebEvent::Screenshot`].
    /// Backends that cannot capture do nothing, which is the default.
    fn capture(&mut self) {}

    /// The pointer moved to `at` in page coordinates, or left the page
    /// (`None`). Only backends that hit-test in Rust — the mock — use this; a
    /// real webview does its own hit-testing inside the page, so the default
    /// is to ignore it.
    fn point_moved(&mut self, at: Option<(f32, f32)>) {
        let _ = at;
    }

    /// The pointer was pressed at `at` in page coordinates. In annotate mode a
    /// backend that hit-tests in Rust answers with a [`WebEvent::Annotation`].
    fn point_clicked(&mut self, at: (f32, f32)) {
        let _ = at;
    }

    /// The element under the pointer, when the backend knows it.
    fn hovered(&self) -> Option<ElementInfo> {
        None
    }

    /// The metadata collected for the annotation numbered `index`.
    fn element_info(&self, index: usize) -> Option<ElementInfo> {
        let _ = index;
        None
    }
}
