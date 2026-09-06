#!/usr/bin/env python3
"""Assemble the assistant screen artboards from the shared chrome in ../common.py."""
import sys, pathlib, json
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from common import *

ASSIST_CSS = """
.sec{border-bottom:1px solid var(--line)}
.role{display:flex;align-items:center;gap:8px;height:44px;padding:0 12px 0 10px;font-weight:600;color:var(--ink)}
.role .i{color:var(--ink-3)}.role .chev{width:12px;height:12px}
.role.closed{color:var(--ink-2)}
.sub{padding:2px 8px 6px 8px}
.gh{display:flex;align-items:center;gap:8px;height:24px;padding:0 4px 0 12px;font-size:11px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-3)}
.gh .rule{flex:1;height:1px;background:var(--line)}
.gh .cnt{font:500 10.5px var(--font-mono);color:var(--ink-3)}
.proj{display:flex;align-items:center;gap:8px;height:28px;padding:0 8px 0 22px;border-radius:var(--r-sm);font-size:12.5px;font-weight:500;color:var(--ink-2)}.proj .i{color:var(--ink-3)}
.proj .n{margin-left:auto;font:500 11px/1 var(--font-mono);color:var(--ink-3)}
.sess{display:flex;align-items:center;gap:8px;height:28px;padding:0 8px 0 40px;border-radius:var(--r-sm);font-size:12.5px;color:var(--ink-3);position:relative}
.sess::before{content:"";position:absolute;left:29px;top:0;bottom:0;width:1px;background:var(--line)}
.sess.on{background:var(--surface-3);color:var(--ink)}.sess .t{margin-left:auto;font:500 11px/1 var(--font-mono);color:var(--ink-3)}.sess .i{color:var(--ink-3)}
.kb{margin:4px 8px 0;padding:10px 10px;border:1px solid var(--line);border-radius:var(--r-md);background:var(--surface-2);font-size:12px}
.kb .caps{margin-bottom:8px}.kb .row{gap:6px;flex-wrap:wrap}
.cite{display:inline-flex;align-items:center;justify-content:center;min-width:16px;height:16px;padding:0 4px;border-radius:4px;background:var(--accent-soft);color:var(--accent-ink);font:600 10px/1 var(--font-mono);vertical-align:2px;margin:0 1px}
.src{border:1px solid var(--line);border-radius:var(--r-lg);background:var(--surface-1);overflow:hidden}
.src .hh{display:flex;align-items:center;gap:8px;height:34px;padding:0 12px;font-weight:600;font-size:12.5px}
.tier{display:flex;align-items:center;gap:8px;padding:6px 12px 4px;font-size:11px;color:var(--ink-3);text-transform:uppercase;letter-spacing:.06em;font-weight:600}
.tier .sw{width:8px;height:8px;border-radius:2px;display:block}
.s{display:flex;align-items:flex-start;gap:10px;padding:6px 12px;font-size:12.5px;position:relative}
.s .n{width:18px;height:18px;border-radius:4px;background:var(--accent-soft);color:var(--accent-ink);font:600 10px/18px var(--font-mono);text-align:center;flex:none}
.s b{font-weight:500;display:block}.s span.sub2{font-size:11.5px;color:var(--ink-3)}
.s .conf{margin-left:auto;display:inline-flex;align-items:center;gap:5px;font:500 10.5px var(--font-mono);color:var(--ink-3);white-space:nowrap}
.s .conf i{width:36px;height:4px;border-radius:2px;background:var(--surface-3);overflow:hidden;display:block}.s .conf i b{display:block;height:100%;background:var(--success)}
.hover{position:absolute;right:24px;top:-8px;width:280px;background:var(--overlay);border:1px solid var(--line-strong);border-radius:var(--r-lg);box-shadow:var(--shadow-3);padding:10px 12px;font-size:12px;z-index:2}
.hover .q{border-left:2px solid var(--accent);padding-left:8px;color:var(--ink-2);margin:6px 0 8px;font-size:12px;line-height:1.5}
.hover .q mark{background:var(--accent-soft);color:var(--ink)}
.fc{display:flex;align-items:center;gap:12px;padding:10px 12px;border:1px solid var(--line);border-radius:var(--r-lg);background:var(--surface-1)}
.fc .ic{width:36px;height:36px;border-radius:8px;display:grid;place-items:center;flex:none}
.fc b{display:block;font-weight:600;font-size:13px}.fc span.d{font-size:12px;color:var(--ink-3)}
.fc .acts{margin-left:auto;display:flex;gap:6px}
/* document pane */
.dtabs{display:flex;align-items:center;gap:2px;height:34px;padding:0 6px;border-bottom:1px solid var(--line)}.dtabs .tab{white-space:nowrap;flex:none}
.tool{display:flex;align-items:center;gap:4px;height:36px;padding:0 10px;border-bottom:1px solid var(--line);font-size:12px;white-space:nowrap;overflow:hidden}.tool .chip{flex:none}
.tool .sepv{width:1px;height:16px;background:var(--line);margin:0 4px}
.tool .sel{height:26px;padding:0 8px;border-radius:5px;border:1px solid var(--line);background:var(--surface-2);font-size:12px;color:var(--ink);display:inline-flex;align-items:center;gap:6px}
.doc{flex:1;background:var(--surface-2);display:flex;justify-content:center;padding:20px;overflow:hidden}
.paper{width:340px;background:#fff;color:#1a1c22;box-shadow:var(--shadow-2);padding:34px 36px;font-family:Georgia,"Times New Roman",serif;font-size:12.5px;line-height:1.6}
.paper h1{font-size:18px;margin:0 0 4px;font-family:var(--font-ui);line-height:1.3}.paper .k{font:11px var(--font-ui);color:#7b818f;margin-bottom:18px}
.paper p{margin:0 0 10px}.paper .ai{background:rgba(86,92,196,.12);border-bottom:2px solid #565CC4}
.paper ol{padding-left:18px;margin:0 0 10px}
.arts{display:flex;gap:6px;padding:6px 10px;border-top:1px solid var(--line);overflow:hidden;align-items:center;white-space:nowrap}.arts .caps{flex:none}
.art{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 8px 0 6px;border:1px solid var(--line);border-radius:6px;font-size:11.5px;color:var(--ink-2);white-space:nowrap;flex:0 1 auto;min-width:0;overflow:hidden}
.art .i,.art .v{flex:none}
.art .nm{overflow:hidden;text-overflow:ellipsis;min-width:0}
.art.more{padding:0 8px;color:var(--ink-3);flex:none}
.art.on{border-color:var(--accent-ring);background:var(--accent-soft);color:var(--ink)}
.art .v{font:500 10px var(--font-mono);color:var(--ink-3)}
.dstat{display:flex;align-items:center;gap:10px;height:28px;padding:0 12px;border-top:1px solid var(--line);font:11px var(--font-ui);color:var(--ink-3)}
/* pdf pane */
.pdf{flex:1;background:var(--surface-2);display:flex;justify-content:center;padding:20px;overflow:hidden}
.pg{width:340px;background:#fff;color:#1a1c22;box-shadow:var(--shadow-2);padding:34px 36px;font:11.5px/1.6 Georgia,"Times New Roman",serif}
.pg h2{font:600 12px/1.4 var(--font-ui);margin:0 0 10px;color:#1a1c22}
.pg p{margin:0 0 9px}.pg .hl{background:rgba(86,92,196,.16);box-shadow:0 0 0 2px rgba(86,92,196,.16)}
.pg .pn{position:absolute}
/* sheet */
.sheet{flex:1;overflow:hidden;background:var(--surface-1);font:11.5px var(--font-ui);display:flex;flex-direction:column}
.fx{display:flex;align-items:center;gap:8px;height:30px;padding:0 10px;border-bottom:1px solid var(--line);font-size:12px;white-space:nowrap}.fx .chip{flex:none}
.fx .cell{width:52px;height:22px;border:1px solid var(--line);border-radius:4px;background:var(--surface-2);font:500 11px/22px var(--font-mono);text-align:center}
.fx .f{flex:1;height:22px;border:1px solid var(--line);border-radius:4px;background:var(--surface-2);font:11px/22px var(--font-mono);padding:0 8px;color:var(--ink-2)}
.grid{display:grid;grid-template-columns:32px repeat(5, minmax(0, 1fr));font-size:11.5px}
.grid div{height:26px;border-right:1px solid var(--line);border-bottom:1px solid var(--line);padding:0 8px;display:flex;align-items:center;white-space:nowrap;overflow:hidden}
.grid .h{background:var(--surface-2);color:var(--ink-3);font:500 10.5px var(--font-mono);justify-content:center}
.grid .hr{background:var(--surface-2);color:var(--ink-3);font:500 10.5px var(--font-mono);justify-content:center}
.grid .num{justify-content:flex-end;font-family:var(--font-mono);font-variant-numeric:tabular-nums}
.grid .th{font-weight:600;background:var(--surface-2)}
.grid .selc{outline:2px solid var(--accent);outline-offset:-2px;background:var(--accent-soft)}
.grid .tot{font-weight:600}
.stabs{display:flex;align-items:center;gap:2px;height:30px;padding:0 8px;border-top:1px solid var(--line);font-size:11.5px}
.stabs span{height:22px;padding:0 10px;border-radius:5px;display:inline-flex;align-items:center;color:var(--ink-3)}
.stabs span.on{background:var(--surface-3);color:var(--ink)}
"""

