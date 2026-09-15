#!/usr/bin/env python3
"""Shared chrome, tokens and helpers for the screen generators (harness and assistant)."""
import pathlib, json, re
ROOT = pathlib.Path(__file__).resolve().parents[1]
TOKENS = (ROOT/"tokens"/"tokens.css").read_text().replace(":root", ".aui")
BASE = (ROOT/"src"/"base.css").read_text()
SPRITE = (ROOT/"src"/"sprite.svg").read_text()

SHELL_CSS = """
body{margin:0;background:#121317}
.aui{width:1440px;height:900px;background:var(--bg);color:var(--ink);font:13px/1.5 var(--font-ui);display:grid;grid-template-columns:252px 1fr 400px;grid-template-rows:44px 1fr;overflow:hidden;position:relative}
.hd{display:flex;align-items:center;gap:6px;padding:0 10px;border-bottom:1px solid var(--line);background:var(--surface-1);min-width:0}
.hd:nth-child(1){border-right:1px solid var(--line)}.hd:nth-child(3){border-left:1px solid var(--line)}
.hd .ttl{font-weight:600;font-size:13px;display:flex;align-items:center;gap:7px;min-width:0;white-space:nowrap}
.hd .btn.ghost{color:var(--ink-3)}
.hd.tabs{padding:0 4px 0 6px;gap:2px}
.hd .tab{height:44px;padding:0 10px}.hd .tab.on::after{bottom:-1px}
.lights{display:flex;gap:7px;margin:0 8px 0 2px}.lights i{width:11px;height:11px;border-radius:50%;display:block}
.lights i:nth-child(1){background:#FF5F57}.lights i:nth-child(2){background:#FEBC2E}.lights i:nth-child(3){background:#28C840}
.side{border-right:1px solid var(--line);background:var(--surface-1);display:flex;flex-direction:column;min-height:0}
.nav{padding:10px 8px 6px}.nav .it{display:flex;align-items:center;gap:8px;height:30px;padding:0 8px;border-radius:var(--r-sm);color:var(--ink-2);font-weight:500}
.nav .it .i{color:var(--ink-3)}.nav .it .n{margin-left:auto;font:500 11px/1 var(--font-mono);color:var(--ink-3)}
.grp{display:flex;align-items:center;gap:6px;height:28px;padding:0 12px;margin-top:8px}
.grp .count{font:500 10.5px/1 var(--font-mono);color:var(--ink-3)}
.pj{display:flex;align-items:center;gap:8px;height:30px;padding:0 10px;margin:4px 8px 0;font-weight:600;font-size:12.5px;color:var(--ink)}
.pj .i{color:var(--ink-3)}.pj .chev{width:11px;height:11px}.pj .n{margin-left:auto;font:500 10.5px/1 var(--font-mono);color:var(--ink-3)}
.sr{display:grid;grid-template-columns:14px 1fr auto;gap:2px 8px;align-items:center;min-height:30px;padding:4px 10px 4px 12px;margin:0 8px 0 8px;border-radius:var(--r-md);font-size:12.5px;color:var(--ink-2);cursor:pointer;position:relative}
.sr.sel{background:var(--surface-3);color:var(--ink)}
.sr .nm{font-weight:500}.sr .t{font:500 11px/1 var(--font-mono);color:var(--ink-3)}
.sr .meta{grid-column:2/4;display:flex;align-items:center;gap:6px;font-size:11.5px;color:var(--ink-3);white-space:nowrap;overflow:hidden}
.sr .st{width:14px;height:14px;display:grid;place-items:center}
.sr.child{margin-left:22px;padding-left:10px}
.sr.child::before{content:"";position:absolute;left:-1px;top:2px;bottom:2px;width:1px;background:var(--line)}
.dg{display:flex;align-items:center;gap:8px;height:26px;padding:0 12px;margin-top:8px;font-size:11px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-3)}
.dg .rule{flex:1;height:1px;background:var(--line)}
.wt{display:grid;grid-template-columns:14px 1fr auto;gap:3px 8px;padding:9px 10px;margin:2px 8px;border-radius:var(--r-md)}
.wt.sel{background:var(--surface-3)}
.wt .dot{margin-top:5px}.wt .nm{font-weight:500;color:var(--ink)}.wt .t{font:500 11px/1 var(--font-mono);color:var(--ink-3);margin-top:3px}
.wt .meta{grid-column:2/4;display:flex;align-items:center;white-space:nowrap;overflow:hidden;gap:6px;font-size:12px;color:var(--ink-3);min-width:0}
.wt .meta .trunc{max-width:160px}
.center{display:flex;flex-direction:column;min-width:0;min-height:0;background:var(--bg)}
.tr{flex:1;overflow:hidden;padding:18px 32px 0;display:flex;flex-direction:column;gap:16px}
.sh{display:flex;align-items:center;white-space:nowrap;overflow:hidden;gap:10px;font-size:12px;color:var(--ink-3);padding-bottom:16px;border-bottom:1px solid var(--line)}
.sh>*{flex:none}.sh .cwd{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis}.sh b{color:var(--ink);font-weight:600}
.u{align-self:flex-end;max-width:72%;background:var(--surface-3);border-radius:12px 12px 4px 12px;padding:9px 13px;font-size:13px;line-height:1.5}
.a{font-size:13.5px;line-height:1.65}
.tc{display:flex;align-items:center;gap:8px;height:34px;padding:0 10px;border:1px solid var(--line);background:var(--surface-1);border-radius:var(--r-md);font-size:12.5px}
.tc .mono{color:var(--ink-2);font-size:12px}.tc .r{margin-left:auto;font:500 11px/1 var(--font-mono);color:var(--ink-3)}
.grp2{display:flex;flex-direction:column;gap:8px}
.status{margin-top:auto;padding:0 0 14px;font-size:12px;color:var(--ink-3)}
.comp{margin:0;border-top:1px solid var(--line);background:var(--surface-1);padding:0}
.comp .txt{font:14px/1.55 var(--font-ui);color:var(--ink-3);min-height:40px;padding:12px 32px 6px}
.comp .bar{display:flex;align-items:center;gap:6px;padding:6px 28px 10px}.comp .bar .chip{height:28px;padding:0 10px 0 8px}
.comp .send{margin-left:auto;width:28px;height:28px;border-radius:var(--r-sm);background:var(--accent);display:grid;place-items:center;color:#fff}
.right{border-left:1px solid var(--line);background:var(--surface-1);display:flex;flex-direction:column;min-height:0}
.rtabs{display:flex;align-items:center;gap:2px;height:34px;padding:0 6px;border-bottom:1px solid var(--line)}
.tab{display:flex;align-items:center;gap:6px;height:34px;padding:0 10px;font-size:12px;color:var(--ink-3);position:relative}
.tab.on{color:var(--ink)}.tab.on::after{content:"";position:absolute;left:8px;right:8px;bottom:-1px;height:2px;background:var(--ink);border-radius:2px 2px 0 0}
.meter{display:inline-flex;align-items:center;gap:6px;font:500 11px/1 var(--font-mono);color:var(--ink-3)}.meter i{width:40px;height:3px;border-radius:2px;background:var(--surface-3);overflow:hidden;display:block}.meter i b{display:block;height:100%;background:var(--ink-3);border-radius:2px}
/* thinking / activity / approval / summary */
.th{border:1px solid var(--line);border-radius:var(--r-md);background:var(--surface-1)}
.th .hh{display:flex;align-items:center;gap:8px;height:34px;padding:0 10px;font-size:12.5px}
.th .hh .t{margin-left:auto;font:500 11px/1 var(--font-mono);color:var(--ink-3)}
.ag{border:1px solid var(--line);border-radius:var(--r-md);background:var(--surface-1);overflow:hidden}
.ag .hh{display:flex;align-items:center;gap:8px;height:34px;padding:0 10px;font-size:12.5px}
.ag .steps{display:flex;gap:3px;margin-left:2px}.ag .steps i{width:14px;height:14px;border-radius:4px;background:var(--surface-3);display:grid;place-items:center;color:var(--ink-3)}
.ag .steps i.ok{background:var(--success-soft);color:var(--success)}.ag .steps i.run{background:var(--accent-soft);color:var(--accent-ink)}
.ag .hh .t{margin-left:auto;font:500 11px/1 var(--font-mono);color:var(--ink-3)}
.ag .tl{border-top:1px solid var(--line);padding:6px 10px 8px;display:flex;flex-direction:column}
.st{display:grid;grid-template-columns:18px 1fr auto;gap:0 8px;align-items:center;min-height:30px;font-size:12.5px;position:relative}
.st::before{content:"";position:absolute;left:8.5px;top:0;bottom:0;width:1px;background:var(--line)}
.st:first-child::before{top:50%}.st:last-child::before{bottom:50%}
.st .g{width:18px;height:18px;border-radius:50%;background:var(--surface-1);display:grid;place-items:center;position:relative;z-index:1}
.st .g i{width:8px;height:8px;border-radius:50%;background:var(--success);display:block}
.st .g i.run{background:var(--accent);box-shadow:0 0 0 3px var(--accent-soft)}.st .g i.pend{background:var(--surface-3);border:1px solid var(--line-strong)}
.st .v{font-weight:500;margin-right:6px}.st .mono{color:var(--ink-2);font-size:12px}.st .r{font:500 11px/1 var(--font-mono);color:var(--ink-3)}.st .r.add{color:var(--success)}
.st.pend{color:var(--ink-3)}
.ap{border:1px solid color-mix(in srgb, var(--warning) 70%, transparent);border-radius:var(--r-lg);background:var(--surface-1);overflow:hidden}
.ap .hh{display:flex;align-items:center;gap:10px;padding:10px 12px}
.ap .ic{width:26px;height:26px;border-radius:7px;background:var(--warning-soft);color:var(--warning);display:grid;place-items:center;flex:none}
.ap .ttl{font-weight:600;font-size:13px}.ap .sub{font-size:12px;color:var(--ink-2)}
.ap .cmd{margin:0 12px 10px;padding:8px 10px;border-radius:var(--r-sm);background:var(--term-bg);font:12px/1.5 var(--font-mono);color:var(--term-fg);display:flex;gap:8px}
.ap .cmd .p{color:var(--term-dim)}
.ap .k{display:flex;gap:8px;font-size:11px;color:var(--ink-3);margin-right:auto}.ap .k span{display:inline-flex;gap:4px;align-items:center}
.sm{border:1px solid var(--line);border-radius:var(--r-lg);background:var(--surface-1);overflow:hidden}
.sm .hh{display:flex;align-items:center;gap:8px;padding:10px 12px;font-weight:600}
.sm .files{padding:0 12px 8px;display:flex;flex-direction:column;gap:2px;font-size:12px}
.sm .f{display:flex;align-items:center;gap:8px;height:26px;font-family:var(--font-mono)}
.sm .f .stt{width:14px;text-align:center;font-weight:600;font-size:10px}.sm .f .stt.m{color:var(--warning)}.sm .f .stt.a{color:var(--success)}
.sm .f .d{margin-left:auto;font-size:11px}
.sm .checks{display:flex;gap:12px;padding:0 12px 10px;font-size:12px;color:var(--ink-2)}
/* right pane: files / diff / terminal */
.files{padding:8px 10px 6px;display:flex;flex-direction:column;gap:2px;font-size:12px}
.fr{display:flex;align-items:center;gap:7px;height:28px;padding:0 6px;border-radius:var(--r-sm);color:var(--ink-2)}
.fr .stt{margin-left:auto;font:600 10px/1 var(--font-mono)}.fr .stt.m{color:var(--warning)}.fr .stt.a{color:var(--success)}.fr .stt.d{color:var(--danger)}
.fr .n{width:14px;height:14px;border-radius:4px;background:var(--accent);color:#fff;font:600 9px/14px var(--font-mono);text-align:center}
.diff{margin:8px 10px;border:1px solid var(--line);border-radius:var(--r-md);overflow:hidden;font:11.5px/1.75 var(--font-mono)}
.diff .fh{display:flex;align-items:center;gap:8px;padding:7px 10px;background:var(--surface-2);font-family:var(--font-ui);font-size:12px}
.diff .hunk{padding:2px 10px;background:var(--surface-2);color:var(--ink-3);font-size:11px;border-top:1px solid var(--line);border-bottom:1px solid var(--line)}
.diff .ln{padding:0 8px}.diff .gutter{width:26px;padding-right:8px}
.note{margin:8px 10px 10px 44px;border:1px solid var(--line-strong);border-left:2px solid var(--accent);background:var(--surface-2);border-radius:var(--r-sm);padding:6px 8px;font-size:12px;line-height:1.5}
.note .caps{color:var(--accent-ink);margin-bottom:3px}.note.pend{border-style:dashed;border-left-style:solid}
.term{flex:1;background:var(--term-bg);display:flex;flex-direction:column;min-height:0}
.term .scroll{flex:1;overflow:hidden;padding:10px 12px;display:flex;flex-direction:column;gap:8px;font:12px/1.6 var(--font-mono);color:var(--term-fg)}
.blk{border-radius:8px;border:1px solid transparent;position:relative}
.blk .cmd{display:flex;align-items:center;gap:8px;padding:5px 10px;color:var(--term-fg)}
.blk .cmd .p{color:var(--accent);font-weight:600}
.blk .cmd .who{display:inline-flex;align-items:center;gap:4px;font:500 10px/1 var(--font-ui);color:var(--ink-3);margin-left:auto;padding-left:8px}
.blk .cmd .dur{font-size:11px;color:var(--term-dim)}
.blk .cmd .ex{width:6px;height:6px;border-radius:50%;background:var(--success);display:block}
.blk .cmd .ex.bad{background:var(--danger)}.blk .cmd .ex.run{background:var(--accent);box-shadow:0 0 0 3px var(--accent-soft)}
.blk .out{padding:2px 10px 8px 26px;color:var(--ink-2);overflow:hidden}.blk .out div{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.blk.old{opacity:.72}.blk.err .cmd{background:var(--danger-soft)}.blk.err .out{color:var(--ink)}
.blk.live{background:var(--surface-1);border-color:var(--line)}
.blk .fold{display:flex;align-items:center;gap:6px;padding:0 10px 6px 26px;font:11px var(--font-ui);color:var(--ink-3)}
.g{color:var(--ansi-green)}.y{color:var(--ansi-yellow)}.rd{color:var(--ansi-red)}.b{color:var(--ansi-blue)}.m{color:var(--ansi-magenta)}.d{color:var(--term-dim)}
.prompt{display:flex;align-items:center;gap:8px;padding:8px 12px;border-top:1px solid var(--line);background:var(--surface-1);font:12px var(--font-mono)}
.prompt .cur{width:7px;height:14px;background:var(--term-cursor);display:inline-block}
.prompt .ctx{margin-left:auto;font:500 10.5px var(--font-ui);color:var(--ink-3);display:flex;gap:8px}
.marker{display:flex;align-items:center;gap:8px;font:10.5px var(--font-ui);color:var(--ink-3);padding:0 2px}
.marker::before,.marker::after{content:"";flex:1;height:1px;background:var(--line)}
.tui{padding:14px 18px 0;font:12.5px/1.7 var(--font-mono);color:var(--term-fg);flex:1;overflow:hidden;display:flex;flex-direction:column}
.tui div{white-space:pre;min-height:1.7em}
.tui .sp{height:1.7em}
.tuiin{margin:0 18px 6px;border:1px solid var(--line-strong);border-radius:6px;padding:8px 12px;font:12.5px/1.5 var(--font-mono);color:var(--term-fg);display:flex;align-items:center;gap:8px}
.tuihint{padding:0 18px 12px;font:11.5px var(--font-mono);color:var(--term-dim)}
.seg{display:inline-flex;background:var(--surface-2);border-radius:var(--r-sm);padding:2px;gap:2px}
.seg span{height:24px;padding:0 10px;border-radius:5px;font-size:12px;display:inline-flex;align-items:center;gap:6px;color:var(--ink-3)}
.seg span.on{background:var(--surface-1);color:var(--ink);box-shadow:var(--shadow-1)}
/* tree */
.tree{padding:8px 6px;font-size:12px}
.tn{display:flex;align-items:center;gap:6px;height:26px;padding:0 6px;border-radius:var(--r-sm);color:var(--ink-2)}
.tn.on{background:var(--surface-3);color:var(--ink)}.tn .i{width:13px;height:13px;color:var(--ink-3)}.tn .fic{width:14px;height:14px}.tn .chev{width:10px;height:10px}
.tn .bd{margin-left:auto;font:600 10px var(--font-mono)}.tn .bd.m{color:var(--warning)}.tn .bd.a{color:var(--success)}.tn .bd.u{color:var(--ink-4)}
.tn.dir{color:var(--ink)}.d1{padding-left:18px}.d2{padding-left:32px}
/* dialog */
.scrim{position:absolute;inset:0;background:rgba(0,0,0,.45);display:grid;place-items:center;z-index:5}
.dlg{width:680px;background:var(--overlay);border:1px solid var(--line-strong);border-radius:var(--r-xl);box-shadow:var(--shadow-3);overflow:hidden}
.dlg .hh{display:flex;align-items:center;gap:8px;height:48px;padding:0 20px;border-bottom:1px solid var(--line);font-weight:600;font-size:14px}
.dlg .body{padding:18px 20px;display:flex;flex-direction:column;gap:16px}
.lbl{font-size:11px;font-weight:600;color:var(--ink-3);letter-spacing:.06em;text-transform:uppercase;margin-bottom:6px}
.field{display:flex;flex-direction:column}
.fld{display:flex;align-items:center;gap:8px;height:32px;padding:0 10px;border:1px solid var(--line);border-radius:var(--r-sm);background:var(--surface-2);font-size:13px}
.fld.ta{height:auto;min-height:88px;padding:10px 12px;align-items:flex-start;font-size:13.5px;line-height:1.5}
.fld .ph{color:var(--ink-3)}
.two{display:grid;grid-template-columns:repeat(2, minmax(0, 1fr));gap:12px}
.envs{display:grid;grid-template-columns:repeat(2, minmax(0, 1fr));gap:8px}
.env{display:flex;gap:10px;align-items:flex-start;padding:10px 12px;border:1px solid var(--line);border-radius:var(--r-md);background:var(--surface-1)}
.env.on{border-color:var(--accent);background:var(--surface-2)}
.env .rb{width:16px;height:16px;border-radius:50%;border:1.5px solid var(--line-strong);margin-top:1px;flex:none;display:grid;place-items:center}
.env.on .rb{border-color:var(--accent)}.env.on .rb i{width:8px;height:8px;border-radius:50%;background:var(--accent);display:block}
.env b{display:block;font-weight:500;font-size:12.5px}.env span{font-size:11.5px;color:var(--ink-3)}
.env .cnt{margin-left:auto;font:500 11px var(--font-mono);color:var(--ink-3);white-space:nowrap}
.agents{display:flex;gap:6px}
.agents .chip{height:30px;padding:0 10px 0 8px}
.agents .chip.on{border-color:var(--ink);color:var(--ink)}
"""

