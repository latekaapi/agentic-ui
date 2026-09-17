//! A minimal `aui` consumer: one window, a three-pane shell, a live transcript.
//!
//! Run it with `cargo run -p aui --example minimal`.
//!
//! The tutorial for `docs/08-getting-started.md`: every step an application
//! actually needs, and nothing else.
//!
//! 1. **Init order.** `with_assets(aui::assets::AuiAssets)` installs the icon set;
//!    `aui::init(theme, cx)` then runs `gpui_kit::init`, loads the bundled fonts,
//!    registers the themes and binds the keymap in [`aui::keys`] — in that order,
//!    once, before any window opens.
//! 2. **Data in, intents out.** The app owns an `aui_protocol::Session`; components
//!    are stateless `RenderOnce` values that read it and hand intents back through
//!    closures. No component holds state or does I/O.
//! 3. **Streaming.** A backend (here a timer loop) emits `Delta`s; `Session::apply`
//!    folds them in and `aui_motion::stream_reveal` fades each new block in.
//! 4. **Keyboard.** The root carries `keys::ROOT_CONTEXT`; the pending approval
//!    card carries `APPROVAL_CONTEXT` and resolves on Y / A / N or on a click.
//!
//! Everything visual comes from the tokens: `cx.aui().colors` for colour,
//! `aui_tokens::scale::*` for size and radius, `AuiStyled` for type — no literal
//! hex value and no hand-picked duration anywhere below.

use std::time::Duration;

use aui::composer::{composer, composer_state, ComposerIntent};
use aui::data::{ContextMeterState, ContextPressure};
use aui::keys::{ApproveAlways, ApproveOnce, Cancel, Deny as DenyAction, FocusNext, FocusPrev, ToggleRightPane, ToggleSidebar};
use aui::nav::{group_header, nav_item, sidebar_footer};
use aui::protocol::{ApprovalDecision, ApprovalScope, ApprovalState, Block, Delta, PermissionMode, Provider, Session, Turn, TurnMeta};
use aui::shell::{app_shell, centre_header, right_header, sidebar_header};
use aui::transcript::{approval_card, user_turn, ProseStyle};
use aui::workbench::cited_answer;
use aui_icons::{IconName, Provider as ProviderMark};
use aui_motion::stream_reveal;
use aui_tokens::{scale, ActiveAui, AuiStyled, ThemeKind};
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, ElementId, Entity, FocusHandle, Focusable, SharedString, Task, Window, WindowBounds, WindowOptions};
use gpui_kit::base::input::TextareaState;
use gpui_kit::base::v_flex;
use gpui_kit::component::Root;

/// The reply already in the transcript when the window opens.
const SEED_REPLY: &str = "Section 2 still cites the 2015 rules and says certificates are checked at bid time. \
    Both are out of date. Ask me to rewrite it and I will show you the change first.";
/// The rule an "always allow" would remember.
const APPROVAL_RULE: &str = "Write *.docx";
/// The reply the fake backend streams back, one word group per tick.
const CANNED_REPLY: &str = "I read the two files you pointed at and rewrote the eligibility paragraph so it \
    matches the current rules. The change is scoped to section 2; nothing else in the draft moved. \
    Say the word and I will do the same pass over the annexes.";
/// One word group of the reply per tick — the shape a token stream has.
const WORD_TIME: Duration = Duration::from_millis(45);
/// How long the fake backend "thinks" before the first token.
const THINK_TIME: Duration = Duration::from_millis(320);
/// Body type for assistant prose: 13.5 px on the 1.65 body leading.
const BODY_TEXT: f32 = 13.5;
/// The transcript's own padding, the one place this example sets geometry.
const TRANSCRIPT_PAD: f32 = scale::SP_7;

