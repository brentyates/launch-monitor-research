using UnityEngine;
using System.Collections;
using LaunchMonitor.Camera;
using LaunchMonitor.Transport;
using UnityEngine.Rendering.Universal;

namespace LaunchMonitor.Core
{
    public class SimulationFlowController : MonoBehaviour
    {
        [Header("References")]
        [SerializeField] private SimulationController simulation;
        [SerializeField] private TimeController timeController;
        [SerializeField] private StereoRig stereoRig;
        [SerializeField] private SensorCapture sensorCapture;
        [SerializeField] private SharedMemoryWriter sharedMemory;
        [SerializeField] private CalibrationBoard calibrationBoard;

        [Header("Auto Processing")]
        [SerializeField] private bool autoProcessOnComplete = false;
        [SerializeField] private float maxFlightTime = 5f;

        private bool isCapturing;
        private bool isBufferingForFlight;
        private float flightStartTime;
        private int capturedFrameCount;
        private bool sharedMemoryEventSubscribed;

        void Start()
        {
            GatherReferences();
            SubscribeEvents();
        }

        private void StartContinuousCapture()
        {
            if (isCapturing) return;
            if (sensorCapture == null) return;

            sensorCapture.StartCapture();
            isCapturing = true;
            Debug.Log("Started continuous frame capture");
        }

        void OnDestroy()
        {
            UnsubscribeEvents();
        }

        private int flowUpdateCount;

        void Update()
        {
            flowUpdateCount++;
            if (flowUpdateCount == 1 || flowUpdateCount == 60 || flowUpdateCount == 120)
            {
                Debug.Log($"SimulationFlowController.Update frame #{flowUpdateCount}");
            }

            GatherReferences();
            EnsureSharedMemoryConnection();
            StartContinuousCapture();

            if (isCapturing)
            {
                CaptureFrameIfNeeded();

                if (isBufferingForFlight && simulation.State == SimulationState.Flight)
                {
                    CheckFlightTimeout();
                }
            }
        }

        private void GatherReferences()
        {
            if (simulation == null)
                simulation = SimulationController.Instance;
            if (timeController == null)
                timeController = TimeController.Instance;
            if (stereoRig == null)
                stereoRig = StereoRig.Instance;
            if (sensorCapture == null)
                sensorCapture = SensorCapture.Instance;
            if (sharedMemory == null)
                sharedMemory = SharedMemoryWriter.Instance;
            if (calibrationBoard == null)
                calibrationBoard = CalibrationBoard.Instance;
        }

        private void SubscribeEvents()
        {
            if (simulation != null)
                simulation.OnStateChanged += OnSimulationStateChanged;
            if (sensorCapture != null)
            {
                sensorCapture.OnCaptureComplete += OnCaptureComplete;
                sensorCapture.OnFrameCaptured += OnFrameCapturedHandler;
            }
        }

        private void UnsubscribeEvents()
        {
            if (simulation != null)
                simulation.OnStateChanged -= OnSimulationStateChanged;
            if (sensorCapture != null)
            {
                sensorCapture.OnCaptureComplete -= OnCaptureComplete;
                sensorCapture.OnFrameCaptured -= OnFrameCapturedHandler;
            }
            if (sharedMemory != null && sharedMemoryEventSubscribed)
            {
                sharedMemory.OnLaunchCommand -= OnLaunchCommand;
                sharedMemoryEventSubscribed = false;
            }
        }

        private int frameCapturedCount;

        private void OnFrameCapturedHandler(CapturedFrame frame)
        {
            // Allow streaming if we are in Flight OR if calibration board is visible (Calibration mode)
            bool isCalibration = calibrationBoard != null && calibrationBoard.IsVisible;
            if (simulation == null || (simulation.State != SimulationState.Flight && !isCalibration))
                return;

            frameCapturedCount++;
            if (frameCapturedCount <= 3)
            {
                Debug.Log($"OnFrameCapturedHandler #{frameCapturedCount}: frame.frameIndex={frame.frameIndex}");
            }

            EnsureSharedMemoryConnection();
            if (sharedMemory != null && sharedMemory.IsConnected)
            {
                sharedMemory.WriteFrame(frame);
            }
        }

