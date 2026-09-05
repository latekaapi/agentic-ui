//! The asset source an `aui` application installs: the design's own glyphs
//! (`icons/aui/*.svg`) layered over gpui-kit's Lucide set (`icons/*.svg`).

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

/// Serves `aui-icons` first, then gpui-kit's bundled icons.
///
/// ```ignore
/// gpui_kit::application().with_assets(aui::assets::AuiAssets)
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct AuiAssets;

impl AssetSource for AuiAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = aui_icons::Assets.load(path)? {
            return Ok(Some(bytes));
        }
        // gpui-kit reports a missing asset as an error; a miss is not an error here.
        Ok(gpui_kit::assets::Assets.load(path).ok().flatten())
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut out = aui_icons::Assets.list(path)?;
        out.extend(gpui_kit::assets::Assets.list(path).unwrap_or_default());
        Ok(out)
    }
}
