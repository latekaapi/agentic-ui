//! Row and control metrics for the two densities.

use gpui::{px, Pixels};

use crate::generated::scale;

/// How tightly rows are packed. Standard is the design; compact trims the row
/// and header heights the way a "compact" preference would, without changing
/// type sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// The values in the design cards.
    #[default]
    Standard,
    /// Rows 4 px shorter, headers 4 px shorter.
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
    /// Shell header cells (44 in the design).
    pub header: Pixels,
    /// Panel header / floating panel chrome (36).
    pub panel_header: Pixels,
    /// Tab strips outside the shell header (34).
    pub tab_strip: Pixels,
    /// Tool-card and thinking headers (34).
    pub card_header: Pixels,
    /// Sidebar rows, timeline rows, project rows (30).
    pub row: Pixels,
    /// Compact rows: file tree rows, file change rows (26).
    pub row_sm: Pixels,
    /// Extra-small control (20).
    pub control_xs: Pixels,
    /// Small control (24).
    pub control_sm: Pixels,
    /// Medium control — buttons, composer toolbar (28).
    pub control_md: Pixels,
    /// Large control (32).
    pub control_lg: Pixels,
    /// Extra-large control — dialog fields (40).
    pub control_xl: Pixels,
}

impl Metrics {
    /// Metrics for a density.
    pub fn for_density(density: Density) -> Self {
        let trim = match density {
            Density::Standard => 0.0,
            Density::Compact => 4.0,
        };
        let small_trim = trim / 2.0;
        Self {
            header: px(44.0 - trim),
            panel_header: px(36.0 - trim),
            tab_strip: px(34.0 - trim),
            card_header: px(34.0 - trim),
            row: px(30.0 - trim),
            row_sm: px(26.0 - small_trim),
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
        assert_eq!(m.header, px(44.0));
        assert_eq!(m.row, px(30.0));
        assert_eq!(m.control_md, px(28.0));
    }

    #[test]
    fn compact_is_tighter() {
        let s = Metrics::for_density(Density::Standard);
        let c = Metrics::for_density(Density::Compact);
        assert!(c.row < s.row);
        assert!(c.header < s.header);
    }
}
