//! The bundled Inter (Fluid Functionalism's face, `fonts/Inter-OFL.txt`) and
//! Geist Mono (`fonts/OFL.txt`) faces, both SIL Open Font License.

use std::borrow::Cow;

use gpui::App;

#[derive(rust_embed::RustEmbed)]
#[folder = "fonts"]
#[include = "*.ttf"]
struct Fonts;

/// File names of the embedded faces (Regular, Medium, SemiBold, Bold and the
/// two italics for Inter; Regular, Medium, SemiBold, Bold for Geist Mono).
/// Inter is static: gpui has no variable-axis support, so FF's 550 weight
/// resolves to the nearest static face.
pub const FONT_FILES: &[&str] = &[
    "Inter-Regular.ttf",
    "Inter-Medium.ttf",
    "Inter-SemiBold.ttf",
    "Inter-Bold.ttf",
    "Inter-Italic.ttf",
    "Inter-MediumItalic.ttf",
    "GeistMono-Regular.ttf",
    "GeistMono-Medium.ttf",
    "GeistMono-SemiBold.ttf",
    "GeistMono-Bold.ttf",
];

/// Registers the embedded faces with gpui's text system so that the families
/// `Inter` and `Geist Mono` resolve without a system install.
pub fn load_fonts(cx: &App) -> anyhow::Result<()> {
    let mut data: Vec<Cow<'static, [u8]>> = Vec::with_capacity(FONT_FILES.len());
    for name in FONT_FILES {
        let file = Fonts::get(name).ok_or_else(|| anyhow::anyhow!("missing embedded font {name}"))?;
        data.push(file.data);
    }
    cx.text_system().add_fonts(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_font_is_embedded() {
        for name in FONT_FILES {
            let f = Fonts::get(name).expect(name);
            assert!(f.data.len() > 10_000, "{name} looks truncated");
            // TrueType magic
            assert_eq!(&f.data[..4], &[0, 1, 0, 0], "{name} is not a TrueType file");
        }
    }
}
