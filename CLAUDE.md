# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Purpose

The project's goal is a **hardware feasibility & optimization study**: determine the cheapest hardware (camera fps/resolution/shutter, lens, lighting) and best placement (mount height/offset, stereo baseline/convergence) that can build a working DIY overhead golf launch monitor meeting an accuracy budget — all in simulation, without buying hardware. The CV pipeline and Blender renderer are the means to explore that design space with exact ground truth.

**Operating loop (the project's working model):** current software → implied hardware requirement → judge affordability → if infeasible, improve software to relax it → repeat. The deliverable is the hardware requirement; software work is demand-driven by affordability gaps.

**`DESIGN.md` is the running research log + current status — read it first.** In brief: position is solved and cheap; spin via classical CV works down to ~1800 fps (commercial range) but hits a wall at the affordable 500–800 fps, where a **learned estimator (ml/)** instead reaches ~8% rate / 0.43° axis (validated EEVEE→Cycles).

## Commands

```bash
cargo build --release          # builds all 4 bins: lm-test, lm-sweep, lm-rig, lm-spin-sweep
cargo test

# Core E2E (overhead stereo position pipeline, 3 standard shots)
./scripts/test-e2e.sh                   # build + render + run (PASS/FAIL)
./scripts/view.sh <case> [--debug]      # stereo contact sheet + MP4 into viz/<case>/ (--debug = overlays)
./scripts/cleanup.sh                    # remove renders/ and debug frames

# Rigs — first-class camera setups in rigs/*.json (generate via scripts/gen_rigs.py)
python3 scripts/gen_rigs.py             # (re)generate rigs/overhead_stereo.json, rigs/spin_overhead_mono.json
RENDER=1 ./target/release/lm-rig <rig>  # render + solve a named rig (default overhead_stereo)

# Sweeps
RENDER=1 ./target/release/lm-sweep [configs/sweep.json]   # hardware-config sweep -> results/sweep.csv + cost frontier
SPIN_METHOD=global RENDER=1 ./target/release/lm-spin-sweep configs/<spec>.json  # spin sweep (SPIN_METHOD=search|dense|global)

# ML spin estimator (Python, uv venv + PyTorch MPS) — see ml/README.md
bash ml/gen.sh train 3000 1000          # render training data (EEVEE; ENGINE=cycles for a Cycles eval set)
ml/.venv/bin/python ml/dataset.py ml/data/train ml/data/train/cache.pt
ml/.venv/bin/python ml/train.py --cache ml/data/train/cache.pt
ml/.venv/bin/python ml/eval.py --cache <cache.pt> --ckpt ml/runs/spinnet.pt
```

Frame sources are Blender-rendered datasets on disk; binaries render any missing data automatically, or `RENDER=1` forces re-render. Override the Blender binary with `BLENDER=...` (default `/Applications/Blender.app/Contents/MacOS/Blender`).

## Architecture

Stereo CV pipeline that loads Blender-rendered stereo frames from disk, detects a golf ball in each stereo view, triangulates 3D positions, estimates launch parameters, and detects spin.

### Data Flow

```
Blender Cycles renderer (blender/render_shot.py)
  → renders/<case>/{left,right}_NNN.png + manifest.json (one shot per case)
    → RenderedDatasetSource reads PNGs + manifest, converts RGB→Gray
      → BallDetector: background subtraction → peak finding → weighted centroid
        → StereoTriangulator: DLT-SVD triangulation → least-squares velocity fit
          → SpinDetector: TP5 Pix chevron pattern matching (coarse→medium→fine search)
            → ProcessingResult { launch, spin, ground_truth, errors }
```

### Module Responsibilities

| Module | Purpose |
|--------|---------|
| `frame_source/rendered_dataset.rs` | Reads `manifest.json` + PNG frame pairs from `renders/<case>/`, converts RGB→Gray (no Y-flip). Replaces the removed Unity shared-memory source |
| `ball_detector.rs` | Temporal-minimum background model, peak-based detection with weighted centroid |
| `triangulation.rs` | OpenCV-style projection matrices (P = K[R|t]), DLT-SVD triangulation, velocity fitting with outlier rejection (mean ± 2σ) |
| `spin_detector.rs` | 3-stage exhaustive search (273K hypotheses at coarse, parallelized with Rayon) over RPM/axis/orientation. Scores by projecting TP5 Pix chevron geometry onto detected features |
| `pipeline.rs` | Orchestrator — runs detection, filters consecutive pairs (max 3-frame gap), filters by median radius, triangulates, estimates spin |
| `config.rs` | `StereoRig::overhead()` — computes intrinsics and converging (toe-in) camera extrinsics from physical parameters |
| `debug.rs` | Overlay rendering (circles, lines, text) onto gray frames, saves annotated PNGs to `debug_frames/` |
| `main.rs` (lm-test) | Core harness — renders missing cases via Blender, runs 3 standard shots (driver/7-iron/wedge) on the overhead stereo rig, compares to GT |
| `config.rs` `Camera`/`CameraDef` | General camera from explicit position+aim+intrinsics (arbitrary placement); `StereoRig::overhead` is one generator |
| `frame_source/rig_dataset.rs` | Loads a rig dataset (manifest with N cameras + per-frame per-camera PNGs); used by lm-rig |
| `spin_tracker.rs` | Newer spin estimators: `estimate_spin_dense` (per-pair appearance NCC, `SPIN_METHOD=dense`) and `estimate_spin_global` (one shared rotation across all pairs, `SPIN_METHOD=global`, best classical) |
| `sweep.rs` (lm-sweep) | Hardware-config sweep over rigs → accuracy + rough-cost frontier |
| `rig_runner.rs` (lm-rig) | Run a named rig (`rigs/*.json`), solve position and/or spin per its `measures` |
| `spin_sweep.rs` (lm-spin-sweep) | Sweep fps × zoom × rpm (× noise seeds) for spin estimators; `SPIN_METHOD` selects the method |

The Unity simulator has been removed; frames now come entirely from the offline Blender renderer.

### Rigs, spin estimators, and ML

- **Rigs are the first-class experiment unit.** `rigs/<name>.json` defines a named device setup — a list of cameras placed by explicit position + aim + intrinsics, plus what it `measures` (`position` / `spin`). `lm-rig` renders+solves one; rigs are independent (own `renders/<rig>/`, `results/<rig>`). The overhead-convergence geometry is one generator (`scripts/gen_rigs.py`); arbitrary placement (side mounts, mono spin cam) is just data.
- **Spin estimators evolved through four approaches** (logged with pre-registered keep/reject bars in `DESIGN.md`): exhaustive `search` (noise-fragile → rejected), `dense` per-pair registration, `global` multi-frame fit (best classical, works to ~37°/frame ≈ 1800 fps), and the **ML** estimator for the cheap-fps regime.
- **ML spin** (`ml/`): PyTorch early-fusion CNN trained on renderer-labeled data; the only thing that works at the affordable 500–800 fps. See `ml/README.md`.
- **Sensor realism**: `render_shot.py --exposure-us` adds global-shutter motion blur (strobe-frozen by default; long exposure is out of scope). Sensor noise / bit-depth are not yet modeled — current accuracy numbers are the low-noise ceiling.

### Render dataset format

Each `renders/<case>/` holds `manifest.json` plus `left_NNN.png` / `right_NNN.png` pairs. `manifest.json` carries width/height/fps, the per-frame timestamps and filenames, and the shot ground truth. `RenderedDatasetSource` is the only consumer; see it and `blender/render_shot.py` for the exact fields.

### Coordinate/Unit Conventions

- Positions: millimeters. Velocities: mm/s internally, converted to mph for output
- Angles: radians in computation, degrees in display/output
- Variable suffixes encode units: `_mm`, `_deg`, `_mph`, `_s`
- Camera model: standard CV convention where `t = -(R * camera_position)`

### Key Design Decisions

- **Converging stereo** (toe-in) rather than parallel baseline — convergence point computed from FOV and hitting zone size
- **Self-consistent renderer** — `blender/render_shot.py` builds its two cameras directly from the same `StereoRig::overhead()` parameters used by triangulation, so geometry matches by construction
- **True pinhole, no calibration** — Blender's cameras have zero lens distortion, so there is no calibration handshake; intrinsics are known exactly
- **Constant-velocity trajectory** — the renderer applies no gravity, drag, or Magnus; the ball travels in a straight line so the velocity fit is exact ground truth
- **Temporal minimum background** — background model built from min pixel values across all frames in the shot
- **Spin estimation evolved** — the original 3-stage exhaustive search (still in `spin_detector.rs`, default in lm-test/lm-rig) proved noise-fragile; superseded by `global` multi-frame registration (classical, accurate to ~1800 fps) and an ML estimator (cheap 500–800 fps). See `DESIGN.md` for the full kept/rejected log.
- **Outlier rejection** — velocity fit uses residual-based filtering (mean + 2σ threshold) when ≥4 frames available
