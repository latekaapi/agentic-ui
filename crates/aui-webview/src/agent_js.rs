//! Ready-made scripts a host can send through
//! [`WebBackend::eval_with_result`](crate::backend::WebBackend::eval_with_result)
//! so an agent can read the page and act on it.
//!
//! Each function returns a self-contained snippet: an immediately-invoked
//! function whose completion value is what the backend JSON-stringifies into
//! the [`Ok`] of
//! [`take_eval_results`](crate::backend::WebBackend::take_eval_results). A
//! thrown error — an unknown selector, for instance — becomes the [`Err`]
//! instead (on the real backend via the wrapper in
//! [`crate::wry_backend`]; on the mock by emulation in
//! [`crate::fake`]).
//!
//! # Injection safety
//!
//! Selectors and text are embedded with [`json_string`] and never
//! string-concatenated raw, so a selector containing quotes or `</script>`
//! stays inside its string literal. The `/*aui-agent-…*/` comments are the
//! markers [`crate::fake`] parses to emulate the script; they carry only a
//! number (`page_text`'s limit), never page data.

/// Marks a [`page_text`] script; the number after the colon is its limit.
pub const PAGE_TEXT_MARKER: &str = "aui-agent-page-text";
/// Marks a [`click`] script.
pub const CLICK_MARKER: &str = "aui-agent-click";
/// Marks a [`type_text`] script.
pub const TYPE_MARKER: &str = "aui-agent-type";
/// Marks a [`list_links`] script.
pub const LINKS_MARKER: &str = "aui-agent-links";

/// Encodes `value` as a JSON string literal, quotes included.
///
/// This is the only way user input enters a script below. Control characters
/// are escaped, so a value cannot close its literal early; `<`, `>` and `/`
/// pass through untouched, which is safe because the scripts run through
/// `evaluateScript` rather than HTML parsing, where `</script>` would matter.
pub fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Reads the page: `{title, url, text}`, with `innerText` cut to `max_chars`
/// characters. Answer with the backend's result `Ok`, parsed as JSON.
pub fn page_text(max_chars: usize) -> String {
    format!(
        "return (function () {{ /*{PAGE_TEXT_MARKER}:{max_chars}*/ var max = {max_chars}; var text = (document.body && document.body.innerText) || \"\"; \
         return {{ title: document.title, url: location.href, text: text.length > max ? text.slice(0, max) : text }}; }})()"
    )
}

/// Clicks the first match of `selector`: scrolls it into view, clicks it, and
/// answers with the element's text. Throws when nothing matches, which the
/// backend reports as `Err`.
///
/// The selector is bound in the first string literal after the marker, which
/// is the contract [`crate::fake`] parses to emulate the click.
pub fn click(selector: &str) -> String {
    let sel = json_string(selector);
    format!(
        "return (function () {{ /*{CLICK_MARKER}*/ var sel = {sel}; var el = document.querySelector(sel); \
         if (!el) {{ throw new Error(\"element not found: \" + sel); }} \
         el.scrollIntoView({{ block: \"center\" }}); el.click(); return el.innerText || \"\"; }})()"
    )
}

/// Types `text` into the first match of `selector`: focuses it, sets the
/// value, and dispatches `input` and `change` so page listeners run. Answers
/// with the text that was set. Throws when nothing matches.
///
/// The selector and the text are the first two string literals after the
/// marker, which is the contract [`crate::fake`] parses to emulate the edit.
pub fn type_text(selector: &str, text: &str) -> String {
    let sel = json_string(selector);
    let value = json_string(text);
    format!(
        "return (function () {{ /*{TYPE_MARKER}*/ var sel = {sel}; var value = {value}; var el = document.querySelector(sel); \
         if (!el) {{ throw new Error(\"element not found: \" + sel); }} \
         el.focus(); \
         if (\"value\" in el) {{ el.value = value; }} else {{ el.textContent = value; }} \
         el.dispatchEvent(new Event(\"input\", {{ bubbles: true }})); \
         el.dispatchEvent(new Event(\"change\", {{ bubbles: true }})); return value; }})()"
    )
}

/// Lists up to `max` links as `[{href, text}]`.
pub fn list_links(max: usize) -> String {
    format!(
        "return (function () {{ /*{LINKS_MARKER}*/ var links = Array.prototype.slice.call(document.querySelectorAll(\"a[href]\"), 0, {max}); \
         return links.map(function (a) {{ return {{ href: a.href, text: (a.innerText || \"\").trim() }}; }}); }})()"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile selector: quotes that could close the literal, and markup
    /// that could break out of a script element.
    const NASTY: &str = "div[data-x=\"a'b\"]</script><script>alert(1)</script>";

    #[test]
    fn json_string_keeps_quotes_and_controls_inside_the_literal() {
        let encoded = json_string("say \"hi\"\nbye\\");
        assert_eq!(encoded, "\"say \\\"hi\\\"\\nbye\\\\\"");
        assert_eq!(json_string(NASTY).chars().next(), Some('"'));
    }

    #[test]
    fn a_hostile_selector_cannot_break_out_of_click() {
        let script = click(NASTY);
        // The raw selector never appears: its quotes are escaped, so it
        // cannot close the literal it sits in.
        assert!(!script.contains(NASTY), "raw selector leaked into {script}");
        // …while its encoded form — the literal — is exactly what is sent.
        assert!(script.contains(&json_string(NASTY)));
        assert!(script.contains(CLICK_MARKER));
    }

    #[test]
    fn a_hostile_selector_and_text_cannot_break_out_of_type_text() {
        let script = type_text(NASTY, "a\") + alert(1) + (\"");
        assert!(!script.contains(NASTY), "raw selector leaked into {script}");
        assert!(script.contains(&json_string(NASTY)));
        assert!(script.contains(&json_string("a\") + alert(1) + (\"")));
        assert!(script.contains(TYPE_MARKER));
        assert!(script.contains("dispatchEvent(new Event(\"input\""));
        assert!(script.contains("dispatchEvent(new Event(\"change\""));
    }

    #[test]
    fn page_text_carries_its_limit_and_reads_title_url_and_text() {
        let script = page_text(2000);
        assert!(script.contains("aui-agent-page-text:2000"));
        assert!(script.contains("document.title"));
        assert!(script.contains("location.href"));
        assert!(script.contains("document.body.innerText"));
    }

    #[test]
    fn list_links_carries_its_limit() {
        let script = list_links(25);
        assert!(script.contains(LINKS_MARKER));
        assert!(script.contains("a[href]"));
        assert!(script.contains(", 0, 25)"));
    }
}
