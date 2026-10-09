//! Row and control metrics for the two densities.

use gpui::{px, Pixels};

use crate::generated::scale;

/// How tightly rows are packed. Standard is the design; compact trims the
/// size ladder the way a "compact" preference would, without changing the
/// shape tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// The values in the design cards.
    #[default]
    Standard,
    /// Controls, rows and bars at their compact values.
    Compact,
}

impl Density {
    /// The other density.
    pub fn toggled(self) -> Self {
        match self {
            Density::Standard => Density::Compact,
            Density::Compact => Density::Standard,
        }
    }

    /// Human-readable name.
    pub fn label(self) -> &'static str {
        match self {
            Density::Standard => "Standard",
            Density::Compact => "Compact",
        }
    }
}

/// Heights that depend on density. Everything else (spacing, radii, type)
/// comes straight from [`scale`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// The shared control height: buttons, rows, tab strips (36 / 28).
    pub control: Pixels,
    /// Two-line rows: sidebar rows, transcript rows (52 / 44).
    pub row_2: Pixels,
    /// Chrome bars: shell header, panel headers (44 / 36).
    pub bar: Pixels,
    /// Glyph box for 14/16 px icons (16 / 14).
    pub icon: Pixels,
    /// Control padding step (12 / 10).
    pub ctl_px: Pixels,
    /// Item padding step (8 / 6).
    pub item_px: Pixels,
    /// Gaps between siblings (8 / 4).
    pub gap: Pixels,
    /// Container insets (16 / 12).
    pub inset: Pixels,
    /// Deprecated v1 shell header cell. Use [`Metrics::bar`]: the header is
    /// the bar at the active density.
    pub header: Pixels,
    /// Deprecated v1 panel header. Always 36 (the bar at its compact value);
    /// use [`Metrics::bar`].
    pub panel_header: Pixels,
    /// Deprecated v1 tab strip. Use [`Metrics::control`].
    pub tab_strip: Pixels,
    /// Deprecated v1 card header. Always 28 (the control at its compact
    /// value); card headers migrate to [`Metrics::control`] in a later lane.
    pub card_header: Pixels,
    /// Deprecated v1 row. Use [`Metrics::control`].
    pub row: Pixels,
    /// Deprecated v1 compact row. Always 28 (the control at its compact
    /// value); use [`Metrics::control`] and read the density.
    pub row_sm: Pixels,
    /// Deprecated v1 extra-small control (20, unchanged).
    pub control_xs: Pixels,
    /// Deprecated v1 small control (24, unchanged).
    pub control_sm: Pixels,
    /// Deprecated v1 medium control (28, unchanged).
    pub control_md: Pixels,
    /// Deprecated v1 large control (32, unchanged).
    pub control_lg: Pixels,
    /// Deprecated v1 extra-large control (40, unchanged).
    pub control_xl: Pixels,
}

impl Metrics {
    /// Metrics for a density.
    pub fn for_density(density: Density) -> Self {
        let (control, row_2, bar, icon, ctl_px, item_px, gap, inset) = match density {
            Density::Standard => (
                scale::CTL_DEFAULT,
                scale::ROW_2_DEFAULT,
                scale::BAR_DEFAULT,
                scale::ICON_DEFAULT,
                scale::CTL_PX_DEFAULT,
                scale::ITEM_PX_DEFAULT,
                scale::GAP_DEFAULT,
                scale::INSET_DEFAULT,
            ),
            Density::Compact => (
                scale::CTL_COMPACT,
                scale::ROW_2_COMPACT,
                scale::BAR_COMPACT,
                scale::ICON_COMPACT,
                scale::CTL_PX_COMPACT,
                scale::ITEM_PX_COMPACT,
                scale::GAP_COMPACT,
                scale::INSET_COMPACT,
            ),
        };
        let trim = match density {
            Density::Standard => 0.0,
            Density::Compact => 4.0,
        };
        let small_trim = trim / 2.0;
        Self {
            control: px(control),
            row_2: px(row_2),
            bar: px(bar),
            icon: px(icon),
            ctl_px: px(ctl_px),
            item_px: px(item_px),
            gap: px(gap),
            inset: px(inset),
            header: px(bar),
            panel_header: px(scale::BAR_COMPACT),
            tab_strip: px(control),
            card_header: px(scale::CTL_COMPACT),
            row: px(control),
            row_sm: px(scale::CTL_COMPACT),
            control_xs: px(scale::H_XS - small_trim),
            control_sm: px(scale::H_SM - small_trim),
            control_md: px(scale::H_MD - small_trim),
            control_lg: px(scale::H_LG - trim),
            control_xl: px(scale::H_XL - trim),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_matches_design() {
        let m = Metrics::for_density(Density::Standard);
        assert_eq!(m.control, px(36.0));
        assert_eq!(m.row, px(36.0));
        assert_eq!(m.bar, px(44.0));
        assert_eq!(m.row_2, px(52.0));
        assert_eq!(m.header, px(44.0));
        assert_eq!(m.panel_header, px(36.0));
        assert_eq!(m.control_md, px(28.0));
    }

    #[test]
    fn compact_is_tighter() {
        let s = Metrics::for_density(Density::Standard);
        let c = Metrics::for_density(Density::Compact);
        assert!(c.row < s.row);
        assert!(c.header < s.header);
        assert_eq!(c.control, px(28.0));
        assert_eq!(c.card_header, px(28.0));
        assert_eq!(c.row_sm, px(28.0));
    }
}
