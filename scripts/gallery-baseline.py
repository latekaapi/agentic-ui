#!/usr/bin/env python3
"""The library's visual baselines: every gallery entry in both themes.

Each entry of `aui-gallery --list` is rendered with
`aui-gallery --screenshot <entry> <png> --theme <dark|light>` (the card at its
declared size, off-screen via `render_to_image`, downsampled to 1x) into
`design/baseline/<group>/<name>-<theme>.png`. Those PNGs are checked in: they
are the definition of what the library looks like (docs/81-audit C-17; they
replace the v1 `design/reference/` renders, which were two generations old).

    python3 scripts/gallery-baseline.py                 # check: render all, compare, exit 1 on a difference
    python3 scripts/gallery-baseline.py --update        # regenerate the baselines (a deliberate act, its own commit)
    python3 scripts/gallery-baseline.py --entry primitives/button --entry overlay/dialog
    python3 scripts/gallery-baseline.py --theme light --jobs 4

How a render is accepted. Every entry is rendered twice and the two PNGs must
be pixel-identical (an entry still animating would make the baseline an
arbitrary frame); up to `--tries` renders are taken until two consecutive ones
agree. An entry that never settles is reported and not written. Entries that
cannot be deterministic by construction are in `EXCLUDED`, each with its
reason; they are listed in every run, never silently skipped.

How a check compares. A pixel differs when any channel moves by more than
`--channel-tol` (default 8 of 255: the Lanczos downsample and font
rasterisation can move an edge pixel by a few levels between machines); an
entry fails when more than `--max-diff` (default 0.05 %) of its pixels differ,
or when its size changed. A failing entry gets
`target/gallery-baseline/diff/<group>/<name>-<theme>.png`: the new render with
the differing pixels painted magenta. The images are compared with Pillow,
nothing else is needed.

A window per render opens at the display's top-left corner and takes no focus;
by default the script waits until the machine has been idle for 60 s (macOS
`HIDIdleTime`) so it never interrupts someone typing, and it keeps waiting
while they work. `--no-idle-wait` skips that (CI, or a machine nobody uses).
It also waits while the pointer rests over the render area (x < 1480 pt from
the display's left edge): a hovered card would be baselined hovered. Captures
are stills: the gallery's `--screenshot` sets reduced motion (loops at rest,
springs landed) and draws frames through the delay, so asynchronous content
lands wherever the pointer is.

Exit 0 clean, 1 differences or unsettled entries, 2 a render failed or a
baseline is missing (that is not a pass).
"""
import argparse, concurrent.futures, os, shutil, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BIN = os.path.join(ROOT, "target", "debug", "aui-gallery")
BASELINE = os.path.join(ROOT, "design", "baseline")
OUT = os.path.join(ROOT, "target", "gallery-baseline")
THEMES = ("dark", "light")

# Entries that are not baselined, and why. Each reason must stay true.
EXCLUDED = {
    "foundations/motion": "the motion playground: its demos loop for as long as the card is open (dots "
                          "travel, rings pulse), so any capture is an arbitrary frame. Two renders can "
                          "still agree by landing on the same loop phase, which is how it slipped "
                          "through the settle check once; the springs it shows are tested in aui-motion",
}


def idle_seconds():
    try:
        out = subprocess.run(["ioreg", "-c", "IOHIDSystem"], capture_output=True, text=True, timeout=10).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    for line in out.splitlines():
        if "HIDIdleTime" in line:
            return int(line.split()[-1]) // 1_000_000_000
    return None


# Render windows open at the display's top-left corner and are at most this
# wide; a pointer resting inside that band hovers the card under it.
RENDER_AREA_W = 1480


def pointer_x():
    """The pointer's x in display points (macOS), or None."""
    if sys.platform != "darwin":
        return None
    try:
        import ctypes, ctypes.util
        cg = ctypes.CDLL(ctypes.util.find_library("CoreGraphics"))

        class Point(ctypes.Structure):
            _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

        cg.CGEventCreate.restype = ctypes.c_void_p
        cg.CGEventGetLocation.restype = Point
        cg.CGEventGetLocation.argtypes = [ctypes.c_void_p]
        cg.CFRelease.argtypes = [ctypes.c_void_p]
        event = cg.CGEventCreate(None)
        x = cg.CGEventGetLocation(event).x
        cg.CFRelease(event)
        return x
    except (OSError, AttributeError):
        return None


