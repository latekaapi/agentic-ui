//! Skills page model tests: intent mapping and model → state. These are pure
//! unit tests over the models — no window needed — so they pin the mapping
//! the renderers report through.

use aui::skills::{
    added_tokens, default_primary_label, format_tokens, selected_rows, segment_widths, CostSegment, ImportRow,
    ImportStatus, InkLevel, SkillChip, SkillMode, SkillRowIntent, SkillRowModel, SkillState, CheckboxState,
};
use aui_tokens::{Palette, ThemeKind};

fn model(dimmed: bool) -> SkillRowModel {
    SkillRowModel {
        id: "git".into(),
        name: "git".into(),
        description: "Branch names the way acme-web does them.".into(),
        chips: vec![SkillChip::new("Overrides built-in git", aui::skills::ChipTone::Muted)],
        tokens: "410".into(),
        mode: Some(SkillMode::Auto),
        on: true,
        dimmed,
        selected: false,
    }
}

// ── row intents ───────────────────────────────────────────────────────────

#[test]
fn enabled_row_emits_toggle() {
    assert_eq!(model(false).toggle_intent(), Some(SkillRowIntent::Toggle));
}

#[test]
fn dimmed_row_emits_no_toggle() {
    // The overridden loser renders its switch disabled and drops the press.
    assert_eq!(model(true).toggle_intent(), None);
}

#[test]
fn dimmed_row_stays_selectable() {
    // The loser is viewable, just not flippable.
    assert_eq!(model(true).select_intent(), Some(SkillRowIntent::Select));
    assert_eq!(model(false).select_intent(), Some(SkillRowIntent::Select));
}

#[test]
fn dimmed_row_emits_no_set_mode() {
    assert_eq!(model(true).mode_intent(SkillMode::Only), None);
    assert_eq!(model(false).mode_intent(SkillMode::Only), Some(SkillRowIntent::SetMode(SkillMode::Only)));
}

// ── mode ──────────────────────────────────────────────────────────────────

#[test]
fn mode_labels_name_the_skill() {
    assert_eq!(SkillMode::Auto.chip_label("decoction"), "Auto");
    assert_eq!(SkillMode::Only.chip_label("decoction"), "Only /decoction");
    assert_eq!(SkillMode::Auto.menu_label("decoction"), "Automatic");
    assert_eq!(SkillMode::Only.menu_label("decoction"), "Only /decoction");
}

#[test]
fn skill_state_round_trips_segments() {
    for state in [SkillState::Automatic, SkillState::OnlyMention, SkillState::Off] {
        assert_eq!(SkillState::from_index(state.index()), state);
    }
    // Anything past Off falls back to Automatic.
    assert_eq!(SkillState::from_index(99), SkillState::Automatic);
}

#[test]
fn skill_state_follows_activation() {
    assert_eq!(SkillState::from_activation(true, &SkillMode::Auto), SkillState::Automatic);
    assert_eq!(SkillState::from_activation(true, &SkillMode::Only), SkillState::OnlyMention);
    assert_eq!(SkillState::from_activation(false, &SkillMode::Auto), SkillState::Off);
    assert_eq!(SkillState::from_activation(false, &SkillMode::Only), SkillState::Off);
}

#[test]
fn skill_state_hints_are_plain_words() {
    for state in [SkillState::Automatic, SkillState::OnlyMention, SkillState::Off] {
        assert!(!SkillState::hint(&state).is_empty());
    }
}

// ── checkbox ──────────────────────────────────────────────────────────────

#[test]
fn checkbox_click_lands_on() {
    assert_eq!(CheckboxState::Off.toggled(), CheckboxState::On);
    assert_eq!(CheckboxState::On.toggled(), CheckboxState::Off);
    assert_eq!(CheckboxState::Mixed.toggled(), CheckboxState::On);
}

