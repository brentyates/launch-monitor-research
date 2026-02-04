using UnityEngine;
using UnityEngine.InputSystem;
using System;
using LaunchMonitor.Core;
using LaunchMonitor.Camera;

namespace LaunchMonitor.UI
{
    public class SimulatorUI : MonoBehaviour
    {
        public static SimulatorUI Instance { get; private set; }

        private SimulationController sim;
        private StereoRig stereoRig;
        private TimeController timeController;
        private SimulationFlowController flowController;

        private bool showUI = true;
        private bool collapsed;
        private Vector2 scrollPosition;
        private Rect windowRect;
        private Rect expandedRect;
        private bool expandedRectValid;

        public bool IsPointerOverUI()
        {
            if (!showUI) return false;
            var mouse = Mouse.current;
            if (mouse == null) return false;
            Vector2 pos = mouse.position.ReadValue();
            Vector2 guiPos = new Vector2(pos.x, Screen.height - pos.y);
            return windowRect.Contains(guiPos);
        }

        private GUIStyle headerStyle;
        private GUIStyle boxStyle;
        private bool stylesInitialized;

        private int selectedPresetIndex = 2;

        void Awake()
        {
            Instance = this;
        }

        void Start()
        {
            sim = SimulationController.Instance;
            stereoRig = StereoRig.Instance;
            timeController = TimeController.Instance;
            flowController = FindFirstObjectByType<SimulationFlowController>();
        }

        void Update()
        {
            var keyboard = Keyboard.current;
            if (keyboard != null && keyboard.tabKey.wasPressedThisFrame)
            {
                if (collapsed)
                    collapsed = false;
                else
                    showUI = !showUI;
            }
        }

        void OnGUI()
        {
            if (!showUI) return;

            InitStyles();

            DrawCameraLabels();

            if (collapsed)
                DrawCollapsedUI();
            else
                DrawExpandedUI();
        }

        void DrawCollapsedUI()
        {
            float btnSize = 28;
            windowRect = new Rect(Screen.width - btnSize - 8, 8, btnSize, btnSize);

            if (GUI.Button(windowRect, "+"))
                collapsed = false;
        }

        void DrawExpandedUI()
        {
            if (!expandedRectValid)
                expandedRect = new Rect(Screen.width - 330, 10, 320, 600);

            expandedRect.x = Screen.width - 330;
            expandedRect = GUILayout.Window(0, expandedRect, DrawWindow, "");
            expandedRectValid = true;
            windowRect = expandedRect;
        }

        void DrawCameraLabels()
        {
            GUI.Label(new Rect(Screen.width / 4 - 70, 10, 140, 25), "Target-Right Cam", headerStyle);
            GUI.Label(new Rect(Screen.width * 3 / 4 - 70, 10, 140, 25), "Target-Left Cam", headerStyle);

            float centerX = Screen.width / 2;
            GUI.DrawTexture(new Rect(centerX - 1, 0, 2, Screen.height), Texture2D.whiteTexture);
        }

        void InitStyles()
        {
            if (stylesInitialized) return;

            headerStyle = new GUIStyle(GUI.skin.label)
            {
                fontStyle = FontStyle.Bold,
                fontSize = 12
            };

            boxStyle = new GUIStyle(GUI.skin.box)
            {
                padding = new RectOffset(8, 8, 8, 8)
            };

            stylesInitialized = true;
        }

        void DrawWindow(int id)
        {
            GUILayout.BeginHorizontal();
            GUILayout.Label("Launch Monitor", headerStyle);
            GUILayout.FlexibleSpace();
            if (GUILayout.Button("−", GUILayout.Width(24), GUILayout.Height(18)))
                collapsed = true;
            GUILayout.EndHorizontal();

            GUILayout.Space(4);

            scrollPosition = GUILayout.BeginScrollView(scrollPosition);

            DrawSimulationControls();
            GUILayout.Space(10);
            DrawLaunchParameters();
            GUILayout.Space(10);
            DrawBallOrientation();
            GUILayout.Space(10);
            DrawHitPosition();
            GUILayout.Space(10);
            DrawCameraSettings();
            GUILayout.Space(10);
            DrawStats();
            GUILayout.Space(10);
            DrawBallPosition();

            GUILayout.EndScrollView();

            GUI.DragWindow(new Rect(0, 0, 10000, 30));
        }

