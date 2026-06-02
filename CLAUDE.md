# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release --bin lm-test    # Build release binary
cargo check                            # Type-check without building
cargo test                             # Run unit tests

./scripts/render.sh                    # Render all three cases with Blender into renders/<case>
./scripts/test-e2e.sh                  # Full E2E: builds Rust, renders, runs all test cases (PASS/FAIL)
./scripts/run.sh                       # Build, render, and run for quick iteration
./scripts/view.sh <case> [--debug]     # Stereo contact sheet + MP4 into viz/<case>/ (--debug = overlays)
./scripts/cleanup.sh                   # Remove renders/ and debug frames

RUST_LOG=debug cargo run               # Run with debug logging
```

The binary is `lm-test`. The only frame source is a Blender-rendered dataset on disk. `lm-test` renders any missing case automatically; set `RENDER=1` to force re-render. Override the Blender binary with `BLENDER=...` (default `/Applications/Blender.app/Contents/MacOS/Blender`).

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
| `main.rs` | Test harness — renders missing cases via Blender, loads each dataset, runs 3 test cases (driver/7-iron/wedge), compares against ground truth |

The Unity simulator has been removed; frames now come entirely from the offline Blender renderer.

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
- **3-stage spin search** — coarse (25 RPM steps) → medium (5 RPM) → fine (1 RPM), each narrowing the search window
- **Outlier rejection** — velocity fit uses residual-based filtering (mean + 2σ threshold) when ≥4 frames available
