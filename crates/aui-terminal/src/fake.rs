//! [`FakePty`] — a recorded session played back through the real parser.
//!
//! The gallery has no shell to run, and a screenshot has to look the same
//! every time. So the fake owns a *script*: a list of [`ScriptChunk`]s, each a
//! timestamp and the bytes the terminal produced at it, complete with OSC 133
//! markers and SGR colour. Nothing here builds a
//! [`TermBlock`](aui::workbench::TermBlock) by hand — the bytes go through
//! [`BlockParser`] exactly as a real pty's would.
//!
//! Playback runs [`REPLAY_SPEED`] times faster than the recording, so a
//! session with a six-second install finishes inside a screenshot delay. The
//! durations stay honest because the parser reads its timestamps from the
//! fake's [`ManualClock`], which tracks the *recorded* time rather than the
//! wall clock.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::backend::{TermEvent, TerminalBackend};
use crate::parser::{BlockParser, ManualClock};

/// Replay runs this much faster than the session was recorded: card 50's
/// twenty-second transcript lands inside a four-second screenshot delay.
pub const REPLAY_SPEED: f64 = 5.0;

/// One recorded burst of output: the bytes, and how far into the session they
/// arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptChunk {
    /// Offset from the start of the session.
    pub at: Duration,
    /// The raw bytes, OSC 133 markers and SGR escapes included.
    pub bytes: Vec<u8>,
}

impl ScriptChunk {
    /// A chunk at `ms` milliseconds carrying `text`.
    pub fn new(ms: u64, text: impl Into<String>) -> Self {
        Self { at: Duration::from_millis(ms), bytes: text.into().into_bytes() }
    }
}

/// A [`TerminalBackend`] that replays a [`ScriptChunk`] list.
///
/// [`poll`](TerminalBackend::poll) hands back whatever the script has reached
/// by now, one timestamp per call, so a caller polling on a UI timer watches
/// the blocks appear one after another. [`write`](TerminalBackend::write) and
/// [`resize`](TerminalBackend::resize) are accepted and ignored: a recording
/// cannot answer.
pub struct FakePty {
    script: Vec<ScriptChunk>,
    next: usize,
    /// Wall-clock instant the replay started; `None` until the first poll.
    origin: Option<Instant>,
    /// The instant the recording's own zero maps to.
    base: Instant,
    clock: ManualClock,
    speed: f64,
}

impl FakePty {
    /// A fake replaying `script`.
    pub fn new(script: Vec<ScriptChunk>) -> Self {
        let base = Instant::now();
        Self { script, next: 0, origin: None, base, clock: ManualClock::new(base), speed: REPLAY_SPEED }
    }

    /// The card 50 session: `git status -sb`, `pnpm i`, a failing `pnpm lint`,
    /// and a `pnpm vitest` that is still running when the recording ends.
    pub fn card50() -> Self {
        Self::new(Self::card50_script())
    }

    /// The agent's TUI screen, streamed line by line and never finishing.
    pub fn tui() -> Self {
        Self::new(Self::tui_script())
    }

    /// The clock the replay drives. Hand it to
    /// [`BlockParser::with_clock`] so the blocks carry the recorded durations
    /// rather than the compressed ones.
    pub fn clock(&self) -> ManualClock {
        self.clock.clone()
    }

    /// The script being replayed, so a test can drive it without a timer.
    pub fn script(&self) -> &[ScriptChunk] {
        &self.script
    }

    /// Replays `script` at once, stamping every chunk with its recorded time.
    /// The parser this returns holds exactly what a completed replay produces.
    pub fn replay(script: &[ScriptChunk]) -> BlockParser {
        let base = Instant::now();
        let clock = ManualClock::new(base);
        let mut parser = BlockParser::with_clock(clock.clone());
        for chunk in script {
            clock.set(base + chunk.at);
            parser.feed(&chunk.bytes);
        }
        parser
    }