def icon(name, extra=""):
    return f'<svg class="i" {extra}><use href="#{name}"></use></svg>'

def mark(kind, letter, size=16):
    s = f'style="width:{size}px;height:{size}px;font-size:{max(6, size//2)}px"' if size != 16 else ""
    return f'<i class="mark {kind}" {s}>{letter}</i>'

RIGHT_TABS = [("git","Diff"),("folder","Files"),("terminal","Terminal"),("globe","Browser")]
def tabs_html(tabs, active):
    return ''.join(f'<div class="tab{" on" if n==active else ""}">{icon(i,"style=\"width:12px;height:12px\"")}{n}</div>' for i, n in tabs)

def header(center_title, center_sub, active_tab, tabs=None, agent=("claude","C")):
    """Three header cells: sidebar controls · provider mark + centre title + pane toggle · right-pane tabs + close."""
    tabs = tabs or RIGHT_TABS
    return f"""
<div class="hd"><div class="lights"><i></i><i></i><i></i></div><button class="btn icon sm ghost">{icon('arrow-left')}</button><button class="btn icon sm ghost" style="opacity:.45">{icon('arrow-right')}</button><span class="grow"></span><button class="btn icon sm ghost">{icon('search')}</button><button class="btn icon sm ghost">{icon('sidebar')}</button></div>
<div class="hd"><span class="ttl">{mark(agent[0], agent[1])}<span class="trunc">{center_title}</span><span class="tag">{center_sub}</span></span><span class="grow"></span><button class="btn icon sm ghost">{icon('dots')}</button><button class="btn icon sm ghost" title="Toggle right pane">{icon('panel-right')}</button></div>
<div class="hd tabs">{tabs_html(tabs, active_tab)}<button class="btn icon xs ghost">{icon('plus','style="width:12px;height:12px"')}</button><span class="grow"></span><button class="btn icon sm ghost">{icon('x')}</button></div>"""