ROLE_ED = icon('grad-cap')
def a_sidebar(active="RFP draft v3"):
    def sess(ic, name, t, on=False):
        return f'<div class="sess{" on" if on else ""}">{icon(ic)}{name}<span class="t">{t}</span></div>'
    return f"""
<aside class="side">
  <div class="sec">
    <div class="role"><svg class="chev"><use href="#chevron-down"></use></svg>{ROLE_ED}Director, Education</div>
    <div class="gh">Projects<span class="rule"></span><span class="cnt">3</span></div>
    <div class="sub">
      <div class="proj">{icon('folder')}Teacher recruitment RFP<span class="n">6</span></div>
      {sess('doc','RFP draft v3','now', active=='RFP draft v3')}
      {sess('sparkle','Eligibility criteria review','2h', active=='Eligibility criteria review')}
      {sess('sheet','Vendor scoring sheet','1d', active=='Vendor scoring sheet')}
      <div class="proj">{icon('folder')}Annual report 2026<span class="n">3</span></div>
      <div class="proj">{icon('folder')}Letters and notes<span class="n">12</span></div>
    </div>
    <div class="gh">Knowledge<span class="rule"></span><span class="cnt">3</span></div>
    <div class="kb"><div class="row"><span class="chip">{icon('book','style="width:11px;height:11px"')}Education Code</span><span class="chip">{icon('book','style="width:11px;height:11px"')}Procurement Rules 2019</span><span class="chip">{icon('book','style="width:11px;height:11px"')}GO 2024-18</span><span class="chip" style="color:var(--accent-ink)">+ add</span></div></div>
    <div style="height:10px"></div>
  </div>
  <div class="sec"><div class="role closed"><svg class="chev"><use href="#chevron"></use></svg>{icon('scale')}Director, Law<span class="grow"></span><span class="cnt tag">2</span></div></div>
  <div class="sec"><div class="role closed"><svg class="chev"><use href="#chevron"></use></svg>{icon('gear')}Operations<span class="grow"></span><span class="cnt tag">4</span></div></div>
  <div style="margin-top:auto;border-top:1px solid var(--line);padding:8px 12px;display:flex;align-items:center;gap:8px;font-size:12px;color:var(--ink-3)"><i class="avatar">B</i><span class="grow trunc">Bharani</span><span class="meter">{mark("claude","C",11)}<i><b style="width:78%"></b></i>78%</span></div>
</aside>"""