        void DrawSimulationControls()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Simulation", headerStyle);

            GUILayout.BeginHorizontal();
            if (GUILayout.Button("Launch (Space)"))
                sim?.Launch();
            if (GUILayout.Button("Reset (R)"))
                sim?.ResetSimulation();
            GUILayout.EndHorizontal();

            GUILayout.BeginHorizontal();
            string pauseText = sim?.State == SimulationState.Armed ? "Resume (P)" : "Pause (P)";
            if (GUILayout.Button(pauseText))
            {
                if (sim?.State == SimulationState.Flight)
                    sim.Pause();
                else if (sim?.State == SimulationState.Armed)
                    sim.Resume();
            }
            if (GUILayout.Button("Process (Enter)"))
                flowController?.ProcessFrames();
            GUILayout.EndHorizontal();

            bool slowMo = timeController?.SlowMoEnabled ?? false;
            bool newSlowMo = GUILayout.Toggle(slowMo, "Slow-Mo (Shift+S)");
            if (newSlowMo != slowMo)
                timeController?.SetSlowMo(newSlowMo);

            if (sim != null)
            {
                sim.ShowTrail = GUILayout.Toggle(sim.ShowTrail, "Show Trail");
            }

            string stateText = sim != null ? $"State: {sim.State}" : "State: N/A";
            GUILayout.Label(stateText);

            if (sim != null && sim.FrameHistory.Count > 0 &&
                (sim.State == SimulationState.Complete || sim.State == SimulationState.Armed))
            {
                int maxFrame = sim.FrameHistory.Count - 1;
                GUILayout.Label($"Frame: {sim.CurrentFrameIndex}");
                int newValue = Mathf.RoundToInt(GUILayout.HorizontalSlider(sim.CurrentFrameIndex, 0, maxFrame));
                if (newValue != sim.CurrentFrameIndex)
                    sim.SeekToFrame(newValue);
            }

            GUILayout.EndVertical();
        }

        void DrawLaunchParameters()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Launch Parameters", headerStyle);

            if (sim != null)
            {
                GUILayout.Label($"Speed: {sim.launchParams.speedMph:F0} mph");
                sim.launchParams.speedMph = GUILayout.HorizontalSlider(sim.launchParams.speedMph, 50, 200);

                GUILayout.Label($"VLA: {sim.launchParams.vlaDeg:F1}°");
                sim.launchParams.vlaDeg = GUILayout.HorizontalSlider(sim.launchParams.vlaDeg, 0, 45);

                GUILayout.Label($"HLA: {sim.launchParams.hlaDeg:F1}°");
                sim.launchParams.hlaDeg = GUILayout.HorizontalSlider(sim.launchParams.hlaDeg, -20, 20);

                GUILayout.Label($"Spin: {sim.launchParams.spinRpm:F0} rpm");
                sim.launchParams.spinRpm = GUILayout.HorizontalSlider(sim.launchParams.spinRpm, 0, 15000);

                GUILayout.Label($"Spin Axis: {sim.launchParams.spinAxisDeg:F1}°");
                sim.launchParams.spinAxisDeg = GUILayout.HorizontalSlider(sim.launchParams.spinAxisDeg, -45, 45);

                GUILayout.BeginHorizontal();
                if (GUILayout.Button("Reset"))
                    sim.launchParams = LaunchParameters.Default;
                if (GUILayout.Button("Randomize"))
                    sim.launchParams.Randomize();
                GUILayout.EndHorizontal();
            }