def sidebar(selected="checkout-flow-v2", view="status"):
    if view == "projects":
        return sidebar_projects(selected)
    if view == "date":
        return sidebar_date(selected)
    def wt(name, t, dot, meta, meta2="", sel=False):
        return f'<div class="wt{" sel" if sel else ""}"><i class="dot {dot}"></i><div class="nm">{name}</div><span class="t">{t}</span><div class="meta">{meta}</div>{meta2}</div>'
    rows = [
        wt("checkout-flow-v2","49m","running pulse",f'<span class="tag">acme-web</span><span class="tag trunc">feature/checkout-flow-v2</span>{mark("claude","C",13)}',
           '<div class="meta" style="color:var(--ink-2)"><i class="spinner" style="width:10px;height:10px"></i><span class="trunc" style="max-width:190px">running checkout regression tests</span></div>', selected=="checkout-flow-v2"),
        wt("infra/notifier","3h","waiting pulse",f'<span class="tag">orca</span><span class="tag">main</span>{mark("codex","O",13)}',
           f'<div class="meta" style="color:var(--warning)">{icon("shield","style=\"width:11px;height:11px\"")}<span class="trunc" style="max-width:190px">awaiting permission · sudo apt install</span></div>'),
        wt("auth-session-refresh","4h","done",'<span class="tag">acme-web</span><span>PR #2491 open</span>'),
    ]
    rows2 = [
        wt("cart-recovery-email","12m","running",f'<span class="tag">acme-web</span><span class="tag trunc">feature/cart-recovery</span>{mark("claude","C",13)}'),
        wt("Webhook retry backoff","2d","",'<span class="tag">acme-internal</span><span class="tag trunc">fix/webhook-retry</span>'),
        wt("Observability tiles","1d","failed",'<span class="tag">acme-internal</span><span style="color:var(--danger)">2 tests failed</span>'),
        wt("checkout-baseline","3d","",'<span class="tag">acme-web</span><span class="tag">feature/checkout-baseline</span>'),
    ]
    return f"""
<aside class="side">
  <div class="nav"><div class="it">{icon('list')}Tasks<span class="n">7</span></div><div class="it">{icon('zap')}Automations</div><div class="it">{icon('inbox')}Inbox<span class="n" style="color:var(--warning)">2</span></div></div>
  <div class="grp"><span class="caps">Workspaces</span><span class="grow"></span><button class="btn icon xs ghost" title="View options">{icon('sliders','style="width:12px;height:12px"')}</button><button class="btn icon xs ghost">{icon('plus','style="width:12px;height:12px"')}</button></div>
  <div class="grp"><svg class="chev" style="transform:rotate(90deg)"><use href="#chevron"></use></svg><span style="font-weight:600;font-size:12px">Pinned</span><span class="count">3</span></div>
  {''.join(rows)}
  <div class="grp"><svg class="chev" style="transform:rotate(90deg)"><use href="#chevron"></use></svg><span style="font-weight:600;font-size:12px">In progress</span><span class="count">17</span></div>
  {''.join(rows2)}
  <div style="margin-top:auto;border-top:1px solid var(--line);padding:10px 12px;display:flex;align-items:center;gap:8px;font-size:12px;color:var(--ink-3)"><i class="avatar">A</i><span class="grow trunc">Alex Rivera · Max</span><span class="meter" title="Claude usage, 5-hour window">{mark("claude","C",11)}<i><b style="width:78%"></b></i>78%</span><button class="btn icon xs ghost">{icon('chevron-down','style="width:12px;height:12px"')}</button></div>
</aside>"""