    /// The bytes of the card 50 session.
    ///
    /// The prompt string between `A` and `B` is included on purpose: it must
    /// not reach a block.
    pub fn card50_script() -> Vec<ScriptChunk> {
        let d = "\u{1b}[2m";
        let r = "\u{1b}[0m";
        let prompt = format!("~/work/acme {d}\u{276f}{r} ");
        let a = "\u{1b}]133;A\u{7}";
        let b = "\u{1b}]133;B\u{7}";
        let c = "\u{1b}]133;C\u{7}";
        let start = |cmd: &str| format!("{a}{prompt}{b}{cmd}{c}");
        let done = |code: i32| format!("\u{1b}]133;D;{code}\u{7}");
        vec![
            ScriptChunk::new(0, start("git status -sb")),
            ScriptChunk::new(
                100,
                format!(
                    "\u{1b}[34m## feature/checkout-flow-v2...origin/feature/checkout-flow-v2{r}\n\
                     \u{1b}[33m M{r} src/checkout/validators.ts\n\
                     \u{1b}[32mA {r} src/checkout/validators.test.ts\n{}",
                    done(0)
                ),
            ),
            ScriptChunk::new(140, start("pnpm i")),
            ScriptChunk::new(200, format!("{d}Lockfile is up to date, resolution step is skipped{r}\n")),
            ScriptChunk::new(6340, format!("{d}Already up to date{r}\n{}", done(0))),
            ScriptChunk::new(6400, start("pnpm lint")),
            ScriptChunk::new(7000, format!("\u{1b}[31m\u{2716}{r} src/checkout/validators.ts\n")),
            ScriptChunk::new(
                7400,
                format!("  46:5  \u{1b}[31merror{r}  'validateCanadianPostal' is not defined  {d}no-undef{r}\n"),
            ),
            ScriptChunk::new(9600, format!("\n\u{1b}[31m\u{2716} 1 problem{r} (1 error, 0 warnings)\n{}", done(1))),
            ScriptChunk::new(9700, start("pnpm vitest run src/checkout")),
            ScriptChunk::new(10100, format!("\u{1b}[32m\u{2713}{r} validators.test.ts {d}(18){r} {d}412ms{r}\n")),
            ScriptChunk::new(11200, format!("\u{1b}[32m\u{2713}{r} AddressForm.test.tsx {d}(9){r} {d}1.1s{r}\n")),
            ScriptChunk::new(12000, format!("{d}\u{283c}{r} checkout.e2e.ts {d}running\u{2026}{r}\n")),
        ]
    }

    /// The bytes of the agent's TUI screen. No OSC 133 markers: a full-screen
    /// program has no blocks, so the parser keeps it as one running block
    /// whose output lines are the screen.
    pub fn tui_script() -> Vec<ScriptChunk> {
        let d = "\u{1b}[2m";
        let r = "\u{1b}[0m";
        let m = "\u{1b}[35m";
        let b = "\u{1b}[34m";
        let y = "\u{1b}[33m";
        vec![
            ScriptChunk::new(
                0,
                format!("{m}\u{2731}{r} \u{1b}[1mClaude Code{r} {d}v2.1.174{r}\n{d}Opus 4.6 \u{b7} ~/work/acme/checkout-flow-v2{r}\n\n"),
            ),
            ScriptChunk::new(400, format!("{d}\u{203a}{r} tighten address validation and add coverage\n\n")),
            ScriptChunk::new(800, format!("{m}\u{25cf}{r} Read {b}src/checkout/validators.ts{r}\n")),
            ScriptChunk::new(1100, format!("  {d}\u{23bf}  Read 180 lines{r}\n")),
            ScriptChunk::new(1500, format!("{m}\u{25cf}{r} Update {b}src/checkout/validators.ts{r}\n")),
            ScriptChunk::new(1800, format!("  {d}\u{23bf}  Added 8 lines, removed 3 lines{r}\n")),
            ScriptChunk::new(2200, format!("{m}\u{25cf}{r} Bash {b}pnpm vitest run src/checkout{r}\n")),
            ScriptChunk::new(2500, format!("  {d}\u{23bf}  Running\u{2026}{r}\n\n")),
            ScriptChunk::new(2800, format!("{y}\u{283c} Thinking\u{2026}{r} {d}(12s \u{b7} esc to interrupt){r}\n")),
        ]
    }
}

