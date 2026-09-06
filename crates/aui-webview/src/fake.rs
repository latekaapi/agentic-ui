//! [`FakeWebBackend`]: the scripted page as a [`WebBackend`].
//!
//! It is the mock the gallery card runs on, and the reference implementation of
//! the contract: a real history, a real URL and title, `Loading` events around
//! a navigation, and annotations produced by hit-testing the pointer against
//! [`crate::page::fake_elements`].
//!
//! It cannot take a screenshot — there is no page to capture, only gpui
//! elements — so [`WebBackend::capture`] does nothing and no
//! [`WebEvent::Screenshot`] is ever produced.

use std::collections::HashMap;

use aui::workbench::Annotation;
use gpui::SharedString;

use crate::backend::{ElementInfo, WebBackend, WebEvent};
use crate::page::{fake_elements, FakeElement};

/// The page the mock opens on.
pub const HOME_URL: &str = "localhost:3000/pricing";
/// Its document title.
pub const HOME_TITLE: &str = "Pricing · Acme";
/// The note a fresh pin carries until it is saved. The pane has no text input
/// over the page yet, so this stands in for what is being typed.
pub const PENDING_NOTE: &str = "Typing…";

/// The scripted page.
pub struct FakeWebBackend {
    /// Every URL visited, oldest first.
    history: Vec<SharedString>,
    /// Where in `history` the page currently is.
    position: usize,
    title: SharedString,
    annotate: bool,
    /// The element under the pointer while annotate mode is on.
    hovered: Option<FakeElement>,
    /// The metadata of every pin, by annotation number: the side table
    /// described in [`crate::backend`].
    elements: HashMap<usize, ElementInfo>,
    /// Waiting to be drained by [`WebBackend::poll_events`].
    queue: Vec<WebEvent>,
}

impl Default for FakeWebBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeWebBackend {
    /// A backend sitting on [`HOME_URL`], with nothing pinned. The home page
    /// is already announced, so the first poll hands the pane its URL and
    /// title without a navigation.
    pub fn new() -> Self {
        let mut backend = Self {
            history: vec![SharedString::from(HOME_URL)],
            position: 0,
            title: SharedString::from(HOME_TITLE),
            annotate: false,
            hovered: None,
            elements: HashMap::new(),
            queue: Vec::new(),
        };
        backend.announce();
        backend
    }

    /// The URL showing in the nav row.
    pub fn url(&self) -> SharedString {
        self.history.get(self.position).cloned().unwrap_or_default()
    }

    /// The document title.
    pub fn title(&self) -> SharedString {
        self.title.clone()
    }

    /// Announces the page at `position`: a load, its URL and title.
    fn announce(&mut self) {
        let url = self.url();
        self.queue.push(WebEvent::Loading(true));
        self.queue.push(WebEvent::Url(url.to_string()));
        self.queue.push(WebEvent::Title(self.title.to_string()));
        self.queue.push(WebEvent::Loading(false));
    }

    /// The element `at` lands in, if any. The list is front to back, so the
    /// first hit wins.
    fn hit_test(&self, at: (f32, f32)) -> Option<FakeElement> {
        fake_elements().into_iter().find(|element| {
            at.0 >= element.origin.0
                && at.1 >= element.origin.1
                && at.0 <= element.origin.0 + element.size.0
                && at.1 <= element.origin.1 + element.size.1
        })
    }
}

/// The metadata a fake element hands the pane. The mock has no DOM, so the
/// `outerHTML` is a plausible one-line stand-in built from the selector.
fn info_for(element: &FakeElement) -> ElementInfo {
    let tag = element.label.clone();
    let classes = element.selector.split('.').skip(1).collect::<Vec<_>>().join(" ");
    let open = if classes.is_empty() { format!("<{tag}>") } else { format!("<{tag} class=\"{classes}\">") };
    ElementInfo {
        selector: element.selector.clone(),
        label: element.label.clone(),
        origin: element.origin,
        size: element.size,
        source: element.source.clone(),
        outer_html: SharedString::from(format!("{open}…</{tag}>")),
    }
}

impl WebBackend for FakeWebBackend {
    fn navigate(&mut self, url: &str) {
        self.history.truncate(self.position + 1);
        self.history.push(SharedString::from(url.to_string()));
        self.position = self.history.len() - 1;
        self.announce();
    }

    fn back(&mut self) {
        if self.position > 0 {
            self.position -= 1;
            self.announce();
        }
    }

    fn forward(&mut self) {
        if self.position + 1 < self.history.len() {
            self.position += 1;
            self.announce();
        }
    }

    fn reload(&mut self) {
        self.announce();
    }

    fn eval(&mut self, js: &str) {
        // There is no JS engine behind the mock; the call is accepted and
        // dropped so the pane can be written once for both backends.
        let _ = js;
    }

    fn set_annotate(&mut self, on: bool) {
        self.annotate = on;
        if !on {
            self.hovered = None;
        }
    }

    fn poll_events(&mut self) -> Vec<WebEvent> {
        std::mem::take(&mut self.queue)
    }

    fn can_go_back(&self) -> bool {
        self.position > 0
    }

    fn can_go_forward(&self) -> bool {
        self.position + 1 < self.history.len()
    }

    fn point_moved(&mut self, at: Option<(f32, f32)>) {
        self.hovered = match (self.annotate, at) {
            (true, Some(at)) => self.hit_test(at),
            _ => None,
        };
    }

    fn point_clicked(&mut self, at: (f32, f32)) {
        if !self.annotate {
            return;
        }
        let Some(element) = self.hit_test(at) else { return };
        let info = info_for(&element);
        // Already pinned? Re-pinning the same element would stack two pins in
        // one place, so the click is a no-op.
        if self.elements.values().any(|pinned| pinned.selector == info.selector) {
            return;
        }
        let index = self.elements.len() + 1;
        self.elements.insert(index, info);
        self.queue.push(WebEvent::Annotation(Annotation::new(index, element.selector.clone(), PENDING_NOTE).pending(true)));
    }

    fn hovered(&self) -> Option<ElementInfo> {
        self.hovered.as_ref().map(info_for)
    }

    fn element_info(&self, index: usize) -> Option<ElementInfo> {
        self.elements.get(&index).cloned()
    }
}