/// The whole application state. Two things matter here: the session is plain
/// [`aui_protocol`] data with no gpui in it, and everything else is view state
/// (what is open, what has the keyboard).
struct MinimalApp {
    /// The transcript, exactly as a real backend would hand it over.
    session: Session,
    /// How many word groups of the streaming block have arrived.
    revealed: usize,
    /// The composer's text buffer, owned by gpui-kit.
    composer: Entity<TextareaState>,
    sidebar_open: bool,
    right_open: bool,
    /// Bumped by every send so a stream from an abandoned turn stops writing.
    run: u64,
    focus_root: FocusHandle,
    focus_approval: FocusHandle,
    /// Set when the next frame should move the keyboard to the approval card.
    focus_pending_approval: bool,
    /// The composer takes the keyboard on the first frame.
    focus_composer: bool,
    /// A `Task` only runs while it is held, so the timers live here.
    tasks: Vec<Task<()>>,
}

/// A user turn is the one shape this example builds twice.
fn user_turn_data(id: impl Into<String>, text: impl Into<String>) -> Turn {
    Turn::User { id: id.into(), text: text.into(), attachments: Vec::new(), mentions: Vec::new(), timestamp: None }
}

impl MinimalApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new_with(window, cx, false)
    }

    /// Like [`Self::new`], but when `show_approval` is set it also drives the
    /// composer's `Send` path once, synchronously, so the window opens with
    /// the pending approval card already up — the state `docs/08-getting-started.md`'s
    /// `images/minimal-approval.png` shows. Used by `--screenshot-approval`.
    fn new_with(window: &mut Window, cx: &mut Context<Self>, show_approval: bool) -> Self {
        // gpui-kit's textarea, with the composer card's auto-grow row limits.
        let composer = cx.new(|cx| composer_state("Ask, draft, or type / for commands", window, cx));
        let mut session = Session::new("demo", Provider::Claude, "Opus 4.6", "~/work/rfp");
        (session.mode, session.branch) = (PermissionMode::PromptUnmatched, Some("main".into()));
        // Seed one exchange so the window is not empty on open.
        session.turns.push(user_turn_data("t0", "Open the RFP draft and tell me what section 2 currently says."));
        session.turns.push(Turn::Assistant {
            id: "t1".into(),
            blocks: vec![Block::Text { text: SEED_REPLY.into(), streaming: false }],
            meta: TurnMeta { model: "Opus 4.6".into(), duration_ms: 2_100, tokens_in: 1_840, tokens_out: 96, reasoning_tokens: 0, cost_usd: 0.014 },
            timestamp: None,
        });
        let (focus_root, focus_approval) = (cx.focus_handle(), cx.focus_handle());
        let mut this = Self { session, revealed: 0, composer, sidebar_open: true, right_open: false, run: 0, focus_root, focus_approval, focus_pending_approval: false, focus_composer: true, tasks: Vec::new() };
        if show_approval {
            this.composer.update(cx, |state, cx| state.set_value("Rewrite section 2 against the current procurement rules.", window, cx));
            this.send(window, cx);
        }
        this
    }

    /// `ComposerIntent::Send`: append the person's turn, then open an assistant
    /// turn whose first block is a permission request. Nothing streams until
    /// that request is answered — this is the whole approval loop.
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).value();
        if text.trim().is_empty() {
            return;
        }
        self.composer.update(cx, |state, cx| state.set_value("", window, cx));
        self.run += 1;
        let run = self.run;
        // Every one of these is a `Delta` a real adapter would emit over its wire.
        self.session.apply(Delta::TurnStarted { turn: user_turn_data(format!("u{run}"), text) });
        self.session.apply(Delta::TurnStarted { turn: Turn::Assistant { id: format!("a{run}"), blocks: Vec::new(), meta: TurnMeta::default(), timestamp: None } });
        self.session.apply(Delta::BlockAdded {
            turn_id: format!("a{run}"),
            block: Block::approval(
                format!("ap{run}"),
                "Write file",
                "Write RFP-draft-v3.docx",
                "Section 2 has to be rewritten against the current procurement rules.",
                "~/work/rfp",
                vec!["write files".into()],
                ApprovalScope::ThisWorktree,
                ApprovalState::Pending,
                Some(APPROVAL_RULE.into()),
            ),
        });
        self.focus_pending_approval = true;
        cx.notify();
    }

    /// Y / A / N on the card, or a click on one of its buttons. Both arrive
    /// here as the same [`ApprovalDecision`].
    fn decide(&mut self, decision: ApprovalDecision, window: &mut Window, cx: &mut Context<Self>) {
        let run = self.run;
        let turn_id = format!("a{run}");
        let Some(index) = self.pending_approval() else { return };
        let Some(mut block) = self.session.turn(&turn_id).and_then(|t| t.blocks().get(index)).cloned() else { return };
        if let Block::Approval { state, .. } = &mut block {
            // The example only emits the built-in triad; `ApprovalDecision`
            // carries the wider MSP set, so the rest fall through to a refusal.
            *state = match decision {
                ApprovalDecision::Once | ApprovalDecision::ApprovedForSession => {
                    ApprovalState::AllowedOnce { exit_code: 0, duration_ms: 380 }
                }
                ApprovalDecision::Always | ApprovalDecision::PolicyAmendment => {
                    ApprovalState::AutoAllowed { rule: APPROVAL_RULE.into() }
                }
                _ => ApprovalState::Denied,
            };
        }
        // A decision is a `BlockUpdated` delta: the card is replaced in place,
        // which is what plays it from pending to resolved.
        self.session.apply(Delta::BlockUpdated { turn_id: turn_id.clone(), block_index: index, block });
        window.focus(&self.composer.focus_handle(cx), cx);
        if matches!(decision, ApprovalDecision::Deny | ApprovalDecision::DeniedPolicyAmendment | ApprovalDecision::TimedOut | ApprovalDecision::Abort) {
            self.session.apply(Delta::BlockAdded { turn_id, block: Block::Text { text: "Left the draft alone.".into(), streaming: false } });
        } else {
            self.stream_reply(run, window, cx);
        }
        cx.notify();
    }

    /// The fake backend. A real one would push these same deltas from a socket
    /// or a child process; the UI cannot tell the difference.
    fn stream_reply(&mut self, run: u64, window: &mut Window, cx: &mut Context<Self>) {
        let turn_id = format!("a{run}");
        self.session.apply(Delta::BlockAdded { turn_id: turn_id.clone(), block: Block::Text { text: String::new(), streaming: true } });
        self.revealed = 0;
        let groups: Vec<String> = CANNED_REPLY.split_whitespace().map(str::to_string).collect();
        let this = cx.entity().downgrade();
        let task = window.spawn(cx, async move |cx| {
            cx.background_executor().timer(THINK_TIME).await;
            for group in groups {
                cx.background_executor().timer(WORD_TIME).await;
                let alive = this
                    .update(cx, |this, cx| {
                        // The turn was abandoned; drop the rest of the stream.
                        if this.run != run {
                            return false;
                        }
                        let Some(index) = this.streaming_block(&turn_id) else { return false };
                        let chunk = if this.revealed == 0 { group.clone() } else { format!(" {group}") };
                        this.session.apply(Delta::TextDelta { turn_id: turn_id.clone(), block_index: index, text: chunk });
                        this.revealed += 1;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    return;
                }
            }
            // `TurnFinished` clears every `streaming` flag on the turn, which
            // is what puts the caret away and settles the footer meta.
            let _ = this.update(cx, |this, cx| {
                this.session.apply(Delta::TurnFinished {
                    turn_id,
                    meta: TurnMeta { model: "Opus 4.6".into(), duration_ms: 4_800, tokens_in: 2_310, tokens_out: 214, reasoning_tokens: 0, cost_usd: 0.021 },
                });
                cx.notify();
            });
        });
        self.tasks.push(task);
    }

    /// The index of the pending approval block in the newest assistant turn.
    fn pending_approval(&self) -> Option<usize> {
        self.session.turns.last()?.blocks().iter().position(|b| matches!(b, Block::Approval { state: ApprovalState::Pending, .. }))
    }

    /// The index of the block still receiving text in `turn_id`.
    fn streaming_block(&self, turn_id: &str) -> Option<usize> {
        self.session.turn(turn_id)?.blocks().iter().rposition(|b| matches!(b, Block::Text { streaming: true, .. }))
    }

    /// Whether anything is still arriving; the composer shows a stop button
    /// instead of a send button while it is true.
    fn streaming(&self) -> bool {
        self.session.turns.last().is_some_and(|t| t.blocks().iter().any(|b| matches!(b, Block::Text { streaming: true, .. })))
    }

    /// The two pane toggles, reached from the keymap and from the headers.
    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) { self.sidebar_open = !self.sidebar_open; cx.notify(); }
    fn toggle_right(&mut self, cx: &mut Context<Self>) { self.right_open = !self.right_open; cx.notify(); }

    /// A few `nav` rows under a group header, plus the account footer. Real
    /// apps put `nav::role_section` or `nav::sidebar` here.
    fn render_sidebar(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .px(px(scale::SP_3))
            .child(group_header("nav-group", "Workspace", true))
            .child(nav_item("nav-inbox", IconName::Inbox, "Inbox").count("3").count_warning())
            .child(nav_item("nav-sessions", IconName::List, "Sessions").count("12"))
            .child(nav_item("nav-files", IconName::Folder, "Files"))
            .child(div().flex_1())
            .child(sidebar_footer("nav-footer", "A", "Alex Rivera").meter(ProviderMark::Claude, 0.62))
    }

    /// One transcript block. Every newly arrived block fades in and rises 3 px
    /// through [`stream_reveal`]; blocks that were already there when the app
    /// opened pass `settled = true` and skip the animation.
    fn render_block(&self, id: ElementId, index: usize, settled: bool, block: &Block, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.aui().colors;
        let reveal = stream_reveal(id.clone(), index, settled, window, cx);
        let body: AnyElement = match block {
            Block::Text { text, streaming } => {
                let style = ProseStyle { ink: p.ink, code_ink: p.accent_ink, code_bg: p.accent_soft, size: BODY_TEXT, line_height: scale::LH_BODY, paragraph_gap: scale::SP_4 };
                div().w_full().child(cited_answer(id.clone(), text.clone(), style).streaming(*streaming)).into_any_element()
            }
            Block::Approval { tool, command, reason, cwd, capabilities, state, rule, .. } => {
                let decide = cx.listener(|this, decision: &ApprovalDecision, window, cx| this.decide(*decision, window, cx));
                let card = approval_card(id.clone(), tool.clone(), command.clone(), state.clone())
                    .reason(reason.clone())
                    .cwd(cwd.clone())
                    .capabilities(capabilities.clone())
                    .rule(rule.clone().unwrap_or_default())
                    .on_decide(move |d, w, cx| decide(&d, w, cx));
                let mut holder = div().w_full();
                // Only the pending card owns the keyboard: `APPROVAL_CONTEXT`
                // is what makes Y / A / N reach these handlers.
                if *state == ApprovalState::Pending {
                    holder = holder
                        .key_context(aui::keys::APPROVAL_CONTEXT)
                        .track_focus(&self.focus_approval)
                        .on_action(cx.listener(|this, _: &ApproveOnce, w, cx| this.decide(ApprovalDecision::Once, w, cx)))
                        .on_action(cx.listener(|this, _: &ApproveAlways, w, cx| this.decide(ApprovalDecision::Always, w, cx)))
                        .on_action(cx.listener(|this, _: &DenyAction, w, cx| this.decide(ApprovalDecision::Deny, w, cx)));
                }
                holder.child(card).into_any_element()
            }
            // The other variants are cards too (`activity_group`, `tool_card`,
            // `question_card`, `code_block`…); this example makes only two.
            _ => div().into_any_element(),
        };
        div().w_full().relative().top(reveal.offset_y).opacity(reveal.opacity).child(body).into_any_element()
    }

    fn render_transcript(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // `min_h(0)` is what lets a flex child actually scroll in gpui.
        let mut list = div().id("transcript").flex_1().min_h(px(0.0)).w_full().overflow_y_scroll().flex().flex_col().p(px(TRANSCRIPT_PAD)).gap(px(scale::SP_5));
        let last = self.session.turns.len().saturating_sub(1);
        for (t, turn) in self.session.turns.iter().enumerate() {
            match turn {
                // A user turn is one right-aligned bubble.
                Turn::User { id, text, .. } => list = list.child(div().w_full().flex().justify_end().child(user_turn(SharedString::from(id.clone()), text.clone()))),
                Turn::Assistant { id, blocks, .. } => {
                    for (b, block) in blocks.iter().enumerate() {
                        let element_id = ElementId::from(SharedString::from(format!("{id}-{b}")));
                        // Only the newest turn animates; history is settled.
                        list = list.child(self.render_block(element_id, b, t != last, block, window, cx));
                    }
                }
            }
        }
        list
    }

    fn render_right(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (p, note) = (cx.aui().colors, "Put the document, terminal or browser pane here. \u{2318}\\ closes it.");
        v_flex().size_full().p(px(scale::SP_6)).gap(px(scale::SP_3)).child(div().text_role(aui_tokens::TextRole::Title).text_color(p.ink).child("Right pane")).child(div().ui(scale::FS_12).text_color(p.ink_3).child(note))
    }
}