STATUSBAR = ''  # removed: no bottom bar
_OLD_STATUSBAR = f'<div class="sb"><span class="meter">{mark("claude","C",12)}<i><b style="width:78%"></b></i>78% 5h</span><span class="meter">{mark("codex","O",12)}<i><b style="width:41%"></b></i>41% 5h</span><span class="grow"></span><span><i class="dot done" style="display:inline-block;width:6px;height:6px;margin-right:5px"></i>SSH build-box</span><span>5 agents</span></div>'

def session_header(mode="Plan"):
    return ''  # identity now lives in the centre header; model and mode in the composer

def _old_session_header(mode="Plan"):
    return f'<div class="sh">{mark("claude","C")}<b>Claude Code</b><span class="mono">v2.1.174</span><span>·</span><span>Opus 4.6 · Claude Max</span><span class="mono cwd">~/work/acme/checkout-flow-v2</span><span class="chip">{icon("terminal","style=\"width:11px;height:11px\"")}Local</span><span class="chip">{mode}</span></div>'

def ok_glyph():
    return '<i class="glyph ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i>'

def composer(placeholder="Reply, or type / for commands and @ to mention files", stop=True):
    send = icon('stop','style="stroke:#fff;width:13px;height:13px"') if stop else icon('arrow-up','style="stroke:#fff;width:13px;height:13px"')
    return f'<div class="comp"><div class="txt">{placeholder}</div><div class="bar"><button class="btn icon">{icon("plus")}</button><span class="chip">{mark("claude","C",12)}Opus 4.6{icon("chevron-down","style=\"width:10px;height:10px\"")}</span><span class="chip">Plan{icon("chevron-down","style=\"width:10px;height:10px\"")}</span><span class="subtle" style="font-size:11px;margin-left:6px">context 34%</span><div class="send">{send}</div></div></div>'

