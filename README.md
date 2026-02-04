# Launch Monitor Research

Research into building a golf launch monitor using computer vision — specifically, developing and validating the CV algorithms needed to extract full launch data (ball speed, launch angles, spin rate, spin axis, and eventually club data) from high-speed stereo camera footage.

## Why a simulator?

The core challenge with launch monitor development is the feedback loop. Real hardware setups are expensive, time-consuming to reconfigure, and produce data with no ground truth to validate against. You can't tell if your triangulation is wrong or your camera placement is bad without already knowing the answer.

This project takes a different approach: build a physically accurate simulator first, then develop the CV pipeline against it. The simulator provides:

- **Known ground truth** for every launch parameter, so algorithm accuracy is measurable
- **Rapid iteration** on camera configurations (overhead, side-mounted, rear) without buying or moving hardware
- **Realistic sensor artifacts** (noise, lens distortion, IR filtering) so algorithms transfer to real cameras

## How it works

The Unity simulator renders a golf ball launch from a configurable stereo camera rig with physically-based sensor simulation. Frames are passed to a Rust CV pipeline via shared memory, where stereo triangulation reconstructs the ball's 3D trajectory and pattern tracking estimates spin.

The simulator runs at 4kHz physics with configurable camera frame rates, resolutions, baselines, and positions. Sensor effects (shot noise, read noise, Brown-Conrady lens distortion, IR filtering) are applied in a unified GPU shader pass to match what real camera hardware produces.

## Project structure

- `unity/LaunchSimulator/` — Unity 6 stereo camera simulator
- `blender/` — Ball model and texture generation scripts
- `assets/` — Texture generation utilities

The Rust CV pipeline and additional tooling will be published separately as they mature.

## Requirements

- Unity 6 (6000.3.x)
- Rust (for the CV pipeline, when published)
