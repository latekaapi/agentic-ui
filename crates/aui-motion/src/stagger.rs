//! Per-child delays for lists that enter together: approval buttons (40 ms),
//! suggestion chips (60 ms), timeline rows.

use std::time::Duration;

use gpui_kit::base::{Stagger, StaggerOrigin};

/// Default interval between children.
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(40);

/// The delay for child `index` of `count`, `interval` apart, counting from the
/// first child.
pub fn stagger_delay(index: usize, count: usize, interval: Duration) -> Duration {
    Stagger::new(interval, StaggerOrigin::First).delay(index, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_grow_linearly_from_the_first_child() {
        assert_eq!(stagger_delay(0, 3, DEFAULT_INTERVAL), Duration::ZERO);
        assert_eq!(stagger_delay(2, 3, DEFAULT_INTERVAL), Duration::from_millis(80));
        assert_eq!(stagger_delay(9, 3, DEFAULT_INTERVAL), Duration::from_millis(80));
    }
}
