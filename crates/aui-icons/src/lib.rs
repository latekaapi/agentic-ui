//! Icon glyphs, provider marks and file-type icon mapping for the Agentic UI library.
//!
//! The glyph set is generated at build time from the design system's sprite
//! sheet (`design/src/sprite.svg`). Every `<symbol>` becomes a variant of
//! [`IconName`] and a standalone SVG file baked into the binary; [`Assets`]
//! serves those files to gpui under `icons/aui/<id>.svg`.
//!
//! ```ignore
//! use aui_icons::{icon, IconName};
//!
//! icon(IconName::ChevronDown).lg().color(theme.ink_3)
//! ```
//!
//! Because gpui rasterises an SVG into an alpha mask that is then tinted with
//! the element's text colour, the generated files carry explicit `stroke` and
//! `fill` presentation attributes (stroke `1.6`, round caps and joins, matching
//! the `.i` rule in `design/src/base.css`) instead of CSS classes, which gpui
//! ignores. An [`Icon`] with no explicit colour inherits the surrounding text
//! colour, which is the equivalent of `currentColor`.
//!
//! The module also carries the two non-glyph icon families from the spec:
//! [`ProviderMark`] (the rounded-square agent letters) and [`FileType`] (the
//! `fic` file-type icons and their muted hues).

#![deny(missing_docs)]

use std::borrow::Cow;

use gpui::{div, prelude::*, px, rgb, svg, App, Hsla, Pixels, Radians, Rgba, SharedString, Window};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

/// The default glyph size: 14 px, per spec section 0.5.
pub const ICON_SIZE: Pixels = px(14.0);

/// The larger glyph size used in headers: 16 px.
pub const ICON_SIZE_LG: Pixels = px(16.0);

/// The default provider mark size: 16 px.
pub const MARK_SIZE: Pixels = px(16.0);

/// A [`gpui::AssetSource`] that serves the bundled Agentic UI icons.
///
/// Paths are of the form `icons/aui/<sprite id>.svg`, which is exactly what
/// [`IconName::path`] returns. Compose this with `gpui-kit`'s own asset source
/// in the application's `AssetSource` implementation so that both icon sets
/// resolve.
#[derive(Debug, Clone, Copy, Default)]
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(lookup(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(IconName::ALL
            .iter()
            .map(|name| name.path())
            .filter(|candidate| candidate.starts_with(path))
            .map(SharedString::new_static)
            .collect())
    }
}

/// A single stroked glyph from the sprite sheet.
///
/// Built with [`icon`]. Defaults to 14 px square, `flex_none`, and the
/// surrounding text colour.
#[derive(Debug, Clone, IntoElement)]
pub struct Icon {
    name: IconName,
    size: Pixels,
    color: Option<Hsla>,
    rotation: Option<Radians>,
}

/// Create an [`Icon`] element for the given glyph.
pub fn icon(name: IconName) -> Icon {
    Icon {
        name,
        size: ICON_SIZE,
        color: None,
        rotation: None,
    }
}

impl Icon {
    /// Use the 16 px header size.
    pub fn lg(mut self) -> Self {
        self.size = ICON_SIZE_LG;
        self
    }

    /// Override the square size of the glyph.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// Tint the glyph. Without this the glyph inherits the parent's text colour.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Rotate the glyph about its centre, in radians.
    ///
    /// Rotation affects painting only: the element keeps its square layout box
    /// and hitbox, which is what chevron and spinner rotations want.
    pub fn rotate(mut self, radians: impl Into<Radians>) -> Self {
        self.rotation = Some(radians.into());
        self
    }

    /// The glyph this icon renders.
    pub fn name(&self) -> IconName {
        self.name
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or_else(|| window.text_style().color);
        let mut element = svg()
            .path(self.name.path())
            .size(self.size)
            .flex_none()
            .text_color(color);
        if let Some(rotation) = self.rotation {
            element = element.with_transformation(gpui::Transformation::rotate(rotation));
        }
        element
    }
}

/// The agent providers the harness shows a mark for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    /// Claude Code.
    Claude,
    /// OpenAI Codex.
    Codex,
    /// xAI Grok.
    Grok,
    /// Google Gemini.
    Gemini,
    /// Inflection Pi.
    Pi,
    /// Cursor.
    Cursor,
    /// Meta's Muse Code.
    Muse,
}

