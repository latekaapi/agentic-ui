//! The stateful mock view.

use aui::composer::{composer, composer_state_rows, ComposerIntent};
use aui::data::{button, status_dot};
use aui::nav::{role_section, sidebar_footer, Project, Role, RoleSession, SessionKind};
use aui::protocol::{ActivityState, Step, StepState};
use aui::shell::{app_shell, centre_header, right_header, sidebar_header, tab_strip, TabItem};
use aui::transcript::{activity_group, prose, user_turn, ProseStyle};
use aui_icons::{icon, IconName, Provider, RoleIcon};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled, TextRole};
use gpui::*;
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::{h_flex, v_flex};

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
/// The assistant body text: 13.5 / 1.65.
const BODY_TEXT: f32 = 13.5;

/// Which pane the right column shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    /// The RFP draft (docx).
    Doc,
    /// The vendor scoring sheet (xlsx).
    Sheet,
}

/// The mock's state.
pub struct AssistantMock {
    right_open: bool,
    sidebar_open: bool,
    right_tab: RightTab,
    open_roles: [bool; 3],
    active_session: SharedString,
    composer: Entity<TextareaState>,
    plus_open: bool,
    streaming: bool,
}

impl AssistantMock {
    /// Creates the mock with the sample state of the `assistant-Main` screen.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| composer_state_rows("Ask, draft, or type / for commands", 1, 8, window, cx));
        Self {
            right_open: true,
            sidebar_open: true,
            right_tab: RightTab::Doc,
            open_roles: [true, false, false],
            active_session: "rfp-v3".into(),
            composer,
            plus_open: false,
            streaming: false,
        }
    }

    fn roles() -> Vec<Role> {
        vec![
            Role::new("education", "Director, Education", RoleIcon::GradCap, 3)
                .open()
                .project(
                    Project::new("rfp", "Teacher recruitment RFP", 6)
                        .session(RoleSession::new("rfp-v3", "RFP draft v3", SessionKind::Document, "now"))
                        .session(RoleSession::new("eligibility", "Eligibility criteria review", SessionKind::Chat, "2h"))
                        .session(RoleSession::new("scoring", "Vendor scoring sheet", SessionKind::Sheet, "1d")),
                )
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
        for (i, mut role) in Self::roles().into_iter().enumerate() {
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
        col.child(div().flex_1()).child(sidebar_footer("assistant-footer", "B", "Bharani").meter(Provider::Claude, 0.78).pad_y(8.0))
    }

    fn render_transcript(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let steps = vec![
            Step { verb: "Searched".into(), target: "Education Code".into(), state: StepState::Done, result: Some("4 passages".into()) },
            Step { verb: "Searched".into(), target: "Procurement Rules 2019".into(), state: StepState::Done, result: Some("5 passages".into()) },
            Step { verb: "Searched".into(), target: "GO 2024-18".into(), state: StepState::Done, result: Some("2 passages".into()) },
        ];
        let answer = "Under the current rules a bidder needs a valid registration and three years of comparable placements [1]. The 2024 order moved certificate verification to onboarding and requires originals [2]. The brief sets the cohort at 240 teachers across 38 institutions [3]. I folded all three into section 2 and left the rest of the draft untouched.";
        v_flex()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .pt(px(TRANSCRIPT_PAD_TOP))
            .px(px(TRANSCRIPT_PAD_X))
            .gap(px(BLOCK_GAP))
            .child(div().w_full().flex().justify_end().child(user_turn(
                "assistant-user-1",
                "Tighten section 2. Eligibility must reflect the current procurement rules and the 2024 verification order.",
            )))
            .child(
                activity_group("assistant-activity", steps, "Searched 3 knowledge sets", "6 s", ActivityState::Done)
                    .detail("· read 2 regulations · 11 passages")
                    .open(false),
            )
            .child(prose("assistant-answer", answer, ProseStyle { ink: p.ink, code_ink: p.accent_ink, code_bg: p.accent_soft, size: BODY_TEXT, line_height: scale::LH_BODY, paragraph_gap: 10.0 }))
            .child(self.file_card(cx))
            .child(div().flex_1())
            .child(
                h_flex()
                    .w_full()
                    .pb(px(STATUS_PAD_BOTTOM))
                    .gap(px(scale::SP_3))
                    .ui(scale::FS_12)
                    .text_color(p.ink_3)
                    .child(status_dot("assistant-status-dot", AgentState::Done))
                    .child("Done")
                    .child("·")
                    .child("3 sources cited"),
            )
    }

    /// `.fc`: the artifact card under the answer.
    fn file_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        h_flex()
            .w_full()
            .gap(px(FILE_CARD_GAP))
            .py(px(FILE_CARD_PAD_Y))
            .px(px(FILE_CARD_PAD_X))
            .rounded(px(scale::R_LG))
            .border_1()
            .border_color(p.line)
            .bg(p.surface_1)
            .child(div().flex_none().size(px(FILE_CARD_TILE)).rounded(px(FILE_CARD_TILE_RADIUS)).bg(p.accent_soft).flex().items_center().justify_center().child(icon(IconName::Doc).color(p.accent_ink)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(0.0))
                    .child(div().ui(scale::FS_13).semibold().text_color(p.ink).child("RFP-draft-v3.docx"))
                    .child(div().ui(scale::FS_12).text_color(p.ink_3).child("Section 2 rewritten · 3 changes highlighted · v3")),
            )
            .child(button("assistant-download", "Download").ghost().sm())
            .child(button("assistant-open-pane", "Open in pane").sm().on_click(cx.listener(|this, _, _, cx| {
                this.right_open = true;
                this.right_tab = RightTab::Doc;
                cx.notify();
            })))
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        composer("assistant-composer", &self.composer, Provider::Claude, "Opus 4.6")
            .docked(true)
            .mode("Education + project")
            .streaming(self.streaming)
            .plus_menu(self.plus_open, None::<Div>)
            .on_intent({
                let handler = cx.listener(|this, intent: &ComposerIntent, _, cx| {
                    match intent {
                        ComposerIntent::TogglePlus => this.plus_open = !this.plus_open,
                        ComposerIntent::Send | ComposerIntent::Stop => this.streaming = !this.streaming,
                        _ => {}
                    }
                    cx.notify();
                });
                move |intent, w, cx| handler(&intent, w, cx)
            })
    }

    fn render_right(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        // The document pane arrives with card 54; until then the pane shows its tab's name.
        let label = match self.right_tab {
            RightTab::Doc => "RFP-draft-v3.docx",
            RightTab::Sheet => "vendor-scoring.xlsx",
        };
        v_flex().size_full().items_center().justify_center().text_role(TextRole::UiSmall).text_color(p.ink_3).child(label)
    }
}

