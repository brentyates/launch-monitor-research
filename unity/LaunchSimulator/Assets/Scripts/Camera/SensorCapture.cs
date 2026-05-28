using UnityEngine;
using UnityEngine.Rendering;
using System;
using System.Collections.Generic;
using System.Text;
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

    // Holds context for a single frame capture request
    public class FrameContext
    {
        public int frameIndex;
        public double timestamp;
        
        // Metadata captured at render time
        public Vector3 ballPosition;
        public Vector3 ballVelocity;
        public Quaternion ballRotation;

        // Buffers for pixel data
        public byte[] cam0Pixels;
        public byte[] cam1Pixels;
        
        // Completion flags
        public bool cam0Complete;
        public bool cam1Complete;

        public FrameContext(int index, double time, int pixelCount)
        {
            frameIndex = index;
            timestamp = time;
            cam0Pixels = new byte[pixelCount];
            cam1Pixels = new byte[pixelCount];
        }
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
        
        // We don't need a queue of requests if we use lambdas to capture context,
        // but we might want to track active contexts if we needed to cancel them.
        // For now, simpler is better.
        
        private int frameIndex;
        private double captureStartTime;
        private float captureAccumulator;

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

        public void StartCapture()
        {
            if (IsCapturing) return;

            capturedFrames.Clear();
            frameIndex = 0;
            captureStartTime = Time.timeAsDouble;
            captureAccumulator = 0f;
            IsCapturing = true;
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

            // 1. Render immediate
            stereoRig.RenderBothCameras();

            // 2. Capture metadata NOW (while simulation is in this state)
            double targetFps = TimeController.Instance.TargetFps;
            double timestamp = frameIndex / targetFps;
            int pixelCount = stereoRig.config.RenderWidth * stereoRig.config.RenderHeight * 4;

            var context = new FrameContext(frameIndex, timestamp, pixelCount);

            var sim = SimulationController.Instance;
            if (sim != null)
            {
                context.ballPosition = sim.BallPositionMm;
                context.ballVelocity = sim.BallVelocityMmS;
                context.ballRotation = sim.BallRotation;
            }

            // 3. Request readback with context
            // We use lambdas to pass the specific context to the callback
            AsyncGPUReadback.Request(stereoRig.Cam0RenderTexture, 0, TextureFormat.RGBA32, (req) => OnCam0ReadbackComplete(req, context));
            AsyncGPUReadback.Request(stereoRig.Cam1RenderTexture, 0, TextureFormat.RGBA32, (req) => OnCam1ReadbackComplete(req, context));

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

        private void OnCam0ReadbackComplete(AsyncGPUReadbackRequest request, FrameContext context)
        {
            if (request.hasError)
            {
                Debug.LogError($"Cam0 readback failed for frame {context.frameIndex}");
                return;
            }

            var data = request.GetData<byte>();
            if (data.IsCreated && data.Length == context.cam0Pixels.Length)
            {
                // Diagnostic: Log first 10 pixels
                if (context.frameIndex % 60 == 0 || context.frameIndex == 0)
                {
                    StringBuilder sb = new StringBuilder();
                    sb.Append($"[SensorCapture] Frame {context.frameIndex} Cam0 Pixels (first 10): ");
                    for (int i = 0; i < Math.Min(10, data.Length / 4); i++) // Log up to 10 pixels, each 4 bytes
                    {
                        int offset = i * 4;
                        byte r = data[offset];
                        byte g = data[offset + 1];
                        byte b = data[offset + 2];
                        byte a = data[offset + 3];
                        sb.Append($"[{i}: R={r} G={g} B={b} A={a}] ");
                    }
                    Debug.Log(sb.ToString());
                }

                data.CopyTo(context.cam0Pixels);
                context.cam0Complete = true;
                TryFinalizeFrame(context);
            }
        }

        private void OnCam1ReadbackComplete(AsyncGPUReadbackRequest request, FrameContext context)
        {
            if (request.hasError)
            {
                Debug.LogError($"Cam1 readback failed for frame {context.frameIndex}");
                return;
            }

            var data = request.GetData<byte>();
            if (data.IsCreated && data.Length == context.cam1Pixels.Length)
            {
                // Diagnostic: Log first 10 pixels
                if (context.frameIndex % 60 == 0 || context.frameIndex == 0)
                {
                    StringBuilder sb = new StringBuilder();
                    sb.Append($"[SensorCapture] Frame {context.frameIndex} Cam1 Pixels (first 10): ");
                    for (int i = 0; i < Math.Min(10, data.Length / 4); i++) // Log up to 10 pixels, each 4 bytes
                    {
                        int offset = i * 4;
                        byte r = data[offset];
                        byte g = data[offset + 1];
                        byte b = data[offset + 2];
                        byte a = data[offset + 3];
                        sb.Append($"[{i}: R={r} G={g} B={b} A={a}] ");
                    }
                    Debug.Log(sb.ToString());
                }

                data.CopyTo(context.cam1Pixels);
                context.cam1Complete = true;
                TryFinalizeFrame(context);
            }
        }

        private int finalizeCount;

        private void TryFinalizeFrame(FrameContext context)
        {
            if (!context.cam0Complete || !context.cam1Complete) return;

            finalizeCount++;
            if (finalizeCount <= 3)
            {
                Debug.Log($"TryFinalizeFrame #{finalizeCount}: both cameras ready for frame {context.frameIndex}, invoking OnFrameCaptured");
            }

            var frame = new CapturedFrame
            {
                frameIndex = context.frameIndex,
                timestamp = context.timestamp,
                cam0Pixels = context.cam0Pixels,
                cam1Pixels = context.cam1Pixels,
                ballPosition = context.ballPosition, // This is now the CORRECT position from render time!
                ballVelocity = context.ballVelocity,
                ballRotation = context.ballRotation
            };

            // Note: Since readbacks are async, frames might complete out of order slightly,
            // but we add them to the list. If order matters for processing, we might need to sort later
            // or insert based on index. For now, append is fine, the consumer checks indices.
            
            // To be safe for linear processing, strict append is okay if we assume readbacks finish roughly in order
            // or if the consumer handles out-of-order. Rust pipeline sorts by timestamp/index usually?
            // Let's just Add.
            capturedFrames.Add(frame);
            
            OnFrameCaptured?.Invoke(frame);
        }

        public List<CapturedFrame> GetCapturedFrames()
        {
            // Sort by frame index to ensure stable order for consumers
            var list = new List<CapturedFrame>(capturedFrames);
            list.Sort((a, b) => a.frameIndex.CompareTo(b.frameIndex));
            return list;
        }

        public void ClearFrames()
        {
            capturedFrames.Clear();
            frameIndex = 0;
        }

        public void TruncateToFrame(int targetFrameIndex)
        {
            // Sort first
            capturedFrames.Sort((a, b) => a.frameIndex.CompareTo(b.frameIndex));
            
            // Allow removal
            // Find removal start
            int removeStart = -1;
            for(int i=0; i<capturedFrames.Count; i++) {
                if (capturedFrames[i].frameIndex >= targetFrameIndex) {
                    removeStart = i;
                    break;
                }
            }
            
            if (removeStart != -1)
            {
                capturedFrames.RemoveRange(removeStart, capturedFrames.Count - removeStart);
                frameIndex = targetFrameIndex;
            }
        }
    }
}