impl TerminalBackend for FakePty {
    /// Rewinds the recording and starts it. The shell and cwd are recorded
    /// into the transcript already, so both arguments are ignored.
    fn spawn(&mut self, _shell: &str, _cwd: &Path) -> std::io::Result<()> {
        self.next = 0;
        self.base = Instant::now();
        self.clock.set(self.base);
        self.origin = Some(Instant::now());
        Ok(())
    }

    /// Ignored: a recording cannot answer input.
    fn write(&mut self, _bytes: &[u8]) {}

    /// Ignored: the recording was made at one size.
    fn resize(&mut self, _cols: u16, _rows: u16) {}

    fn poll(&mut self) -> Vec<TermEvent> {
        let origin = *self.origin.get_or_insert_with(Instant::now);
        let elapsed = origin.elapsed().mul_f64(self.speed);
        // The parser stamps a block at the moment it is fed, so a poll must
        // hand over one recorded timestamp at a time and park the clock on it.
        let pending = self.script.get(self.next).map(|c| c.at);
        match pending {
            Some(at) if at <= elapsed => {
                self.clock.set(self.base + at);
                let mut out = Vec::new();
                while self.script.get(self.next).map(|c| c.at) == Some(at) {
                    out.push(TermEvent::Output(self.script[self.next].bytes.clone()));
                    self.next += 1;
                }
                out
            }
            // Between chunks the clock still moves, so the live block ticks;
            // it never runs past the next chunk it has to be stamped with.
            Some(at) => {
                self.clock.set(self.base + elapsed.min(at));
                Vec::new()
            }
            None => {
                self.clock.set(self.base + elapsed);
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aui::workbench::BlockState;

    #[test]
    fn the_card50_script_replays_into_four_blocks() {
        let parser = FakePty::replay(&FakePty::card50_script());
        let blocks = parser.blocks();
        assert_eq!(blocks.len(), 4);
        let commands: Vec<_> = blocks.iter().map(|b| b.command.to_string()).collect();
        assert_eq!(commands, vec!["git status -sb", "pnpm i", "pnpm lint", "pnpm vitest run src/checkout"]);
        let states: Vec<_> = blocks.iter().map(|b| b.state).collect();
        assert_eq!(states, vec![BlockState::Done, BlockState::Done, BlockState::Failed, BlockState::Running]);
        let durations: Vec<_> = blocks.iter().map(|b| b.duration.to_string()).collect();
        assert_eq!(durations, vec!["0.1 s", "6.2 s", "3.2 s · exit 1", "2 s"]);
        // The zsh prompt string never leaks into a block.
        assert!(!blocks.iter().any(|b| b.command.contains('\u{276f}')));
        assert_eq!(blocks[2].output.len(), 4);
    }

    #[test]
    fn the_tui_script_replays_into_one_running_screen() {
        let parser = FakePty::replay(&FakePty::tui_script());
        assert_eq!(parser.blocks().len(), 1);
        let block = &parser.blocks()[0];
        assert_eq!(block.state, BlockState::Running);
        assert_eq!(block.command, "");
        assert_eq!(block.output.len(), 13);
        assert!(block.output[0].contains("Claude Code"));
    }

    #[test]
    fn polling_a_fake_hands_the_script_over_one_timestamp_at_a_time() {
        let mut fake = FakePty::card50();
        fake.spawn("/bin/zsh", Path::new("/")).unwrap();
        let mut parser = BlockParser::with_clock(fake.clock());
        // The replay is time-driven, so poll until the recording is drained.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            for event in fake.poll() {
                if let TermEvent::Output(bytes) = event {
                    parser.feed(&bytes);
                }
            }
            if fake.next == fake.script.len() || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(fake.next, fake.script.len(), "the whole script was replayed");
        assert_eq!(parser.blocks().len(), 4);
        assert_eq!(parser.blocks()[3].state, BlockState::Running);
        assert_eq!(parser.blocks()[1].duration, "6.2 s");
    }
}