impl Provider {
    /// Every provider, in the order the design cards list them.
    pub const ALL: &'static [Provider] = &[
        Provider::Claude,
        Provider::Codex,
        Provider::Grok,
        Provider::Gemini,
        Provider::Pi,
        Provider::Cursor,
        Provider::Muse,
    ];

    /// The glyph shown inside the mark, as used by `design/src/cards/foundations/05-icons.html`.
    pub fn letter(self) -> &'static str {
        match self {
            Provider::Claude => "C",
            Provider::Codex => "O",
            Provider::Grok => "X",
            Provider::Gemini => "G",
            Provider::Pi => "π",
            Provider::Cursor => "▮",
            Provider::Muse => "M",
        }
    }

    /// The mark's background colour, from the `.mark.*` rules in `design/src/base.css`.
    pub fn color(self) -> Rgba {
        match self {
            Provider::Claude => rgb(0xC9714F),
            Provider::Codex => rgb(0x1F9A7A),
            Provider::Grok => rgb(0x2A2D33),
            Provider::Gemini => rgb(0x4C86D9),
            Provider::Pi => rgb(0x7460D9),
            Provider::Cursor => rgb(0x3A3D44),
            // Meta blue, the one mark that identifies the agent this harness
            // drives; every other sidebar glyph stays muted line work.
            Provider::Muse => rgb(0x0064E0),
        }
    }
}

/// The rounded-square provider mark (`.mark`).
///
/// A square of the provider's colour with the provider's letter in white at
/// weight 700, radius 4 and font size 9 at the default 16 px, both scaled
/// proportionally by [`ProviderMark::size`]. The font family is inherited from
/// the parent so the mark follows the app's UI font.
#[derive(Debug, Clone, IntoElement)]
pub struct ProviderMark {
    provider: Provider,
    size: Pixels,
}

/// Create a [`ProviderMark`] at the default 16 px size.
pub fn provider_mark(provider: Provider) -> ProviderMark {
    ProviderMark {
        provider,
        size: MARK_SIZE,
    }
}

impl ProviderMark {
    /// Override the square size. The spec uses 16 (default), 13, 12 and 11 px.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// The provider this mark stands for.
    pub fn provider(&self) -> Provider {
        self.provider
    }
}

impl RenderOnce for ProviderMark {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let scale = f32::from(self.size) / f32::from(MARK_SIZE);
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded(px(4.0 * scale))
            .bg(self.provider.color())
            .text_color(gpui::white())
            .text_size(px(9.0 * scale))
            .font_weight(gpui::FontWeight::BOLD)
            .line_height(self.size)
            .opacity(0.92)
            .child(self.provider.letter())
    }
}

/// The file-type icon family (`fic`) used by trees, mentions and diff headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    /// A closed directory.
    Folder,
    /// An expanded directory.
    FolderOpen,
    /// A TypeScript source file.
    Ts,
    /// A TypeScript JSX source file.
    Tsx,
    /// A JSON document.
    Json,
    /// A Markdown document.
    Md,
    /// A test or spec file.
    Test,
    /// A stylesheet.
    Css,
    /// A lockfile.
    Lock,
    /// A raster or vector image.
    Image,
    /// Anything else.
    File,
}

impl FileType {
    /// Every file type, in spec order.
    pub const ALL: &'static [FileType] = &[
        FileType::Folder,
        FileType::FolderOpen,
        FileType::Ts,
        FileType::Tsx,
        FileType::Json,
        FileType::Md,
        FileType::Test,
        FileType::Css,
        FileType::Lock,
        FileType::Image,
        FileType::File,
    ];

    /// The `ft-*` glyph for this file type.
    pub fn icon(self) -> IconName {
        match self {
            FileType::Folder => IconName::FtFolder,
            FileType::FolderOpen => IconName::FtFolderOpen,
            FileType::Ts => IconName::FtTs,
            FileType::Tsx => IconName::FtTsx,
            FileType::Json => IconName::FtJson,
            FileType::Md => IconName::FtMd,
            FileType::Test => IconName::FtTest,
            FileType::Css => IconName::FtCss,
            FileType::Lock => IconName::FtLock,
            FileType::Image => IconName::FtImage,
            FileType::File => IconName::FtFile,
        }
    }

    /// The muted hue this file type is tinted with.
    ///
    /// `None` means the icon carries no hue of its own: folders use `ink-3`,
    /// lockfiles, images and generic files use `ink-4` / `ink-3` as the caller
    /// sees fit. The library does not know the theme, so it hands back only the
    /// hues the spec fixes.
    pub fn hue(self) -> Option<Rgba> {
        match self {
            FileType::Ts => Some(rgb(0x5B8DD6)),
            FileType::Tsx => Some(rgb(0x4FA8B8)),
            FileType::Json => Some(rgb(0xC9A15A)),
            FileType::Md => Some(rgb(0x7C8AA5)),
            FileType::Test => Some(rgb(0x8C9C6E)),
            FileType::Css => Some(rgb(0x8E79C9)),
            FileType::Folder
            | FileType::FolderOpen
            | FileType::Lock
            | FileType::Image
            | FileType::File => None,
        }
    }

    /// Classify a path by its file name.
    ///
    /// Directories are never inferred — the caller knows whether an entry is a
    /// directory and picks [`FileType::Folder`] or [`FileType::FolderOpen`]
    /// itself. Test and lockfile patterns win over the plain extension, so
    /// `api.test.ts` is [`FileType::Test`] and `package-lock.json` is
    /// [`FileType::Lock`].
    pub fn from_path(path: &str) -> FileType {
        let name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_ascii_lowercase();

        if name.ends_with(".lock") || name == "package-lock.json" {
            return FileType::Lock;
        }
        if name.contains(".test.") || name.contains(".spec.") {
            return FileType::Test;
        }

        match name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("") {
            "ts" | "mts" | "cts" => FileType::Ts,
            "tsx" => FileType::Tsx,
            "json" | "jsonc" => FileType::Json,
            "md" | "mdx" => FileType::Md,
            "css" | "scss" | "sass" | "less" => FileType::Css,
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => FileType::Image,
            _ => FileType::File,
        }
    }
}

