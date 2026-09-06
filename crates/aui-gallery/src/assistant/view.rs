//! The stateful mock view.

use aui::composer::{composer, composer_state_rows, ComposerIntent};
use aui::data::{button, status_dot};
use aui::nav::{rail, role_section, sidebar_footer, Project, RailItem, Role, RoleSession, SessionKind};
use aui::protocol::{ActivityState, Step, StepState};
use aui::shell::{app_shell, centre_header, right_header, sidebar_header, tab_strip, TabItem};
use aui::transcript::{activity_group, user_turn, ProseStyle};
use aui::workbench::{
    artifact_strip, cited_answer, doc_pane, doc_toolbar, pane_status, pane_status_row, pdf_pane, sheet_pane, source_hover_card, sources_card, Artifact, ArtifactKind, DocBlock, DocPage,
    DocRun, PdfPage, PdfRun, SheetCell, Source, SourceTier,
};
use aui_icons::{icon, IconName, Provider, RoleIcon};
use aui_tokens::{scale, ActiveAui, AgentState, AuiStyled};
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
/// The assistant body text: 13.5 / 1.65.
const BODY_TEXT: f32 = 13.5;

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

/// The mock's state.
pub struct AssistantMock {
    screen: AssistantScreen,
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
    /// Creates the mock in the sample state of one assistant screen.
    pub fn new(screen: AssistantScreen, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| composer_state_rows(screen.placeholder(), 1, 8, window, cx));
        Self {
            screen,
            right_open: true,
            sidebar_open: !crate::cards::app_shell::collapsed_by_default(),
            right_tab: screen.right_tab(),
            open_roles: [true, false, false],
            active_session: screen.session().into(),
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

    /// The collapsed sidebar: one cell per role, then the open project's
    /// sessions by kind. No sliver of the expanded sidebar is drawn here.
    fn render_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let roles = Self::roles();
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

    fn render_transcript(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let style = ProseStyle { ink: p.ink, code_ink: p.accent_ink, code_bg: p.accent_soft, size: BODY_TEXT, line_height: scale::LH_BODY, paragraph_gap: 10.0 };
        let (prompt, answer, status) = match self.screen {
            AssistantScreen::Main => (
                "Tighten section 2. Eligibility must reflect the current procurement rules and the 2024 verification order.",
                "Under the current rules a bidder needs a valid registration and three years of comparable placements[[1]]. The 2024 order moved certificate verification to onboarding and requires originals[[2]]. The brief sets the cohort at 240 teachers across 38 institutions[[3]]. I folded all three into section 2 and left the rest of the draft untouched.",
                "3 sources cited",
            ),
            AssistantScreen::Sources => (
                "What does a bidder need to qualify, and where does that come from?",
                "Three requirements, each from a different source. A valid registration with the Directorate and at least three years of comparable placements in the preceding five[[1]]. Original certificates verified at onboarding rather than at bid time[[2]]. And for this RFP specifically, capacity for 240 teachers across 38 institutions[[3]]. Nothing in the Education Code adds to this; it defers to the procurement rules.",
                "hovering citation 1 \u{b7} source open on the right",
            ),
            AssistantScreen::Sheet => (
                "Score the four vendors against the Annex A matrix. Weight experience 45, coverage 30, price 25.",
                "Northlight leads on the weighted score, mostly on experience. Civic Talent is cheapest but thin on experience, which the matrix penalises hardest. The weights sit in row 7 so you can change them and the totals follow.",
                "E2 selected in the sheet",
            ),
        };
        let done = |verb: &str, target: &str, result: Option<&str>| Step {
            verb: verb.to_string(),
            target: target.to_string(),
            state: StepState::Done,
            result: result.map(|r| r.to_string()),
        };
        let activity = match self.screen {
            AssistantScreen::Sheet => activity_group(
                "assistant-activity",
                vec![done("Read", "Annex A", None), done("Built", "the scoring matrix", None), done("Wrote", "vendor-scoring.xlsx", None)],
                "Read Annex A",
                "14 s",
                ActivityState::Done,
            )
            .detail("\u{b7} built the matrix \u{b7} wrote vendor-scoring.xlsx")
            .open(false),
            _ => activity_group(
                "assistant-activity",
                vec![
                    done("Searched", "Education Code", Some("4 passages")),
                    done("Searched", "Procurement Rules 2019", Some("5 passages")),
                    done("Searched", "GO 2024-18", Some("2 passages")),
                ],
                "Searched 3 knowledge sets",
                "6 s",
                ActivityState::Done,
            )
            .detail("\u{b7} read 2 regulations \u{b7} 11 passages")
            .open(false),
        };
        // `assistant-Sources` closes with the sources card and the hover card
        // held open over citation 1; the other two close with the artifact card.
        let body: AnyElement = match self.screen {
            AssistantScreen::Sources => {
                let start = SOURCE_QUOTE.find(SOURCE_HIGHLIGHT).unwrap_or(0);
                let hover = source_hover_card("assistant-hover", "Rule 14(2) \u{b7} Eligibility of bidders", SOURCE_QUOTE, start..start + SOURCE_HIGHLIGHT.len(), 31).at_rest();
                sources_card("assistant-sources", Self::source_tiers()).cited(3).retrieved(11).hover_card(0, hover).into_any_element()
            }
            _ => self.file_card(cx).into_any_element(),
        };
        v_flex()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .pt(px(TRANSCRIPT_PAD_TOP))
            .px(px(TRANSCRIPT_PAD_X))
            .gap(px(BLOCK_GAP))
            .child(div().w_full().flex().justify_end().child(user_turn("assistant-user-1", prompt)))
            .child(activity)
            .child(cited_answer("assistant-answer", answer, style).streaming(self.streaming))
            .child(body)
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
                    .child("\u{b7}")
                    .child(status),
            )
    }