def a_header(center_title, center_sub, right_title, right_sub):
    return f"""
<div class="hd"><div class="lights"><i></i><i></i><i></i></div><button class="btn icon sm ghost">{icon('arrow-left')}</button><button class="btn icon sm ghost" style="opacity:.45">{icon('arrow-right')}</button><span class="grow"></span><button class="btn icon sm ghost">{icon('search')}</button><button class="btn icon sm ghost">{icon('sidebar')}</button></div>
<div class="hd"><span class="ttl">{icon('grad-cap','style="color:var(--ink-3)"')}<span class="trunc">{center_title}</span><span class="tag">{center_sub}</span></span><span class="grow"></span><button class="btn icon sm ghost">{icon('dots')}</button><button class="btn icon sm ghost" title="Toggle right pane">{icon('panel-right')}</button></div>
<div class="hd tabs">{dtabs(right_title)}<button class="btn icon xs ghost">{icon('plus','style="width:12px;height:12px"')}</button><span class="grow"></span><button class="btn icon sm ghost">{icon('x')}</button></div>"""

A_STATUS = ''  # removed: no bottom bar
_OLD_A_STATUS = f'<div class="sb"><span>{icon("book","style=\"width:11px;height:11px;vertical-align:-2px\"")} grounded on Education · 3 sources</span><span class="grow"></span><span>files in ~/Desk/Education</span><span>Opus 4.6</span></div>'

