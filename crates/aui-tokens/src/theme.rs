//! The active token set as a gpui global, plus theme switching.

use gpui::{App, Global, Window};
use gpui_kit::component::{Theme, ThemeMode, ThemeRegistry};

use crate::{
    density::{Density, Metrics},
    fonts::load_fonts,
    generated::{dark, light, scale, Palette},
    kit_theme::{theme_set_json, KIT_THEME_DARK, KIT_THEME_LIGHT},
};

/// Which of the two first-class themes is active. The harness defaults to
/// dark, the assistant to light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeKind {
    /// Light palette (assistant default).
    Light,
    /// Dark palette (harness default).
    #[default]
    Dark,
}

impl ThemeKind {
    /// The opposite theme.
    pub fn toggled(self) -> Self {
        match self {
            ThemeKind::Light => ThemeKind::Dark,
            ThemeKind::Dark => ThemeKind::Light,
        }
    }

    /// Whether this is the dark theme.
    pub fn is_dark(self) -> bool {
        matches!(self, ThemeKind::Dark)
    }

    /// Human-readable name ("Light" / "Dark").
    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Light => "Light",
            ThemeKind::Dark => "Dark",
        }
    }

    fn kit_mode(self) -> ThemeMode {
        match self {
            ThemeKind::Light => ThemeMode::Light,
            ThemeKind::Dark => ThemeMode::Dark,
        }
    }

    /// Parses `light` / `dark` (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "light" => Some(ThemeKind::Light),
            "dark" => Some(ThemeKind::Dark),
            _ => None,
        }
    }
}

/// The five agent states the sidebar and status rows colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentState {
    /// Working; coloured with the accent.
    Running,
    /// Needs the person (approval, question); warning.
    Waiting,
    /// Finished successfully; success.
    Done,
    /// Failed; danger.
    Failed,
    /// Nothing happening; ink-4.
    Idle,
}

/// One of the three named easings (`out`, `inout`, `std`) as a cubic bezier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Easing(pub [f32; 4]);

impl Easing {
    /// `cubic-bezier(.16,1,.3,1)`: enters, reveals.
    pub const OUT: Easing = Easing(scale::E_OUT);
    /// `cubic-bezier(.65,0,.35,1)`: bobbing, symmetric moves.
    pub const INOUT: Easing = Easing(scale::E_INOUT);
    /// `cubic-bezier(.2,0,0,1)`: colour and hover tints.
    pub const STD: Easing = Easing(scale::E_STD);

    /// Evaluates the bezier at `t` in `0..=1` (x is time, y is progress).
    pub fn sample(self, t: f32) -> f32 {
        let [x1, y1, x2, y2] = self.0;
        let t = t.clamp(0.0, 1.0);
        // Solve x(s) = t by bisection, then evaluate y(s).
        let bez = |a: f32, b: f32, s: f32| 3.0 * a * s * (1.0 - s).powi(2) + 3.0 * b * s * s * (1.0 - s) + s.powi(3);
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        let mut s = t;
        for _ in 0..24 {
            let x = bez(x1, x2, s);
            if (x - t).abs() < 1e-5 {
                break;
            }
            if x < t {
                lo = s;
            } else {
                hi = s;
            }
            s = (lo + hi) * 0.5;
        }
        bez(y1, y2, s).clamp(0.0, 1.0)
    }

    /// The easing as a boxed function usable with gpui's `Animation::with_easing`.
    pub fn function(self) -> impl Fn(f32) -> f32 + 'static {
        move |t| self.sample(t)
    }
}

/// The active token set. Lives as a gpui [`Global`]; read it with
/// [`ActiveAui::aui`].
#[derive(Debug, Clone)]
pub struct AuiTheme {
    /// Light or dark.
    pub kind: ThemeKind,
    /// Standard or compact row metrics.
    pub density: Density,
    /// The active colour palette.
    pub colors: Palette,
    /// Row and control heights for the active density.
    pub metrics: Metrics,
}

impl Global for AuiTheme {}

