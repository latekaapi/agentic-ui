//! The stateful mock view.
//!
//! The mock renders a [`Transcript`] and nothing
//! else, so the sample state that stands in for the three design references and
//! the state [`super::script`] builds while a turn runs go through the same
//! code. On top of that it owns the shell's overlays — the ⌘K palette, the
//! composer's `+` menu, the toast stack — and the keyboard, through the actions
//! in [`aui::keys`].

use std::time::Duration;

use aui::composer::{composer, composer_state_rows, plus_menu, ComposerIntent, PlusMenuItem};
use aui::data::{button, status_dot};
use aui::feedback::{toast_stack, ToastData};
use aui::keys::{ApproveAlways, ApproveOnce, Cancel, Confirm, Deny as DenyAction, FocusNext, FocusPrev, SelectNext, SelectPrev, ToggleRightPane, ToggleSidebar, TogglePalette};
use aui::nav::{rail, role_section, sidebar_footer, Project, RailItem, Role, RoleSession, SessionKind};
use aui::overlay::{command_palette, popover_layer, PaletteIcon, PaletteItem, PaletteSection};
use aui::protocol::{ApprovalDecision, ApprovalState};
use aui::shell::{app_shell, centre_header, right_header, sidebar_header, tab_strip, TabItem};
use aui::transcript::{activity_group, answered_row, approval_card, question_card, user_turn, ProseStyle};
use aui::util::{interaction_flags, TrackInteraction};
use aui::workbench::{
    artifact_strip, cited_answer, doc_pane, doc_toolbar, pane_status, pane_status_row, pdf_pane, sheet_pane, source_hover_card, sources_card, Artifact, ArtifactKind, DocBlock, DocPage,
    DocRun, PdfControls, PdfPage, PdfRun, SheetCell, Source, SourceTier,
};
use aui_icons::{icon, IconName, Provider, RoleIcon};
use aui_tokens::{scale, ActiveAui, AuiStyled};
use gpui::*;
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::{h_flex, v_flex};

use super::model::{revealed_text, Block, BlockKind, Transcript, BODY_TEXT};
use super::script::STEPS_DELAY;

/// `.tr{padding:18px 32px 0;gap:16px}` and the status row's 12 px bottom padding.
const TRANSCRIPT_PAD_TOP: f32 = 18.0;
const TRANSCRIPT_PAD_X: f32 = 32.0;
const BLOCK_GAP: f32 = 16.0;
const STATUS_PAD_BOTTOM: f32 = 12.0;
/// `.fc{gap:12px;padding:10px 12px}` with a 36 px icon tile.
const FILE_CARD_GAP: f32 = 12.0;
const FILE_CARD_PAD_Y: f32 = 10.0;
const FILE_CARD_PAD_X: f32 = 12.0;
const FILE_CARD_TILE: f32 = 36.0;
const FILE_CARD_TILE_RADIUS: f32 = 8.0;
/// `.fc .ic svg{width:18px;height:18px}`.
const FILE_CARD_GLYPH: f32 = 18.0;
/// The passage the `assistant-Sources` hover card quotes, and the span the
/// retrieval matched (`<mark>` in the screen source).
const SOURCE_QUOTE: &str =
    "\u{201c}\u{2026}shall hold a valid registration with the Directorate and shall have completed not less than three years of comparable placements in the preceding five years.\u{201d}";
const SOURCE_HIGHLIGHT: &str = "not less than three years of comparable placements";
/// `screens/all`: three 1440 x 900 screens, 24 px apart, on a 20 px ground,
/// each under a caption.
const SCREEN_W: f32 = 1440.0;
const SCREEN_H: f32 = 900.0;
const SCREENS_GAP: f32 = 24.0;
const SCREENS_PAD_X: f32 = 20.0;
const SCREENS_PAD_Y: f32 = 12.0;
const SCREENS_CAPTION: f32 = 28.0;
/// `.scrim{padding-top:56px}` in card 12 — how far below the window's top edge
/// the palette floats, and the scrim's own black tint over the shell.
const PALETTE_TOP: f32 = 56.0;
const SCRIM_TINT: f32 = 0.32;
/// Card 13 draws the stack in a 340 px column; in the shell it sits in the
/// bottom-right corner on the standard 24 px inset.
const TOAST_COLUMN: f32 = 340.0;
const TOAST_INSET: f32 = 24.0;
/// The stack reserves the height of one toast so the fan has room to grow into.
const TOAST_STACK_H: f32 = 84.0;

/// Which of the three assistant screen references the mock stands in for.
/// `AUI_GALLERY_SCREEN=<Main|Sources|Sheet>` picks one for a parity render;
/// the `screens/all` entry passes each of them explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantScreen {
    /// `assistant-Main`: the RFP session with the document pane.
    Main,
    /// `assistant-Sources`: the cited answer, the sources card and the open
    /// hover card over citation 1, with the regulation in the PDF pane.
    Sources,
    /// `assistant-Sheet`: the scoring sheet the session created.
    Sheet,
}

impl AssistantScreen {
    /// Reads `AUI_GALLERY_SCREEN`; anything unrecognised is `Main`.
    pub fn from_env() -> Self {
        match std::env::var("AUI_GALLERY_SCREEN").unwrap_or_default().as_str() {
            "Sources" | "sources" => AssistantScreen::Sources,
            "Sheet" | "sheet" => AssistantScreen::Sheet,
            _ => AssistantScreen::Main,
        }
    }

