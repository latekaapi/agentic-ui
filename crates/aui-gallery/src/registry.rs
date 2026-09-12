//! The list of gallery entries. One entry per design card (plus the motion
//! playground, the screens page and the assistant mock), keyed by the ids
//! used in `docs/03-parity-process.md`.

use aui_tokens::ThemeKind;
use gpui::{AnyElement, App, Window};

/// Which theme a card is designed in (`theme=` in the card's `ds:` header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // all three variants appear once every card is ported
pub enum CardTheme {
    /// Rendered in dark only.
    Dark,
    /// Rendered in light only.
    Light,
    /// Designed for both; follows the gallery's theme switch.
    Both,
}

impl CardTheme {
    /// The theme this card forces when opened, if any.
    pub fn fixed_kind(self) -> Option<ThemeKind> {
        match self {
            CardTheme::Dark => Some(ThemeKind::Dark),
            CardTheme::Light => Some(ThemeKind::Light),
            CardTheme::Both => None,
        }
    }
}

/// A gallery entry.
pub struct Entry {
    /// Stable id, `group/name`, as listed in the parity checklist.
    pub id: &'static str,
    /// Sidebar group ("Foundations", "Shell", …).
    pub group: &'static str,
    /// Card title from the `ds:` header.
    pub title: &'static str,
    /// Card subtitle from the `ds:` header.
    pub subtitle: &'static str,
    /// Card width in logical pixels (the reference PNG size).
    pub width: f32,
    /// Card height in logical pixels.
    pub height: f32,
    /// Theme the card is designed in.
    pub theme: CardTheme,
    /// Builds the card content. The gallery wraps it in the card frame
    /// (card background, 20 px padding) exactly like `body.ds` in the design.
    pub build: fn(&mut Window, &mut App) -> AnyElement,
}

impl Entry {
    /// Screens are full-bleed: their reference PNG is a 1440×900 app window,
    /// not a card sitting on the design's 20 px card ground, so the stage
    /// must render them edge to edge.
    pub fn full_bleed(&self) -> bool {
        self.group == "Screens"
    }
}

