# Launch Monitor Research

**Goal: find the cheapest hardware that can build a working DIY overhead golf launch monitor — without buying any hardware to find out.** The optimal trade-off across mount placement, stereo geometry, frame rate, resolution, lens, and lighting, for the lowest cost that still hits an acceptable accuracy budget. See [DESIGN.md](DESIGN.md) for the objective, the design space, the accuracy budget, and the methodology.

The CV pipeline and the Blender renderer in this repo are the *means*: they let us explore that hardware design space in simulation, with exact ground truth, instead of buying and re-mounting cameras to find out what works.

## Why render synthetic shots?

The core challenge with launch monitor development is the feedback loop. Real hardware setups are expensive, time-consuming to reconfigure, and produce data with no ground truth to validate against. You can't tell if your triangulation is wrong or your camera placement is bad without already knowing the answer — and you can't compare a dozen candidate hardware configs without buying a dozen cameras.

This project develops the CV pipeline against rendered shots with exact ground truth, so any rig configuration can be measured before a dollar is spent. The renderer provides:

- **Known ground truth** for every launch parameter, so algorithm accuracy is measurable
- **Rapid iteration** on camera configurations without buying or moving hardware
- **Exact, distortion-free geometry** — a true pinhole camera with known intrinsics, so triangulation can be validated against an analytic answer

## How it works

A Blender Cycles script (`blender/render_shot.py`) renders a golf ball launch from a stereo camera rig and writes stereo PNG frame pairs plus a `manifest.json` per shot into `renders/<case>/`. A Rust CV pipeline loads those frames, triangulates the ball's 3D trajectory, and tracks the TP5 Pix chevron pattern to estimate spin.

The two render cameras are built directly from the same `StereoRig::overhead()` parameters the pipeline triangulates with, so the simulated geometry is self-consistent with the solver. The trajectory is constant-velocity (no gravity, drag, or Magnus), giving an exact velocity ground truth.

## Project structure

- `blender/` — `render_shot.py` renderer plus the ball model and texture generation scripts
- `src/` — Rust CV pipeline (`lm-test` binary)
- `scripts/` — render / run / e2e / cleanup helpers
- `assets/` — texture generation utilities

## Requirements

- Blender (path defaults to `/Applications/Blender.app/Contents/MacOS/Blender`, override with `BLENDER=...`)
- Rust

## Usage

```bash
cargo build --release --bin lm-test
./scripts/render.sh      # render the three test shots
./scripts/test-e2e.sh    # build, render, run, report PASS/FAIL
./scripts/view.sh driver # stereo contact sheet + playback MP4 into viz/<case>/
```

Add `--debug` to `view.sh` to visualize the annotated detection overlays (projected vs detected ball) instead of the raw renders.