    /// The session the sidebar marks current.
    fn session(self) -> &'static str {
        match self {
            AssistantScreen::Main => "rfp-v3",
            AssistantScreen::Sources => "eligibility",
            AssistantScreen::Sheet => "scoring",
        }
    }

    /// The centre header's branch tag.
    fn branch(self) -> &'static str {
        match self {
            AssistantScreen::Main => "RFP draft v3",
            AssistantScreen::Sources => "Eligibility criteria review",
            AssistantScreen::Sheet => "Vendor scoring sheet",
        }
    }

    /// The composer placeholder.
    fn placeholder(self) -> &'static str {
        match self {
            AssistantScreen::Main => "Ask, draft, or type / for commands",
            AssistantScreen::Sources => "Ask a follow-up, or drag a passage here to quote it",
            AssistantScreen::Sheet => "Ask about the selection, or change the weights",
        }
    }

    /// The pane the right column opens on.
    fn right_tab(self) -> RightTab {
        match self {
            AssistantScreen::Main => RightTab::Doc,
            AssistantScreen::Sources => RightTab::Pdf,
            AssistantScreen::Sheet => RightTab::Sheet,
        }
    }

    /// The caption used by the `screens/all` page.
    pub fn caption(self) -> &'static str {
        match self {
            AssistantScreen::Main => "assistant-Main \u{b7} project session with document",
            AssistantScreen::Sources => "assistant-Sources \u{b7} answer with citations",
            AssistantScreen::Sheet => "assistant-Sheet \u{b7} spreadsheet created in chat",
        }
    }
}

/// Which pane the right column shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    /// The RFP draft (docx).
    Doc,
    /// The vendor scoring sheet (xlsx).
    Sheet,
    /// The cited regulation (pdf).
    Pdf,
}

/// Where the keyboard should go on the next frame. gpui focuses through a
/// `Window`, which the async script does not hold, so a scenario parks its
/// request here and [`AssistantMock::render`] applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The composer's textarea: the mock's resting focus.
    Composer,
    /// The command palette's query row.
    Palette,
    /// The newest pending approval card, so Y / A / N reach it.
    Approval,
    /// The newest pending question card.
    Question,
}

/// The open command palette.
struct Palette {
    /// What has been typed into the query row.
    query: SharedString,
    /// The highlighted row, across all sections in order.
    selected: usize,
    /// What had the keyboard when the palette opened.
    restore: Option<FocusHandle>,
}

/// The mock's state.
pub struct AssistantMock {
    screen: AssistantScreen,
    right_open: bool,
    sidebar_open: bool,
    right_tab: RightTab,
    open_roles: [bool; 3],
    active_session: SharedString,
    /// Sessions the palette's "New session" command added to the open project.
    new_sessions: Vec<SharedString>,
    pub(super) composer: Entity<TextareaState>,
    pub(super) plus_open: bool,
    /// The transcript the view renders.
    pub(super) transcript: Transcript,
    /// Bumped by every send, so a scenario left over from an interrupted turn
    /// stops touching the model.
    pub(super) run: u64,
    palette: Option<Palette>,
    /// The toasts the shell is showing, oldest first.
    pub(super) toasts: Vec<ToastData>,
    /// Whether the pointer is over the stack, which holds the dismiss timers.
    pub(super) toast_hovered: bool,
    pub(super) want_focus: Option<Focus>,
    /// The handle the palette took the keyboard from, restored when it closes.
    restore_focus: Option<FocusHandle>,
    focus_root: FocusHandle,
    focus_palette: FocusHandle,
    focus_approval: FocusHandle,
    focus_question: FocusHandle,
    /// `AUI_GALLERY_STEPS`, applied once after the first frame.
    steps: Vec<String>,
    started: bool,
    /// Timers keep running only while their task is alive.
    pub(super) tasks: Vec<Task<()>>,
}

