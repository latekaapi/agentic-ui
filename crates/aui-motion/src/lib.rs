//! # aui-motion
//!
//! The motion layer of the Agentic UI library (design card 04, spec §0.4).
//! It is deliberately thin: gpui-pre ships an analytic spring solver and
//! gpui-base ships keyed, reduced-motion-aware state machines for value
//! transitions, presence and keyframe playback. This crate binds those to the
//! design's constants and adds the composite primitives the cards use.
//!
//! | primitive | what it does | timing |
//! |---|---|---|
//! | [`spring`] | retargetable spring on `f32` / `Pixels` | press · swap · layout · gentle |
//! | [`tween`] | CSS-style transition on any interpolable value | fast · base · enter · exit · slow |
//! | [`presence`] | enter / exit with an exit hold, so a leaving element can fade | enter 220 · exit 160 |
//! | [`collapse`] | height 0 ↔ auto | layout spring |
//! | [`stagger`] | per-child delay | 40–60 ms |
//! | [`shimmer`] | text sweep and surface skeleton | 1.8 s · 1.4 s |
//! | [`icon_morph`] | swap two glyphs with fade + 3 px + scale | swap spring |
//! | [`ticker`] | numbers that count to their value | base |
//! | [`reveal`] | streamed chunks fade + 3 px rise | base |
//! | [`pulse`] | expanding ring around a status dot | 2 s ease-out, loops |
//! | [`shake`] | ±4 px error shake | 500 ms |
//! | [`check`] | check-mark draw | 320 ms ease-out |
//! | [`looping`] | a 0→1 phase that repeats (spinners) | any |
//!
//! Every primitive is a function you call while rendering; state is keyed by
//! an element id and lives in the window, exactly like gpui-base. When the OS
//! asks for reduced motion (`cx.reduce_motion()`) every duration is zero and
//! every spring resolves instantly, which satisfies the design rule.
//!
//! What gpui cannot do today, and how the primitives compensate: there is no
//! element-level `transform`, so "scale" is expressed as a size change and
//! "translate" as a relative offset; both are documented per primitive.

#![warn(missing_docs)]

pub mod check;
mod collapse;
mod icon_morph;
mod looping;
mod presence;
pub mod pulse;
pub mod reveal;
pub mod shake;
pub mod shimmer;
mod spring;
pub mod stagger;
pub mod ticker;
mod tween;

pub use aui_tokens::{durations, springs, Easing};
pub use check::{check_draw, CheckDraw};
pub use collapse::{collapse, Reveal};
pub use icon_morph::{icon_morph, IconMorph, MorphSample};
pub use looping::{looping, Loop};
pub use presence::{presence, EnterExit, PresenceSample, PresenceStyle};
pub use pulse::{pulse_ring, PulseRing};
pub use reveal::{stream_reveal, RevealSample};
pub use shake::shake_offset;
pub use shimmer::{shimmer_text, skeleton};
pub use spring::{spring, spring_phase, spring_px, SpringKind};
pub use stagger::stagger_delay;
pub use ticker::number_ticker;
pub use tween::{tint_fade, tween, Tween};

/// Delivery phase of this crate, from `docs/01-research-and-plan.md`.
pub const PHASE: &str = "phase 2 · motion engine";

/// Derives a child id from a parent id and an index or generation, so one
/// element can own several independently keyed motions.
pub fn child_id(parent: impl Into<gpui::ElementId>, index: usize) -> gpui::ElementId {
    gpui::ElementId::NamedChild(parent.into().into(), gpui::SharedString::from(index.to_string()))
}