def wait_idle(need, max_wait):
    """Wait until nobody has touched the machine for `need` s and the pointer
    is clear of the render area (a resting pointer there hovers a card)."""
    start = time.time()
    while True:
        idle = idle_seconds()
        x = pointer_x()
        pointer_clear = x is None or x >= RENDER_AREA_W
        if (idle is None or idle >= need) and pointer_clear:
            return True
        if (idle is None or idle >= need) and not pointer_clear:
            if time.time() - start > max_wait:
                return False
            print(f"  waiting: the pointer is over the render area (x {x:.0f} < {RENDER_AREA_W}); "
                  "move it to the right edge of the screen", flush=True)
            time.sleep(5)
            continue
        if time.time() - start > max_wait:
            return False
        print(f"  waiting: the machine has been idle {idle} s, need {need} s", flush=True)
        time.sleep(max(5, need - idle))


def list_entries():
    out = subprocess.run([BIN, "--list"], capture_output=True, text=True, check=True, timeout=60).stdout
    return [line.split()[0] for line in out.splitlines() if line.strip()]


def stem(entry, theme):
    group, _, name = entry.partition("/")
    return os.path.join(group, f"{name}-{theme}.png")


def render(entry, theme, path, delay):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path):
        os.remove(path)
    try:
        p = subprocess.run([BIN, "--screenshot", entry, path, "--theme", theme, "--screenshot-delay", str(delay)],
                           cwd=ROOT, capture_output=True, text=True, timeout=90)
    except subprocess.TimeoutExpired:
        return "timed out"
    if not os.path.exists(path):
        return (p.stderr or p.stdout).strip()[-300:] or f"exit {p.returncode}, no image"
    return None


def same_pixels(a, b):
    from PIL import Image, ImageChops
    ia, ib = Image.open(a).convert("RGBA"), Image.open(b).convert("RGBA")
    return ia.size == ib.size and ImageChops.difference(ia, ib).getbbox() is None


def settled(entry, theme, tmp, delay, tries):
    """Render until two consecutive renders agree; return (path, error, renders)."""
    prev = None
    for k in range(tries):
        path = os.path.join(tmp, stem(entry, theme).replace(".png", f".{k}.png"))
        err = render(entry, theme, path, delay)
        if err:
            return None, err, k + 1
        if prev and same_pixels(prev, path):
            return path, None, k + 1
        prev = path
    return None, f"never rendered the same twice in {tries} renders (still animating?)", tries


def compare(base, new, channel_tol, diff_path):
    from PIL import Image
    a, b = Image.open(base).convert("RGB"), Image.open(new).convert("RGB")
    if a.size != b.size:
        return f"size {a.size[0]}×{a.size[1]} → {b.size[0]}×{b.size[1]}", 1.0
    pa, pb = a.load(), b.load()
    w, h = a.size
    marked, n = b.copy(), 0
    pm = marked.load()
    for y in range(h):
        for x in range(w):
            ca, cb = pa[x, y], pb[x, y]
            if abs(ca[0] - cb[0]) > channel_tol or abs(ca[1] - cb[1]) > channel_tol or abs(ca[2] - cb[2]) > channel_tol:
                n += 1
                pm[x, y] = (255, 0, 255)
    frac = n / float(w * h)
    if n:
        os.makedirs(os.path.dirname(diff_path), exist_ok=True)
        marked.save(diff_path)
    return f"{n} px differ ({frac * 100:.3f} %)", frac