impl AssistantMock {
    /// Creates the mock in the sample state of one assistant screen.
    pub fn new(screen: AssistantScreen, scripted: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| composer_state_rows(screen.placeholder(), 1, 8, window, cx));
        let steps = if scripted { steps_from_env() } else { Vec::new() };
        Self {
            screen,
            right_open: true,
            sidebar_open: !crate::cards::app_shell::collapsed_by_default(),
            right_tab: screen.right_tab(),
            open_roles: [true, false, false],
            active_session: screen.session().into(),
            new_sessions: Vec::new(),
            composer,
            plus_open: false,
            transcript: Transcript::sample(screen),
            run: 0,
            palette: None,
            toasts: Vec::new(),
            toast_hovered: false,
            want_focus: Some(Focus::Composer),
            restore_focus: None,
            focus_root: cx.focus_handle(),
            focus_palette: cx.focus_handle(),
            focus_approval: cx.focus_handle(),
            focus_question: cx.focus_handle(),
            steps,
            started: false,
            tasks: Vec::new(),
        }
    }

    fn roles(&self) -> Vec<Role> {
        let mut rfp = Project::new("rfp", "Teacher recruitment RFP", 6)
            .session(RoleSession::new("rfp-v3", "RFP draft v3", SessionKind::Document, "now"))
            .session(RoleSession::new("eligibility", "Eligibility criteria review", SessionKind::Chat, "2h"))
            .session(RoleSession::new("scoring", "Vendor scoring sheet", SessionKind::Sheet, "1d"));
        for (index, name) in self.new_sessions.iter().enumerate() {
            rfp = rfp.session(RoleSession::new(format!("new-{index}"), name.clone(), SessionKind::Chat, "now"));
        }
        vec![
            Role::new("education", "Director, Education", RoleIcon::GradCap, 3)
                .open()
                .project(rfp)
                .project(Project::new("annual", "Annual report 2026", 3))
                .project(Project::new("letters", "Letters and notes", 12))
                .knowledge("Education Code")
                .knowledge("Procurement Rules 2019")
                .knowledge("GO 2024-18"),
            Role::new("law", "Director, Law", RoleIcon::Scale, 2),
            Role::new("ops", "Operations", RoleIcon::Gear, 4),
        ]
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut col = v_flex().size_full();
        for (i, mut role) in self.roles().into_iter().enumerate() {
            role.open = self.open_roles[i];
            let toggle = cx.listener(move |this, _: &str, _, cx| {
                this.open_roles[i] = !this.open_roles[i];
                cx.notify();
            });
            let select = cx.listener(|this, id: &str, _, cx| {
                this.active_session = id.to_string().into();
                cx.notify();
            });
            col = col.child(
                role_section(SharedString::from(format!("assistant-role-{}", role.id)), &role)
                    .active_session(Some(self.active_session.as_ref()))
                    .on_toggle_role(move |id, w, cx| toggle(id, w, cx))
                    .on_select_session(move |id, w, cx| select(id, w, cx)),
            );
        }
        col.child(div().flex_1()).child(sidebar_footer("assistant-footer", "A", "Alex Rivera").meter(Provider::Claude, 0.78).pad_y(8.0))
    }

    /// The collapsed sidebar: one cell per role, then the open project's
    /// sessions by kind. No sliver of the expanded sidebar is drawn here.
    fn render_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let roles = self.roles();
        let mut items: Vec<RailItem> = roles
            .iter()
            .enumerate()
            .map(|(i, role)| RailItem::nav(role.id.clone(), role.icon.icon()).current(self.open_roles[i]))
            .collect();
        items.push(RailItem::separator());
        for project in roles.first().map(|r| r.projects.as_slice()).unwrap_or_default() {
            for session in &project.sessions {
                items.push(RailItem::nav(session.id.clone(), session.kind.icon()).current(session.id == self.active_session));
            }
        }
        let select = cx.listener(|this, id: &str, _, cx| {
            this.active_session = id.to_string().into();
            cx.notify();
        });
        rail("assistant-rail", items).flat(true).avatar("B").on_action(move |id, w, cx| select(id, w, cx))
    }

    /// The sources card's two tiers, as the `assistant-Sources` screen groups them.
    fn source_tiers() -> Vec<SourceTier> {
        vec![
            SourceTier::new("Role")
                .name("Director, Education")
                .source(Source::cited(1, "Procurement Rules 2019, Rule 14(2)", "procurement-rules-2019.pdf \u{b7} p. 31 \u{b7} registration and experience threshold", 0.92))
                .source(Source::cited(2, "GO 2024-18 \u{b7} Verification of credentials", "go-2024-18.pdf \u{b7} \u{a7}4 \u{b7} original certificates at onboarding", 0.88)),
            SourceTier::new("Project").name("Teacher recruitment RFP").source(Source::cited(3, "Project brief", "brief.docx \u{b7} cohort size and institution count", 0.97)),
        ]
    }

    /// One transcript block.
    fn render_block(&self, index: usize, block: &Block, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        let id = block.id.clone();
        match &block.kind {
            BlockKind::UserTurn { text } => div().w_full().flex().justify_end().child(user_turn(id, text.clone())).into_any_element(),
            BlockKind::Activity { steps, summary, detail, elapsed, state, open } => {
                let toggle = cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                    if let BlockKind::Activity { open, .. } = &mut this.transcript.blocks[index].kind {
                        *open = !*open;
                    }
                    cx.notify();
                });
                let mut group = activity_group(id, steps.clone(), summary.clone(), elapsed.clone(), *state).open(*open).on_toggle(toggle);
                if let Some(detail) = detail {
                    group = group.detail(detail.clone());
                }
                group.into_any_element()
            }
            BlockKind::Answer { text, revealed, streaming } => {
                let style = ProseStyle { ink: p.ink, code_ink: p.accent_ink, code_bg: p.accent_soft, size: BODY_TEXT, line_height: scale::LH_BODY, paragraph_gap: 10.0 };
                cited_answer(id, revealed_text(text, *revealed), style).streaming(*streaming).into_any_element()
            }
            BlockKind::Approval { tool, command, reason, state } => {
                let decide = cx.listener(move |this, decision: &ApprovalDecision, window, cx| this.decide(*decision, window, cx));
                let card = approval_card(id, tool.clone(), command.clone(), state.clone())
                    .reason(reason.clone())
                    .cwd("~/Documents/Teacher recruitment RFP")
                    .capabilities(["write files"])
                    .rule("Write *.xlsx")
                    .on_decide(move |decision, w, cx| decide(&decision, w, cx));
                let pending = *state == ApprovalState::Pending;
                let mut holder = div().w_full();
                if pending {
                    holder = holder
                        .key_context(aui::keys::APPROVAL_CONTEXT)
                        .track_focus(&self.focus_approval)
                        .on_action(cx.listener(|this, _: &ApproveOnce, window, cx| this.decide(ApprovalDecision::Once, window, cx)))
                        .on_action(cx.listener(|this, _: &ApproveAlways, window, cx| this.decide(ApprovalDecision::Always, window, cx)))
                        .on_action(cx.listener(|this, _: &DenyAction, window, cx| this.decide(ApprovalDecision::Deny, window, cx)));
                }
                holder.child(card).into_any_element()
            }
            BlockKind::Question { prompt, subtitle, options, selected, answered } => {
                if let Some(chip) = answered {
                    return answered_row(id, vec![chip.clone()]).into_any_element();
                }
                let pick = cx.listener(move |this, choice: &usize, window, cx| this.answer_question(*choice, window, cx));
                let card = question_card(id, prompt.clone(), options)
                    .subtitle(subtitle.clone())
                    .selected(selected.map(|s| vec![s]).unwrap_or_default())
                    .allow_other(true)
                    .hint("\u{2191}\u{2193} to move \u{b7} \u{21a9} to choose")
                    .on_select(move |choice, w, cx| pick(&choice, w, cx));
                div()
                    .w_full()
                    .key_context(aui::keys::MENU_CONTEXT)
                    .track_focus(&self.focus_question)
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_question(1, cx)))
                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_question(-1, cx)))
                    .on_action(cx.listener(|this, _: &Confirm, window, cx| this.confirm_question(window, cx)))
                    .child(card)
                    .into_any_element()
            }
            BlockKind::FileCard { sheet } => self.file_card(id, *sheet, cx).into_any_element(),
            BlockKind::Sources => {
                let start = SOURCE_QUOTE.find(SOURCE_HIGHLIGHT).unwrap_or(0);
                let hover = source_hover_card("assistant-hover", "Rule 14(2) \u{b7} Eligibility of bidders", SOURCE_QUOTE, start..start + SOURCE_HIGHLIGHT.len(), 31).at_rest();
                sources_card(id, Self::source_tiers()).cited(3).retrieved(11).hover_card(0, hover).into_any_element()
            }
        }
    }

    fn render_transcript(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let mut list = div()
            .id("assistant-transcript")
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pt(px(TRANSCRIPT_PAD_TOP))
            .px(px(TRANSCRIPT_PAD_X))
            .gap(px(BLOCK_GAP));
        for (index, block) in self.transcript.blocks.iter().enumerate() {
            list = list.child(self.render_block(index, block, cx));
        }
        let status = &self.transcript.status;
        v_flex().flex_1().min_h(px(0.0)).w_full().child(list).child(
            h_flex()
                .w_full()
                .px(px(TRANSCRIPT_PAD_X))
                .pb(px(STATUS_PAD_BOTTOM))
                .gap(px(scale::SP_3))
                .ui(scale::FS_12)
                .text_color(p.ink_3)
                .child(status_dot("assistant-status-dot", status.state))
                .child(status.label.clone())
                .child("\u{b7}")
                .child(status.detail.clone()),
        )
    }

    /// `.fc`: the artifact card under the answer. The tile is surface-2 with
    /// the file type's tint, as `file_card()` builds it in the screen source.
    fn file_card(&self, id: SharedString, sheet: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let (glyph, tint, name, desc) = if sheet {
            (IconName::Sheet, p.success, "vendor-scoring.xlsx", "Scores, Matrix and Notes sheets \u{b7} formulas live \u{b7} v1")
        } else {
            (IconName::Doc, p.info, "RFP-draft-v3.docx", "Section 2 rewritten \u{b7} 3 changes highlighted \u{b7} v3")
        };
        let tab = if sheet { RightTab::Sheet } else { RightTab::Doc };
        h_flex()
            .id(ElementId::from(id))
            .w_full()
            .gap(px(FILE_CARD_GAP))
            .py(px(FILE_CARD_PAD_Y))
            .px(px(FILE_CARD_PAD_X))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(
                div()
                    .flex_none()
                    .size(px(FILE_CARD_TILE))
                    .rounded(px(FILE_CARD_TILE_RADIUS))
                    .bg(p.surface_2)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(glyph).size(px(FILE_CARD_GLYPH)).color(tint)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().ui(scale::FS_13).semibold().text_color(p.ink).child(name))
                    .child(div().ui(scale::FS_12).text_color(p.ink_3).child(desc)),
            )
            .child(button("assistant-download", "Download").ghost().sm())
            .child(button("assistant-open-pane", "Open in pane").sm().on_click(cx.listener(move |this, _, _, cx| {
                this.right_open = true;
                this.right_tab = tab;
                cx.notify();
            })))
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let streaming = self.transcript.streaming_answer().is_some();
        let menu = self.plus_open.then(|| {
            popover_layer(
                div()
                    .key_context(aui::keys::MENU_CONTEXT)
                    .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                        this.plus_open = false;
                        cx.notify();
                    }))
                    .child(plus_menu(
                        "assistant-plus-menu",
                        vec![
                            PlusMenuItem::new("attach", IconName::Paperclip, "Attach file").key("\u{2318}U"),
                            PlusMenuItem::new("knowledge", IconName::Book, "Add knowledge source"),
                            PlusMenuItem::new("mention", IconName::At, "Mention a document").key("@"),
                            PlusMenuItem::new("commands", IconName::Slash, "Commands").key("/"),
                        ],
                        true,
                    )),
            )
        });
        composer("assistant-composer", &self.composer, Provider::Claude, "Opus 4.6")
            .docked(true)
            .mode("Education + project")
            .knowledge_first(true)
            .streaming(streaming)
            .plus_menu(self.plus_open, menu)
            .on_intent({
                let handler = cx.listener(|this, intent: &ComposerIntent, window, cx| this.on_intent(intent.clone(), window, cx));
                move |intent, w, cx| handler(&intent, w, cx)
            })
    }

    /// Every composer intent, in one place, so the `AUI_GALLERY_STEPS` harness
    /// can reach the same handlers a click does.
    pub(super) fn on_intent(&mut self, intent: ComposerIntent, window: &mut Window, cx: &mut Context<Self>) {
        match intent {
            ComposerIntent::TogglePlus => {
                self.plus_open = !self.plus_open;
                cx.notify();
            }
            ComposerIntent::Send => self.send(window, cx),
            ComposerIntent::Stop => self.stop(cx),
            _ => {}
        }
    }

    /// The "created in chat" strip. The document pane also lists the notes
    /// file; the sheet pane shows only the two versioned artifacts.
    fn artifacts(&self, with_notes: bool) -> Vec<Artifact> {
        let mut out = vec![
            Artifact::new("RFP-draft-v3.docx", ArtifactKind::Doc).version("v3").active(self.right_tab == RightTab::Doc),
            Artifact::new("vendor-scoring.xlsx", ArtifactKind::Sheet).version("v1").active(self.right_tab == RightTab::Sheet),
        ];
        if with_notes {
            out.push(Artifact::new("notes.md", ArtifactKind::Note));
        }
        out
    }

    fn render_right(&self, cx: &mut Context<Self>) -> AnyElement {
        let strip = artifact_strip("assistant-artifacts", self.artifacts(self.right_tab == RightTab::Doc))
            // The right pane is `shell::RIGHT_WIDTH` wide: two chips plus the
            // `+N` chip is what stays readable there.
            .max_visible(2)
            .on_select(cx.listener(|this, id: &SharedString, _, cx| {
            this.right_tab = if id.as_ref().ends_with(".xlsx") { RightTab::Sheet } else { RightTab::Doc };
            cx.notify();
        }));
        match self.right_tab {
            RightTab::Doc => {
                let page = DocPage::new(
                    "Request for Proposal: Teacher Recruitment Services",
                    "Directorate of Education · Draft v3 · 5 September 2026",
                    vec![
                        DocBlock::Paragraph(vec![
                            DocRun::bold("1. Purpose."),
                            DocRun::text(" The Directorate invites proposals from qualified agencies for the recruitment of 240 secondary-school teachers across 38 institutions for the 2027 academic year."),
                        ]),
                        DocBlock::Paragraph(vec![
                            DocRun::bold("2. Eligibility."),
                            DocRun::text(" Bidders must hold a valid registration under the "),
                            DocRun::changed("Procurement Rules 2019, Rule 14(2)"),
                            DocRun::text(" and demonstrate "),
                            DocRun::changed("three years of comparable placements"),
                            DocRun::text(". "),
                            DocRun::changed("Original certificates are verified at onboarding in line with GO 2024-18."),
                        ]),
                        DocBlock::Paragraph(vec![DocRun::bold("3. Scope of services.")]),
                        DocBlock::list([
                            "Sourcing and screening against the qualification matrix in Annex A.",
                            "Document verification in line with GO 2024-18.",
                            "Onboarding support through the first term.",
                        ]),
                    ],
                );
                v_flex()
                    .size_full()
                    .child(doc_toolbar("assistant-doc-toolbar", "Body text", "Georgia · 11").without_font_select().without_export().ask_label("Ask"))
                    .child(doc_pane("assistant-doc", page).paper(340.0, 34.0, 36.0))
                    .child(strip)
                    .child(pane_status_row(
                        "assistant-doc-status",
                        vec![pane_status("Page 1 of 4"), pane_status("·"), pane_status("1,214 words")],
                        vec![pane_status("3 changes from chat highlighted").accent(), pane_status("·"), pane_status("Saved")],
                    ))
                    .into_any_element()
            }
            RightTab::Sheet => {
                let rows = vec![
                    vec![SheetCell::text("Vendor"), SheetCell::text("Experience"), SheetCell::text("Coverage"), SheetCell::text("Price"), SheetCell::text("Weighted")],
                    vec![SheetCell::text("Northlight Staffing"), SheetCell::num("4.5"), SheetCell::num("4.0"), SheetCell::num("3.5"), SheetCell::num("4.05").bold()],
                    vec![SheetCell::text("Meridian Educators"), SheetCell::num("3.0"), SheetCell::num("4.5"), SheetCell::num("4.5"), SheetCell::num("3.90").bold()],
                    vec![SheetCell::text("Bright Path"), SheetCell::num("5.0"), SheetCell::num("3.0"), SheetCell::num("3.0"), SheetCell::num("3.85").bold()],
                    vec![SheetCell::text("Civic Talent"), SheetCell::num("2.5"), SheetCell::num("3.5"), SheetCell::num("5.0"), SheetCell::num("3.45").bold()],
                    vec![SheetCell::text("Weight"), SheetCell::num("0.45"), SheetCell::num("0.30"), SheetCell::num("0.25"), SheetCell::text("")],
                ];
                v_flex()
                    .size_full()
                    .child(
                        div().flex_1().min_h(px(0.0)).w_full().child(
                            sheet_pane("assistant-sheet", ["A", "B", "C", "D", "E"].into_iter().map(Into::into).collect(), rows)
                                .selected(1, 4)
                                .formula("E2", "=SUMPRODUCT(B2:D2,B$7:D$7)")
                                .tabs(vec!["Scores".into(), "Matrix".into(), "Notes".into()], 0)
                                .tabs_note("weighted by Annex A"),
                        ),
                    )
                    .child(strip)
                    .child(pane_status_row("assistant-sheet-status", vec![pane_status("E2 selected")], vec![pane_status("Saved")]))
                    .into_any_element()
            }
            RightTab::Pdf => {
                let page = PdfPage {
                    heading: "Chapter IV · Eligibility and qualification of bidders".into(),
                    paragraphs: vec![
                        vec![
                            PdfRun::Bold("14. Eligibility of bidders.".into()),
                            PdfRun::Text(" (1) Every bidder shall be a legal entity registered in the State and shall not be blacklisted by any department of the Government at the time of submission.".into()),
                        ],
                        vec![
                            PdfRun::Text("(2) A bidder for recruitment or staffing services ".into()),
                            PdfRun::Highlight("shall hold a valid registration with the Directorate and shall have completed not less than three years of comparable placements in the preceding five years".into()),
                            PdfRun::Text(", evidenced by completion certificates from the engaging authority.".into()),
                        ],
                        vec![PdfRun::Text("(3) Joint ventures shall satisfy sub-rule (2) through the lead member, whose share shall not be less than fifty-one percent.".into())],
                        vec![
                            PdfRun::Bold("15. Disqualification.".into()),
                            PdfRun::Text(" A bidder shall be disqualified where the bid contains a material misrepresentation, or where the bidder has been convicted of an offence involving fraud within the preceding five years.".into()),
                        ],
                    ],
                    footer: "Procurement Rules 2019 · 31".into(),
                };
                v_flex()
                    .size_full()
                    .child(div().flex_1().min_h(px(0.0)).w_full().child(pdf_pane("assistant-pdf", page.clone(), 31, 88).cited_as(1)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h(px(0.0))
                            .w_full()
                            .child(div().w_full().px(px(12.0)).pt(px(8.0)).ui(scale::FS_12).child("Read-only preview"))
                            .child(div().flex_1().min_h(px(0.0)).w_full().child(pdf_pane("assistant-pdf-readonly", page, 31, 88).cited_as(1).controls(PdfControls::reading_only()))),
                    )
                    .child(pane_status_row("assistant-pdf-status", vec![pane_status("Highlight from citation 1")], vec![pane_status("Opened from chat")]))
                    .into_any_element()
            }
        }
    }

    // ---- overlays -------------------------------------------------------

    /// The commands the ⌘K palette offers.
    fn palette_sections(&self, query: &str) -> Vec<PaletteSection> {
        let items = vec![
            PaletteItem::new("toggle-right", PaletteIcon::Glyph(IconName::PanelRight), "Toggle right pane").matching(query).key("\u{2318}").key("\\"),
            PaletteItem::new("collapse-sidebar", PaletteIcon::Glyph(IconName::Sidebar), "Collapse sidebar").matching(query).key("\u{2318}").key("B"),
            PaletteItem::new("new-session", PaletteIcon::Glyph(IconName::Plus), "New session").matching(query).key("\u{2318}").key("N"),
        ];
        let items: Vec<PaletteItem> = if query.is_empty() {
            items
        } else {
            items.into_iter().filter(|i| i.label.to_lowercase().contains(&query.to_lowercase())).collect()
        };
        if items.is_empty() {
            Vec::new()
        } else {
            vec![PaletteSection::new("Actions", items)]
        }
    }

    /// Every row of the palette, in the order the arrows walk them.
    fn palette_rows(&self, query: &str) -> Vec<SharedString> {
        self.palette_sections(query).into_iter().flat_map(|s| s.items.into_iter().map(|i| i.id)).collect()
    }

    fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.is_some() {
            self.close_palette(cx);
        } else {
            self.palette = Some(Palette { query: SharedString::default(), selected: 0, restore: window.focused(cx) });
            self.want_focus = Some(Focus::Palette);
        }
        cx.notify();
    }

    fn close_palette(&mut self, cx: &mut Context<Self>) {
        if let Some(palette) = self.palette.take() {
            match palette.restore {
                Some(handle) => self.restore_focus = Some(handle),
                None => self.want_focus = Some(Focus::Composer),
            }
        }
        cx.notify();
    }

    fn move_palette(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = self.palette.as_ref().map(|p| self.palette_rows(&p.query).len()).unwrap_or(0);
        if rows == 0 {
            return;
        }
        if let Some(palette) = &mut self.palette {
            palette.selected = ((palette.selected as isize + delta).rem_euclid(rows as isize)) as usize;
        }
        cx.notify();
    }

    fn confirm_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = &self.palette else { return };
        let rows = self.palette_rows(&palette.query);
        let Some(id) = rows.get(palette.selected).cloned() else { return };
        self.run_command(&id, window, cx);
    }

    /// Runs one palette command by id, whether it was clicked or confirmed.
    fn run_command(&mut self, id: &str, _window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "toggle-right" => self.right_open = !self.right_open,
            "collapse-sidebar" => self.sidebar_open = !self.sidebar_open,
            "new-session" => {
                let n = self.new_sessions.len() + 1;
                self.new_sessions.push(SharedString::from(format!("New session {n}")));
                self.active_session = SharedString::from(format!("new-{}", n - 1));
            }
            _ => {}
        }
        self.close_palette(cx);
    }

    fn render_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette.as_ref()?;
        let p = cx.aui().colors;
        let sections = self.palette_sections(&palette.query);
        let selected = palette.selected;
        let hover = cx.listener(|this, index: &usize, _, cx| {
            if let Some(palette) = &mut this.palette {
                palette.selected = *index;
            }
            cx.notify();
        });
        let select = cx.listener(|this, id: &SharedString, window, cx| this.run_command(id.as_ref(), window, cx));
        let dismiss = cx.listener(|this, _: &gpui::ClickEvent, _, cx| this.close_palette(cx));
        let _ = p;
        Some(
            popover_layer(
                div()
                    .id("assistant-palette-layer")
                    .absolute()
                    .inset_0()
                    .bg(black().opacity(SCRIM_TINT))
                    .on_click(dismiss)
                    .child(
                        h_flex()
                            .absolute()
                            .inset_0()
                            .justify_center()
                            .items_start()
                            .pt(px(PALETTE_TOP))
                            .child(
                                div()
                                    .key_context(aui::keys::MENU_CONTEXT)
                                    .track_focus(&self.focus_palette)
                                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.move_palette(1, cx)))
                                    .on_action(cx.listener(|this, _: &SelectPrev, _, cx| this.move_palette(-1, cx)))
                                    .on_action(cx.listener(|this, _: &Confirm, window, cx| this.confirm_palette(window, cx)))
                                    .on_action(cx.listener(|this, _: &Cancel, _, cx| this.close_palette(cx)))
                                    .child(
                                        command_palette("assistant-palette", palette.query.clone(), sections, selected)
                                            .placeholder("Search actions, sessions, documents\u{2026}")
                                            .on_hover(move |index, w, cx| hover(&index, w, cx))
                                            .on_select(move |id, w, cx| select(id, w, cx))
                                            .on_dismiss({
                                            let close = cx.listener(|this, _: &(), _, cx| this.close_palette(cx));
                                            move |w, cx| close(&(), w, cx)
                                        }),
                                    ),
                            ),
                    ),
            )
            .into_any_element(),
        )
    }

    fn render_toasts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.toasts.is_empty() {
            return None;
        }
        // The dismiss timers hold while the pointer is over the stack, so the
        // hover flag is read back out of the frame's interaction state.
        let (state, flags) = interaction_flags("assistant-toasts", window, cx);
        self.toast_hovered = flags.hovered;
        let close = cx.listener(|this, _: &(), _, cx| {
            this.toasts.pop();
            cx.notify();
        });
        Some(
            popover_layer(
                div()
                    .id("assistant-toasts")
                    .absolute()
                    .right(px(TOAST_INSET))
                    .bottom(px(TOAST_INSET))
                    .w(px(TOAST_COLUMN))
                    .h(px(TOAST_STACK_H))
                    .track_interaction(&state)
                    .child(toast_stack("assistant-toast-stack", self.toasts.clone()).on_action(|_, _, _| {}).on_close(move |w, cx| close(&(), w, cx))),
            )
            .into_any_element(),
        )
    }

    // ---- the AUI_GALLERY_STEPS harness ----------------------------------

    /// Applies one step of `AUI_GALLERY_STEPS`. Every step goes through the
    /// same action or intent the UI produces, never straight into the model.
    fn apply_step(&mut self, step: &str, window: &mut Window, cx: &mut Context<Self>) {
        let dispatch = |action: Box<dyn Action>, window: &mut Window, cx: &mut App| window.dispatch_action(action, cx);
        match step.split_once(':') {
            Some(("send", text)) => {
                let text = text.to_string();
                self.composer.update(cx, |state, cx| state.set_value(text, window, cx));
                self.on_intent(ComposerIntent::Send, window, cx);
            }
            Some(("answer", n)) => {
                let choice = n.parse::<usize>().unwrap_or(1).saturating_sub(1);
                self.answer_question(choice, window, cx);
            }
            _ => match step {
                "cmdk" => dispatch(Box::new(TogglePalette), window, cx),
                "cmdb" => dispatch(Box::new(ToggleSidebar), window, cx),
                "cmdright" => dispatch(Box::new(ToggleRightPane), window, cx),
                "up" => dispatch(Box::new(SelectPrev), window, cx),
                "down" => dispatch(Box::new(SelectNext), window, cx),
                "enter" => dispatch(Box::new(Confirm), window, cx),
                "esc" => dispatch(Box::new(Cancel), window, cx),
                "tab" => dispatch(Box::new(FocusNext), window, cx),
                "shift-tab" => dispatch(Box::new(FocusPrev), window, cx),
                "approve" => dispatch(Box::new(ApproveOnce), window, cx),
                "always" => dispatch(Box::new(ApproveAlways), window, cx),
                "deny" => dispatch(Box::new(DenyAction), window, cx),
                "stop" => self.on_intent(ComposerIntent::Stop, window, cx),
                "plus" => self.on_intent(ComposerIntent::TogglePlus, window, cx),
                // `shot` is a marker for `--screenshot-delay`; it does nothing.
                "shot" | "" => {}
                other => eprintln!("AUI_GALLERY_STEPS: unknown step `{other}`"),
            },
        }
    }

    /// Starts the harness after the first frame.
    fn start_steps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let steps = std::mem::take(&mut self.steps);
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            cx.background_executor().timer(STEPS_DELAY).await;
            for step in steps {
                if let Some(ms) = step.strip_prefix("wait:") {
                    let ms: u64 = ms.parse().unwrap_or(0);
                    cx.background_executor().timer(Duration::from_millis(ms)).await;
                    continue;
                }
                if this.update_in(cx, |this, window, cx| this.apply_step(&step, window, cx)).is_err() {
                    return;
                }
            }
        });
        self.tasks.push(task);
    }
}