def rtabs(active):
    return ''  # tabs now live in the right header cell

def wrap(title, body, tweaks=True, theme_default="dark"):
    props = '{"theme":{"editor":"enum","options":["dark","light"],"default":"%s","section":"Theme"}}' % theme_default
    return f"""<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500;600&display=swap">
  <style>
{TOKENS}
{BASE}
{SHELL_CSS}
  a {{ color: var(--accent-ink); }} a:hover {{ color: var(--accent); }}
  </style>
</helmet>
<div class="aui" data-theme="{{{{theme}}}}">
{SPRITE}
{body}
</div>
</x-dc>
<script data-dc-script data-props='{props}'>
class Component extends DCLogic {{
  renderVals() {{
    return {{ theme: this.props.theme ?? '{theme_default}' }};
  }}
}}
</script>
</body>
</html>
"""


def ft(kind, extra=""):
    """File-type icon: folder, folder-open, ts, tsx, json, md, test, css, lock, image, file."""
    cls = {"folder-open":"folder"}.get(kind, kind)
    return f'<svg class="fic {cls}" {extra}><use href="#ft-{kind}"></use></svg>'


SIDEBAR_FOOTER = f'<div style="margin-top:auto;border-top:1px solid var(--line);padding:10px 12px;display:flex;align-items:center;gap:8px;font-size:12px;color:var(--ink-3)"><i class="avatar">A</i><span class="grow trunc">Alex Rivera · Max</span><span class="meter">{mark("claude","C",11)}<i><b style="width:78%"></b></i>78%</span><button class="btn icon xs ghost">{icon("chevron-down","style=\"width:12px;height:12px\"")}</button></div>'
SIDEBAR_NAV = f'<div class="nav"><div class="it">{icon("list")}Tasks<span class="n">7</span></div><div class="it">{icon("zap")}Automations</div><div class="it">{icon("inbox")}Inbox<span class="n" style="color:var(--warning)">2</span></div></div>'

