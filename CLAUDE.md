# Launch Monitor Research

Golf ball launch monitor research project with computer vision algorithms for tracking and spin detection. Supports multiple camera configurations (overhead, side-mounted, rear) with stereo triangulation.

Rust crate: `launch_monitor_research`

## Project Structure

```
src/
  lib.rs              # Main library exports
  main.rs             # WebSocket server binary (lm-server)
  server.rs           # Frame receiver + CV pipeline (WebSocket mode)
  shared_memory.rs    # Frame receiver + CV pipeline (Unity mode)
  ball_detector.rs    # Ball detection in frames
  triangulation.rs    # Stereo triangulation for VLA/HLA/speed
  spin_detector.rs    # Spin detection via pattern matching
  config.rs           # Camera rig configuration
  debug.rs            # Debug visualization overlays

simulator/
  index.html          # Three.js stereo flight simulator
  tp5_pix_ball.glb    # TP5 Pix ball model

unity/LaunchSimulator/ # Unity 6 stereo simulator (realistic artifacts)
  Assets/Scripts/
    Core/             # SimulationController, TimeController, GolfBall, CalibrationBoard
    Camera/           # StereoRig, SensorCapture, CameraSensorFeature, IRSimulation
    Transport/        # SharedMemoryWriter (IPC to Rust)
    UI/               # SimulatorUI, StereoDisplayView
  Assets/Shaders/
    CameraSensor.shader    # Unified sensor simulation (noise, distortion, IR)
```

## Commands

```bash
# Build and run WebSocket server (Three.js mode)
cargo run

# Run with Unity shared memory mode
cargo run -- --unity

# Check compilation
cargo check

# Run tests
cargo test

# Open Three.js simulator in browser
open simulator/index.html

# Open Unity project (requires Unity 6)
# Unity Hub -> Add -> unity-simulator/
```

## Architecture

### Rust CV Pipeline

The WebSocket server (`lm-server`) receives stereo frames from the browser simulator:

1. **Frame Reception** - Binary WebSocket messages with left/right RGBA frames
2. **Ball Detection** - Find ball centroid in each camera view
3. **Stereo Triangulation** - Compute 3D position from matched detections
4. **Launch Estimation** - Fit velocity vector to 3D positions over time
5. **Spin Detection** - Track TP5 Pix chevron pattern rotation

### Three.js Simulator

Browser-based test environment with two modes:

- **Stereo Simulation** - Dual camera views, flight physics, frame streaming
- **Ball Inspector** - OrbitControls for close-up ball model inspection

Configurable parameters:
- Launch: speed, VLA, HLA, spin RPM, spin axis
- Camera: FPS, FOV, resolution, baseline, height, forward offset
- Ball: starting orientation (pitch/yaw/roll)

### Unity 6 Simulator

Higher fidelity simulator with realistic camera artifacts:

- **Physics**: 4kHz fixed timestep (vs per-frame in Three.js)
- **Capture**: AsyncGPUReadback for non-blocking frame capture
- **IPC**: Shared memory ring buffer (faster than WebSocket)
- **Rendering**: URP with proper lighting and materials

Run with `cargo run -- --unity` to use shared memory mode.

### Camera Configurations

Default is overhead stereo rig, but camera positions are configurable:

```javascript
// In simulator/index.html
CAM_HEIGHT = 3048;    // mm above ground
CAM_FORWARD = 1092;   // mm forward of hitting zone
BASELINE = 350;       // stereo separation
```

Modify these to simulate side-mounted or rear-mounted configurations.

## WebSocket Protocol (Three.js)

1. Browser sends JSON metadata: `{type: 'meta', width, height, frameCount, fps, groundTruth}`
2. Browser sends binary frames: `[frameIndex:u32][left RGBA][right RGBA]`
3. Browser sends JSON end: `{type: 'end'}`
4. Server responds with JSON: `{frame_count, ball_detections, launch, spin_result, errors}`

## Shared Memory Protocol (Unity)

Ring buffer with 3 frame slots at `/dev/shm/LaunchMonitorSharedMemory`:

```
[SharedHeader: ~80 bytes]
  magic: 0x474F4C46 ("GOLF")
  state: 0=idle, 1=ready, 2=streaming, 3=complete
  writeHead, frameCount, width, height, fps
  groundTruth: speedMph, vlaDeg, hlaDeg, spinRpm, spinAxisDeg

[Frame 0..2]
  [FrameHeader: frameIndex, timestamp, ballPosition, ballVelocity]
  [Left RGBA pixels]
  [Right RGBA pixels]
```

Unity writes frames, Rust polls `writeHead` and processes when `state=complete`.

## Camera Simulation

All sensor effects are handled by a unified `CameraSensorFeature` URP renderer feature.

**Setup**: Add `CameraSensorFeature` to your URP Renderer asset.

### Sensor Noise (Physically-Based)

Realistic noise model with two components applied identically in IR and color modes:

- **Shot noise**: Signal-dependent (σ ∝ √brightness), simulates photon counting statistics
- **Read noise**: Constant floor from sensor electronics

Parameters in `StereoConfig`:
- `noiseEnabled`: Toggle noise on/off
- `shotNoiseScale`: Shot noise intensity (0.02-0.1 typical)
- `readNoiseScale`: Read noise intensity (0.01-0.05 typical)

### Lens Distortion (Brown-Conrady Model)

Radial distortion matching standard CV calibration:
```
x' = x(1 + k1·r² + k2·r⁴)
y' = y(1 + k1·r² + k2·r⁴)
```

Parameters in `StereoConfig`:
- `distortionK1`: First radial coefficient (-0.5 to 0.1, typical: -0.15 for barrel)
- `distortionK2`: Second radial coefficient (-0.1 to 0.2, typical: 0.02)
- `distortionEnabled`: Toggle distortion on/off

### Calibration Board

Standard checkerboard pattern for camera calibration:
- Default: 9×6 inner corners (10×7 squares)
- Square size: 25mm (fits on A4/Letter paper: 250mm × 175mm)
- Add `CalibrationBoard` component to a GameObject in scene

The same calibration code in Rust works for both simulated and real cameras.

### IR Camera Simulation

Toggle `irFilterEnabled` for monochrome view with:
- Grayscale conversion with configurable contrast (`irContrast`)
- Same sensor noise as color mode (unified pipeline)
- LED strobe lighting (configurable power and beam angle)

## Key Algorithms

### Stereo Triangulation
Uses converging (toe-in) cameras with OpenCV-style projection matrices. Ball centroid detected in both views, triangulated to 3D position.

### Spin Detection
Pattern-aware rotation estimation using known TP5 Pix chevron positions. Exhaustive search over RPM (0-15000), axis angles (±45°), and initial orientations.

### Ball Detection
Threshold + contour detection on grayscale frames. Works with small balls (tested down to 4px diameter).

## Ground Truth Validation

Simulator provides exact launch parameters as ground truth. Server response includes error comparison:
- Speed error: < 2% is good
- VLA/HLA error: < 1° is good
- Spin: depends on ball size and FPS

## Dependencies

- `nalgebra` - Linear algebra
- `image` / `imageproc` - Image processing
- `tokio` / `axum` - Async HTTP/WebSocket server
- `memmap2` - Shared memory (Unity mode)
- `serde` / `serde_json` - Serialization
- `tracing` - Logging
