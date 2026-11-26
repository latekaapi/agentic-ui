//! Settings › Usage (harness docs/85 16.1, rule 30: the mock before the
//! build): three providers' limits — two windows each with their resets,
//! one not reported yet — and thirty days of the ledger split by model.

use aui::screens::{usage_page, UsageLimits, UsageLine, UsagePageModel, UsageWindow};
use aui_icons::Provider;
use gpui::{prelude::*, AnyElement, App, Window};

fn line(key: &str, detail: &str, cost: &str) -> UsageLine {
    UsageLine { key: key.to_owned().into(), detail: detail.to_owned().into(), cost: cost.to_owned().into() }
}

/// `usage/page`.
pub fn build_page(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let model = UsagePageModel {
        limits: vec![
            UsageLimits {
                provider: Provider::Claude,
                plan: Some("Max".into()),
                windows: vec![UsageWindow::new("5-hour", 0.42, "2h 10m"), UsageWindow::new("Weekly", 0.18, "4d 11h")],
                as_of: Some("2m ago".into()),
            },
            UsageLimits {
                provider: Provider::Codex,
                plan: Some("Pro".into()),
                windows: vec![UsageWindow::new("5-hour", 0.86, "34m"), UsageWindow::new("Weekly", 0.51, "2d 6h")],
                as_of: Some("1m ago".into()),
            },
            // Two providers: the row the owner saw with a gap (harness
            // docs/87 18.11); the cards share it equally.
        ],
        range: 1,
        split: 1,
        lines: vec![
            line("claude-sonnet-5-5", "4.8M tokens \u{b7} 71% cached \u{b7} 212 turns", "included"),
            line("gpt-6-astra", "2.1M tokens \u{b7} 88% cached \u{b7} 96 turns", "included"),
            line("muse-spark-1.3", "640k tokens \u{b7} 40% cached \u{b7} 31 turns", "included"),
            line("claude-haiku-5-5", "210k tokens \u{b7} 12% cached \u{b7} 44 turns", "$0.31"),
        ],
        total: "7.8M tokens, 383 turns, $0.31 billed in the last 30 days".into(),
    };
    usage_page("usage-page", model).into_any_element()
}