def st_glyph(kind):
    """Right-aligned session status: running spinner, waiting shield, done check, failed x, idle nothing."""
    if kind == "running": return '<i class="spinner" style="width:11px;height:11px"></i>'
    if kind == "waiting": return icon("shield", 'style="width:12px;height:12px;color:var(--warning)"')
    if kind == "done": return icon("check", 'style="width:12px;height:12px;color:var(--success)"')
    if kind == "failed": return icon("x", 'style="width:12px;height:12px;color:var(--danger)"')
    return ''

def sr(name, t, dot, sel=False, child=False, meta=""):
    m = f'<div class="meta">{meta}</div>' if meta else ''
    return f'<div class="sr{" sel" if sel else ""}{" child" if child else ""}"><i class="dot {dot}"></i><span class="nm trunc">{name}</span><span class="t">{t}</span>{m}</div>'

def sidebar_projects(selected="checkout-flow-v2"):
    """Projects → sessions (worktrees) → child sessions, Claude-app style."""
    return f"""
<aside class="side">
  {SIDEBAR_NAV}
  <div class="grp"><span class="caps">Projects</span><span class="grow"></span><button class="btn icon xs ghost" title="View options">{icon('sliders','style="width:12px;height:12px"')}</button><button class="btn icon xs ghost">{icon('plus','style="width:12px;height:12px"')}</button></div>
  <div class="pj"><svg class="chev" style="transform:rotate(90deg)"><use href="#chevron"></use></svg>{ft('folder-open')}acme-web<span class="n">5</span></div>
  {sr("checkout-flow-v2","49m","running pulse", selected=="checkout-flow-v2", meta='<i class="spinner" style="width:10px;height:10px"></i><span class="trunc">running checkout regression tests</span>')}
  {sr("redesign auth flow","8m","running", child=False, meta='<span>2 children · PR 1/2 ready</span>')}
  {sr("PR 1/2 · migrate users.sql","6m","done", child=True)}
  {sr("PR 2/2 · withSession middleware","now","running", child=True)}
  {sr("cart-recovery-email","12m","running")}
  {sr("auth-session-refresh","4h","done", meta='<span>PR #2491 open</span>')}
  {sr("checkout-baseline","3d","")}
  <div class="pj"><svg class="chev" style="transform:rotate(90deg)"><use href="#chevron"></use></svg>{ft('folder-open')}orca<span class="n">2</span></div>
  {sr("infra/notifier","3h","waiting pulse", meta=f'<span style="color:var(--warning)">awaiting permission · sudo apt install</span>')}
  {sr("Improve agent handoff summary","1d","")}
  <div class="pj"><svg class="chev"><use href="#chevron"></use></svg>{ft('folder')}acme-internal<span class="n">4</span></div>
  <div class="pj" style="color:var(--ink-3);font-weight:500"><svg class="chev"><use href="#chevron"></use></svg>{ft('folder')}Archived<span class="n">37</span></div>
  {SIDEBAR_FOOTER}
</aside>"""