impl Render for MinimalApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // gpui focuses through a `Window` the async stream never holds, so
        // focus requests are parked on `self` and applied here.
        if std::mem::take(&mut self.focus_pending_approval) {
            window.focus(&self.focus_approval, cx);
        } else if std::mem::take(&mut self.focus_composer) {
            window.focus(&self.composer.focus_handle(cx), cx);
        }
        let transcript = self.render_transcript(window, cx);
        let sidebar = self.render_sidebar(cx);
        let right = self.render_right(cx);
        let streaming = self.streaming();
        let composer = composer("composer", &self.composer, ProviderMark::Claude, "Opus 4.6")
            .docked(true)
            .mode("Ask")
            // The meter is a pure function of the session's own counters; a
            // real app fills these from the server's context notification.
            .context(ContextMeterState {
                used_tokens: 19_328,
                window_tokens: Some(200_000),
                pressure: ContextPressure::Normal,
                prompt_tokens: 19_213,
                output_tokens: 115,
                total_tokens: 19_328,
            })
            .streaming(streaming)
            .on_intent({
                // Data in, intents out: the composer never touches app state.
                let handler = cx.listener(|this, intent: &ComposerIntent, window, cx| match intent {
                    ComposerIntent::Send => this.send(window, cx),
                    // Bumping the run counter is all it takes to abandon a stream.
                    ComposerIntent::Stop => { this.run += 1; cx.notify(); }
                    _ => {}
                });
                move |intent, w, cx| handler(&intent, w, cx)
            });
        // The root element owns `ROOT_CONTEXT`: Tab, Escape and the window-wide
        // ⌘B / ⌘\ shortcuts bound by `aui::keys::bind`.
        div()
            .size_full()
            .relative()
            .key_context(aui::keys::ROOT_CONTEXT)
            .track_focus(&self.focus_root)
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
            .on_action(cx.listener(|this, _: &ToggleRightPane, _, cx| this.toggle_right(cx)))
            // Escape closes whatever is open; nothing is, in this example.
            .on_action(cx.listener(|_this, _: &Cancel, _, _cx| {}))
            // Tab moves the keyboard, so it arms the focus ring; a mouse press
            // anywhere disarms it again, which `app_shell` wires up for you.
            .on_action(|_: &FocusNext, window, cx| { aui::keys::set_keyboard_nav(true, cx); window.focus_next(cx); })
            .on_action(|_: &FocusPrev, window, cx| { aui::keys::set_keyboard_nav(true, cx); window.focus_prev(cx); })
            .child(
                app_shell("shell")
                    .traffic_lights(true)
                    .sidebar_open(self.sidebar_open)
                    .right_open(self.right_open)
                    .header_sidebar(
                        sidebar_header("hd-side")
                            .traffic_lights(true)
                            .collapsed(!self.sidebar_open)
                            .on_toggle_sidebar(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    )
                    .header_centre(
                        centre_header("hd-centre", "Teacher recruitment RFP")
                            .glyph(IconName::GradCap)
                            .branch("main")
                            .on_toggle_right(cx.listener(|this, _, _, cx| this.toggle_right(cx))),
                    )
                    .header_right(right_header("hd-right").on_close(cx.listener(|this, _, _, cx| this.toggle_right(cx))))
                    .sidebar(sidebar)
                    .centre(v_flex().size_full().child(transcript).child(composer))
                    .right(right),
            )
    }
}

