//! Row and control metrics for the two densities.

use gpui::{px, Pixels};

use crate::generated::scale;

/// How tightly rows are packed (Settings › Appearance › Density).
/// Comfortable is the design (docs/83 §2.4–2.5); Compact is the type and
/// row height Baaz drew before task 2.10 — every text role at its `-c`
/// size and the shared row/control at 28 instead of 32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// The design's values (docs/83 §2.4–2.5).
    #[default]
    Comfortable,
    /// Today's (pre-2.10) type and 28 px rows.
    Compact,
}

impl Density {
    /// The other density.
    pub fn toggled(self) -> Self {
        match self {
            Density::Comfortable => Density::Compact,
            Density::Compact => Density::Comfortable,
        }
    }

    /// Human-readable name.
    pub fn label(self) -> &'static str {
        match self {
            Density::Comfortable => "Comfortable",
            Density::Compact => "Compact",
        }
    }

    /// The value stored in settings (`"comfortable"` / `"compact"`).
    pub fn key(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }

    /// Parses [`Density::key`].
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "comfortable" => Some(Density::Comfortable),
            "compact" => Some(Density::Compact),
            _ => None,
        }
    }
}

/// Heights that depend on density. Everything else (spacing, radii) comes
/// straight from [`scale`]; type comes from [`crate::TextRole`].
///
/// Only the shared control/row height and the two-line row change with
/// density: the 44 px bar keeps the three header lines on one y in both,
/// and spacing, icons and the component heights in `tokens.json`
/// (`menu-row`, `tree-row`, `card-head`, `sub-header`, `palette-row`,
/// `btn-h`) are the same in both.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// The shared row and control height: list rows, tabs (32 / 28).
    pub control: Pixels,
    /// Two-line rows (52 / 44).
    pub row_2: Pixels,
    /// Chrome bars: every column header (44 in both densities).
    pub bar: Pixels,
    /// Glyph box for icons (16).
    pub icon: Pixels,
    /// Control padding step (12).
    pub ctl_px: Pixels,
    /// Item padding step (8).
    pub item_px: Pixels,
    /// Gaps between siblings (8).
    pub gap: Pixels,
    /// Container insets (16).
    pub inset: Pixels,
    /// Deprecated v1 shell header cell: the bar.
    pub header: Pixels,
    /// Deprecated v1 panel header: the 36 px sub-header (`sub-header`).
    pub panel_header: Pixels,
    /// Deprecated v1 tab strip: the control.
    pub tab_strip: Pixels,
    /// Deprecated v1 card header (28, `ctl-compact`).
    pub card_header: Pixels,
    /// Deprecated v1 row: the control.
    pub row: Pixels,
    /// Deprecated v1 compact row (28, `ctl-compact`).
    pub row_sm: Pixels,
    /// In-row control, extra small (20, `h-xs`).
    pub control_xs: Pixels,
    /// In-row control, small (24, `h-sm`).
    pub control_sm: Pixels,
    /// In-row control, medium (28, `h-md`).
    pub control_md: Pixels,
    /// The list row: the control (32 / 28).
    pub control_lg: Pixels,
    /// Large control (40, `h-xl`).
    pub control_xl: Pixels,
}

impl Metrics {
    /// Metrics for a density.
    pub fn for_density(density: Density) -> Self {
        let (control, row_2) = match density {
            Density::Comfortable => (scale::CTL_DEFAULT, scale::ROW_2_DEFAULT),
            Density::Compact => (scale::CTL_COMPACT, scale::ROW_2_COMPACT),
        };
        Self {
            control: px(control),
            row_2: px(row_2),
            bar: px(scale::BAR_DEFAULT),
            icon: px(scale::ICON_DEFAULT),
            ctl_px: px(scale::CTL_PX_DEFAULT),
            item_px: px(scale::ITEM_PX_DEFAULT),
            gap: px(scale::GAP_DEFAULT),
            inset: px(scale::INSET_DEFAULT),
            header: px(scale::BAR_DEFAULT),
            panel_header: px(scale::SUB_HEADER),
            tab_strip: px(control),
            card_header: px(scale::CTL_COMPACT),
            row: px(control),
            row_sm: px(scale::CTL_COMPACT),
            control_xs: px(scale::H_XS),
            control_sm: px(scale::H_SM),
            control_md: px(scale::H_MD),
            control_lg: px(control),
            control_xl: px(scale::H_XL),
        }
    }

    /// The metrics of the active density (what [`crate::AuiTheme::set_density`]
    /// last set), for code that holds no `App`.
    pub fn active() -> Self {
        Self::for_density(crate::styled::active_density())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_matches_design() {
        // docs/83 §2.5: rows 32, bars 44, two-line rows 52.
        let m = Metrics::for_density(Density::Comfortable);
        assert_eq!(m.control, px(32.0));
        assert_eq!(m.row, px(32.0));
        assert_eq!(m.control_lg, px(32.0));
        assert_eq!(m.bar, px(44.0));
        assert_eq!(m.row_2, px(52.0));
        assert_eq!(m.header, px(44.0));
        assert_eq!(m.panel_header, px(36.0));
        assert_eq!(m.control_md, px(28.0));
    }

    #[test]
    fn compact_is_tighter() {
        let s = Metrics::for_density(Density::Comfortable);
        let c = Metrics::for_density(Density::Compact);
        assert!(c.row < s.row);
        // The bar never changes: the three header lines share one y.
        assert_eq!(c.header, s.header);
        assert_eq!(c.control_lg, px(28.0));
        assert_eq!(Density::parse(Density::Compact.key()), Some(Density::Compact));
        assert_eq!(c.control, px(28.0));
        assert_eq!(c.card_header, px(28.0));
        assert_eq!(c.row_sm, px(28.0));
    }
}