def a_session_header(kind):
    return ''  # identity lives in the centre header

def _old_a_session_header(kind):
    return f'<div class="sh">{icon("grad-cap","style=\"color:var(--ink-3)\"")}<b>Director, Education</b><span>·</span><span>Teacher recruitment RFP</span><span class="mono cwd">{kind}</span><span class="chip">{icon("book","style=\"width:11px;height:11px\"")}Grounded</span><span class="chip">{mark("claude","C",12)}Opus 4.6</span></div>'

def a_composer(ph="Ask, draft, or type / for commands"):
    return f'<div class="comp"><div class="txt">{ph}</div><div class="bar"><button class="btn icon">{icon("plus")}</button><span class="chip">{icon("book","style=\"width:11px;height:11px\"")}Education + project{icon("chevron-down","style=\"width:10px;height:10px\"")}</span><span class="chip">{mark("claude","C",12)}Opus 4.6{icon("chevron-down","style=\"width:10px;height:10px\"")}</span><div class="send">{icon("arrow-up","style=\"stroke:#fff;width:13px;height:13px\"")}</div></div></div>'

def dtabs(active):
    tabs = [("doc","RFP-draft-v3.docx","var(--info)"),("sheet","vendor-scoring.xlsx","var(--success)"),("pdf","GO-2024-18.pdf","var(--danger)"),("pdf","procurement-rules-2019.pdf","var(--danger)")]
    tabs = [t for i, t in enumerate(tabs) if t[1] == active or i < 2][:3]
    return ''.join(f'<div class="tab{" on" if n==active else ""}">{icon(ic, "style=\"width:12px;height:12px\"")}{n}</div>' for ic, n, c in tabs)

def file_card(ic, color, name, desc, primary="Open"):
    return f'<div class="fc"><span class="ic" style="background:var(--surface-2);color:{color}">{icon(ic,"style=\"width:18px;height:18px\"")}</span><div><b>{name}</b><span class="d">{desc}</span></div><div class="acts"><button class="btn sm ghost">Download</button><button class="btn sm">{primary}</button></div></div>'

def sources_card(with_hover=False):
    hover = ''
    if with_hover:
        hover = f'<div class="hover"><b style="font-weight:600">Rule 14(2) · Eligibility of bidders</b><div class="q">“…shall hold a valid registration with the Directorate and shall have completed <mark>not less than three years of comparable placements</mark> in the preceding five years.”</div><div class="row" style="gap:6px"><button class="btn xs">Open page 31</button><button class="btn xs ghost">Insert quote</button></div></div>'
    return f"""
<div class="src"><div class="hh">{icon('book')}Sources<span class="subtle" style="font-weight:400">· 3 cited · 11 retrieved</span><span class="grow"></span><button class="btn xs ghost">Show all</button></div>
  <div class="tier">Role · Director, Education</div>
  <div class="s{' on' if with_hover else ''}"><span class="n">1</span><div><b>Procurement Rules 2019, Rule 14(2)</b><span class="sub2">procurement-rules-2019.pdf · p. 31 · registration and experience threshold</span></div><span class="conf"><i><b style="width:92%"></b></i>.92</span>{hover}</div>
  <div class="s"><span class="n">2</span><div><b>GO 2024-18 · Verification of credentials</b><span class="sub2">go-2024-18.pdf · §4 · original certificates at onboarding</span></div><span class="conf"><i><b style="width:88%"></b></i>.88</span></div>
  <div class="tier">Project · Teacher recruitment RFP</div>
  <div class="s"><span class="n">3</span><div><b>Project brief</b><span class="sub2">brief.docx · cohort size and institution count</span></div><span class="conf"><i><b style="width:97%"></b></i>.97</span></div>
</div>"""