        private void OnLaunchCommand(LaunchCommand cmd)
        {
            Debug.Log($"OnLaunchCommand called: command={cmd.command}, speed={cmd.speedMph}");
            if (cmd.command == (int)RustCommand.Launch)
            {
                Debug.Log($"Received launch command from Rust: {cmd.speedMph} mph, VLA {cmd.vlaDeg}°");

                if (simulation == null)
                {
                    Debug.LogError("simulation is NULL!");
                    return;
                }

                Debug.Log($"Setting launch params and calling simulation.Launch()");
                simulation.launchParams = new LaunchParameters
                {
                    speedMph = cmd.speedMph,
                    vlaDeg = cmd.vlaDeg,
                    hlaDeg = cmd.hlaDeg,
                    spinRpm = cmd.spinRpm,
                    spinAxisDeg = cmd.spinAxisDeg
                };

                // Restore camera settings after calibration
                RestoreCameraSettings();

                // FIX: Update shared memory state so Rust knows we are starting
                if (sharedMemory != null && sharedMemory.IsConnected)
                {
                    // Target total frames (e.g. 60)
                    sharedMemory.SetReady(simulation.launchParams, 60);
                }

                autoProcessOnComplete = true;
                Debug.Log("About to call simulation.Launch()");
                simulation.Launch();
                Debug.Log("Called simulation.Launch()");
            }
            else if (cmd.command == (int)RustCommand.Reset)
            {
                Debug.Log("Received reset command from Rust");
                
                if (calibrationBoard != null)
                    calibrationBoard.SetVisible(false);

                simulation.ResetSimulation();
                
                // Force shared memory to Idle state (in case sim was already Idle)
                if (sharedMemory != null && sharedMemory.IsConnected)
                {
                    sharedMemory.SetIdle();
                }
            }
            else if (cmd.command == (int)RustCommand.Calibrate)
            {
                Debug.Log("Received calibrate command from Rust");

                if (calibrationBoard != null)
                {
                    calibrationBoard.squareSizeMm = 50f;
                    calibrationBoard.CreateBoard();
                    
                    // Force the simulation to Idle if not already
                    if (simulation.State != SimulationState.Idle)
                    {
                        simulation.ResetSimulation();
                    }

                    // Disable Camera Sensor Effects for Calibration (so we can see!)
                    if (stereoRig != null)
                    {
                        // FIX: Position board 1.5 meters in front of rig so it's visible
                        // Ensure it's perfectly centered and parallel to the rig's baseline
                        calibrationBoard.transform.position = stereoRig.transform.position + stereoRig.transform.forward * 1.5f;
                        calibrationBoard.transform.rotation = stereoRig.transform.rotation;
                        calibrationBoard.transform.Rotate(90, 0, 0); // Face the rig (Corrected from -90)
                        
                        calibrationBoard.SetVisible(true);

                        // FIX: Force cameras to LookAt the board center symmetrically
                        stereoRig.config.exposureSimEnabled = false;
                        stereoRig.config.noiseEnabled = false;
                        stereoRig.config.irFilterEnabled = false; // Calibration uses ambient
                        stereoRig.config.distortionEnabled = true; // Enable distortion so we can calibrate it!
                        stereoRig.ApplyConfiguration();

                        stereoRig.Cam0.transform.LookAt(calibrationBoard.transform);
                        stereoRig.Cam1.transform.LookAt(calibrationBoard.transform);

                        // Force SolidColor for both cameras
                        stereoRig.Cam0.clearFlags = CameraClearFlags.SolidColor;
                        stereoRig.Cam1.clearFlags = CameraClearFlags.SolidColor;
                        stereoRig.Cam0.backgroundColor = Color.gray;
                        stereoRig.Cam1.backgroundColor = Color.gray;
                        stereoRig.Cam0.cullingMask = -1;
                        stereoRig.Cam1.cullingMask = -1;

                        Debug.Log($"[SimulationFlowController] Calibration Stage: Board at {calibrationBoard.transform.position}, Cam0 at {stereoRig.Cam0.transform.position}");

                        // Force bright ambient for calibration board visibility
                        // MUST BE AFTER ApplyConfiguration because DisableIRMode resets it
                        RenderSettings.ambientMode = UnityEngine.Rendering.AmbientMode.Flat;
                        RenderSettings.ambientLight = Color.white;
                        RenderSettings.ambientIntensity = 1.0f;

                        // Ensure subsequent IR mode enable works
                        stereoRig.ResetIRState();
                    }

                    // Tell shared memory we are ready/streaming so Rust can see the board
                    // We don't need flight buffering, just live frames
                    if (sharedMemory != null && sharedMemory.IsConnected)
                    {
                        // Use default/zero params for calibration
                        var emptyParams = new LaunchParameters();
                        // Set frame count to 0 or arbitrary high number since we are just streaming
                        sharedMemory.SetReady(emptyParams, 0); 
                        sharedMemory.SetStreaming();
                    }
                }
                else
                {
                    Debug.LogError("CalibrationBoard reference is missing!");
                }
            }
        }

