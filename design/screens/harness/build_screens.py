#!/usr/bin/env python3
"""Assemble the harness screen artboards from the shared chrome in ../common.py."""
import sys, pathlib, json
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from common import *

# ---------- Main: active worktree ----------
def main_screen():
    transcript = f"""
<main class="center">
  <div class="tr">
    {session_header()}
    <div class="u">tighten address validation and add coverage</div>
    <div class="a">I'm checking the existing form flow, then I'll patch the validator and run the focused tests.</div>
    <div class="th"><div class="hh">{icon('brain','style="color:var(--ink-3)"')}<span>Thought for 14 s</span><span class="subtle">· branch per country, keep copy, add CA/GB/empty cases</span><span class="t"></span><svg class="chev"><use href="#chevron"></use></svg></div></div>
    <div class="ag"><div class="hh"><i class="spinner"></i><span class="shimmer" style="font-weight:500">Running tests</span><span class="steps"><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="run"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#terminal"></use></svg></i><i></i></span><span class="t">38 s</span><svg class="chev" style="transform:rotate(90deg)"><use href="#chevron"></use></svg></div>
      <div class="tl">
        <div class="st"><span class="g"><i></i></span><span><span class="v">Searched</span><span class="mono">validateAddress in src/checkout</span></span><span class="r">2 hits</span></div>
        <div class="st"><span class="g"><i></i></span><span><span class="v">Read</span><span class="mono">src/checkout/validators.ts</span></span><span class="r">180 lines</span></div>
        <div class="st"><span class="g"><i></i></span><span><span class="v">Edited</span><span class="mono">src/checkout/validators.ts</span></span><span class="r add">+8 −3</span></div>
        <div class="st"><span class="g"><i class="run"></i></span><span><span class="v">Running</span><span class="mono">pnpm vitest run src/checkout</span></span><span class="r">12 s</span></div>
        <div class="st pend"><span class="g"><i class="pend"></i></span><span><span class="v">Lint</span><span class="mono">eslint on touched files</span></span><span class="r">queued</span></div>
      </div></div>
    <div class="a">The pg native module needs a system library before the e2e run. I need permission for one install.</div>
    <div class="ap"><div class="hh"><span class="ic">{icon('shield')}</span><div><div class="ttl">Allow Claude Code to run this command?</div><div class="sub">Runs in <span class="mono">~/work/acme/checkout-flow-v2</span> · can modify files and network</div></div><span class="pill warning" style="margin-left:auto">Waiting</span></div>
      <div class="cmd"><span class="p">$</span><span>sudo apt install -y libpq-dev</span></div>
      <div class="actions tint"><div class="k"><span><span class="kbd">Y</span>allow</span><span><span class="kbd">A</span>always</span><span><span class="kbd">N</span>deny</span></div><button class="btn sm danger">Deny</button><button class="btn sm">Always allow <span class="mono" style="opacity:.7">apt install</span></button><button class="btn primary sm">Allow once</button></div></div>
    <div class="row status">{icon('shield','style="color:var(--warning);width:12px;height:12px"')}<span style="color:var(--ink-2)">Waiting for you</span><span>·</span><span class="mono">1 m 04 s</span></div>
  </div>
  {composer(stop=False)}
</main>"""
    right = f"""
<aside class="right">
  {rtabs('Terminal')}
  <div class="term"><div class="scroll">
    <div class="marker">restored scrollback · 09:02</div>
    <div class="blk old"><div class="cmd"><span class="ex"></span><span class="p">$</span>git status -sb<span class="dur">0.1 s</span></div><div class="out"><div><span class="b">## feature/checkout-flow-v2...origin</span></div><div><span class="y"> M</span> src/checkout/validators.ts</div><div><span class="g">A </span> src/checkout/validators.test.ts</div></div></div>
    <div class="blk old"><div class="cmd"><span class="ex"></span><span class="p">$</span>pnpm i<span class="who">{mark('claude','C',11)}claude</span><span class="dur">6.2 s</span></div><div class="out"><div><span class="d">Lockfile is up to date</span></div><div><span class="d">Already up to date</span></div></div><div class="fold">{icon('chevron','style="width:11px;height:11px"')}38 lines folded</div></div>
    <div class="blk err"><div class="cmd"><span class="ex bad"></span><span class="p">$</span>pnpm lint<span class="who">{mark('claude','C',11)}claude</span><span class="dur">3.2 s · exit 1</span></div><div class="out"><div><span class="rd">✖</span> src/checkout/validators.ts</div><div>&nbsp;&nbsp;46:5 &nbsp;<span class="rd">error</span> &nbsp;'validateCanadianPostal' is not defined</div></div></div>
    <div class="blk live"><div class="cmd"><span class="ex run"></span><span class="p">$</span>pnpm vitest run src/checkout<span class="who">{mark('claude','C',11)}claude</span><span class="dur">12 s</span></div><div class="out"><div><span class="g">✓</span> validators.test.ts <span class="d">(18)</span></div><div><span class="g">✓</span> AddressForm.test.tsx <span class="d">(9)</span></div><div><span class="d">⠼</span> checkout.e2e.ts <span class="d">running…</span></div></div></div>
  </div>
  <div class="prompt"><span style="color:var(--accent);font-weight:600">$</span><span class="cur"></span><span class="ctx"><span>⎇ feature/checkout-flow-v2</span></span></div></div>
</aside>"""
    return header("checkout-flow-v2","feature/checkout-flow-v2","Terminal") + sidebar(view="projects") + transcript + right + STATUSBAR

