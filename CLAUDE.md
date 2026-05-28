# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build --release --bin lm-test    # Build release binary
cargo check                            # Type-check without building
cargo test                             # Run unit tests

./scripts/test-e2e.sh                  # Full E2E: builds Rust, launches Unity, runs all test cases
./scripts/run.sh                       # Launch Unity + Rust together (requires pre-built binary)
./scripts/build-unity.sh               # Build Unity project from CLI (requires Unity 6000.4.3f1)
./scripts/cleanup.sh                   # Kill stale processes, remove shared memory file

RUST_LOG=debug cargo run               # Run with debug logging
```

The binary is `lm-test`. There is no separate `lm-server` or WebSocket mode in the current code — the only frame source is shared memory from Unity.

## Architecture

Stereo CV pipeline that receives frames from a Unity 6 simulator via shared memory, detects a golf ball in each stereo view, triangulates 3D positions, estimates launch parameters, and detects spin.

### Data Flow

```
Unity Simulator (4kHz physics, configurable render FPS)
  → Shared memory ring buffer ({project_dir}/LaunchMonitorSharedMemory, 12 slots)
    → SharedMemorySource reads frames, flips Y, converts RGBA→Gray
      → BallDetector: background subtraction → peak finding → weighted centroid
        → StereoTriangulator: DLT-SVD triangulation → least-squares velocity fit
          → SpinDetector: TP5 Pix chevron pattern matching (coarse→medium→fine search)
            → ProcessingResult { launch, spin, ground_truth, errors }
```

### Module Responsibilities

| Module | Purpose |
|--------|---------|
| `frame_source/shared_memory.rs` | Mmap-based reader for Unity's ring buffer. Handles volatile reads, Y-flip, RGBA→Gray conversion |
| `ball_detector.rs` | Temporal-minimum background model, peak-based detection with weighted centroid |
| `triangulation.rs` | OpenCV-style projection matrices (P = K[R|t]), DLT-SVD triangulation, velocity fitting with outlier rejection (mean ± 2σ) |
| `spin_detector.rs` | 3-stage exhaustive search (273K hypotheses at coarse, parallelized with Rayon) over RPM/axis/orientation. Scores by projecting TP5 Pix chevron geometry onto detected features |
| `pipeline.rs` | Orchestrator — runs detection, filters consecutive pairs (max 3-frame gap), filters by median radius, triangulates, estimates spin |
| `config.rs` | `StereoRig::overhead()` — computes intrinsics and converging (toe-in) camera extrinsics from physical parameters |
| `debug.rs` | Overlay rendering (circles, lines, text) onto gray frames, saves annotated PNGs to `debug_frames/` |
| `main.rs` | Test harness — connects to shared memory, runs 3 test cases (driver/7-iron/wedge), compares against ground truth |

### Shared Memory Protocol

Header (104 bytes) followed by 12 frame slots. Unity writes, Rust polls `write_head`. States: Idle(0) → Ready(1) → Streaming(2) → Complete(3). Rust sends commands back via `rust_command` field in header (1=launch with params, 2=reset).

### Coordinate/Unit Conventions

- Positions: millimeters. Velocities: mm/s internally, converted to mph for output
- Angles: radians in computation, degrees in display/output
- Variable suffixes encode units: `_mm`, `_deg`, `_mph`, `_s`
- Camera model: standard CV convention where `t = -(R * camera_position)`

### Key Design Decisions

- **Converging stereo** (toe-in) rather than parallel baseline — convergence point computed from FOV and hitting zone size
- **Temporal minimum background** — no separate calibration; background model built from min pixel values across all frames in the shot
- **12-slot ring buffer** — fixed memory, prevents unbounded growth
- **3-stage spin search** — coarse (25 RPM steps) → medium (5 RPM) → fine (1 RPM), each narrowing the search window
- **Outlier rejection** — velocity fit uses residual-based filtering (mean + 2σ threshold) when ≥4 frames available