/// `AUI_GALLERY_STEPS` split into steps, empty when the variable is unset.
fn steps_from_env() -> Vec<String> {
    std::env::var("AUI_GALLERY_STEPS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl Render for AssistantMock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus requests parked by the async script (which holds no Window).
        if let Some(handle) = self.restore_focus.take() {
            window.focus(&handle, cx);
        } else if let Some(target) = self.want_focus.take() {
            let handle = match target {
                Focus::Composer => self.composer.focus_handle(cx),
                Focus::Palette => self.focus_palette.clone(),
                Focus::Approval => self.focus_approval.clone(),
                Focus::Question => self.focus_question.clone(),
            };
            window.focus(&handle, cx);
        }
        if !self.started {
            self.started = true;
            if !self.steps.is_empty() {
                self.start_steps(window, cx);
            }
        }

        // `dtabs()` keeps the two chat-created files and adds the open PDF
        // only when it is the one being read.
        let mut tabs = vec![
            TabItem::new("doc", "RFP-draft-v3.docx", IconName::Doc).closable(false),
            TabItem::new("sheet", "vendor-scoring.xlsx", IconName::Sheet).closable(false),
        ];
        if self.right_tab == RightTab::Pdf {
            tabs.push(TabItem::new("pdf", "procurement-rules-2019.pdf", IconName::Pdf).closable(false));
        }
        let active = match self.right_tab {
            RightTab::Doc => 0,
            RightTab::Sheet => 1,
            RightTab::Pdf => 2,
        };
        let strip = tab_strip("assistant-tabs", tabs, active).on_select(cx.listener(|this, id: &SharedString, _, cx| {
            this.right_tab = match id.as_ref() {
                "sheet" => RightTab::Sheet,
                "pdf" => RightTab::Pdf,
                _ => RightTab::Doc,
            };
            cx.notify();
        }));
        let sidebar = self.render_sidebar(cx);
        let rail = self.render_rail(cx);
        let transcript = self.render_transcript(window, cx);
        let composer = self.render_composer(cx);
        let right = self.render_right(cx);
        let palette = self.render_palette(cx);
        let toasts = self.render_toasts(window, cx);
        div()
            .size_full()
            .relative()
            .key_context(aui::keys::ROOT_CONTEXT)
            .track_focus(&self.focus_root)
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleRightPane, _, cx| {
                this.right_open = !this.right_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &TogglePalette, window, cx| this.toggle_palette(window, cx)))
            .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                if this.palette.is_some() {
                    this.close_palette(cx);
                } else if this.plus_open {
                    this.plus_open = false;
                    cx.notify();
                }
            }))
            // Tab is keyboard navigation, so it arms the focus ring.
            .on_action(|_: &FocusNext, window, cx| {
                aui::keys::set_keyboard_nav(true, cx);
                window.focus_next(cx);
            })
            .on_action(|_: &FocusPrev, window, cx| {
                aui::keys::set_keyboard_nav(true, cx);
                window.focus_prev(cx);
            })
            .child(
                app_shell("assistant-shell")
                    .traffic_lights(true)
                    .right_open(self.right_open)
                    .sidebar_open(self.sidebar_open)
                    .header_sidebar(
                        sidebar_header("assistant-hd-side").traffic_lights(true).collapsed(!self.sidebar_open).on_toggle_sidebar(cx.listener(
                            |this, _, _, cx| {
                                this.sidebar_open = !this.sidebar_open;
                                cx.notify();
                            },
                        )),
                    )
                    .header_centre({
                        let mut centre = centre_header("assistant-hd-centre", "Teacher recruitment RFP")
                            .glyph(IconName::GradCap)
                            .branch(self.screen.branch())
                            .on_toggle_right(cx.listener(|this, _, _, cx| {
                                this.right_open = !this.right_open;
                                cx.notify();
                            }));
                        if !self.sidebar_open {
                            centre = centre.on_expand_sidebar(cx.listener(|this, _, _, cx| {
                                this.sidebar_open = true;
                                cx.notify();
                            }));
                        }
                        centre
                    })
                    .header_right(
                        right_header("assistant-hd-right")
                            .tabs(strip)
                            .on_add(|_, _, _| {})
                            .on_close(cx.listener(|this, _, _, cx| {
                                this.right_open = false;
                                cx.notify();
                            })),
                    )
                    .sidebar(sidebar)
                    .rail(rail)
                    .centre(v_flex().size_full().child(transcript).child(composer))
                    .right(right),
            )
            .children(palette)
            .children(toasts)
    }
}