def main():
    ap = argparse.ArgumentParser(description="Render and check the gallery's visual baselines.")
    ap.add_argument("--update", action="store_true", help="write design/baseline/ instead of checking")
    ap.add_argument("--entry", action="append", help="only these entries (repeatable)")
    ap.add_argument("--theme", choices=THEMES, action="append", help="only these themes")
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--delay", type=int, default=600, help="ms before each capture (default 600)")
    ap.add_argument("--tries", type=int, default=4, help="renders per entry until two agree")
    ap.add_argument("--channel-tol", type=int, default=8)
    ap.add_argument("--max-diff", type=float, default=0.05, help="percent of pixels allowed to differ")
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--no-idle-wait", action="store_true")
    a = ap.parse_args()

    if not a.no_build:
        subprocess.run(["cargo", "build", "-q", "-p", "aui-gallery"], cwd=ROOT, check=True)
    if not os.path.exists(BIN):
        sys.exit(f"{BIN} is missing: cargo build -p aui-gallery")
    entries = list_entries()
    unknown = [e for e in (a.entry or []) if e not in entries]
    if unknown:
        sys.exit(f"unknown entries: {', '.join(unknown)}")
    chosen = [e for e in entries if not a.entry or e in a.entry]
    themes = a.theme or list(THEMES)
    for e, why in EXCLUDED.items():
        if e not in entries:
            sys.exit(f"EXCLUDED names '{e}', which is no longer a gallery entry: drop it")
        if e in chosen:
            print(f"  excluded {e}: {why}")
    jobs = [(e, t) for e in chosen if e not in EXCLUDED for t in themes]

    if not a.no_idle_wait and not wait_idle(60, 3 * 3600):
        sys.exit("the machine never went idle for 60 s; not opening windows")

    tmp = tempfile.mkdtemp(prefix="gallery-baseline-")
    results = {}
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, a.jobs)) as pool:
            futs = {pool.submit(settled, e, t, tmp, a.delay, a.tries): (e, t) for e, t in jobs}
            for f in concurrent.futures.as_completed(futs):
                results[futs[f]] = f.result()

        failed_render, unsettled, differs, missing, written, clean = [], [], [], [], 0, 0
        for e, t in jobs:
            path, err, n = results[(e, t)]
            key = stem(e, t)
            if path is None:
                (unsettled if err.startswith("never") else failed_render).append((key, err))
                continue
            base = os.path.join(BASELINE, key)
            if a.update:
                os.makedirs(os.path.dirname(base), exist_ok=True)
                shutil.copyfile(path, base)
                written += 1
                continue
            if not os.path.exists(base):
                missing.append(key)
                continue
            new = os.path.join(OUT, "new", key)
            os.makedirs(os.path.dirname(new), exist_ok=True)
            shutil.copyfile(path, new)
            what, frac = compare(base, path, a.channel_tol, os.path.join(OUT, "diff", key))
            if frac * 100 > a.max_diff:
                differs.append((key, what))
            else:
                clean += 1

        if a.update:
            # A baseline whose entry is gone (or now excluded) is deleted.
            keep = {stem(e, t) for e in entries if e not in EXCLUDED for t in THEMES}
            stale = []
            for d, _, files in os.walk(BASELINE):
                for f in files:
                    rel = os.path.relpath(os.path.join(d, f), BASELINE)
                    if f.endswith(".png") and rel not in keep and not a.entry and not a.theme:
                        os.remove(os.path.join(d, f))
                        stale.append(rel)
            print(f"gallery baseline: wrote {written} of {len(jobs)} ({len(entries)} entries × {len(themes)} themes, "
                  f"{len(EXCLUDED)} excluded); removed {len(stale)} stale")
        else:
            print(f"gallery baseline: {clean} of {len(jobs)} match "
                  f"(tolerance {a.channel_tol}/255 per channel, {a.max_diff} % of pixels)")
        for key, err in failed_render:
            print(f"  RENDER FAILED {key}: {err}")
        for key, err in unsettled:
            print(f"  UNSETTLED     {key}: {err}")
        for key in missing:
            print(f"  NO BASELINE   {key} (run --update)")
        for key, what in differs:
            print(f"  DIFFERS       {key}: {what}  → target/gallery-baseline/diff/{key}")
        if failed_render or missing:
            return 2
        return 1 if unsettled or differs else 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