def retrieval_group():
    return f"""<div class="ag"><div class="hh">{ok_glyph()}<span style="font-weight:500">Searched 3 knowledge sets</span><span class="subtle">· read 2 regulations · 11 passages</span><span class="steps"><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i></span><span class="t">6 s</span><svg class="chev"><use href="#chevron"></use></svg></div></div>"""

PAPER = """<div class="paper"><h1>Request for Proposal: Teacher Recruitment Services</h1><div class="k">Directorate of Education · Draft v3 · 5 September 2026</div><p><b>1. Purpose.</b> The Directorate invites proposals from qualified agencies for the recruitment of 240 secondary-school teachers across 38 institutions for the 2027 academic year.</p><p><b>2. Eligibility.</b> Bidders must hold a valid registration under the <span class="ai">Procurement Rules 2019, Rule 14(2)</span> and demonstrate <span class="ai">three years of comparable placements</span>. <span class="ai">Original certificates are verified at onboarding in line with GO 2024-18.</span></p><p><b>3. Scope of services.</b></p><ol><li>Sourcing and screening against the qualification matrix in Annex A.</li><li>Document verification in line with GO 2024-18.</li><li>Onboarding support through the first term.</li></ol></div>"""

def doc_pane():
    return f"""
<aside class="right">
  <div class="tool"><span class="sel">Body text{icon('chevron-down','style="width:10px;height:10px"')}</span><span class="sepv"></span><button class="btn icon xs ghost"><b>B</b></button><button class="btn icon xs ghost"><i>I</i></button><button class="btn icon xs ghost"><u>U</u></button><span class="sepv"></span><button class="btn icon xs ghost">{icon('list','style="width:12px;height:12px"')}</button><button class="btn icon xs ghost">{icon('link','style="width:12px;height:12px"')}</button><button class="btn icon xs ghost">{icon('image','style="width:12px;height:12px"')}</button><span class="grow"></span><span class="chip" style="color:var(--accent-ink)">{icon('sparkle','style="width:11px;height:11px"')}Ask</span></div>
  <div class="doc">{PAPER}</div>
  <div class="arts"><span class="caps" style="margin-right:4px">Created in chat</span><span class="art on">{icon('doc','style="width:11px;height:11px;color:var(--info)"')}<span class="nm">RFP-draft-v3.docx</span><span class="v">v3</span></span><span class="art">{icon('sheet','style="width:11px;height:11px;color:var(--success)"')}<span class="nm">vendor-scoring.xlsx</span><span class="v">v1</span></span><span class="art more">+1</span></div>
  <div class="dstat"><span>Page 1 of 4</span><span>·</span><span>1,214 words</span><span class="grow"></span><span style="color:var(--accent-ink)">3 changes from chat highlighted</span><span>·</span><span>Saved</span></div>
</aside>"""