/// The role icons used by the assistant sidebar's role headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoleIcon {
    /// Study / learning roles.
    GradCap,
    /// Legal / review roles.
    Scale,
    /// Operations / settings roles.
    Gear,
}

impl RoleIcon {
    /// Every role icon.
    pub const ALL: &'static [RoleIcon] = &[RoleIcon::GradCap, RoleIcon::Scale, RoleIcon::Gear];

    /// The glyph for this role.
    pub fn icon(self) -> IconName {
        match self {
            RoleIcon::GradCap => IconName::GradCap,
            RoleIcon::Scale => IconName::Scale,
            RoleIcon::Gear => IconName::Gear,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The number of `<symbol>` elements in `design/src/sprite.svg`.
    const SPRITE_SYMBOL_COUNT: usize = 66;

    #[test]
    fn every_sprite_symbol_has_a_variant() {
        assert_eq!(IconName::ALL.len(), SPRITE_SYMBOL_COUNT);

        let sprite = include_str!("../../../design/src/sprite.svg");
        let ids: Vec<&str> = sprite
            .match_indices("<symbol id=\"")
            .map(|(at, marker)| {
                let rest = &sprite[at + marker.len()..];
                &rest[..rest.find('"').expect("closing quote on symbol id")]
            })
            .collect();
        assert_eq!(ids.len(), SPRITE_SYMBOL_COUNT);

        for id in ids {
            let name = IconName::ALL
                .iter()
                .find(|name| name.id() == id)
                .unwrap_or_else(|| panic!("no IconName variant for sprite id `{id}`"));
            assert_eq!(name.path(), format!("icons/aui/{id}.svg"));
        }
    }

    #[test]
    fn ids_round_trip_through_lookup() {
        for name in IconName::ALL {
            let bytes = lookup(name.path()).unwrap_or_else(|| {
                panic!("lookup failed for {}", name.path());
            });
            assert_eq!(bytes, name.bytes());
        }
        assert!(lookup("icons/aui/not-a-glyph.svg").is_none());
        assert!(lookup("icons/kit/file.svg").is_none());
    }

    #[test]
    fn generated_svgs_are_standalone_and_stroked() {
        for name in IconName::ALL {
            let svg = std::str::from_utf8(name.bytes()).expect("generated SVG is UTF-8");
            assert!(svg.starts_with("<svg"), "{} is not standalone", name.id());
            assert!(svg.ends_with("</svg>"), "{} is unterminated", name.id());
            assert!(
                svg.contains(r##"stroke="#000""##),
                "{} has no explicit stroke",
                name.id()
            );
            assert!(
                !svg.contains("currentColor"),
                "{} still uses currentColor",
                name.id()
            );
        }
    }

    #[test]
    fn asset_source_serves_and_lists_icons() {
        use gpui::AssetSource as _;

        let assets = Assets;
        assert!(assets
            .load(IconName::ChevronDown.path())
            .expect("load succeeds")
            .is_some());
        assert!(assets
            .load("icons/aui/nope.svg")
            .expect("load succeeds")
            .is_none());
        assert_eq!(
            assets.list("icons/aui/").expect("list succeeds").len(),
            SPRITE_SYMBOL_COUNT
        );
        assert_eq!(assets.list("icons/kit/").expect("list succeeds").len(), 0);
    }

    #[test]
    fn file_type_from_path() {
        use FileType::*;
        for (path, expected) in [
            ("src/app.ts", Ts),
            ("src/App.TSX", Tsx),
            ("tsconfig.json", Json),
            ("docs/README.md", Md),
            ("src/api.test.ts", Test),
            ("src/api.spec.tsx", Test),
            ("styles/base.css", Css),
            ("pnpm-lock.yaml", File),
            ("Cargo.lock", Lock),
            ("package-lock.json", Lock),
            ("design/src/sprite.svg", Image),
            ("shots/hero.PNG", Image),
            ("Makefile", File),
            ("", File),
        ] {
            assert_eq!(FileType::from_path(path), expected, "path `{path}`");
        }
    }

    #[test]
    fn file_type_hues_match_the_spec() {
        assert_eq!(FileType::Ts.hue(), Some(rgb(0x5B8DD6)));
        assert_eq!(FileType::Css.hue(), Some(rgb(0x8E79C9)));
        assert!(FileType::Folder.hue().is_none());
        assert!(FileType::Lock.hue().is_none());
        assert_eq!(FileType::Tsx.icon(), IconName::FtTsx);
    }

    #[test]
    fn providers_and_roles_map_to_the_design() {
        assert_eq!(Provider::Claude.letter(), "C");
        assert_eq!(Provider::Claude.color(), rgb(0xC9714F));
        assert_eq!(Provider::ALL.len(), 7);
        assert_eq!(RoleIcon::Scale.icon(), IconName::Scale);
    }

    #[test]
    fn typescript_extensions_all_land_on_ts() {
        use FileType::*;
        for (path, expected) in [
            ("src/app.mts", Ts),
            ("src/app.cts", Ts),
            ("src/app.tsx", Tsx),
            ("tsconfig.jsonc", Json),
            ("notes.mdx", Md),
            ("theme.scss", Css),
            ("theme.sass", Css),
            ("theme.less", Css),
            ("hero.jpeg", Image),
            ("hero.webp", Image),
            ("anim.gif", Image),
        ] {
            assert_eq!(FileType::from_path(path), expected, "path `{path}`");
        }
    }

    #[test]
    fn a_directory_prefix_never_changes_the_type() {
        assert_eq!(FileType::from_path("a/b/c/x.spec.tsx"), FileType::Test);
        assert_eq!(FileType::from_path("app/package-lock.json"), FileType::Lock);
        // Windows separators too: only the last segment is the file name.
        assert_eq!(FileType::from_path(r"src\ui\app.ts"), FileType::Ts);
        assert_eq!(FileType::from_path(r"C:\repo\Cargo.lock"), FileType::Lock);
        // A dot in a directory name is not the file's extension.
        assert_eq!(FileType::from_path("v1.2/README"), FileType::File);
    }

    #[test]
    fn test_and_lock_patterns_win_over_the_extension() {
        assert_eq!(FileType::from_path("api.test.ts"), FileType::Test);
        assert_eq!(FileType::from_path("api.spec.json"), FileType::Test);
        assert_eq!(FileType::from_path("Api.Test.TS"), FileType::Test);
        assert_eq!(FileType::from_path("deps.lock"), FileType::Lock);
        // `test` has to be its own dotted segment.
        assert_eq!(FileType::from_path("latest.ts"), FileType::Ts);
    }

    #[test]
    fn names_without_a_usable_extension_are_generic_files() {
        for path in ["", "Makefile", "LICENSE", ".gitignore", "archive.tar.zzz", "src/"] {
            assert_eq!(FileType::from_path(path), FileType::File, "path `{path}`");
        }
    }

    #[test]
    fn from_path_never_infers_a_directory() {
        for path in ["src", "src/", "folder.ts", "node_modules"] {
            let ft = FileType::from_path(path);
            assert!(ft != FileType::Folder && ft != FileType::FolderOpen, "path `{path}` inferred {ft:?}");
        }
    }

    #[test]
    fn every_file_type_has_a_distinct_glyph_and_a_defined_hue() {
        let glyphs: std::collections::HashSet<IconName> = FileType::ALL.iter().map(|ft| ft.icon()).collect();
        assert_eq!(glyphs.len(), FileType::ALL.len(), "two file types share a glyph");

        let hued: Vec<FileType> = FileType::ALL.iter().copied().filter(|ft| ft.hue().is_some()).collect();
        assert_eq!(hued, vec![FileType::Ts, FileType::Tsx, FileType::Json, FileType::Md, FileType::Test, FileType::Css]);
    }
}