        private void OnSimulationStateChanged(SimulationState state)
        {
            switch (state)
            {
                case SimulationState.Idle:
                    StopFlightBuffering();
                    if (sharedMemory != null && sharedMemory.IsConnected)
                    {
                        sharedMemory.SetIdle();
                    }
                    break;

                case SimulationState.Flight:
                    StartFlightBuffering();
                    break;

                case SimulationState.Complete:
                    StopFlightBuffering();

                    // Update shared memory state to Complete
                    if (sharedMemory != null && sharedMemory.IsConnected)
                    {
                        Debug.Log($"Flight complete, updating shared memory state to Complete with {capturedFrameCount} frames");
                        sharedMemory.SetComplete(capturedFrameCount);
                    }

                    if (autoProcessOnComplete)
                    {
                        StartCoroutine(ProcessAfterDelay(0.1f));
                    }
                    break;
            }
        }

        private void StartFlightBuffering()
        {
            if (isBufferingForFlight) return;

            isBufferingForFlight = true;
            flightStartTime = Time.time;
            capturedFrameCount = 0;

            sensorCapture.ClearFrames();
            timeController.ResetCaptureAccumulator();

            if (sharedMemory != null && sharedMemory.IsConnected)
            {
                sharedMemory.UpdateGroundTruth(simulation.launchParams);
            }

            Debug.Log("Started flight buffering");
        }

        private void StopFlightBuffering()
        {
            if (!isBufferingForFlight) return;

            isBufferingForFlight = false;

            Debug.Log($"Stopped flight buffering after {capturedFrameCount} frames");
        }

        private void CaptureFrameIfNeeded()
        {
            if (sensorCapture.ShouldCaptureFrame(Time.deltaTime))
            {
                sensorCapture.CaptureFrame();

                if (isBufferingForFlight)
                {
                    simulation.RecordFrame(Time.timeAsDouble);
                    capturedFrameCount++;
                }
            }
        }

        private void CheckFlightTimeout()
        {
            if (Time.time - flightStartTime > maxFlightTime)
            {
                Debug.LogWarning($"Flight exceeded {maxFlightTime}s, forcing stop");
                simulation.ResetSimulation();
            }
        }

        private void OnCaptureComplete(System.Collections.Generic.List<CapturedFrame> frames)
        {
            Debug.Log($"Capture complete: {frames.Count} frames captured");
        }

