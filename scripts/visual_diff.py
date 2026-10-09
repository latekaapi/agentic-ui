#!/usr/bin/env python3
"""Gate a gallery capture against its mock PNG.

    visual_diff.py --mock mock.png --capture capture.png [--threshold 0.02] [--out diff.png]

Both images must be the same size (nothing is resized). A pixel differs when
its largest channel difference exceeds 24/255. The script prints
`diff=<fraction> threshold=<t> PASS|FAIL` and exits 0 on PASS, 1 on FAIL, and
2 on a size mismatch or a missing file. With --out it writes the capture
dimmed, with differing pixels in red.
"""
import argparse
import os
import sys

from PIL import Image, ImageChops

CHANNEL_LIMIT = 24


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--mock", required=True)
    ap.add_argument("--capture", required=True)
    ap.add_argument("--threshold", type=float, default=0.02)
    ap.add_argument("--out")
    args = ap.parse_args()

    for path in (args.mock, args.capture):
        if not os.path.isfile(path):
            print(f"missing file: {path}", file=sys.stderr)
            return 2
    mock = Image.open(args.mock).convert("RGBA")
    cap = Image.open(args.capture).convert("RGBA")
    if mock.size != cap.size:
        print(f"size mismatch: mock={mock.size[0]}x{mock.size[1]} capture={cap.size[0]}x{cap.size[1]}", file=sys.stderr)
        return 2

    diff = ImageChops.difference(mock, cap)
    channels = diff.split()
    # Max over R, G, B (alpha ignored), then threshold to a mask.
    peak = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
    mask = peak.point(lambda v: 255 if v > CHANNEL_LIMIT else 0)
    total = mask.size[0] * mask.size[1]
    differing = mask.histogram()[255]
    fraction = differing / total if total else 0.0

    if args.out:
        dim = Image.blend(cap.convert("RGB"), Image.new("RGB", cap.size, (0, 0, 0)), 0.6)
        dim.paste(Image.new("RGB", cap.size, (255, 0, 0)), mask=mask)
        dim.save(args.out)

    ok = fraction <= args.threshold
    print(f"diff={fraction:.6f} threshold={args.threshold} {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