#[test]
fn checkbox_checked_is_on_only() {
    assert!(CheckboxState::On.checked());
    assert!(!CheckboxState::Off.checked());
    assert!(!CheckboxState::Mixed.checked());
}

// ── cost meter ────────────────────────────────────────────────────────────

#[test]
fn segment_widths_sum_to_one() {
    let segments =
        vec![CostSegment::new("a", 0.18, InkLevel::Ink), CostSegment::new("b", 0.16, InkLevel::Ink2)];
    let widths = segment_widths(&segments);
    assert!((widths.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    assert!((widths[0] - 0.18 / 0.34).abs() < 1e-6);
}

#[test]
fn segment_widths_clamp_and_zero() {
    let segments = vec![CostSegment::new("a", -2.0, InkLevel::Ink), CostSegment::new("b", 0.0, InkLevel::Ink2)];
    assert_eq!(segment_widths(&segments), vec![0.0, 0.0]);
    let empty: Vec<CostSegment> = vec![];
    assert!(segment_widths(&empty).is_empty());
}

#[test]
fn ink_levels_are_the_palette_greys() {
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        let p = Palette::for_kind(kind);
        assert_eq!(InkLevel::Ink.color(&p), p.ink, "Ink in {kind:?}");
        assert_eq!(InkLevel::Ink2.color(&p), p.ink_2, "Ink2 in {kind:?}");
        assert_eq!(InkLevel::Ink3.color(&p), p.ink_3, "Ink3 in {kind:?}");
        assert_eq!(InkLevel::Ink4.color(&p), p.ink_4, "Ink4 in {kind:?}");
    }
}

// ── import preview ────────────────────────────────────────────────────────

fn import_rows() -> Vec<ImportRow> {
    vec![
        ImportRow::new("pdf-tools", "pdf-tools", "Fill PDFs.", ImportStatus::New, "640", 640),
        ImportRow::new("changelog", "changelog", "Keep a changelog.", ImportStatus::Replaces, "300", 300),
        ImportRow::new("decoction", "decoction", "Already in Personal, unchanged", ImportStatus::Installed, "1.1k", 1100)
            .unchecked(),
    ]
}

#[test]
fn installed_rows_emit_no_toggle() {
    let rows = import_rows();
    assert!(rows[0].toggle_intent().is_some());
    assert!(rows[1].toggle_intent().is_some());
    assert_eq!(rows[2].toggle_intent(), None);
}

#[test]
fn summary_counts_picked_importable_rows() {
    let rows = import_rows();
    assert_eq!(selected_rows(&rows).len(), 2);
    // The installed row adds nothing even when checked.
    assert_eq!(added_tokens(&rows), 940);
    let mut none = import_rows();
    for row in &mut none {
        row.checked = false;
    }
    assert_eq!(selected_rows(&none).len(), 0);
    assert_eq!(added_tokens(&none), 0);
}

#[test]
fn token_counts_format_like_rows() {
    assert_eq!(format_tokens(640), "640");
    assert_eq!(format_tokens(999), "999");
    assert_eq!(format_tokens(1000), "1k");
    assert_eq!(format_tokens(1100), "1.1k");
    assert_eq!(format_tokens(1760), "1.8k");
    assert_eq!(format_tokens(6900), "6.9k");
}

#[test]
fn primary_label_counts_the_pick() {
    assert_eq!(default_primary_label(0), "Import");
    assert_eq!(default_primary_label(1), "Import 1 skill");
    assert_eq!(default_primary_label(3), "Import 3 skills");
}

#[test]
fn import_status_chips_and_selectability() {
    assert_eq!(ImportStatus::New.chip_label(), Some("New"));
    assert_eq!(ImportStatus::Replaces.chip_label(), Some("Replaces yours"));
    assert_eq!(ImportStatus::Installed.chip_label(), None);
    assert!(ImportStatus::New.selectable());
    assert!(ImportStatus::Replaces.selectable());
    assert!(!ImportStatus::Installed.selectable());
}