        private IEnumerator ProcessAfterDelay(float delay)
        {
            yield return new WaitForSeconds(delay);
            ProcessFrames();
        }

        public void ProcessFrames()
        {
            var allFrames = sensorCapture.GetCapturedFrames();
            var frames = capturedFrameCount > 0 && capturedFrameCount < allFrames.Count
                ? allFrames.GetRange(0, capturedFrameCount)
                : allFrames;

            if (frames.Count == 0)
            {
                Debug.LogWarning("No frames to process");
                return;
            }

            Debug.Log($"Processing {frames.Count} flight frames (of {allFrames.Count} total captured)");

            EnsureSharedMemoryConnection();

            sharedMemory.SetReady(simulation.launchParams, frames.Count);
            sharedMemory.SetStreaming();

            for (int i = 0; i < frames.Count; i++)
            {
                var frame = frames[i];
                frame.frameIndex = i;
                sharedMemory.WriteFrame(frame);
            }

            sharedMemory.SetComplete(frames.Count);

            Debug.Log($"Sent {frames.Count} frames to Rust backend");
        }

        private void EnsureSharedMemoryConnection()
        {
            if (sharedMemory == null || sharedMemory.IsConnected) return;
            if (stereoRig == null || stereoRig.config == null)
            {
                Debug.LogWarning("StereoRig not ready yet, deferring shared memory connection");
                return;
            }

            bool connected = sharedMemory.Connect(
                stereoRig.config.RenderWidth,
                stereoRig.config.RenderHeight
            );

            if (connected)
            {
                if (!sharedMemoryEventSubscribed)
                {
                    sharedMemory.OnLaunchCommand += OnLaunchCommand;
                    sharedMemoryEventSubscribed = true;
                    Debug.Log("Subscribed to shared memory launch commands");
                }
            }
            else
            {
                Debug.LogError("Failed to connect to shared memory");
            }
        }

        public void LaunchAndCapture()
        {
            simulation.Launch();
        }

        public void LaunchCaptureAndProcess()
        {
            autoProcessOnComplete = true;
            simulation.Launch();
        }

        private void RestoreCameraSettings()
        {
            Debug.Log("[SimulationFlowController] Restoring Camera Settings for Launch");
            
            if (calibrationBoard != null)
                calibrationBoard.SetVisible(false);

            if (stereoRig != null)
            {
                // Restore IR mode for launch
                stereoRig.config.exposureSimEnabled = true; 
                stereoRig.config.noiseEnabled = true; 
                stereoRig.config.distortionEnabled = true; 
                stereoRig.config.irFilterEnabled = true;   
                stereoRig.config.ambientLux = 0f;
                stereoRig.config.strobePower = 500f; // Higher power for better Signal-to-Noise
                stereoRig.config.readNoiseScale = 0.005f; // Realistic read noise
                stereoRig.config.shotNoiseScale = 0.01f;
                stereoRig.ApplyConfiguration();

                stereoRig.Cam0.backgroundColor = Color.black; 
                stereoRig.Cam1.backgroundColor = Color.black;
                stereoRig.Cam0.clearFlags = CameraClearFlags.SolidColor;
                stereoRig.Cam1.clearFlags = CameraClearFlags.SolidColor;
                
                // ISOLATION: Cull ONLY virtual/debug objects (Layer 31)
                // We keep everything else (Default, Floor, Grass, etc.) for realism
                stereoRig.Cam0.cullingMask = ~(1 << 31); 
                stereoRig.Cam1.cullingMask = ~(1 << 31);

                var data0 = stereoRig.Cam0.GetComponent<UniversalAdditionalCameraData>();
                if (data0 != null) data0.renderPostProcessing = true; 
                var data1 = stereoRig.Cam1.GetComponent<UniversalAdditionalCameraData>();
                if (data1 != null) data1.renderPostProcessing = true;
            }
        }
    }
}
