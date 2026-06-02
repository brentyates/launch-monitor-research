# Design & Goals

## The actual goal

**Determine the cheapest hardware that can build a working DIY overhead golf launch monitor — without buying any hardware to find out.**

Everything in this repo is in service of that one question. The CV pipeline, the Blender renderer, the accuracy metrics — they exist so we can explore the hardware/placement design space *in simulation*, measure what each configuration can actually recover, and find the **minimum viable spec**: the least expensive cameras, lenses, lighting, and mounting that still hit an acceptable accuracy budget.

This is a feasibility-and-optimization study, not a product. "Make the CV pipeline work" is the *means*. "What should I buy, and where do I put it?" is the *end*.

10,000 fps and a $5k camera would obviously work. The interesting question is how far down the cost curve we can go before accuracy falls off a cliff — and what configuration choices buy back the most accuracy per dollar.

## What we're optimizing

Minimize total hardware cost subject to meeting an accuracy budget across a realistic shot envelope.

### Accuracy budget (starting targets — adjust to taste)

Two tiers. The sweep reports which configs clear which tier.

| Metric | "Useful" (practice feedback) | "Good" (near-commercial) |
|--------|------------------------------|--------------------------|
| Ball speed | ±2% | ±1% |
| Launch angle (VLA) | ±1.0° | ±0.5° |
| Horizontal angle (HLA) | ±1.0° | ±0.5° |
| Spin rate | ±10% | ±5% |
| Spin axis | ±3° | ±1.5° |

