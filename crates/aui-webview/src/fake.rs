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

use crate::agent_js::{json_string, CLICK_MARKER, LINKS_MARKER, PAGE_TEXT_MARKER, TYPE_MARKER};
use crate::backend::{BackendFeature, DownloadState, ElementInfo, FindState, PageSecurity, PermissionDecision, PermissionKind, PermissionRequest, WebBackend, WebEvent};
use crate::page::{fake_elements, FakeElement};

/// The page the mock opens on.
pub const HOME_URL: &str = "localhost:3000/pricing";
/// Its document title.
pub const HOME_TITLE: &str = "Pricing · Acme";
/// The note a fresh pin carries until it is saved. The pane has no text input
/// over the page yet, so this stands in for what is being typed.
pub const PENDING_NOTE: &str = "Typing…";
/// What `document.body.innerText` reads on the scripted page: the headline,
/// the lead and the three plan cards, one per line.
pub const FAKE_PAGE_TEXT: &str = "Simple pricing\nStart free, upgrade when your team needs it. All plans include unlimited worktrees.\nStarter $0 / month 3 agents, local only Try free\nTeam $24 / seat SSH + cloud VMs Choose\nEnterprise Custom SSO, audit, support Contact";

/// The text a `click` answer carries for each element of the scripted page.
/// The mock has no DOM, so this is the stand-in for the element's `innerText`.
fn element_text(selector: &str) -> Option<&'static str> {
    match selector {
        "h1" => Some("Simple pricing"),
        "p.lead" => Some("Start free, upgrade when your team needs it. All plans include unlimited worktrees."),
        "div.card.starter" => Some("Starter $0 / month 3 agents, local only Try free"),
        "div.card.team" => Some("Team $24 / seat SSH + cloud VMs Choose"),
        "div.card.enterprise" => Some("Enterprise Custom SSO, audit, support Contact"),
        _ => None,
    }
}

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
    /// Evaluation answers waiting for [`WebBackend::take_eval_results`].
    eval_results: Vec<(u64, Result<String, String>)>,
    /// What [`crate::agent_js::type_text`] scripts have set, by selector.
    typed: HashMap<String, String>,
    /// Whether the page holds the keyboard: set by [`WebBackend::set_focused`]
    /// and inferred on [`WebBackend::point_clicked`] (a mouse-down inside a
    /// real page makes its webview first responder), cleared by handing focus
    /// back. The scripted page is gpui elements and cannot steal AppKit focus,
    /// so this is tracked, not queried.
    focused: bool,
    /// Every [`WebBackend::set_focused`] argument, oldest first: the record
    /// the keyboard hand-back tests assert against.
    focus_calls: Vec<bool>,
    /// How many times [`WebBackend::stop`] ran.
    stop_calls: u32,
    /// Every [`WebBackend::set_zoom`] argument, oldest first.
    zoom_calls: Vec<u16>,
    /// Every [`WebBackend::find`] call, as `(query, forward, match_case)`,
    /// oldest first.
    find_calls: Vec<(String, bool, bool)>,
    /// How many times [`WebBackend::stop_find`] ran.
    stop_find_calls: u32,
    /// How many times [`WebBackend::show_devtools`] ran.
    devtools_calls: u32,
    /// Popup pages waiting for [`WebBackend::take_popup_backends`], oldest
    /// first, as `(url, foreground)`.
    popups: Vec<(String, bool)>,
    /// "Open this URL" requests waiting for
    /// [`WebBackend::take_open_requests`], oldest first.
    open_requests: Vec<(String, bool)>,
    /// Every [`WebBackend::answer_permission`] call, oldest first.
    answers: Vec<(u64, PermissionDecision)>,
    /// Every [`WebBackend::cancel_download`] id, oldest first.
    cancelled_downloads: Vec<u32>,
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
            eval_results: Vec::new(),
            typed: HashMap::new(),
            focused: false,
            focus_calls: Vec::new(),
            stop_calls: 0,
            zoom_calls: Vec::new(),
            find_calls: Vec::new(),
            stop_find_calls: 0,
            devtools_calls: 0,
            popups: Vec::new(),
            open_requests: Vec::new(),
            answers: Vec::new(),
            cancelled_downloads: Vec::new(),
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

    /// What a [`crate::agent_js::type_text`] script last set into `selector`,
    /// if anything.
    pub fn typed_text(&self, selector: &str) -> Option<&str> {
        self.typed.get(selector).map(String::as_str)
    }

    /// How many times [`WebBackend::stop`] ran.
    pub fn stop_calls(&self) -> u32 {
        self.stop_calls
    }

    /// Every [`WebBackend::set_zoom`] argument, oldest first.
    pub fn zoom_calls(&self) -> &[u16] {
        &self.zoom_calls
    }

    /// Every [`WebBackend::find`] call, as `(query, forward, match_case)`,
    /// oldest first.
    pub fn find_calls(&self) -> &[(String, bool, bool)] {
        &self.find_calls
    }

    /// How many times [`WebBackend::stop_find`] ran.
    pub fn stop_find_calls(&self) -> u32 {
        self.stop_find_calls
    }

    /// How many times [`WebBackend::show_devtools`] ran.
    pub fn devtools_calls(&self) -> u32 {
        self.devtools_calls
    }

    /// Queues a [`WebEvent::Favicon`]: the toolbar reads the icon off this.
    pub fn script_favicon(&mut self, url: Option<&str>) {
        self.queue.push(WebEvent::Favicon(url.map(str::to_owned)));
    }

    /// Queues a [`WebEvent::Progress`], 0.0–1.0.
    pub fn script_progress(&mut self, progress: f32) {
        self.queue.push(WebEvent::Progress(progress));
    }

    /// Queues a [`WebEvent::Security`].
    pub fn script_security(&mut self, security: PageSecurity) {
        self.queue.push(WebEvent::Security(security));
    }

    /// Queues a [`WebEvent::Audible`].
    pub fn script_audible(&mut self, audible: bool) {
        self.queue.push(WebEvent::Audible(audible));
    }

    /// Queues a [`WebEvent::Loading`]: one half of a crash batch
    /// (`Crashed` + `Loading(false)`) or a bare load edge.
    pub fn script_loading(&mut self, loading: bool) {
        self.queue.push(WebEvent::Loading(loading));
    }

    /// Queues a [`WebEvent::Crashed`]: the tab reads as crashed until the
    /// next successful navigation.
    pub fn script_crashed(&mut self, reason: &str) {
        self.queue.push(WebEvent::Crashed(reason.to_owned()));
    }

    /// Queues a [`WebEvent::Find`]: one find-strip update ("3 of 12").
    pub fn script_find(&mut self, query: &str, active: u32, total: u32, final_update: bool) {
        self.queue.push(WebEvent::Find(FindState { query: query.to_owned(), active, total, final_update }));
    }

    /// Queues a [`WebEvent::Zoom`], in percent.
    pub fn script_zoom(&mut self, percent: u16) {
        self.queue.push(WebEvent::Zoom(percent));
    }

    /// Queues a [`WebEvent::PopupOpened`] and the popup page behind it: the
    /// next [`WebBackend::take_popup_backends`] hands back a scripted page on
    /// `url`.
    pub fn script_popup(&mut self, url: &str, foreground: bool) {
        self.popups.push((url.to_owned(), foreground));
        self.queue.push(WebEvent::PopupOpened { url: url.to_owned(), foreground });
    }

    /// Queues an "open this URL" request for
    /// [`WebBackend::take_open_requests`] (no event: the host asked for it).
    pub fn script_open_request(&mut self, url: &str, foreground: bool) {
        self.open_requests.push((url.to_owned(), foreground));
    }

    /// Queues a [`WebEvent::Permission`].
    pub fn script_permission(&mut self, id: u64, origin: &str, kinds: Vec<PermissionKind>) {
        self.queue.push(WebEvent::Permission(PermissionRequest { id, origin: origin.to_owned(), kinds }));
    }

    /// Queues a [`WebEvent::PermissionDismissed`].
    pub fn script_permission_dismissed(&mut self, id: u64) {
        self.queue.push(WebEvent::PermissionDismissed(id));
    }

    /// Queues a [`WebEvent::Download`].
    pub fn script_download(&mut self, state: DownloadState) {
        self.queue.push(WebEvent::Download(state));
    }

    /// Every [`WebBackend::answer_permission`] call, oldest first.
    pub fn answers(&self) -> &[(u64, PermissionDecision)] {
        &self.answers
    }

    /// Every [`WebBackend::cancel_download`] id, oldest first.
    pub fn cancelled_downloads(&self) -> &[u32] {
        &self.cancelled_downloads
    }

    /// Announces the page at `position`: a load, its URL and title.
    fn announce(&mut self) {
        let url = self.url();
        self.queue.push(WebEvent::Loading(true));
        self.queue.push(WebEvent::Url(url.to_string()));
        self.queue.push(WebEvent::Title(self.title.to_string()));
        self.queue.push(WebEvent::Loading(false));
    }

    /// Answers an [`WebBackend::eval_with_result`] script immediately.
    ///
    /// The mock has no JS engine, so it emulates a small fixed set: the
    /// [`crate::agent_js`] helpers (found by their markers, with their
    /// JSON-encoded arguments decoded back out) and the three bare reads
    /// `document.title`, `location.href` and `document.body.innerText`.
    /// Anything else is `Err("unsupported in the fake page")`. Every `Ok`
    /// holds the JSON-stringified value, exactly as the real backend delivers.
    fn answer(&mut self, js: &str) -> Result<String, String> {
        if let Some(selector) = literal_after(js, CLICK_MARKER) {
            return match element_text(&selector) {
                Some(text) => Ok(json_string(text)),
                None => Err(format!("element not found: {selector}")),
            };
        }
        if let Some(after) = js.split(TYPE_MARKER).nth(1) {
            let mut literals = json_literals(after).into_iter();
            return match (literals.next(), literals.next()) {
                (Some(selector), Some(text)) => match element_text(&selector) {
                    Some(_) => {
                        self.typed.insert(selector.clone(), text.clone());
                        Ok(json_string(&text))
                    }
                    None => Err(format!("element not found: {selector}")),
                },
                _ => Err(String::from("unsupported in the fake page")),
            };
        }
        if js.contains(PAGE_TEXT_MARKER) {
            let max = js
                .split(PAGE_TEXT_MARKER)
                .nth(1)
                .and_then(|after| after.trim_start_matches(':').split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|digits| digits.parse::<usize>().ok())
                .unwrap_or(usize::MAX);
            let text: String = FAKE_PAGE_TEXT.chars().take(max).collect();
            return Ok(format!(
                "{{\"title\":{},\"url\":{},\"text\":{}}}",
                json_string(&self.title),
                json_string(&self.url()),
                json_string(&text)
            ));
        }
        if js.contains(LINKS_MARKER) {
            // The scripted page has no links.
            return Ok(String::from("[]"));
        }
        let bare = js.trim().trim_end_matches(';').trim();
        if bare == "document.title" {
            return Ok(json_string(&self.title));
        }
        if bare == "location.href" {
            return Ok(json_string(&self.url()));
        }
        if bare == "document.body.innerText" {
            return Ok(json_string(FAKE_PAGE_TEXT));
        }
        Err(String::from("unsupported in the fake page"))
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

/// Decodes the first JSON string literal after `marker`, which is how the
/// mock reads the JSON-encoded arguments back out of an [`crate::agent_js`]
/// script.
fn literal_after(haystack: &str, marker: &str) -> Option<String> {
    let after = haystack.split(marker).nth(1)?;
    json_literals(after).into_iter().next()
}

/// Every JSON string literal in `text`, decoded. Only literals are collected:
/// scanning starts at each `"` and a failed decode skips one character rather
/// than giving up, so code around the literals cannot confuse it.
fn json_literals(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            match decode_literal(&bytes[i..]) {
                Some((value, len)) => {
                    out.push(value);
                    i += len;
                    continue;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Decodes one JSON string literal at the start of `bytes` (leading `"`
/// included), answering the value and its length in bytes.
fn decode_literal(bytes: &[u8]) -> Option<(String, usize)> {
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut value = String::new();
    let mut i = 1;
    loop {
        let byte = *bytes.get(i)?;
        match byte {
            b'"' => return Some((value, i + 1)),
            b'\\' => {
                i += 1;
                match *bytes.get(i)? {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'/' => value.push('/'),
                    b'b' => value.push('\u{08}'),
                    b'f' => value.push('\u{0C}'),
                    b'n' => value.push('\n'),
                    b'r' => value.push('\r'),
                    b't' => value.push('\t'),
                    b'u' => {
                        if bytes.get(i + 1) == Some(&b'{') {
                            return None;
                        }
                        let hex = std::str::from_utf8(bytes.get(i + 1..i + 5)?).ok()?;
                        let mut code = u32::from_str_radix(hex, 16).ok()?;
                        i += 4;
                        // A high surrogate followed by another `\uXXXX` is one
                        // character, not two.
                        if (0xD800..0xDC00).contains(&code)
                            && bytes.get(i + 1) == Some(&b'\\')
                            && bytes.get(i + 2) == Some(&b'u')
                        {
                            let low_hex = std::str::from_utf8(bytes.get(i + 3..i + 7)?).ok()?;
                            let low = u32::from_str_radix(low_hex, 16).ok()?;
                            if (0xDC00..0xE000).contains(&low) {
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                                i += 6;
                            }
                        }
                        value.push(char::from_u32(code)?);
                    }
                    _ => return None,
                }
                i += 1;
            }
            _ => {
                // A literal is UTF-8; consume one character, not one byte.
                let rest = std::str::from_utf8(&bytes[i..]).ok()?;
                let c = rest.chars().next()?;
                value.push(c);
                i += c.len_utf8();
            }
        }
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

    fn eval_with_result(&mut self, request_id: u64, js: &str) {
        // No page to run it on, but the answer is known: emulate it now.
        let answer = self.answer(js);
        self.eval_results.push((request_id, answer));
    }

    fn take_eval_results(&mut self) -> Vec<(u64, Result<String, String>)> {
        std::mem::take(&mut self.eval_results)
    }

    fn set_annotate(&mut self, on: bool) {
        self.annotate = on;
        if !on {
            self.hovered = None;
        }
    }

    fn set_visible(&mut self, visible: bool) {
        // Hiding a page that holds the keyboard must hand it back first: a
        // hidden webview that is still first responder keeps eating every
        // keystroke. An unfocused hide records nothing.
        if !visible && self.focused {
            self.set_focused(false);
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.focus_calls.push(focused);
        self.focused = focused;
    }

    fn holds_keyboard(&self) -> bool {
        self.focused
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
        // Any mouse-down inside the page makes a real webview first responder
        // — annotate mode or not — so the page holds the keyboard from here.
        self.focused = true;
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

    fn stop(&mut self) {
        self.stop_calls += 1;
    }

    fn set_zoom(&mut self, percent: u16) {
        self.zoom_calls.push(percent);
    }

    fn find(&mut self, query: &str, forward: bool, match_case: bool) {
        self.find_calls.push((query.to_owned(), forward, match_case));
    }

    fn stop_find(&mut self) {
        self.stop_find_calls += 1;
    }

    fn show_devtools(&mut self) {
        self.devtools_calls += 1;
    }

    fn take_popup_backends(&mut self) -> Vec<(String, bool, Box<dyn WebBackend>)> {
        std::mem::take(&mut self.popups)
            .into_iter()
            .map(|(url, foreground)| {
                // A fresh scripted page sitting on the popup's URL, with its
                // opening announcement drained so it polls clean.
                let mut page = FakeWebBackend::new();
                page.navigate(&url);
                let _ = page.poll_events();
                (url, foreground, Box::new(page) as Box<dyn WebBackend>)
            })
            .collect()
    }

    fn take_open_requests(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.open_requests)
    }

    fn answer_permission(&mut self, id: u64, decision: PermissionDecision) {
        self.answers.push((id, decision));
    }

    fn cancel_download(&mut self, id: u32) {
        self.cancelled_downloads.push(id);
    }

    fn supports(&self, _feature: BackendFeature) -> bool {
        // The scripted page can drive every toolbar control.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_js;

    /// Sends one script and takes its single answer.
    fn ask(backend: &mut FakeWebBackend, id: u64, js: &str) -> (u64, Result<String, String>) {
        backend.eval_with_result(id, js);
        let mut answers = backend.take_eval_results();
        assert_eq!(answers.len(), 1);
        answers.pop().expect("one answer")
    }

    #[test]
    fn the_fake_answers_title_url_and_text() {
        let mut backend = FakeWebBackend::new();
        assert_eq!(ask(&mut backend, 1, "document.title"), (1, Ok(json_string(HOME_TITLE))));
        assert_eq!(ask(&mut backend, 2, "location.href"), (2, Ok(json_string(HOME_URL))));
        assert_eq!(ask(&mut backend, 3, "document.body.innerText"), (3, Ok(json_string(FAKE_PAGE_TEXT))));
        // Drained: nothing lingers for the next take.
        assert!(backend.take_eval_results().is_empty());
    }

    #[test]
    fn page_text_answers_title_url_and_trimmed_text() {
        let mut backend = FakeWebBackend::new();
        let (_, answer) = ask(&mut backend, 1, &agent_js::page_text(1_000_000));
        let expected = format!(
            "{{\"title\":{},\"url\":{},\"text\":{}}}",
            json_string(HOME_TITLE),
            json_string(HOME_URL),
            json_string(FAKE_PAGE_TEXT)
        );
        assert_eq!(answer, Ok(expected));

        let (_, trimmed) = ask(&mut backend, 2, &agent_js::page_text(12));
        let expected_text: String = FAKE_PAGE_TEXT.chars().take(12).collect();
        assert!(trimmed.expect("an answer").contains(&json_string(&expected_text)));
    }

    #[test]
    fn click_on_a_fake_element_returns_its_text() {
        let mut backend = FakeWebBackend::new();
        let (id, answer) = ask(&mut backend, 4, &agent_js::click("div.card.starter"));
        assert_eq!(id, 4);
        assert_eq!(answer, Ok(json_string("Starter $0 / month 3 agents, local only Try free")));
    }

    #[test]
    fn click_on_an_unknown_selector_is_an_error() {
        let mut backend = FakeWebBackend::new();
        let (_, answer) = ask(&mut backend, 5, &agent_js::click("button.ghost"));
        assert_eq!(answer, Err(String::from("element not found: button.ghost")));
    }

    #[test]
    fn type_text_sets_the_value_and_answers_with_it() {
        let mut backend = FakeWebBackend::new();
        let (_, answer) = ask(&mut backend, 6, &agent_js::type_text("p.lead", "hello \"there\""));
        assert_eq!(answer, Ok(json_string("hello \"there\"")));
        assert_eq!(backend.typed_text("p.lead"), Some("hello \"there\""));
    }

    #[test]
    fn type_text_on_an_unknown_selector_is_an_error() {
        let mut backend = FakeWebBackend::new();
        let (_, answer) = ask(&mut backend, 7, &agent_js::type_text("input.missing", "x"));
        assert_eq!(answer, Err(String::from("element not found: input.missing")));
        assert_eq!(backend.typed_text("input.missing"), None);
    }

    #[test]
    fn anything_else_is_unsupported_in_the_fake_page() {
        let mut backend = FakeWebBackend::new();
        let (_, answer) = ask(&mut backend, 8, "document.querySelector('h1').remove()");
        assert_eq!(answer, Err(String::from("unsupported in the fake page")));
    }

    #[test]
    fn a_mouse_down_in_the_page_takes_the_keyboard() {
        use crate::page::fake_elements;
        let mut backend = FakeWebBackend::new();
        assert!(!backend.holds_keyboard());
        // Annotate mode is off: the click pins nothing, but a real webview
        // would still become first responder.
        let element = &fake_elements()[0];
        backend.point_clicked((element.origin.0 + 1.0, element.origin.1 + 1.0));
        assert!(backend.holds_keyboard());
    }

    #[test]
    fn hiding_a_focused_webview_hands_the_keyboard_back_first() {
        let mut backend = FakeWebBackend::new();
        backend.set_focused(true);
        backend.set_visible(false);
        assert_eq!(backend.focus_calls, vec![true, false]);
        assert!(!backend.holds_keyboard());
    }

    #[test]
    fn hiding_an_unfocused_webview_calls_nothing() {
        let mut backend = FakeWebBackend::new();
        assert!(!backend.holds_keyboard());
        backend.set_visible(false);
        backend.set_visible(true);
        assert!(backend.focus_calls.is_empty());
    }

    #[test]
    fn quoted_selectors_round_trip_through_the_script() {
        let mut backend = FakeWebBackend::new();
        // A selector the page does not have, but with quotes that must survive
        // the trip into the script and back out to the error message intact.
        let selector = "div[data-x=\"a'b\"]";
        let (_, answer) = ask(&mut backend, 9, &agent_js::click(selector));
        assert_eq!(answer, Err(format!("element not found: {selector}")));
    }

    #[test]
    fn scripted_page_state_events_poll_in_order() {
        let mut backend = FakeWebBackend::new();
        // Drain the opening announcement.
        backend.poll_events();
        backend.script_favicon(Some("https://example.com/icon.png"));
        backend.script_progress(0.5);
        backend.script_security(PageSecurity::Secure);
        backend.script_audible(true);
        backend.script_crashed("the page crashed (-1)");
        backend.script_find("price", 3, 12, true);
        backend.script_zoom(125);
        backend.script_favicon(None);
        assert_eq!(
            backend.poll_events(),
            vec![
                WebEvent::Favicon(Some(String::from("https://example.com/icon.png"))),
                WebEvent::Progress(0.5),
                WebEvent::Security(PageSecurity::Secure),
                WebEvent::Audible(true),
                WebEvent::Crashed(String::from("the page crashed (-1)")),
                WebEvent::Find(FindState { query: String::from("price"), active: 3, total: 12, final_update: true }),
                WebEvent::Zoom(125),
                WebEvent::Favicon(None),
            ]
        );
        // Drained: nothing lingers for the next poll.
        assert!(backend.poll_events().is_empty());
    }

    #[test]
    fn the_fake_records_page_commands() {
        let mut backend = FakeWebBackend::new();
        backend.stop();
        backend.set_zoom(125);
        backend.set_zoom(110);
        backend.find("price", true, false);
        backend.find("price", true, true);
        backend.stop_find();
        backend.show_devtools();
        assert_eq!(backend.stop_calls(), 1);
        assert_eq!(backend.zoom_calls(), &[125, 110]);
        assert_eq!(
            backend.find_calls(),
            &[(String::from("price"), true, false), (String::from("price"), true, true)]
        );
        assert_eq!(backend.stop_find_calls(), 1);
        assert_eq!(backend.devtools_calls(), 1);
        for feature in [
            BackendFeature::Find,
            BackendFeature::Zoom,
            BackendFeature::DevTools,
            BackendFeature::Stop,
            BackendFeature::Favicon,
            BackendFeature::Security,
        ] {
            assert!(backend.supports(feature), "{feature:?} must be supported by the fake");
        }
    }

    #[test]
    fn the_fake_scripts_popups_permissions_downloads_and_records_replies() {
        use crate::backend::{DownloadStatus, PermissionDecision, PermissionKind};
        let mut backend = FakeWebBackend::new();
        let _ = backend.poll_events();
        backend.script_popup("https://example.com/pop", true);
        backend.script_open_request("https://example.com/menu", false);
        backend.script_permission(7, "https://example.com", vec![PermissionKind::Camera]);
        backend.script_permission_dismissed(8);
        backend.script_download(DownloadState {
            id: 3,
            url: String::from("https://example.com/f.zip"),
            file_name: String::from("f.zip"),
            path: String::from("/tmp/f.zip"),
            received: 10,
            total: Some(100),
            state: DownloadStatus::Downloading,
        });
        let events = backend.poll_events();
        assert!(matches!(&events[0], WebEvent::PopupOpened { url, foreground } if url == "https://example.com/pop" && *foreground));
        assert!(matches!(&events[1], WebEvent::Permission(req) if req.id == 7 && req.kinds == vec![PermissionKind::Camera]));
        assert_eq!(events[2], WebEvent::PermissionDismissed(8));
        assert!(matches!(&events[3], WebEvent::Download(state) if state.id == 3 && state.total == Some(100)));
        // The popup page behind the event sits on its URL …
        let mut popups = backend.take_popup_backends();
        assert_eq!(popups.len(), 1);
        let (url, foreground, mut page) = popups.pop().expect("one popup");
        assert_eq!((url.as_str(), foreground), ("https://example.com/pop", true));
        assert!(page.poll_events().is_empty());
        // … and a second take sees nothing.
        assert!(backend.take_popup_backends().is_empty());
        assert_eq!(backend.take_open_requests(), vec![(String::from("https://example.com/menu"), false)]);
        assert!(backend.take_open_requests().is_empty());
        // Replies are recorded for the state to assert against.
        backend.answer_permission(7, PermissionDecision::Deny);
        backend.cancel_download(3);
        assert_eq!(backend.answers(), &[(7, PermissionDecision::Deny)]);
        assert_eq!(backend.cancelled_downloads(), &[3]);
    }
}