/// One mock in window state, keyed so the `screens/all` page can hold three.
pub fn mock(key: &'static str, screen: AssistantScreen, scripted: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state(SharedString::from(key), cx, move |window, cx| AssistantMock::new(screen, scripted, window, cx));
    div().size_full().child(view).into_any_element()
}

/// The `screens/assistant` entry: the screen named by `AUI_GALLERY_SCREEN`.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    mock("assistant-mock", AssistantScreen::from_env(), true, window, cx)
}

/// `screens/all`: the three assistant screens side by side at their reference
/// size. gpui cannot scale an element, so the row scrolls horizontally rather
/// than shrinking the screens.
pub fn build_all(window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.aui().colors;
    let mut row = h_flex().h_full().items_start().gap(px(SCREENS_GAP)).px(px(SCREENS_PAD_X)).py(px(SCREENS_PAD_Y));
    for (key, screen) in [
        ("assistant-mock-main", AssistantScreen::Main),
        ("assistant-mock-sources", AssistantScreen::Sources),
        ("assistant-mock-sheet", AssistantScreen::Sheet),
    ] {
        row = row.child(
            v_flex()
                .flex_none()
                .w(px(SCREEN_W))
                .gap(px(scale::SP_3))
                .child(
                    h_flex()
                        .h(px(SCREENS_CAPTION))
                        .items_center()
                        .gap(px(scale::SP_3))
                        .child(div().ui(scale::FS_13).semibold().text_color(p.ink).child(screen.caption()))
                        .child(div().mono(scale::FS_11).text_color(p.ink_3).child("1440\u{d7}900")),
                )
                .child(div().w(px(SCREEN_W)).h(px(SCREEN_H)).flex_none().overflow_hidden().child(mock(key, screen, false, window, cx))),
        );
    }
    div().id("screens-all").size_full().overflow_x_scroll().child(row).into_any_element()
}