def pdf_pane():
    return f"""
<aside class="right">
  <div class="tool"><button class="btn icon xs ghost">{icon('arrow-left','style="width:12px;height:12px"')}</button><span class="sel"><span class="mono">31</span><span class="subtle">/ 88</span></span><button class="btn icon xs ghost">{icon('arrow-right','style="width:12px;height:12px"')}</button><span class="sepv"></span><span class="sel">100%{icon('chevron-down','style="width:10px;height:10px"')}</span><button class="btn icon xs ghost">{icon('search','style="width:12px;height:12px"')}</button><span class="grow"></span><span class="pill line">{icon('link','style="width:10px;height:10px"')}cited as 1</span><button class="btn xs">Insert quote</button></div>
  <div class="pdf"><div class="pg"><h2>Chapter IV · Eligibility and qualification of bidders</h2><p><b>14. Eligibility of bidders.</b> (1) Every bidder shall be a legal entity registered in the State and shall not be blacklisted by any department of the Government at the time of submission.</p><p>(2) A bidder for recruitment or staffing services <span class="hl">shall hold a valid registration with the Directorate and shall have completed not less than three years of comparable placements in the preceding five years</span>, evidenced by completion certificates from the engaging authority.</p><p>(3) Joint ventures shall satisfy sub-rule (2) through the lead member, whose share shall not be less than fifty-one percent.</p><p><b>15. Disqualification.</b> A bidder shall be disqualified where the bid contains a material misrepresentation, or where the bidder has been convicted of an offence involving fraud within the preceding five years.</p><p style="color:#7b818f;font-size:10.5px;margin-top:22px">Procurement Rules 2019 · 31</p></div></div>
  <div class="dstat"><span>Highlight from citation 1</span><span class="grow"></span><span>Opened from chat</span></div>
</aside>"""

def sheet_pane():
    rows = [("Vendor","Experience","Coverage","Price","Weighted"),
            ("Northlight Staffing","4.5","4.0","3.5","4.05"),
            ("Meridian Educators","3.0","4.5","4.5","3.90"),
            ("Bright Path","5.0","3.0","3.0","3.85"),
            ("Civic Talent","2.5","3.5","5.0","3.45")]
    weights = ("Weight","0.45","0.30","0.25","")
    cells = '<div class="h"></div>' + ''.join(f'<div class="h">{c}</div>' for c in "ABCDE")
    def row(n, r, cls=""):
        out = f'<div class="hr">{n}</div>'
        for j, v in enumerate(r):
            k = "th" if n == 1 else ("num" if j > 0 else "")
            if cls and j == 4: k += " " + cls
            if n == 2 and j == 4: k += " selc"
            out += f'<div class="{k}">{v}</div>'
        return out
    body = cells + row(1, rows[0]) + ''.join(row(i+2, r, "tot") for i, r in enumerate(rows[1:])) + row(6, weights)
    return f"""
<aside class="right">
  <div class="fx"><span class="cell">E2</span><span class="f">=SUMPRODUCT(B2:D2,B$7:D$7)</span><span class="chip" style="color:var(--accent-ink)">{icon('sparkle','style="width:11px;height:11px"')}Ask about selection</span></div>
  <div class="sheet"><div class="grid">{body}</div></div>
  <div class="stabs"><span class="on">Scores</span><span>Matrix</span><span>Notes</span><span class="grow"></span><span class="tag">weighted by Annex A</span></div>
  <div class="arts"><span class="caps" style="margin-right:4px">Created in chat</span><span class="art">{icon('doc','style="width:11px;height:11px;color:var(--info)"')}<span class="nm">RFP-draft-v3.docx</span><span class="v">v3</span></span><span class="art on">{icon('sheet','style="width:11px;height:11px;color:var(--success)"')}<span class="nm">vendor-scoring.xlsx</span><span class="v">v1</span></span></div>
  <div class="dstat"><span>E2 selected</span><span class="grow"></span><span>Saved</span></div>
</aside>"""

def screen_main():
    center = f"""
<main class="center">
  <div class="tr">
    {a_session_header('RFP draft v3')}
    <div class="u">Tighten section 2. Eligibility must reflect the current procurement rules and the 2024 verification order.</div>
    {retrieval_group()}
    <div class="a">Under the current rules a bidder needs a valid registration and three years of comparable placements<span class="cite">1</span>. The 2024 order moved certificate verification to onboarding and requires originals<span class="cite">2</span>. The brief sets the cohort at 240 teachers across 38 institutions<span class="cite">3</span>. I folded all three into section 2 and left the rest of the draft untouched.</div>
    {file_card('doc','var(--info)','RFP-draft-v3.docx','Section 2 rewritten · 3 changes highlighted · v3','Open in pane')}
    <div class="row status"><i class="dot done" style="width:6px;height:6px"></i><span>Done</span><span>·</span><span>3 sources cited</span></div>
  </div>
  {a_composer()}
</main>"""
    return a_header("Teacher recruitment RFP","RFP draft v3","RFP-draft-v3.docx","Saved · v3") + a_sidebar() + center + doc_pane() + A_STATUS