# ---------- DiffReview ----------
def diff_screen():
    transcript = f"""
<main class="center">
  <div class="tr">
    {session_header()}
    <div class="u">tighten address validation and add coverage</div>
    <div class="ag"><div class="hh">{ok_glyph()}<span style="font-weight:500">Ran 4 commands</span><span class="subtle">· read 2 files · edited 3 files</span><span class="t">4 m 12 s</span><svg class="chev"><use href="#chevron"></use></svg></div></div>
    <div class="a">Updated validation behaviour and added regression tests for US ZIP+4, Canadian postal codes and a missing country. The checkout copy is unchanged.</div>
    <div class="sm"><div class="hh">{ok_glyph()}Done · address validation tightened<span class="grow"></span><span class="subtle mono" style="font-size:11px;font-weight:400">4 m 12 s · $0.31</span></div>
      <div class="files"><div class="f"><span class="stt m">M</span>src/checkout/validators.ts<span class="d"><span style="color:var(--success)">+8</span> <span style="color:var(--danger)">−3</span></span></div><div class="f"><span class="stt a">A</span>src/checkout/validators.test.ts<span class="d"><span style="color:var(--success)">+41</span></span></div><div class="f"><span class="stt m">M</span>src/checkout/AddressForm.tsx<span class="d"><span style="color:var(--success)">+2</span> <span style="color:var(--danger)">−1</span></span></div></div>
      <div class="checks"><span>{icon('check','style="width:12px;height:12px;color:var(--success);vertical-align:-2px"')} 27 tests pass</span><span>{icon('check','style="width:12px;height:12px;color:var(--success);vertical-align:-2px"')} lint clean</span><span>{icon('check','style="width:12px;height:12px;color:var(--success);vertical-align:-2px"')} typecheck</span></div>
      <div class="actions tint"><span class="hint">3 files changed</span><span class="spacer"></span><button class="btn sm ghost">Create PR</button><button class="btn sm">Commit…</button><button class="btn sm primary">Review diff</button></div></div>
    <div class="row status"><i class="dot done" style="width:6px;height:6px"></i><span>Idle</span><span>·</span><span>2 notes pending in Diff</span></div>
  </div>
  {composer(stop=False)}
</main>"""
    def ln(cls, a, b, text):
        return f'<div class="ln {cls}"><span class="gutter">{a}</span><span class="gutter" style="width:22px;color:var(--ink-4)">{b}</span>{text}</div>'
    right = f"""
<aside class="right">
  {rtabs('Diff')}
  <div class="row" style="padding:10px 12px 6px;font-size:12px;color:var(--ink-3)"><span class="chip" style="color:var(--ink);border-color:var(--line-strong)">This turn</span><span class="chip">Branch</span><span class="chip">Unstaged</span><span class="grow"></span><span class="tag">vs main</span></div>
  <div class="files"><div class="fr" style="background:var(--surface-3);color:var(--ink)">{ft('ts')}validators.ts<span class="tag">src/checkout</span><span class="n" style="margin-left:6px">2</span><span class="stt m">M</span></div><div class="fr">{ft('test')}validators.test.ts<span class="tag">src/checkout</span><span class="stt a">A</span></div><div class="fr">{ft('tsx')}AddressForm.tsx<span class="tag">src/checkout</span><span class="stt m">M</span></div><div class="fr">{ft('ts')}legacy-zip.ts<span class="tag">src/checkout</span><span class="stt d">D</span></div></div>
  <div class="diff"><div class="fh"><span class="mono">validators.ts</span><span class="tag"><span style="color:var(--success)">+8</span> <span style="color:var(--danger)">−3</span></span><span class="grow"></span><button class="btn xs ghost">Open in editor</button></div>
    <div class="hunk">@@ -42,7 +42,9 @@ export function validateAddress</div>
    {ln('', '44', '44', 'export function validateAddress(values) {')}
    {ln('del', '45', '', '  if (!values.country) return true;')}
    {ln('add', '', '45', "  if (!values.country) return invalid('country');")}
    {ln('add', '', '46', "  if (values.country === 'CA') return validateCanadianPostal(values);")}
  </div>
  <div class="note"><div class="caps">Note 1 · line 46</div>Also handle 'GB' here, postcode format differs.</div>
  <div class="diff" style="margin-top:0"><div class="hunk" style="border-top:0">@@ -88,3 +90,4 @@ function backfillUsers(rows)</div>
    {ln('', '88', '90', '  for (const row of rows) {')}
    {ln('del', '89', '', '    db.exec(sql, row)')}
    {ln('add', '', '91', '    if (!row.tier) continue')}
  </div>
  <div class="note pend"><div class="caps">Note 2 · line 91</div>Silently skipping rows. Log the count or surface it.<div class="row" style="margin-top:6px;gap:6px;justify-content:flex-end"><button class="btn xs ghost">Cancel</button><button class="btn xs primary">Add note</button></div></div>
  <div class="actions tint" style="margin-top:auto"><span class="hint">2 notes on 1 file</span><span class="spacer"></span><button class="btn sm ghost">Clear</button><button class="btn sm primary">Send to Claude Code</button></div>
</aside>"""
    return header("checkout-flow-v2","feature/checkout-flow-v2","Diff") + sidebar() + transcript + right + STATUSBAR