/// Pulls `--flag <value>` out of the process args, for the two screenshot
/// flags below. Not a general parser — this example only ever takes one.
fn arg_value(flag: &str) -> Option<std::path::PathBuf> {
    let mut args = std::env::args();
    while let Some(a) = args.next() {
        if a == flag {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}

fn main() {
    // `--screenshot <out.png>` / `--screenshot-approval <out.png>`: render
    // the window off-screen a couple of frames in and quit, the same shape
    // as the gallery's `--screenshot` (`aui-gallery/src/shot.rs`). This is
    // how `docs/images/minimal.png` and `minimal-approval.png` are made.
    let screenshot = arg_value("--screenshot");
    let screenshot_approval = arg_value("--screenshot-approval");
    let show_approval = screenshot_approval.is_some();
    let capture_path = screenshot.or(screenshot_approval);

    // 1. The asset source first: it serves `aui-icons` over gpui-kit's set.
    gpui_kit::application().with_assets(aui::assets::AuiAssets).run(move |cx| {
        // 2. One call does gpui_kit::init, the fonts, the themes and the
        //    keymap. Nothing before it, and no window before it.
        aui::init(ThemeKind::Dark, cx);
        // 3. Optional: the product text scale (1.1); parity renders use 1.0.
        aui_tokens::AuiTheme::set_text_scale(scale::TEXT_SCALE, None, cx);

        // 4. The window. `TitleBar::window_options()` gives the frameless
        //    window the shell's own traffic lights and header expect.
        let bounds = gpui::Bounds::centered(None, gpui::size(px(1180.0), px(760.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(gpui::size(px(720.0), px(480.0))),
            ..gpui_kit::component::TitleBar::window_options()
        };
        // `Root` is gpui-kit's window root; overlays (popovers, the command
        // palette, toasts) need it in the tree.
        let handle = cx
            .open_window(options, |window, cx| {
                let view = cx.new(|cx| {
                    if show_approval {
                        MinimalApp::new_with(window, cx, true)
                    } else {
                        MinimalApp::new(window, cx)
                    }
                });
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("open window");
        cx.activate(true);

        if let Some(path) = capture_path {
            cx.spawn(async move |cx| {
                // Give fonts, layout and the first paint time to settle.
                cx.background_executor().timer(Duration::from_millis(350)).await;
                let result = cx.update(|cx| {
                    handle.update(cx, |_root, window, _cx| {
                        let image = window.render_to_image()?;
                        if let Some(parent) = path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        image.save(&path)?;
                        anyhow::Ok(())
                    })
                });
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => eprintln!("screenshot failed: {err:#}"),
                    Err(err) => eprintln!("screenshot failed: {err:#}"),
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}