impl AuiTheme {
    /// Builds a theme without touching gpui globals (for tests and tooling).
    pub fn new(kind: ThemeKind, density: Density) -> Self {
        Self {
            kind,
            density,
            colors: match kind {
                ThemeKind::Light => light(),
                ThemeKind::Dark => dark(),
            },
            metrics: Metrics::for_density(density),
        }
    }

    /// Installs the tokens: loads the bundled fonts, registers the generated
    /// gpui-kit theme set, makes it gpui-kit's light/dark pair and activates
    /// `kind`. Call once after `gpui_kit::init(cx)`.
    pub fn init(kind: ThemeKind, cx: &mut App) {
        if let Err(err) = load_fonts(cx) {
            eprintln!("aui-tokens: failed to load bundled fonts: {err:#}");
        }
        let json = theme_set_json();
        ThemeRegistry::global_mut(cx)
            .load_themes_from_str(&json)
            .expect("generated theme set is valid");
        let (light_cfg, dark_cfg) = {
            let registry = ThemeRegistry::global(cx);
            (
                registry.themes().get(KIT_THEME_LIGHT).cloned().expect("light theme registered"),
                registry.themes().get(KIT_THEME_DARK).cloned().expect("dark theme registered"),
            )
        };
        {
            let theme = Theme::global_mut(cx);
            theme.light_theme = light_cfg;
            theme.dark_theme = dark_cfg;
        }
        cx.set_global(AuiTheme::new(kind, Density::Standard));
        Self::apply(cx, None);
    }

    /// Switches the theme (both the token palette and gpui-kit's theme).
    pub fn set_kind(kind: ThemeKind, window: Option<&mut Window>, cx: &mut App) {
        {
            let this = cx.global_mut::<AuiTheme>();
            this.kind = kind;
            this.colors = Palette::for_kind(kind);
        }
        Self::apply(cx, window);
    }

    /// Flips between light and dark.
    pub fn toggle_kind(window: Option<&mut Window>, cx: &mut App) {
        let next = cx.global::<AuiTheme>().kind.toggled();
        Self::set_kind(next, window, cx);
    }

    /// Switches the row density.
    pub fn set_density(density: Density, window: Option<&mut Window>, cx: &mut App) {
        {
            let this = cx.global_mut::<AuiTheme>();
            this.density = density;
            this.metrics = Metrics::for_density(density);
        }
        if let Some(window) = window {
            window.refresh();
        } else {
            cx.refresh_windows();
        }
    }

    fn apply(cx: &mut App, window: Option<&mut Window>) {
        let kind = cx.global::<AuiTheme>().kind;
        Theme::change(kind.kit_mode(), window, cx);
        cx.refresh_windows();
    }

    /// Whether the person asked the OS for reduced motion.
    pub fn reduce_motion(cx: &App) -> bool {
        cx.reduce_motion()
    }
}

/// Access to the active [`AuiTheme`] from any context that derefs to [`App`].
pub trait ActiveAui {
    /// The active token set.
    fn aui(&self) -> &AuiTheme;
}

impl ActiveAui for App {
    #[inline(always)]
    fn aui(&self) -> &AuiTheme {
        self.global::<AuiTheme>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_endpoints_and_monotonic() {
        for e in [Easing::OUT, Easing::INOUT, Easing::STD] {
            assert!((e.sample(0.0)).abs() < 1e-3);
            assert!((e.sample(1.0) - 1.0).abs() < 1e-3);
            let mut last = 0.0;
            for i in 1..=20 {
                let v = e.sample(i as f32 / 20.0);
                assert!(v >= last - 1e-4, "{e:?} not monotonic at {i}");
                last = v;
            }
        }
        // ease-out is fast early
        assert!(Easing::OUT.sample(0.25) > 0.7);
    }

    #[test]
    fn theme_new_picks_palette() {
        let t = AuiTheme::new(ThemeKind::Light, Density::Compact);
        assert_eq!(t.colors, light());
        assert_eq!(t.metrics, Metrics::for_density(Density::Compact));
    }
}