# ---------- TerminalMode ----------
def terminal_screen():
    center = f"""
<main class="center">
  <div class="row" style="height:40px;padding:0 16px;border-bottom:1px solid var(--line);background:var(--surface-1);gap:10px"><span class="seg"><span>{icon('sparkle','style="width:12px;height:12px"')}Chat</span><span class="on">{icon('terminal','style="width:12px;height:12px"')}Terminal</span></span><span class="chip">{mark('claude','C',12)}Claude Code</span><span class="chip">{mark('pi','π',12)}Pi</span><span class="subtle" style="font-size:11.5px">Runs in a PTY with your own login.</span><span class="grow"></span><button class="btn icon sm ghost">{icon('split')}</button></div>
  <div class="term"><div class="tui">
<div><span class="m">✱</span> <b>Claude Code</b> <span class="d">v2.1.174</span></div>
<div><span class="d">Opus 4.6 · Claude Max · ~/work/acme/checkout-flow-v2</span></div>
<div class="sp"></div>
<div><span class="d">›</span> tighten address validation and add coverage</div>
<div class="sp"></div>
<div><span class="m">●</span> Read <span class="b">src/checkout/validators.ts</span></div>
<div>  <span class="d">⎿  Read 180 lines</span></div>
<div><span class="m">●</span> Update <span class="b">src/checkout/validators.ts</span></div>
<div>  <span class="d">⎿  Added 8 lines, removed 3 lines</span></div>
<div><span class="m">●</span> Bash <span class="b">pnpm vitest run src/checkout</span></div>
<div>  <span class="d">⎿  PASS validators.test.ts (18)</span></div>
<div>  <span class="d">⎿  PASS AddressForm.test.tsx (9)</span></div>
<div><span class="m">●</span> Bash <span class="b">pnpm lint</span></div>
<div>  <span class="d">⎿  No ESLint warnings or errors</span></div>
<div class="sp"></div>
<div>Updated validation behaviour and added regression tests for US ZIP+4,</div>
<div>Canadian postal codes, and a missing country.</div>
<div class="sp"></div>
<div><span class="y">⠼ Thinking…</span> <span class="d">(4s · esc to interrupt)</span></div>
<div style="flex:1"></div>
</div>
  <div class="tuiin"><span class="d">›</span><span style="display:inline-block;width:7px;height:14px;background:var(--term-cursor)"></span></div>
  <div class="tuihint">⇧⇥ plan mode · ? for shortcuts · ⌘D split</div>
  <div class="prompt" style="font-family:var(--font-ui);font-size:12px;color:var(--ink-3)"><span class="tag">Opus 4.6 · 1M</span><span class="tag">bypass permissions off</span><span class="grow"></span><span class="tag">⎇ feature/checkout-flow-v2</span></div></div>
</main>"""
    right = f"""
<aside class="right">
  {rtabs('Files')}
  <div class="tree">
    <div class="tn dir"><svg class="chev i" style="transform:rotate(90deg)"><use href="#chevron"></use></svg>{ft('folder-open')}src<span class="bd m">M</span></div>
    <div class="tn dir d1"><svg class="chev i" style="transform:rotate(90deg)"><use href="#chevron"></use></svg>{ft('folder-open')}checkout<span class="bd m">M</span></div>
    <div class="tn d2">{ft('tsx')}AddressForm.tsx<span class="bd m">M</span></div>
    <div class="tn d2 on">{ft('ts')}validators.ts<span class="bd m">M</span></div>
    <div class="tn d2">{ft('test')}validators.test.ts<span class="bd a">A</span></div>
    <div class="tn d2">{ft('ts')}index.ts</div>
    <div class="tn dir d1"><svg class="chev i"><use href="#chevron"></use></svg>{ft('folder')}components</div>
    <div class="tn dir d1"><svg class="chev i"><use href="#chevron"></use></svg>{ft('folder')}lib</div>
    <div class="tn dir"><svg class="chev i"><use href="#chevron"></use></svg>{ft('folder')}tests</div>
    <div class="tn">{ft('json')}package.json</div>
    <div class="tn">{ft('lock')}pnpm-lock.yaml</div>
    <div class="tn">{ft('md')}PLAN.md<span class="bd u">U</span></div>
    <div class="tn">{ft('md')}README.md</div>
  </div>
  <div class="actions" style="margin-top:auto;border-top:1px solid var(--line)"><span class="hint">worktree checkout-flow-v2 · 2 modified · 1 added</span></div>
</aside>"""
    return header("checkout-flow-v2","feature/checkout-flow-v2","Files") + sidebar(view="date") + center + right + STATUSBAR

