#!/usr/bin/env python3
"""Build a stereo contact sheet and playback MP4 from a rendered case.

Reads renders/<case>/ (raw frames) or debug_frames/<case>/ (annotated
detection overlays with --debug) and writes a side-by-side left|right
contact sheet PNG plus an MP4 into viz/<case>/.
"""

import argparse
import glob
import json
import os
import re
import shutil
import subprocess
import tempfile

from PIL import Image, ImageDraw

PROJECT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GAP = 6
LABEL_H = 16
BG = (12, 12, 12)


def find_pairs(src_dir):
    lefts = sorted(glob.glob(os.path.join(src_dir, "*_left.png")))
    pairs = []
    for left in lefts:
        right = left.replace("_left.png", "_right.png")
        if os.path.exists(right):
            m = re.search(r"(\d+)", os.path.basename(left))
            idx = int(m.group(1)) if m else len(pairs)
            pairs.append((idx, left, right))
    pairs.sort(key=lambda p: p[0])
    return pairs


def load_labels(case):
    path = os.path.join(PROJECT_DIR, "renders", case, "manifest.json")
    if not os.path.exists(path):
        return {}
    d = json.load(open(path))
    out = {}
    for f in d.get("frames", []):
        p = f["ball_pos_mm"]
        out[f["index"]] = "f%d  (%.0f,%.0f,%.0f)mm" % (f["index"], p[0], p[1], p[2])
    return out


def stereo_frame(left_path, right_path, label):
    left = Image.open(left_path).convert("RGB")
    right = Image.open(right_path).convert("RGB")
    w, h = left.size
    combined = Image.new("RGB", (w * 2 + GAP, h + LABEL_H), BG)
    combined.paste(left, (0, LABEL_H))
    combined.paste(right, (w + GAP, LABEL_H))
    draw = ImageDraw.Draw(combined)
    draw.text((2, 3), label, fill=(180, 220, 180))
    return combined


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--case", required=True)
    p.add_argument("--debug", action="store_true", help="use annotated debug_frames overlays")
    p.add_argument("--fps", type=int, default=6, help="playback fps for the MP4")
    p.add_argument("--cols", type=int, default=3, help="contact sheet columns")
    args = p.parse_args()

    src_dir = os.path.join(
        PROJECT_DIR, "debug_frames" if args.debug else "renders", args.case
    )
    if not os.path.isdir(src_dir):
        raise SystemExit("no such dir: %s" % src_dir)

    pairs = find_pairs(src_dir)
    if not pairs:
        raise SystemExit("no left/right frame pairs in %s" % src_dir)

    labels = {} if args.debug else load_labels(args.case)
    out_dir = os.path.join(PROJECT_DIR, "viz", args.case)
    os.makedirs(out_dir, exist_ok=True)

    frames = [
        stereo_frame(l, r, labels.get(i, "f%d" % i)) for i, l, r in pairs
    ]

    fw, fh = frames[0].size
    cols = min(args.cols, len(frames))
    rows = (len(frames) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * fw + (cols + 1) * GAP, rows * fh + (rows + 1) * GAP), BG)
    for n, fr in enumerate(frames):
        cx = n % cols
        cy = n // cols
        sheet.paste(fr, (GAP + cx * (fw + GAP), GAP + cy * (fh + GAP)))
    suffix = "_debug" if args.debug else ""
    sheet_path = os.path.join(out_dir, "contact_sheet%s.png" % suffix)
    sheet.save(sheet_path)
    print("wrote", sheet_path)

    if shutil.which("ffmpeg"):
        tmp = tempfile.mkdtemp()
        for n, fr in enumerate(frames):
            fr.save(os.path.join(tmp, "%04d.png" % n))
        mp4_path = os.path.join(out_dir, "playback%s.mp4" % suffix)
        subprocess.run(
            ["ffmpeg", "-y", "-framerate", str(args.fps), "-i", os.path.join(tmp, "%04d.png"),
             "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-pix_fmt", "yuv420p", mp4_path],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        shutil.rmtree(tmp)
        print("wrote", mp4_path)
    else:
        print("ffmpeg not found; skipped MP4")


if __name__ == "__main__":
    main()