            GUILayout.EndVertical();
        }

        void DrawBallOrientation()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Ball Orientation", headerStyle);

            if (sim != null)
            {
                var prev = sim.startOrientation;

                GUILayout.Label($"Pitch: {sim.startOrientation.pitchDeg:F0}°");
                sim.startOrientation.pitchDeg = GUILayout.HorizontalSlider(sim.startOrientation.pitchDeg, -180, 180);

                GUILayout.Label($"Yaw: {sim.startOrientation.yawDeg:F0}°");
                sim.startOrientation.yawDeg = GUILayout.HorizontalSlider(sim.startOrientation.yawDeg, -180, 180);

                GUILayout.Label($"Roll: {sim.startOrientation.rollDeg:F0}°");
                sim.startOrientation.rollDeg = GUILayout.HorizontalSlider(sim.startOrientation.rollDeg, -180, 180);

                bool changed = prev.pitchDeg != sim.startOrientation.pitchDeg ||
                               prev.yawDeg != sim.startOrientation.yawDeg ||
                               prev.rollDeg != sim.startOrientation.rollDeg;

                GUILayout.BeginHorizontal();
                if (GUILayout.Button("Reset"))
                {
                    sim.startOrientation = new BallOrientation();
                    changed = true;
                }
                if (GUILayout.Button("Randomize"))
                {
                    sim.startOrientation.Randomize();
                    changed = true;
                }
                GUILayout.EndHorizontal();

                if (changed)
                    sim.ApplyStartingPose();
            }

            GUILayout.EndVertical();
        }

        void DrawHitPosition()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Hit Position", headerStyle);

            if (sim != null)
            {
                var prev = sim.hitPosition;

                GUILayout.Label($"X Offset: {sim.hitPosition.xOffsetMm:F0} mm");
                sim.hitPosition.xOffsetMm = GUILayout.HorizontalSlider(sim.hitPosition.xOffsetMm, -75, 75);

                GUILayout.Label($"Y Offset: {sim.hitPosition.yOffsetMm:F0} mm");
                sim.hitPosition.yOffsetMm = GUILayout.HorizontalSlider(sim.hitPosition.yOffsetMm, -75, 75);

                bool changed = prev.xOffsetMm != sim.hitPosition.xOffsetMm ||
                               prev.yOffsetMm != sim.hitPosition.yOffsetMm;

                GUILayout.BeginHorizontal();
                if (GUILayout.Button("Reset"))
                {
                    sim.hitPosition = new HitPosition();
                    changed = true;
                }
                if (GUILayout.Button("Randomize"))
                {
                    sim.hitPosition.Randomize();
                    changed = true;
                }
                GUILayout.EndHorizontal();

                if (changed)
                    sim.ApplyStartingPose();
            }

            GUILayout.EndVertical();
        }

        void DrawCameraSettings()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Camera Settings", headerStyle);

            if (timeController != null && stereoRig != null)
            {
                int maxFps = SensorCropPresets.GetMaxFpsForResolution(
                    stereoRig.config.width, stereoRig.config.height);
                GUILayout.Label($"FPS: {timeController.TargetFps:F0} (max {maxFps})");
                float newFps = GUILayout.HorizontalSlider(timeController.TargetFps, 60, maxFps);
                if (Mathf.Abs(newFps - timeController.TargetFps) > 1)
                    timeController.TargetFps = Mathf.Min(newFps, maxFps);
            }
            else if (timeController != null)
            {
                GUILayout.Label($"FPS: {timeController.TargetFps:F0}");
                float newFps = GUILayout.HorizontalSlider(timeController.TargetFps, 60, 536);
                if (Mathf.Abs(newFps - timeController.TargetFps) > 1)
                    timeController.TargetFps = newFps;
            }

            if (stereoRig != null)
            {
                GUILayout.Label($"Focal Length: {stereoRig.config.focalLengthMm:F1}mm");
                float newFocal = GUILayout.HorizontalSlider(stereoRig.config.focalLengthMm, 2.5f, 35f);
                if (Mathf.Abs(newFocal - stereoRig.config.focalLengthMm) > 0.1f)
                {
                    stereoRig.config.focalLengthMm = newFocal;
                    stereoRig.ApplyConfiguration();
                }

                bool portrait = stereoRig.config.portraitMode;
                bool newPortrait = GUILayout.Toggle(portrait, "Portrait Mode");
                if (newPortrait != portrait)
                {
                    stereoRig.config.portraitMode = newPortrait;
                    stereoRig.ApplyConfiguration();
                }

                bool irFilter = stereoRig.config.irFilterEnabled;
                bool newIrFilter = GUILayout.Toggle(irFilter, "IR Filter (Mono)");
                if (newIrFilter != irFilter)
                {
                    stereoRig.config.irFilterEnabled = newIrFilter;
                    stereoRig.ApplyConfiguration();
                }

                if (stereoRig.config.irFilterEnabled)
                {
                    GUILayout.Label($"LED Power: {stereoRig.config.strobePower:F0}W");
                    float newPower = GUILayout.HorizontalSlider(stereoRig.config.strobePower, 5, 200);
                    if (Mathf.Abs(newPower - stereoRig.config.strobePower) > 1f)
                    {
                        stereoRig.config.strobePower = newPower;
                        stereoRig.UpdateStrobeSettings();
                    }

                    GUILayout.Label($"Beam Angle: {stereoRig.config.strobeBeamAngle:F0}°");
                    float newAngle = GUILayout.HorizontalSlider(stereoRig.config.strobeBeamAngle, 15, 120);
                    if (Mathf.Abs(newAngle - stereoRig.config.strobeBeamAngle) > 1f)
                    {
                        stereoRig.config.strobeBeamAngle = newAngle;
                        stereoRig.UpdateStrobeSettings();
                    }
                }

                bool distortion = stereoRig.config.distortionEnabled;
                bool newDistortion = GUILayout.Toggle(distortion, "Lens Distortion");
                if (newDistortion != distortion)
                {
                    stereoRig.config.distortionEnabled = newDistortion;
                    stereoRig.ApplyConfiguration();
                }

                if (stereoRig.config.distortionEnabled)
                {
                    GUILayout.Label($"K1 (barrel): {stereoRig.config.distortionK1:F3}");
                    float newK1 = GUILayout.HorizontalSlider(stereoRig.config.distortionK1, -0.5f, 0.1f);
                    if (Mathf.Abs(newK1 - stereoRig.config.distortionK1) > 0.001f)
                    {
                        stereoRig.config.distortionK1 = newK1;
                        stereoRig.UpdateSensorSettings();
                    }

                    GUILayout.Label($"K2 (correction): {stereoRig.config.distortionK2:F3}");
                    float newK2 = GUILayout.HorizontalSlider(stereoRig.config.distortionK2, -0.1f, 0.2f);
                    if (Mathf.Abs(newK2 - stereoRig.config.distortionK2) > 0.001f)
                    {
                        stereoRig.config.distortionK2 = newK2;
                        stereoRig.UpdateSensorSettings();
                    }
                }

                bool noiseOn = stereoRig.config.noiseEnabled;
                bool newNoiseOn = GUILayout.Toggle(noiseOn, "Sensor Noise");
                if (newNoiseOn != noiseOn)
                {
                    stereoRig.config.noiseEnabled = newNoiseOn;
                    stereoRig.ApplyConfiguration();
                }

                if (stereoRig.config.noiseEnabled)
                {
                    GUILayout.Label($"Shot Noise: {stereoRig.config.shotNoiseScale:F3}");
                    float newShot = GUILayout.HorizontalSlider(stereoRig.config.shotNoiseScale, 0f, 0.2f);
                    if (Mathf.Abs(newShot - stereoRig.config.shotNoiseScale) > 0.001f)
                    {
                        stereoRig.config.shotNoiseScale = newShot;
                        stereoRig.UpdateSensorSettings();
                    }

                    GUILayout.Label($"Read Noise: {stereoRig.config.readNoiseScale:F3}");
                    float newRead = GUILayout.HorizontalSlider(stereoRig.config.readNoiseScale, 0f, 0.1f);
                    if (Mathf.Abs(newRead - stereoRig.config.readNoiseScale) > 0.001f)
                    {
                        stereoRig.config.readNoiseScale = newRead;
                        stereoRig.UpdateSensorSettings();
                    }
                }

                bool exposureSim = stereoRig.config.exposureSimEnabled;
                bool newExposureSim = GUILayout.Toggle(exposureSim, "Exposure/Gain Sim");
                if (newExposureSim != exposureSim)
                {
                    stereoRig.config.exposureSimEnabled = newExposureSim;
                    stereoRig.ApplyConfiguration();
                }

                if (stereoRig.config.exposureSimEnabled)
                {
                    GUILayout.Label("--- You Control ---", headerStyle);

                    GUILayout.Label($"Ambient Light: {stereoRig.config.ambientLux:F0} lux");
                    float newLux = GUILayout.HorizontalSlider(stereoRig.config.ambientLux, 0f, 1000f);
                    if (Mathf.Abs(newLux - stereoRig.config.ambientLux) > 5f)
                    {
                        stereoRig.config.ambientLux = newLux;
                        stereoRig.UpdateSensorSettings();
                    }
                    GUILayout.BeginHorizontal();
                    if (GUILayout.Button("Dark", GUILayout.Width(45))) { stereoRig.config.ambientLux = 50f; stereoRig.UpdateSensorSettings(); }
                    if (GUILayout.Button("Room", GUILayout.Width(45))) { stereoRig.config.ambientLux = 300f; stereoRig.UpdateSensorSettings(); }
                    if (GUILayout.Button("Bright", GUILayout.Width(45))) { stereoRig.config.ambientLux = 800f; stereoRig.UpdateSensorSettings(); }
                    GUILayout.EndHorizontal();

                    GUILayout.Label($"Strobe Pulse: {stereoRig.config.strobePulseMicroseconds:F0} μs");
                    float newPulse = GUILayout.HorizontalSlider(stereoRig.config.strobePulseMicroseconds, 1f, 500f);
                    if (Mathf.Abs(newPulse - stereoRig.config.strobePulseMicroseconds) > 1f)
                    {
                        stereoRig.config.strobePulseMicroseconds = newPulse;
                        stereoRig.UpdateSensorSettings();
                    }

                    GUILayout.Label($"Max Gain: {stereoRig.config.maxGainDb:F0} dB");
                    float newGain = GUILayout.HorizontalSlider(stereoRig.config.maxGainDb, 0f, 60f);
                    if (Mathf.Abs(newGain - stereoRig.config.maxGainDb) > 1f)
                    {
                        stereoRig.config.maxGainDb = newGain;
                        stereoRig.UpdateSensorSettings();
                    }

                    GUILayout.Space(8);
                    GUILayout.Label("--- Physics Calculates ---", headerStyle);

                    float exposureMs = stereoRig.config.CalculatedExposureMs;
                    GUILayout.Label($"Exposure (from FPS): {exposureMs:F2} ms");

                    float strobeWm2 = stereoRig.config.CalculateStrobeIrradianceAtBall();
                    GUILayout.Label($"Strobe at ball: {strobeWm2:F1} W/m²");

                    float signal = stereoRig.config.CalculateEffectiveSignal();
                    GUILayout.Label($"Signal level: {signal * 100:F1}%");

                    float gainDb = stereoRig.config.CalculateRequiredGainDb();
                    float gainLinear = stereoRig.config.CalculateRequiredGain();
                    string gainStatus = gainDb > 30 ? " (NOISY!)" : gainDb > 20 ? " (noisy)" : " (clean)";
                    GUILayout.Label($"Required Gain: {gainDb:F1} dB ({gainLinear:F1}x){gainStatus}");

                    float snr = stereoRig.config.CalculateSNR();
                    string snrStatus = snr > 100 ? "Excellent" : snr > 30 ? "Good" : snr > 10 ? "Usable" : "Poor";
                    GUILayout.Label($"Est. SNR: {snr:F0}:1 ({snrStatus})");
                }

                var calibBoard = CalibrationBoard.Instance;
                if (calibBoard != null)
                {
                    bool calibVisible = calibBoard.IsVisible;
                    bool newCalibVisible = GUILayout.Toggle(calibVisible, "Calibration Board");
                    if (newCalibVisible != calibVisible)
                    {
                        calibBoard.SetVisible(newCalibVisible);
                    }
                }

                GUILayout.Label("Sensor Crop (IMX296):");
                string[] presetLabels = SensorCropPresets.GetAllLabels();
                int newPresetIndex = GUILayout.SelectionGrid(selectedPresetIndex, presetLabels, 2);
                if (newPresetIndex != selectedPresetIndex)
                {
                    selectedPresetIndex = newPresetIndex;
                    var preset = (SensorCropPreset)selectedPresetIndex;
                    var data = SensorCropPresets.Get(preset);
                    stereoRig.config.width = data.width;
                    stereoRig.config.height = data.height;
                    if (timeController != null)
                    {
                        int maxFps = SensorCropPresets.GetMaxFpsForResolution(data.width, data.height);
                        timeController.TargetFps = maxFps;
                    }
                    stereoRig.ApplyConfiguration();
                }
            }

            GUILayout.EndVertical();
        }

        void DrawStats()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Stats", headerStyle);

            if (stereoRig != null)
            {
                var config = stereoRig.config;
                GUILayout.Label($"Height: {config.heightMm / 304.8f:F1} ft ({config.heightMm:F0} mm)");
                GUILayout.Label($"Forward: {config.forwardMm / 304.8f:F1} ft ({config.forwardMm:F0} mm)");
                GUILayout.Label($"Baseline: {config.baselineMm:F0} mm");
                GUILayout.Label($"Render: {config.RenderWidth}×{config.RenderHeight}");
                GUILayout.Label($"Config FOV: {config.EffectiveHorizontalFov:F1}° × {config.EffectiveFov:F1}°");
                GUILayout.Label($"Full Sensor FOV: {config.FullSensorHorizontalFov:F1}° × {config.FullSensorVerticalFov:F1}°");
                GUILayout.Label($"Ball @ Tee: {stereoRig.CalculateBallPixelSize():F1} px");
                GUILayout.Label($"Look-ahead: {stereoRig.CalculateLookAheadDistance():F0} mm");

                if (sim != null)
                {
                    var diag = stereoRig.GetBallSizeDiagnostics(sim.BallPositionMm);
                    GUILayout.Space(4);
                    GUILayout.Label("Ball Diagnostics:", headerStyle);
                    GUILayout.Label($"  Distance: {diag.distanceMm:F0} mm");
                    GUILayout.Label($"  Unity FOV: {diag.unityFov:F2}°");
                    GUILayout.Label($"  Expected: {diag.expectedPx:F1} px");
                    GUILayout.Label($"  Projected: {diag.actualPx:F1} px");
                }
            }

            var capture = SensorCapture.Instance;
            if (capture != null)
            {
                GUILayout.Label($"Captured Frames: {capture.CapturedFrameCount}");
            }

            GUILayout.EndVertical();
        }

        void DrawBallPosition()
        {
            GUILayout.BeginVertical(boxStyle);
            GUILayout.Label("Ball Position", headerStyle);

            if (sim != null)
            {
                var pos = sim.BallPositionMm;
                var vel = sim.BallVelocityMmS;
                float speed = vel.magnitude / SimulationController.MPH_TO_MM_S;

                GUILayout.Label($"X: {pos.x:F1} mm");
                GUILayout.Label($"Y: {pos.y:F1} mm");
                GUILayout.Label($"Z: {pos.z:F1} mm");
                GUILayout.Label($"Speed: {speed:F1} mph");
            }

            GUILayout.EndVertical();
        }
    }
}