def sidebar_date(selected="checkout-flow-v2"):
    """Flat, most recent first, with day headers."""
    return f"""
<aside class="side">
  {SIDEBAR_NAV}
  <div class="grp"><span class="caps">Recent</span><span class="grow"></span><button class="btn icon xs ghost" title="View options">{icon('sliders','style="width:12px;height:12px"')}</button><button class="btn icon xs ghost">{icon('plus','style="width:12px;height:12px"')}</button></div>
  <div class="dg">Today<span class="rule"></span></div>
  {sr("checkout-flow-v2","49m","running pulse", selected=="checkout-flow-v2", meta='<span class="tag">acme-web</span><span class="trunc">running checkout regression tests</span>')}
  {sr("redesign auth flow","8m","running", meta='<span class="tag">acme-web</span><span>2 children</span>')}
  {sr("cart-recovery-email","12m","running", meta='<span class="tag">acme-web</span>')}
  {sr("infra/notifier","3h","waiting pulse", meta='<span class="tag">orca</span><span style="color:var(--warning)">awaiting permission</span>')}
  {sr("auth-session-refresh","4h","done", meta='<span class="tag">acme-web</span><span>PR #2491 open</span>')}
  <div class="dg">Yesterday<span class="rule"></span></div>
  {sr("Observability tiles","1d","failed", meta='<span class="tag">acme-internal</span><span style="color:var(--danger)">2 tests failed</span>')}
  {sr("Improve agent handoff summary","1d","", meta='<span class="tag">orca</span>')}
  <div class="dg">This week<span class="rule"></span></div>
  {sr("Webhook retry backoff","2d","", meta='<span class="tag">acme-internal</span>')}
  {sr("checkout-baseline","3d","", meta='<span class="tag">acme-web</span>')}
  {SIDEBAR_FOOTER}
</aside>"""