(Commercial units claim ~±1 mph, ±0.3°, ±100–200 rpm. The current `main.rs` pass gate is 2% / 2° / 2° and ignores spin — that's looser than even the "Useful" tier and should be tightened once the sweep exists.)

### Shot envelope (what the rig must handle)

| | Speed (mph) | VLA (°) | HLA (°) | Spin (rpm) |
|---|---|---|---|---|
| Driver | 150–180 | 8–15 | −5…+5 | 2000–3500 |
| Mid iron | 110–135 | 14–20 | −5…+5 | 5000–8000 |
| Wedge | 70–95 | 24–32 | −3…+3 | 8000–11000 |

A spec only "passes" if it meets the budget across this whole range (the fast/low driver and the slow/high wedge stress different things), ideally with several randomized shots per config rather than one nominal shot each.

## The design space (what we sweep)

**Camera / sensor** — the dominant cost driver:
- **Frame rate** — {120, 240, 480, 960} fps. More frames in the field of view = better velocity/spin fits, but fps is the single biggest cost and availability constraint.
- **Resolution** — {1456×1088, 728×544, 512×384, 320×240}. Higher res → larger ball in pixels → better detection/triangulation precision, but lowers max fps and raises cost/bandwidth.
- **Strobe duration** — the ball is always frozen by a short IR strobe; the only open question is *how short* the pulse must be. Minor for positioning, but critical for the zoomed spin camera, where the zoom magnifies any residual blur and smears the markings.
- **Bit depth** — 8 vs 10/12. Affects faint-feature contrast (chevrons for spin).

**Global shutter and short-exposure (strobe-frozen) capture are fixed assumptions, not variables.** Rolling shutter skews a ~74 m/s ball and long exposure smears it, so any viable build uses a global-shutter sensor with a mandatory short IR strobe; the study only considers that regime. Motion blur is still modeled, but only to verify a chosen strobe duration adequately freezes the ball (especially for the spin camera) — never to evaluate long exposure as a cost-saving option.

**Optics:**
- **Focal length** — {4, 6, 8, 12} mm. Trades field-of-view coverage against ball pixel size.

**Geometry / placement:**
- **Mount height** — 2.0–3.5 m
- **Forward offset** — 0.5–1.5 m (downrange of the tee)
- **Stereo baseline** — 200–500 mm
- **Convergence angle** — derived from FOV + hitting zone, or swept directly

**Lighting / exposure:**
- Ambient vs **IR strobe**; exposure time (sets motion blur). Cheap IR LEDs are how you freeze a fast ball without an expensive global-shutter-at-high-fps sensor.

**Fixed:** golf ball Ø 42.67 mm; hitting zone ~150 mm; two cameras; overhead-converging geometry.

## The physics that drives the trade-offs

These relationships are *why* the sweep matters — every config is a balance of them:

- **Frames captured** ≈ `FOV_width · fps / ball_speed`. A 165 mph ball travels ~308 mm per frame at 240 fps. Higher mount / wider lens = more frames but a smaller ball; higher fps = more frames at fixed cost in $$.
- **Ball size in pixels** ≈ `f_px · D_ball / distance`. Detection and triangulation precision scale with this. Longer focal, higher res, or closer mount help — but shrink the FOV and frame count.
- **Triangulation depth precision** ∝ `distance² / (baseline · f_px)`. Wider baseline, longer focal, closer mount = better depth.
- **Motion-blur streak** ≈ `ball_speed · exposure · f_px / distance`. Must stay well under the ball size to "freeze" — which forces short exposures and therefore more light (IR strobe) at high speed.

The optimum is a saddle: push any single knob and another degrades. Finding it empirically across the shot envelope is the whole point.

## Cost model (rough, refine with real listings)

The output we want is an **accuracy-vs-dollars Pareto frontier**, so configs need a cost estimate. Rough order-of-magnitude per camera (×2 for stereo):

| Capability | ~Cost each |
|---|---|
| 120 fps, 1 MP, global shutter | $50–100 |
| 240–480 fps, ~0.5–1 MP | $150–400 |
| 960 fps+ | $500–1500 |
| Multi-kfps | thousands (out of scope) |

Plus lens ($20–80), IR strobe LEDs + driver ($20–60), mounts, and a compute box (Raspberry Pi / mini-PC). These are placeholders — replace with real part numbers as candidates emerge.

## Methodology

1. **Config as data** — one config file per candidate rig (camera + optics + geometry + lighting), read by *both* the Blender renderer and the Rust solver (single source of truth; today the params are hardcoded in two places).
2. **Sweep** — render the shot envelope for each config, run the solver, record errors vs ground truth.
3. **Aggregate** — per config: does it clear "Useful" / "Good" across the envelope, and what's the worst-case error?
4. **Frontier** — cross accuracy against the cost model → the cheapest config(s) that pass → the recommended build.

Implemented by `lm-sweep` (`configs/sweep.json` → `results/sweep.csv` + a printed cost frontier). Two caveats on current results:

- **Resolution is modeled as a sensor crop at fixed pixel pitch** (fewer pixels = narrower FOV), so lower-res configs catch *fewer frames* of a fast ball. A real cheap camera might instead trade pixel pitch or lens, so treat the resolution axis as one interpretation, not gospel.
- **Accuracy is still the geometric ceiling** (clean pinhole, Phase 3 not done): every config that captures ≥2 frames currently passes "Good" easily, so the binding constraint right now is *frame count* (FOV × fps vs ball speed), not pixel precision. The frontier only becomes a real accuracy-vs-cost trade once sensor realism is added.

## Simulation fidelity — what's modeled, what isn't

Honest accounting, because it bounds what the results mean.

**Modeled today:** geometry, optics as an ideal pinhole, resolution, frame rate, basic lighting, constant-velocity trajectory, exact ground truth.

**Not yet modeled (Phase 3):** sensor noise, motion blur from finite exposure, bit-depth quantization, lens distortion, compression artifacts. (Rolling shutter is intentionally out of scope — global shutter only.)

This matters: a clean pinhole render gives the **geometric ceiling** for a configuration. It cannot yet distinguish a $60 noisy rolling-shutter camera from a $2000 global-shutter one at the same fps/resolution. Modeling the sensor degradations is what turns "this geometry could work" into "this *hardware* will work," and is required before any cost claim is trustworthy.

## Roadmap

1. **Document the goal** (this file). ✅
2. **Config-driven sweep** — make rig params data; build the sweep + accuracy aggregation. Answers the geometric trade-offs.
3. **Sensor realism** — motion blur (finite exposure), sensor noise, bit depth — so "cheap vs expensive" actually diverges and the cost frontier becomes real. (Global shutter only; rolling shutter is out of scope.)
4. **Machine learning** (later — not yet, but a major intended direction).

## Key findings so far

- **Positioning is easy and cheap.** Sub-1% speed/angle across the envelope at modest hardware. At the geometric ceiling the binding constraint is *frame count* (FOV × fps vs ball speed), not pixel precision.
- **The strobe matters more than the camera price.** The same cheap camera is GOOD with a short strobe and fails badly with a long exposure (motion blur) — spend on lighting/strobe first. (Long exposure is ruled out as a technique regardless.)
- **A single zoomed overhead camera can read spin in the mid-envelope** (7-iron: 0.6% rate, chevrons legible at 53 px), so the 2-stereo + 1-spin hybrid is viable in principle.
- **But the current spin detector is pathologically noise-sensitive.** With identical geometry/shot/detector, changing only the Cycles sample count 24→32 (sub-pixel render noise, invisible to the eye) swung spin-rate error between 59% and 0.6%. So current spin accuracy is *noise-dominated, not condition-dominated*; a "perfect" reading is fragile. Real sensor noise dwarfs that perturbation, so **a robust/learned spin estimator is the critical path** — hardware (fps for aliasing, zoom for pixels) is necessary but not sufficient.
- **Statistical probe settled it (5 noise seeds × fps 480/960/1440 × rpm 3000/7000, focal 24, ~70 px):** *no* condition is robustly accurate, and adding fps/frames does **not** help — at 1440 fps / 6 frames / 12.5° per frame (trivially solvable conditions) the detector is systematically ~327% wrong. So spin is **algorithm-bounded, not hardware-bounded**: the geometric exhaustive-search detector must be replaced (better classical pattern-tracking, then learned), not tuned with more camera. Aliasing remains a separate hard hardware limit for genuinely high spin.

## Spin algorithm attempts (scientific log)

Rule: if an approach can't meaningfully improve the result against a pre-registered bar, throw it out *as the solution* and record the learning.

- **Geometric exhaustive search (original `SpinDetector`)** — REJECTED. Pathologically noise-sensitive (sub-pixel noise swings rate error 0.6%↔59%↔327%); more fps/frames doesn't help. Brittle global hypothesis search over noisy unordered features with a multimodal, symmetry-aliased score.
- **Inter-frame rotation (Kabsch on back-projected chevron points, `spin_tracker.rs`)** — attempt 1 REJECTED against the bar. It *fixed the instability* (std collapsed 15–138% → ~3–5%) but is **systematically biased** (55–93% rate error). Learning: the temporal/geometric family is noise-robust, but sparse chevron-centroid nearest-neighbor correspondence systematically mismeasures the rotation. (`SPIN_METHOD=interframe`.)
- **Dense correspondence-free registration (`SPIN_METHOD=dense`)** — ACCEPTED in-regime. Per consecutive pair, search the 3D rotation that maximizes NCC of the high-passed ball appearance back-projected onto the sphere (high-pass removes the fixed overhead shading so texture drives the match). Result: **robust 1–3% rate error across 3000–11000 rpm when rotation-per-frame is in the ~7–12° sweet spot** (validated, 5 noise seeds/cell). Degrades **above ~17°/frame** (ambiguity/aliasing) and, notably, **below ~5°/frame** (rotation too small to resolve vs noise). So spin is solvable; "perfect" = rotation-per-frame ≈ 7–12°.
  - **Spec implication:** optimal fps scales with spin (`fps ≈ 0.5–0.85 × rpm`).
  - **Multi-frame baseline (implemented):** scans frame spacings and picks the one landing rotation in the 7–12° window. Extends usable accuracy to low rotation-per-frame (3–5°/frame: 24–50% → ~6.7%, no regressions).
- **Global multi-frame fit (`SPIN_METHOD=global`)** — ACCEPTED, current best, and the result that closed the software→hardware loop. Search one shared (axis, per-frame rotation) maximizing summed dense NCC across ALL consecutive pairs at once; the constant-spin constraint over-constrains the fit and disambiguates *large* per-frame rotations (a wrong rotation can fake-match one pair but not all). Works up to **~37°/frame**. **At 1800 fps it solves 7000 rpm (2.9%) and 11000 rpm (3.1%)**, 3000 rpm ~7% (useful); above ~40°/frame (1500 fps @ 11000 rpm = 44°) it still degrades.
  - **Loop closed:** this relaxed the spin-camera fps requirement from ~4–5 kfps (per-pair dense, infeasible) to **~1800 fps — inside the 1300–1800 fps commercial overhead range, i.e. buildable.** The hardware demand exceeded budget → a better algorithm brought it back in reach. Remaining: 3000 rpm at ~7% (low-spin residual) and >40°/frame (needs either fps or more algorithm). lm-spin-sweep uses it via `SPIN_METHOD=global`; lm-rig still uses the old search.

## Machine learning opportunities (future)

The renderer is, in effect, a **perfect-label synthetic-data factory**: every rendered frame comes with exact ground truth (3D ball position/velocity, spin rate/axis, launch params, camera pose). That makes this project unusually well-suited to ML, and ML is a strong lever on the core goal — *learned models can make cheaper, noisier hardware viable*, pushing the cost frontier down further than classical CV alone. Candidate problems, roughly easiest → most ambitious:

- **Learned ball detection / sub-pixel centroid** — a small CNN detector to replace peak+centroid, robust to noise, motion blur, low contrast, and partial clipping (where the classical detector is weakest).
- **Spin estimation** — the current exhaustive chevron search is brittle and slow (273K hypotheses, 8–154% error). A learned model regressing spin rate + axis from the frame sequence is a natural, high-value replacement.
- **Denoise / deblur / rolling-shutter correction** — learned preprocessing that recovers usable frames from cheap sensors, directly expanding the set of viable hardware.
- **Super-resolution** — recover ball detail from low-resolution cheap cameras.
- **End-to-end regression** — map raw stereo frame sequences directly to launch parameters, skipping explicit triangulation, as an accuracy ceiling reference.
- **Config surrogate / optimizer** — a model that predicts accuracy from a hardware config, so the design-space search becomes Bayesian optimization over a fast surrogate instead of brute-force rendering every point.

The throughline: domain-randomized synthetic training data (varying lighting, noise, textures, geometry) with sim-to-real transfer as the eventual bridge to physical hardware. Sensor realism (Phase 3) is a prerequisite for any of this to transfer.
