using UnityEngine;
using UnityEngine.Rendering;
using System;
using System.Collections.Generic;
using LaunchMonitor.Core;

namespace LaunchMonitor.Camera
{
    public struct CapturedFrame
    {
        public int frameIndex;
        public double timestamp;
        public byte[] cam0Pixels;
        public byte[] cam1Pixels;
        public Vector3 ballPosition;
        public Vector3 ballVelocity;
        public Quaternion ballRotation;
    }

    public class SensorCapture : MonoBehaviour
    {
        public static SensorCapture Instance { get; private set; }

        [Header("References")]
        [SerializeField] private StereoRig stereoRig;

        [Header("Capture State")]
        public bool IsCapturing { get; private set; }
        public int CapturedFrameCount => capturedFrames.Count;

        private List<CapturedFrame> capturedFrames = new List<CapturedFrame>();
        private Queue<AsyncGPUReadbackRequest> pendingRequests = new Queue<AsyncGPUReadbackRequest>();
        private int frameIndex;
        private double captureStartTime;
        private float captureAccumulator;

        private byte[] pendingCam0Pixels;
        private byte[] pendingCam1Pixels;
        private int pendingFrameIndex;
        private double pendingTimestamp;
        private Vector3 pendingPosition;
        private Vector3 pendingVelocity;
        private Quaternion pendingRotation;
        private bool cam0Complete;
        private bool cam1Complete;

        public event Action<CapturedFrame> OnFrameCaptured;
        public event Action<List<CapturedFrame>> OnCaptureComplete;

        void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }
            Instance = this;
        }

        void Start()
        {
            if (stereoRig == null)
                stereoRig = StereoRig.Instance;
        }

        void Update()
        {
            ProcessPendingRequests();
        }

        public void StartCapture()
        {
            if (IsCapturing) return;

            capturedFrames.Clear();
            frameIndex = 0;
            captureStartTime = Time.timeAsDouble;
            captureAccumulator = 0f;
            IsCapturing = true;

            pendingCam0Pixels = null;
            pendingCam1Pixels = null;
            cam0Complete = false;
            cam1Complete = false;
        }

        public void StopCapture()
        {
            IsCapturing = false;
            OnCaptureComplete?.Invoke(capturedFrames);
        }

        private int captureFrameDebugCount;

        public void CaptureFrame()
        {
            captureFrameDebugCount++;
            if (captureFrameDebugCount <= 3)
            {
                Debug.Log($"CaptureFrame #{captureFrameDebugCount}: IsCapturing={IsCapturing}, stereoRig={(stereoRig != null ? "not null" : "null")}");
            }

            if (!IsCapturing || stereoRig == null) return;

            stereoRig.RenderBothCameras();

            double timestamp = Time.timeAsDouble - captureStartTime;

            pendingFrameIndex = frameIndex;
            pendingTimestamp = timestamp;

            var sim = SimulationController.Instance;
            if (sim != null)
            {
                pendingPosition = sim.BallPositionMm;
                pendingVelocity = sim.BallVelocityMmS;
                pendingRotation = sim.BallRotation;
            }

            int pixelCount = stereoRig.config.RenderWidth * stereoRig.config.RenderHeight * 4;
            pendingCam0Pixels = new byte[pixelCount];
            pendingCam1Pixels = new byte[pixelCount];
            cam0Complete = false;
            cam1Complete = false;

            var cam0Request = AsyncGPUReadback.Request(stereoRig.Cam0RenderTexture, 0, TextureFormat.RGBA32, OnCam0ReadbackComplete);
            var cam1Request = AsyncGPUReadback.Request(stereoRig.Cam1RenderTexture, 0, TextureFormat.RGBA32, OnCam1ReadbackComplete);

            frameIndex++;
        }

        private int shouldCaptureDebugCount;

        public bool ShouldCaptureFrame(float deltaTime)
        {
            if (!IsCapturing) return false;

            float targetDt = 1f / TimeController.Instance.TargetFps;
            captureAccumulator += deltaTime;

            shouldCaptureDebugCount++;
            if (shouldCaptureDebugCount <= 3)
            {
                Debug.Log($"ShouldCaptureFrame #{shouldCaptureDebugCount}: deltaTime={deltaTime}, targetDt={targetDt}, accum={captureAccumulator}");
            }

            if (captureAccumulator >= targetDt)
            {
                captureAccumulator -= targetDt;
                return true;
            }
            return false;
        }

        private int cam0ReadbackCount;

        private void OnCam0ReadbackComplete(AsyncGPUReadbackRequest request)
        {
            cam0ReadbackCount++;

            if (request.hasError || pendingCam0Pixels == null)
            {
                if (request.hasError) Debug.LogError($"Cam0 readback #{cam0ReadbackCount} failed");
                return;
            }

            var data = request.GetData<byte>();
            if (data.IsCreated && data.Length == pendingCam0Pixels.Length)
            {
                data.CopyTo(pendingCam0Pixels);
                cam0Complete = true;
                TryFinalizeFrame();
            }
        }

        private void OnCam1ReadbackComplete(AsyncGPUReadbackRequest request)
        {
            if (request.hasError || pendingCam1Pixels == null)
            {
                if (request.hasError) Debug.LogError("Cam1 readback failed");
                return;
            }

            var data = request.GetData<byte>();
            if (data.IsCreated && data.Length == pendingCam1Pixels.Length)
            {
                data.CopyTo(pendingCam1Pixels);
                cam1Complete = true;
                TryFinalizeFrame();
            }
        }

        private int finalizeCount;

        private void TryFinalizeFrame()
        {
            if (!cam0Complete || !cam1Complete) return;

            finalizeCount++;
            if (finalizeCount <= 3)
            {
                Debug.Log($"TryFinalizeFrame #{finalizeCount}: both cameras ready, invoking OnFrameCaptured");
            }

            var frame = new CapturedFrame
            {
                frameIndex = pendingFrameIndex,
                timestamp = pendingTimestamp,
                cam0Pixels = pendingCam0Pixels,
                cam1Pixels = pendingCam1Pixels,
                ballPosition = pendingPosition,
                ballVelocity = pendingVelocity,
                ballRotation = pendingRotation
            };

            capturedFrames.Add(frame);
            OnFrameCaptured?.Invoke(frame);

            pendingCam0Pixels = null;
            pendingCam1Pixels = null;
            cam0Complete = false;
            cam1Complete = false;
        }

        private void ProcessPendingRequests()
        {
            while (pendingRequests.Count > 0)
            {
                var request = pendingRequests.Peek();
                if (request.done)
                {
                    pendingRequests.Dequeue();
                }
                else
                {
                    break;
                }
            }
        }

        public List<CapturedFrame> GetCapturedFrames()
        {
            return new List<CapturedFrame>(capturedFrames);
        }

        public void ClearFrames()
        {
            capturedFrames.Clear();
            frameIndex = 0;
        }

        public void TruncateToFrame(int targetFrameIndex)
        {
            if (targetFrameIndex < capturedFrames.Count)
            {
                capturedFrames.RemoveRange(targetFrameIndex, capturedFrames.Count - targetFrameIndex);
                frameIndex = targetFrameIndex;
            }
        }
    }
}