    /// `.fc`: the artifact card under the answer. The tile is surface-2 with
    /// the file type's tint, as `file_card()` builds it in the screen source.
    fn file_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.aui().colors;
        let sheet = self.screen == AssistantScreen::Sheet;
        let (glyph, tint, name, desc) = if sheet {
            (IconName::Sheet, p.success, "vendor-scoring.xlsx", "Scores, Matrix and Notes sheets \u{b7} formulas live \u{b7} v1")
        } else {
            (IconName::Doc, p.info, "RFP-draft-v3.docx", "Section 2 rewritten \u{b7} 3 changes highlighted \u{b7} v3")
        };
        let tab = if sheet { RightTab::Sheet } else { RightTab::Doc };
        h_flex()
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
        composer("assistant-composer", &self.composer, Provider::Claude, "Opus 4.6")
            .docked(true)
            .mode("Education + project")
            .knowledge_first(true)
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
        let strip = artifact_strip("assistant-artifacts", self.artifacts(self.right_tab == RightTab::Doc)).on_select(cx.listener(|this, id: &SharedString, _, cx| {
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
                    .child(doc_toolbar("assistant-doc-toolbar", "Body text", "Georgia · 11").ask_label("Ask"))
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
                    .child(div().flex_1().min_h(px(0.0)).w_full().child(pdf_pane("assistant-pdf", page, 31, 88).cited_as(1)))
                    .child(pane_status_row("assistant-pdf-status", vec![pane_status("Highlight from citation 1")], vec![pane_status("Opened from chat")]))
                    .into_any_element()
            }
        }
    }
}

impl Render for AssistantMock {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        div().size_full().child(
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
    }
}

/// One mock in window state, keyed so the `screens/all` page can hold three.
pub fn mock(key: &'static str, screen: AssistantScreen, window: &mut Window, cx: &mut App) -> AnyElement {
    let view = window.use_keyed_state(SharedString::from(key), cx, move |window, cx| AssistantMock::new(screen, window, cx));
    div().size_full().child(view).into_any_element()
}

/// The `screens/assistant` entry: the screen named by `AUI_GALLERY_SCREEN`.
pub fn build(window: &mut Window, cx: &mut App) -> AnyElement {
    mock("assistant-mock", AssistantScreen::from_env(), window, cx)
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
                .child(div().w(px(SCREEN_W)).h(px(SCREEN_H)).flex_none().overflow_hidden().child(mock(key, screen, window, cx))),
        );
    }
    div().id("screens-all").size_full().overflow_x_scroll().child(row).into_any_element()
}