# ---------- NewTask ----------
def newtask_screen():
    base = main_screen()
    dialog = f"""
<div class="scrim"><div class="dlg">
  <div class="hh">{icon('plus','style="color:var(--ink-3)"')}New task<span class="grow"></span><span class="tag">⌘N</span><button class="btn icon sm ghost">{icon('x')}</button></div>
  <div class="body">
    <div class="field"><div class="fld ta"><span>Fix the flaky checkout e2e test: it fails when the country select is left empty. Add a regression test and keep the existing copy.</span></div></div>
    <div class="two">
      <div class="field"><div class="lbl">Project</div><div class="fld">{icon('folder','style="color:var(--ink-3)"')}acme-web<span class="grow"></span>{icon('chevron-down','style="width:11px;height:11px;color:var(--ink-3)"')}</div></div>
      <div class="field"><div class="lbl">Base branch</div><div class="fld"><span class="mono">main</span><span class="grow"></span>{icon('chevron-down','style="width:11px;height:11px;color:var(--ink-3)"')}</div></div>
    </div>
    <div class="two">
      <div class="field"><div class="lbl">Worktree</div><div class="fld"><span class="mono">fix/flaky-checkout-e2e</span><span class="grow"></span><span class="tag">auto</span></div></div>
      <div class="field"><div class="lbl">Agent</div><div class="agents"><span class="chip on">{mark('claude','C',12)}Claude Code</span><span class="chip">{mark('codex','O',12)}Codex</span><span class="chip">{mark('pi','π',12)}Pi</span><span class="chip" style="color:var(--ink-3)">+</span></div></div>
    </div>
    <div class="two">
      <div class="field"><div class="lbl">Model and mode</div><div class="row" style="gap:6px"><span class="chip" style="height:32px">{mark('claude','C',12)}Opus 4.6{icon('chevron-down','style="width:10px;height:10px"')}</span><span class="chip" style="height:32px">Plan first{icon('chevron-down','style="width:10px;height:10px"')}</span><span class="chip" style="height:32px">{icon('brain','style="width:11px;height:11px"')}High</span></div></div>
      <div class="field"><div class="lbl">Fan out</div><div class="fld"><span>Run</span><span class="mono" style="font-weight:600">1</span><span>agent on this task</span><span class="grow"></span><button class="btn icon xs ghost">−</button><button class="btn icon xs ghost">+</button></div></div>
    </div>
    <div class="field"><div class="lbl">Where should the agent run?</div>
      <div class="envs">
        <div class="env on"><span class="rb"><i></i></span><div><b>This laptop</b><span>Local · runs while the lid is open</span></div><span class="cnt">3 agents</span></div>
        <div class="env"><span class="rb"></span><div><b>build-box</b><span>SSH · dev@build-box</span></div><span class="cnt">12 agents</span></div>
        <div class="env"><span class="rb"></span><div><b>office-server</b><span>Always on · Orca server</span></div><span class="cnt">8 agents</span></div>
        <div class="env"><span class="rb"></span><div><b>Fresh cloud VM</b><span>Created for this task, torn down after</span></div><span class="cnt">on demand</span></div>
      </div></div>
  </div>
  <div class="actions tint"><span class="hint">Creates the worktree, opens a transcript and starts the agent.</span><span class="spacer"></span><button class="btn sm ghost">Cancel</button><button class="btn sm primary">Create task<span class="kbd" style="background:transparent;border-color:rgba(255,255,255,.35);color:#fff">⌘↩</span></button></div>
</div></div>"""
    return base + dialog

