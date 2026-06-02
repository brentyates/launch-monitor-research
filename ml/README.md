# ML spin estimator

Learned spin estimator for the **cheap-fps regime (500–800 fps)** where the classical
CV methods hit a wall (consecutive-frame rotation too large; see `../DESIGN.md`). The
Blender renderer is a perfect-label data factory, which makes this a clean supervised
problem.

## Result (validated)

Trained on EEVEE shots at 500–1000 fps; evaluated on a held-out **Cycles** test set the
model never saw:

- **rpm error ~8% median, axis error ~0.43° median**, uniform across driver→wedge.
- No domain collapse EEVEE→Cycles (augmentation bridges the engine gap).
- vs 50–300% for every hand-built CV method at the same fps.

Still data-bound (train ≪ val) → more data should push rate toward the ≤5% ("good") bar.

## Environment

```bash
uv venv --python 3.12 ml/.venv
VIRTUAL_ENV=ml/.venv uv pip install torch numpy pillow   # torch 2.12, MPS on Apple Silicon
```

## Pipeline

1. **Generate data** (Blender, one-time background cost — *not* the inner loop):
   ```bash
   bash ml/gen.sh train 3000 1000              # EEVEE, ~1 shot/s, stable
   ENGINE=cycles SAMPLES=16 bash ml/gen.sh test_cycles 250 900000   # held-out Cycles eval set
   ```
   `blender/gen_dataset.py` renders randomized shots (rpm/axis/flight/fps/zoom/noise) with
   exact GT ball center+size per frame → `ml/data/<name>/{raw,labels.jsonl}`. EEVEE avoids
   the Cycles Metal long-session crash; the Cycles path is chunked (Blender restarts) to
   dodge it.

2. **Cache** (crop around GT center, resize to 64×64, normalize):
   ```bash
   ml/.venv/bin/python ml/dataset.py ml/data/train ml/data/train/cache.pt
   ```

3. **Train** (the fast ~2–3 min loop on cached data — iterate architecture/aug here):
   ```bash
   ml/.venv/bin/python ml/train.py --cache ml/data/train/cache.pt --epochs 120
   ```

4. **Evaluate** a checkpoint on any cache (e.g. the Cycles transfer set):
   ```bash
   ml/.venv/bin/python ml/eval.py --cache ml/data/test_cycles/cache.pt --ckpt ml/runs/spinnet.pt
   ```

## Model (`model.py`)

`SpinNet`: **early fusion** — the frame sequence is fed as CNN input channels so the first
conv compares frames (the inter-frame rotation cue). Regresses rpm + axis, with fps as an
input. A per-frame CNN + GRU was tried first and *underfit* (it can't see rotation across
independently-encoded frames).

## Notes

- Karpathy-style fast loop: data gen is a one-time background factory; experiments are
  cache+train+eval (~minutes). Get a first signal on a small partial pool, scale data while
  iterating.
- `data/`, `runs/`, `.venv/` are gitignored.
