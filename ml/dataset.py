import json
import os

import numpy as np
import torch
from PIL import Image

NMAX = 6
CROP = 96


def load_crop(path, u, v, ball_px):
    img = Image.open(path).convert("L")
    half = max(ball_px * 0.8, 8.0)
    crop = img.crop((int(u - half), int(v - half), int(u + half), int(v + half)))
    crop = crop.resize((CROP, CROP), Image.BILINEAR)
    return np.asarray(crop, dtype=np.float32) / 255.0


def build_cache(data_dir, out_pt):
    raw_root = os.path.join(data_dir, "raw")
    labels = os.path.join(data_dir, "labels.jsonl")
    samples = []
    for line in open(labels):
        d = json.loads(line)
        frames = d["frames"][:NMAX]
        if len(frames) < 2:
            continue
        arrs = [load_crop(os.path.join(raw_root, f["file"]), f["u"], f["v"], f["ball_px"]) for f in frames]
        n = len(arrs)
        while len(arrs) < NMAX:
            arrs.append(np.zeros((CROP, CROP), dtype=np.float32))
        samples.append({
            "x": torch.from_numpy(np.stack(arrs)),
            "n": n,
            "rpm": float(d["rpm"]),
            "axis": float(d["axis_deg"]),
            "fps": float(d["fps"]),
        })
    torch.save(samples, out_pt)
    print("cached %d shots -> %s" % (len(samples), out_pt))
    return len(samples)


class SpinSet(torch.utils.data.Dataset):
    def __init__(self, pt):
        self.s = torch.load(pt)

    def __len__(self):
        return len(self.s)

    def __getitem__(self, i):
        d = self.s[i]
        return (
            d["x"],
            torch.tensor(d["n"], dtype=torch.long),
            torch.tensor(d["rpm"], dtype=torch.float32),
            torch.tensor(d["axis"], dtype=torch.float32),
            torch.tensor(d["fps"], dtype=torch.float32),
        )


if __name__ == "__main__":
    import sys
    build_cache(sys.argv[1], sys.argv[2])