impl Render for AssistantMock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = vec![
            TabItem::new("doc", "RFP-draft-v3.docx", IconName::Doc).closable(false),
            TabItem::new("sheet", "vendor-scoring.xlsx", IconName::Sheet).closable(false),
        ];
        let active = match self.right_tab {
            RightTab::Doc => 0,
            RightTab::Sheet => 1,
        };
        let strip = tab_strip("assistant-tabs", tabs, active).on_select(cx.listener(|this, id: &SharedString, _, cx| {
            this.right_tab = if id.as_ref() == "sheet" { RightTab::Sheet } else { RightTab::Doc };
            cx.notify();
        }));
        let sidebar = self.render_sidebar(cx);
        let transcript = self.render_transcript(window, cx);
        let composer = self.render_composer(cx);
        let right = self.render_right(cx);
        div().size_full().child(
            app_shell("assistant-shell")
                .right_open(self.right_open)
                .sidebar_open(self.sidebar_open)
                .header_sidebar(sidebar_header("assistant-hd-side").traffic_lights(true).on_toggle_sidebar(cx.listener(|this, _, _, cx| {
                    this.sidebar_open = !this.sidebar_open;
                    cx.notify();
                })))
                .header_centre(
                    centre_header("assistant-hd-centre", "Teacher recruitment RFP")
                        .glyph(IconName::GradCap)
                        .branch("RFP draft v3")
                        .on_toggle_right(cx.listener(|this, _, _, cx| {
                            this.right_open = !this.right_open;
                            cx.notify();
                        })),
                )
                .header_right(right_header("assistant-hd-right").tabs(strip).on_close(cx.listener(|this, _, _, cx| {
                    this.right_open = false;
                    cx.notify();
                })))
                .sidebar(sidebar)
                .centre(v_flex().size_full().child(transcript).child(composer))
                .right(right),
        )
    }
}

/// The gallery entry: holds the mock view in window state.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state("assistant-mock", cx, |window, cx| AssistantMock::new(window, cx));
    div().size_full().child(view).into_any_element()
}
