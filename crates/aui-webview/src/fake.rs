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
use crate::backend::{ElementInfo, WebBackend, WebEvent};
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
    fn quoted_selectors_round_trip_through_the_script() {
        let mut backend = FakeWebBackend::new();
        // A selector the page does not have, but with quotes that must survive
        // the trip into the script and back out to the error message intact.
        let selector = "div[data-x=\"a'b\"]";
        let (_, answer) = ask(&mut backend, 9, &agent_js::click(selector));
        assert_eq!(answer, Err(format!("element not found: {selector}")));
    }
}