/// All entries, in card order.
pub static ENTRIES: &[Entry] = &[
    Entry {
        id: "foundations/colour",
        group: "Foundations",
        title: "Colour",
        subtitle: "Semantic tokens, agent states, diff and terminal palettes in both themes",
        width: 960.0,
        height: 720.0,
        theme: CardTheme::Dark,
        build: crate::cards::colour::build,
    },
    Entry {
        id: "foundations/type",
        group: "Foundations",
        title: "Typography",
        subtitle: "Geist for UI, Geist Mono for code and terminals; scale 11\u{2013}24",
        width: 760.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::typography::build,
    },
    Entry {
        id: "foundations/space",
        group: "Foundations",
        title: "Spacing, radius, elevation",
        subtitle: "2/4/8/12/16/24/32 \u{b7} radii 4\u{2013}16 \u{b7} three shadow levels",
        width: 760.0,
        height: 420.0,
        theme: CardTheme::Dark,
        build: crate::cards::space::build,
    },
    Entry {
        id: "foundations/motion",
        group: "Foundations",
        title: "Motion",
        subtitle: "Durations, easings and the four springs; live samples",
        width: 900.0,
        height: 520.0,
        theme: CardTheme::Dark,
        build: crate::cards::motion::build,
    },
    Entry {
        id: "foundations/icons",
        group: "Foundations",
        title: "Icons and marks",
        subtitle: "Lucide-style 14/16 px glyphs, provider marks, status glyphs, file types",
        width: 760.0,
        height: 380.0,
        theme: CardTheme::Dark,
        build: crate::cards::icons::build,
    },
    Entry {
        id: "shell/app-shell",
        group: "Shell",
        title: "App shell",
        subtitle: "Three columns, each with its own header row; sidebar, transcript, workbench",
        width: 1280.0,
        height: 820.0,
        theme: CardTheme::Dark,
        build: crate::cards::app_shell::build,
    },
    Entry {
        id: "shell/app-shell-native",
        group: "Shell",
        title: "App shell, native lights",
        subtitle: "Collapsed sidebar with the native traffic-light reservation; the header row stays put",
        width: 1280.0,
        height: 820.0,
        theme: CardTheme::Dark,
        build: crate::cards::app_shell_native::build,
    },
    Entry {
        id: "shell/panel-chrome",
        group: "Shell",
        title: "Panel chrome and tabs",
        subtitle: "Tab strip with sliding indicator, panel header, drag-to-dock drop zones",
        width: 860.0,
        height: 460.0,
        theme: CardTheme::Dark,
        build: crate::cards::panel_chrome::build,
    },
    Entry {
        id: "shell/command-palette",
        group: "Shell",
        title: "Command palette",
        subtitle: "\u{2318}K with sections, fuzzy matches, shortcut hints; quick open \u{2318}P",
        width: 760.0,
        height: 520.0,
        theme: CardTheme::Dark,
        build: crate::cards::command_palette::build,
    },
    Entry {
        id: "shell/toasts",
        group: "Shell",
        title: "Toasts and banners",
        subtitle: "Stacked toasts, newest in front; inline banners for waiting, context, provider and connection states",
        width: 820.0,
        height: 740.0,
        theme: CardTheme::Dark,
        build: crate::cards::toasts::build,
    },
    Entry {
        id: "overlay/dialog",
        group: "Shell",
        title: "Modal dialog",
        subtitle: "Scrim over the window, one card: kind tile, body, mono detail, secondary and primary",
        width: 720.0,
        height: 480.0,
        theme: CardTheme::Both,
        build: crate::cards::dialog::build,
    },
    Entry {
        id: "sidebar/rows",
        group: "Sidebar",
        title: "Worktree rows",
        subtitle: "Every state: running, needs you, done with PR, failed, idle, selected, hover actions incl. archive, inline rename, child tasks",
        width: 720.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::rows::build,
    },
    Entry {
        id: "sidebar/sidebar",
        group: "Sidebar",
        title: "Sidebar",
        subtitle: "Header, primary nav, collapsible groups with counts, filters, account footer, and the collapsed rail",
        width: 760.0,
        height: 640.0,
        theme: CardTheme::Dark,
        build: crate::cards::sidebar::build,
    },
    Entry {
        id: "sidebar/assistant",
        group: "Sidebar",
        title: "Assistant sidebar",
        subtitle: "Roles \u{2192} projects \u{2192} sessions, with knowledge sources per role and recent documents",
        width: 760.0,
        height: 640.0,
        theme: CardTheme::Light,
        build: crate::cards::assistant::build,
    },
    Entry {
        id: "sidebar/views",
        group: "Sidebar",
        title: "Sidebar views",
        subtitle: "Three groupings of the same sessions: by status, by project with nested sessions, by date with a pinned group; plus the view-options menu",
        width: 1180.0,
        height: 680.0,
        theme: CardTheme::Dark,
        build: crate::cards::views::build,
    },
    Entry {
        id: "transcript/markers",
        group: "Transcript",
        title: "Header identity and markers",
        subtitle: "The agent mark and worktree live in the centre header cell; model and mode in the composer; marker rows inside the transcript",
        width: 760.0,
        height: 380.0,
        theme: CardTheme::Dark,
        build: crate::cards::markers::build,
    },
    Entry {
        id: "transcript/turns",
        group: "Transcript",
        title: "User and assistant turns",
        subtitle: "User bubble with attachments and mentions; assistant turn streaming with reveal, hover toolbar and footer meta; reduced action set",
        width: 760.0,
        height: 2800.0,
        theme: CardTheme::Dark,
        build: crate::cards::turns::build,
    },
    Entry {
        id: "transcript/thinking",
        group: "Transcript",
        title: "Thinking block",
        subtitle: "Shimmer label with elapsed time, capped streaming viewport, collapses to a summary when done",
        width: 760.0,
        height: 520.0,
        theme: CardTheme::Dark,
        build: crate::cards::thinking::build,
    },
    Entry {
        id: "transcript/activity",
        group: "Transcript",
        title: "Agent activity group",
        subtitle: "Consecutive tool calls fold into one summary row with a step timeline; live state while working",
        width: 760.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::activity::build,
    },
    Entry {
        id: "transcript/tool-cards",
        group: "Transcript",
        title: "Tool call cards",
        subtitle: "Shell, read, edit, search, web and browser variants with the shared header: status, verb, target, duration",
        width: 800.0,
        height: 760.0,
        theme: CardTheme::Dark,
        build: crate::cards::tool_cards::build,
    },
    Entry {
        id: "transcript/tool-group",
        group: "Transcript",
        title: "Tool-call group",
        subtitle: "Consecutive tool calls under one summary: collapsed preview rows, open full cards",
        width: 800.0,
        height: 760.0,
        theme: CardTheme::Both,
        build: crate::cards::tool_group::build,
    },
    Entry {
        id: "transcript/approval",
        group: "Transcript",
        title: "Approval card",
        subtitle: "Permission request: pending, approving, approved, denied, remembered; keyboard Y / A / N; server-minted choices, staged subjects, feedback, badges, policy and judge resolutions",
        width: 760.0,
        height: 1780.0,
        theme: CardTheme::Dark,
        build: crate::cards::approval::build,
    },
    Entry {
        id: "transcript/question-plan-todo",
        group: "Transcript",
        title: "Question, plan and todo cards",
        subtitle: "Single and multi-select questions with Other, header, option previews, timeout and clarify; every settlement outcome; plan proposal with and without sections; todo list with morphing marks",
        width: 800.0,
        height: 1560.0,
        theme: CardTheme::Dark,
        build: crate::cards::question_plan_todo::build,
    },
    Entry {
        id: "transcript/markdown",
        group: "Transcript",
        title: "Markdown blocks",
        subtitle: "Headings, lists, fences, tables, quotes, rules and image placeholders with clickable links",
        width: 760.0,
        height: 820.0,
        theme: CardTheme::Both,
        build: crate::cards::markdown::build,
    },
    Entry {
        id: "transcript/code-diff",
        group: "Transcript",
        title: "Code and diff blocks",
        subtitle: "Inline code block with header actions and copy morph; unified diff block with per-line note affordance",
        width: 760.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::code_diff::build,
    },
    Entry {
        id: "transcript/summary-status",
        group: "Transcript",
        title: "Summary, error and status rows",
        subtitle: "Turn summary with changed files and PR CTA; error card with retry; working line, retry-scheduled row, needs-you banner, jump-to-latest; goal card and the unknown-item fallback",
        width: 760.0,
        height: 1000.0,
        theme: CardTheme::Dark,
        build: crate::cards::summary_status::build,
    },
    Entry {
        id: "composer/composer",
        group: "Composer",
        title: "Composer",
        subtitle: "Auto-growing input, meta strip, actions menu, model and mode pickers, send/stop morph, queued messages",
        width: 760.0,
        height: 620.0,
        theme: CardTheme::Both,
        build: crate::cards::composer::build,
    },
    Entry {
        id: "composer/menus",
        group: "Composer",
        title: "Slash commands and mentions",
        subtitle: "Inline / menu with sections and keyboard hints; @ picker for files, symbols, worktrees; $ skills",
        width: 800.0,
        height: 1240.0,
        theme: CardTheme::Dark,
        build: crate::cards::menus::build,
    },
    Entry {
        id: "composer/pickers",
        group: "Composer",
        title: "Chip menus, context meter and queue strip",
        subtitle: "Model / effort / approval-mode pickers, the context ring in every pressure state with its breakdown, the queued strip, and the composer in plan mode",
        width: 900.0,
        height: 1040.0,
        theme: CardTheme::Both,
        build: crate::cards::pickers::build,
    },
    Entry {
        id: "composer/attachments",
        group: "Composer",
        title: "Attachments and drop",
        subtitle: "Image thumbs, file rows with upload states, drop overlay over the transcript, paste hint",
        width: 760.0,
        height: 460.0,
        theme: CardTheme::Dark,
        build: crate::cards::attachments::build,
    },
    Entry {
        id: "workbench/terminal",
        group: "Workbench",
        title: "Terminal",
        subtitle: "Block terminal: each command is a card with status, duration and foldable output; agent commands are tagged; splits and tabs",
        width: 980.0,
        height: 720.0,
        theme: CardTheme::Dark,
        build: crate::cards::terminal::build,
    },
    Entry {
        id: "workbench/browser",
        group: "Workbench",
        title: "Browser and annotator",
        subtitle: "Tabs and nav; annotate mode pins numbered comments on elements, lists them beside the page, and sends the batch with metadata and a screenshot to the agent",
        width: 980.0,
        height: 640.0,
        theme: CardTheme::Dark,
        build: crate::cards::browser::build,
    },
    Entry {
        id: "workbench/diff",
        group: "Workbench",
        title: "Diff review",
        subtitle: "Scope switch, file list with change counts, unified diff with hunks, batched line notes sent back to an agent",
        width: 900.0,
        height: 640.0,
        theme: CardTheme::Dark,
        build: crate::cards::diff_review::build,
    },
    Entry {
        id: "workbench/git",
        group: "Workbench",
        title: "Git and PR",
        subtitle: "Changes list, AI-drafted commit, ahead/behind, Create PR form with base branch and checks",
        width: 900.0,
        height: 520.0,
        theme: CardTheme::Dark,
        build: crate::cards::git_pr::build,
    },
    Entry {
        id: "workbench/files-docs",
        group: "Workbench",
        title: "File tree and document panes",
        subtitle: "Worktree-aware file explorer with git badges; assistant document pane chrome for docx, xlsx, pdf and markdown with chat-created artifacts",
        width: 980.0,
        height: 600.0,
        theme: CardTheme::Both,
        build: crate::cards::files_docs::build,
    },
    Entry {
        id: "transcript/citations",
        group: "Transcript",
        title: "Sources and citations",
        subtitle: "Assistant answer with inline citation markers, sources list grouped by tier (role, project, session), and a source hover card",
        width: 760.0,
        height: 520.0,
        theme: CardTheme::Light,
        build: crate::cards::citations::build,
    },
    Entry {
        id: "workbench/webview",
        group: "Workbench",
        title: "Webview (live)",
        subtitle: "The browser pane driven by a WebBackend: navigation, the annotator's hover outline and click-to-pin, and the annotations panel \u{2014} on the scripted fake backend",
        width: 980.0,
        height: 640.0,
        theme: CardTheme::Dark,
        build: crate::cards::webview_live::build,
    },
    #[cfg(feature = "wry")]
    Entry {
        id: "workbench/webview-real",
        group: "Workbench",
        title: "Webview (real)",
        subtitle: "The same pane over a real WKWebView in a wry child view: real navigation, real page events, the annotator posting from the page itself \u{2014} and \u{2318}K hiding the native view so gpui can paint over it",
        width: 980.0,
        height: 640.0,
        theme: CardTheme::Dark,
        build: crate::cards::webview_live::build_real,
    },
    Entry {
        id: "workbench/terminal-live",
        group: "Workbench",
        title: "Terminal (live)",
        subtitle: "The block terminal fed by a TerminalBackend: a scripted PTY replays commands through the OSC 133 block parser and the blocks grow live",
        width: 980.0,
        height: 720.0,
        theme: CardTheme::Dark,
        build: crate::cards::terminal_live::build,
    },
    Entry {
        id: "workbench/tui-live",
        group: "Workbench",
        title: "TUI (live)",
        subtitle: "The TUI pane over a terminal grid: a scripted agent screen streamed through the same backend",
        width: 980.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::terminal_live::build_tui,
    },
    #[cfg(feature = "pty")]
    Entry {
        id: "workbench/terminal-real",
        group: "Workbench",
        title: "Terminal (real)",
        subtitle: "The block terminal over a real pseudo-terminal: your login zsh with OSC 133 shell integration, split into blocks as you run things",
        width: 980.0,
        height: 720.0,
        theme: CardTheme::Dark,
        build: crate::cards::terminal_live::build_real,
    },
    #[cfg(all(feature = "pty", feature = "tui"))]
    Entry {
        id: "workbench/tui-real",
        group: "Workbench",
        title: "TUI (real)",
        subtitle: "The TUI pane over a real alacritty grid: a full-screen program in a pty, redrawing in place, with its cursor",
        width: 980.0,
        height: 560.0,
        theme: CardTheme::Dark,
        build: crate::cards::terminal_live::build_tui_real,
    },
    Entry {
        id: "data/secret_field",
        group: "Data",
        title: "Secret field",
        subtitle: "The masked single-line field beside its revealed twin: bordered box, ghost eye, focus ring on the box",
        width: 960.0,
        height: 420.0,
        theme: CardTheme::Both,
        build: crate::cards::secret_field::build,
    },
    Entry {
        id: "screens/login",
        group: "Screens",
        title: "Sign in",
        subtitle: "The two-method sign-in screen in its five states: the method choice, the device flow, the API-key form and a failure",
        width: 1440.0,
        height: 2400.0,
        theme: CardTheme::Both,
        build: crate::cards::login::build,
    },
    Entry {
        id: "screens/assistant",
        group: "Screens",
        title: "Assistant",
        subtitle: "The day-job assistant: roles, grounded answer with citations, document pane \u{2014} live, assembled from the library",
        width: 1440.0,
        height: 900.0,
        theme: CardTheme::Light,
        build: crate::assistant::build,
    },
    Entry {
        id: "screens/all",
        group: "Screens",
        title: "Screens",
        subtitle: "The three assistant screens side by side at their reference size, in a row that scrolls horizontally",
        // 3 x 1440 + 2 x 24 gap + 2 x 20 ground; 900 plus a 28 px caption,
        // an 8 px gap and 12 px of ground above and below.
        width: 4408.0,
        height: 960.0,
        theme: CardTheme::Light,
        build: crate::assistant::build_all,
    },
];

/// Looks an entry up by id.
pub fn find(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.id == id)
}

/// The distinct groups in order of first appearance.
pub fn groups() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for e in ENTRIES {
        if !out.contains(&e.group) {
            out.push(e.group);
        }
    }
    out
}
