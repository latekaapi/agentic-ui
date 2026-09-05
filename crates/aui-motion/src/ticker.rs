//! Number ticker: counts, percentages and costs glide to their new value
//! over the base duration instead of jumping.

use gpui::{App, Window};
use gpui_kit::base::TransitionId;

use crate::tween::{tween, Tween};

/// The displayed value for `target`, easing from the previous value.
pub fn number_ticker(id: impl Into<TransitionId>, target: f32, window: &mut Window, cx: &mut App) -> f32 {
    tween(id, target, Tween::BASE, window, cx)
}