screens = {
    "Main.dc.html": ("Active worktree", main_screen()),
    "DiffReview.dc.html": ("Diff review with notes", diff_screen()),
    "TerminalMode.dc.html": ("Terminal-only mode", terminal_screen()),
    "NewTask.dc.html": ("New task dialog", newtask_screen()),
}
for name, (title, body) in screens.items():
    pathlib.Path(name).write_text(wrap(title, body))
canvas = {
  "artboards": [
    {"file": "Main.dc.html", "title": "Active worktree", "x": 0, "y": 0, "w": 1440, "h": 900},
    {"file": "DiffReview.dc.html", "title": "Diff review with notes", "x": 1520, "y": 0, "w": 1440, "h": 900},
    {"file": "TerminalMode.dc.html", "title": "Terminal-only mode", "x": 0, "y": 1040, "w": 1440, "h": 900},
    {"file": "NewTask.dc.html", "title": "New task", "x": 1520, "y": 1040, "w": 1440, "h": 900},
  ],
  "annotations": [
    {"id": "brief", "x": 0, "y": -160, "w": 520, "text": "Harness v1 screens on the Agentic UI design system. Dark is the default; every artboard has a theme tweak. Static mockups; copy is illustrative sample data."}
  ],
  "launch": {"view": "canvas"}
}
pathlib.Path("canvas.json").write_text(json.dumps(canvas, indent=2))
print("wrote", list(screens), "+ canvas.json")