def screen_sources():
    center = f"""
<main class="center">
  <div class="tr">
    {a_session_header('Eligibility criteria review')}
    <div class="u">What does a bidder need to qualify, and where does that come from?</div>
    {retrieval_group()}
    <div class="a">Three requirements, each from a different source. A valid registration with the Directorate and at least three years of comparable placements in the preceding five<span class="cite">1</span>. Original certificates verified at onboarding rather than at bid time<span class="cite">2</span>. And for this RFP specifically, capacity for 240 teachers across 38 institutions<span class="cite">3</span>. Nothing in the Education Code adds to this; it defers to the procurement rules.</div>
    {sources_card(with_hover=True)}
    <div class="row status"><i class="dot done" style="width:6px;height:6px"></i><span>Done</span><span>·</span><span>hovering citation 1 · source open on the right</span></div>
  </div>
  {a_composer('Ask a follow-up, or drag a passage here to quote it')}
</main>"""
    return a_header("Teacher recruitment RFP","Eligibility criteria review","procurement-rules-2019.pdf","page 31 of 88") + a_sidebar('Eligibility criteria review') + center + pdf_pane() + A_STATUS

def screen_sheet():
    center = f"""
<main class="center">
  <div class="tr">
    {a_session_header('Vendor scoring sheet')}
    <div class="u">Score the four vendors against the Annex A matrix. Weight experience 45, coverage 30, price 25.</div>
    <div class="ag"><div class="hh">{ok_glyph()}<span style="font-weight:500">Read Annex A</span><span class="subtle">· built the matrix · wrote vendor-scoring.xlsx</span><span class="steps"><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i><i class="ok"><svg class="i" style="width:9px;height:9px;stroke-width:3"><use href="#check"></use></svg></i></span><span class="t">14 s</span><svg class="chev"><use href="#chevron"></use></svg></div></div>
    <div class="a">Northlight leads on the weighted score, mostly on experience. Civic Talent is cheapest but thin on experience, which the matrix penalises hardest. The weights sit in row 7 so you can change them and the totals follow.</div>
    {file_card('sheet','var(--success)','vendor-scoring.xlsx','Scores, Matrix and Notes sheets · formulas live · v1','Open in pane')}
    <div class="row status"><i class="dot done" style="width:6px;height:6px"></i><span>Done</span><span>·</span><span>E2 selected in the sheet</span></div>
  </div>
  {a_composer('Ask about the selection, or change the weights')}
</main>"""
    return a_header("Teacher recruitment RFP","Vendor scoring sheet","vendor-scoring.xlsx","Saved · v1") + a_sidebar('Vendor scoring sheet') + center + sheet_pane() + A_STATUS

import common
common.SHELL_CSS = common.SHELL_CSS + ASSIST_CSS
screens = {
    "Main.dc.html": ("Project session with document", screen_main()),
    "Sources.dc.html": ("Answer with citations and source", screen_sources()),
    "Sheet.dc.html": ("Spreadsheet created in chat", screen_sheet()),
}
for name, (title, body) in screens.items():
    pathlib.Path(name).write_text(common.wrap(title, body, theme_default="light"))
canvas = {
  "artboards": [
    {"file": "Main.dc.html", "title": "Project session with document", "x": 0, "y": 0, "w": 1440, "h": 900},
    {"file": "Sources.dc.html", "title": "Answer with citations", "x": 1520, "y": 0, "w": 1440, "h": 900},
    {"file": "Sheet.dc.html", "title": "Spreadsheet created in chat", "x": 0, "y": 1040, "w": 1440, "h": 900},
  ],
  "annotations": [
    {"id": "brief", "x": 0, "y": -160, "w": 560, "text": "Assistant v1 screens. Light is the default here; each artboard has a theme tweak. Roles → projects → sessions in the sidebar; the right pane holds documents the chat created. Static mockups with sample content."}
  ],
  "launch": {"view": "canvas"}
}
pathlib.Path("canvas.json").write_text(json.dumps(canvas, indent=2))
print("wrote", list(screens), "+ canvas.json")
