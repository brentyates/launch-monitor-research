using UnityEngine;
using System.Collections;
using LaunchMonitor.Camera;
using LaunchMonitor.Transport;

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
            if (simulation == null || simulation.State != SimulationState.Flight)
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

                autoProcessOnComplete = true;
                Debug.Log("About to call simulation.Launch()");
                simulation.Launch();
                Debug.Log("Called simulation.Launch()");
            }
            else if (cmd.command == (int)RustCommand.Reset)
            {
                Debug.Log("Received reset command from Rust");
                simulation.ResetSimulation();
            }
        }

        private void OnSimulationStateChanged(SimulationState state)
        {
            switch (state)
            {
                case SimulationState.Idle:
                    StopFlightBuffering();
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
            var frames = sensorCapture.GetCapturedFrames();
            if (frames.Count == 0)
            {
                Debug.LogWarning("No frames to process");
                return;
            }

            EnsureSharedMemoryConnection();

            sharedMemory.SetReady(simulation.launchParams, frames.Count);
            sharedMemory.SetStreaming();

            foreach (var frame in frames)
            {
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
    }
}
