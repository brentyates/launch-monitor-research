using UnityEngine;
using System;
using System.Collections.Generic;
using LaunchMonitor.Camera;

namespace LaunchMonitor.Core
{
    public enum SimulationState
    {
        Idle,
        Armed,
        Flight,
        Complete
    }

    [Serializable]
    public struct LaunchParameters
    {
        public float speedMph;
        public float vlaDeg;
        public float hlaDeg;
        public float spinRpm;
        public float spinAxisDeg;

        public static LaunchParameters Default => new LaunchParameters
        {
            speedMph = 130f,
            vlaDeg = 12f,
            hlaDeg = 0f,
            spinRpm = 3000f,
            spinAxisDeg = 0f
        };

        public void Randomize()
        {
            speedMph = UnityEngine.Random.Range(50f, 200f);
            vlaDeg = UnityEngine.Random.Range(0f, 45f);
            hlaDeg = UnityEngine.Random.Range(-20f, 20f);
            spinRpm = UnityEngine.Random.Range(0f, 15000f);
            spinAxisDeg = UnityEngine.Random.Range(-45f, 45f);
        }
    }

    [Serializable]
    public struct BallOrientation
    {
        public float pitchDeg;
        public float yawDeg;
        public float rollDeg;

        public Quaternion ToQuaternion()
        {
            return Quaternion.Euler(pitchDeg, yawDeg, rollDeg);
        }

        public void Randomize()
        {
            pitchDeg = UnityEngine.Random.Range(-180f, 180f);
            yawDeg = UnityEngine.Random.Range(-180f, 180f);
            rollDeg = UnityEngine.Random.Range(-180f, 180f);
        }
    }

    [Serializable]
    public struct HitPosition
    {
        public float xOffsetMm;
        public float yOffsetMm;

        public void Randomize()
        {
            xOffsetMm = UnityEngine.Random.Range(-75f, 75f);
            yOffsetMm = UnityEngine.Random.Range(-75f, 75f);
        }
    }

    public struct FrameRecord
    {
        public int frameIndex;
        public double timestamp;
        public Vector3 position;
        public Vector3 velocity;
        public Quaternion rotation;
        public float omega;
    }

    public class SimulationController : MonoBehaviour
    {
        public static SimulationController Instance { get; private set; }

        [Header("Launch Parameters")]
        public LaunchParameters launchParams = LaunchParameters.Default;
        public BallOrientation startOrientation;
        public HitPosition hitPosition;

        public const float BALL_RADIUS_MM = 21.335f;
        public const float MPH_TO_MM_S = 447.04f;

        public SimulationState State { get; private set; } = SimulationState.Idle;
        public bool ShowTrail { get; set; } = true;

        public Vector3 BallPositionMm { get; private set; }
        public Vector3 BallVelocityMmS { get; private set; }
        public Quaternion BallRotation { get; private set; }
        public float OmegaRadS { get; private set; }
        public Vector3 SpinAxis { get; private set; }

        [Header("References")]
        [SerializeField] private Transform ballTransform;
        [SerializeField] private LineRenderer trailRenderer;

        public List<FrameRecord> FrameHistory { get; private set; } = new List<FrameRecord>();
        public int CurrentFrameIndex { get; private set; }
        private bool didSeekDuringPause;

        public event Action<SimulationState> OnStateChanged;
        public event Action<FrameRecord> OnFrameCaptured;

        private List<Vector3> trailPoints = new List<Vector3>();
        private Vector3[] trailArray = new Vector3[256];
        private bool wasEverVisible;
        private int framesOutsideView;

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
            if (ballTransform == null)
            {
                var golfBall = FindFirstObjectByType<GolfBall>();
                if (golfBall != null)
                {
                    ballTransform = golfBall.transform;
                    Debug.Log($"Found GolfBall transform: {ballTransform.name}");
                }
                else
                {
                    Debug.LogError("GolfBall component not found!");
                }
            }
            else
            {
                Debug.Log($"BallTransform already assigned: {ballTransform.name}");
            }

            if (trailRenderer == null)
            {
                var trailObj = new GameObject("BallTrail");
                trailRenderer = trailObj.AddComponent<LineRenderer>();
                trailRenderer.startWidth = 0.01f;
                trailRenderer.endWidth = 0.01f;
                trailRenderer.material = new Material(Shader.Find("Sprites/Default"));
                trailRenderer.startColor = Color.yellow;
                trailRenderer.endColor = Color.red;
                trailRenderer.positionCount = 0;
            }

            ResetBall();
        }

        private int fixedUpdateCount = 0;

        void FixedUpdate()
        {
            if (State == SimulationState.Flight)
            {
                fixedUpdateCount++;
                if (fixedUpdateCount <= 5 || fixedUpdateCount % 50 == 0)
                {
                    Debug.Log($"FixedUpdate #{fixedUpdateCount} in Flight state, dt={Time.fixedDeltaTime}, ballPos={BallPositionMm}, ballVel={BallVelocityMmS}");
                }
                UpdatePhysics(Time.fixedDeltaTime);
            }
        }

        public void Launch()
        {
            Debug.Log($"SimulationController.Launch() called, current state: {State}");
            if (State != SimulationState.Idle && State != SimulationState.Complete)
            {
                Debug.LogWarning($"Launch() rejected, state is {State}, not Idle or Complete");
                return;
            }

            Debug.Log("Resetting ball and applying launch impulse");
            ResetBall();
            ApplyLaunchImpulse();
            FrameHistory.Clear();
            CurrentFrameIndex = 0;
            trailPoints.Clear();
            wasEverVisible = false;
            framesOutsideView = 0;

            SetState(SimulationState.Flight);
        }

        public void Pause()
        {
            if (State == SimulationState.Flight)
            {
                didSeekDuringPause = false;
                SetState(SimulationState.Armed);
            }
        }

        public void Resume()
        {
            if (State == SimulationState.Armed)
            {
                if (didSeekDuringPause)
                {
                    int keepCount = CurrentFrameIndex + 1;
                    if (keepCount < FrameHistory.Count)
                    {
                        FrameHistory.RemoveRange(keepCount, FrameHistory.Count - keepCount);
                        SensorCapture.Instance?.TruncateToFrame(keepCount);
                    }
                    CurrentFrameIndex = keepCount;
                }
                SetState(SimulationState.Flight);
            }
        }

        public void ResetSimulation()
        {
            ResetBall();
            FrameHistory.Clear();
            CurrentFrameIndex = 0;
            trailPoints.Clear();
            UpdateTrailRenderer();
            SetState(SimulationState.Idle);
        }

        public void ApplyStartingPose()
        {
            if (State == SimulationState.Idle || State == SimulationState.Complete)
            {
                ResetBall();
            }
        }

        public void SeekToFrame(int frameIndex)
        {
            if (frameIndex < 0 || frameIndex >= FrameHistory.Count)
                return;

            var frame = FrameHistory[frameIndex];
            BallPositionMm = frame.position;
            BallVelocityMmS = frame.velocity;
            BallRotation = frame.rotation;
            OmegaRadS = frame.omega;
            CurrentFrameIndex = frameIndex;
            didSeekDuringPause = true;

            UpdateBallTransform();

            if (ShowTrail)
            {
                trailPoints.Clear();
                for (int i = 0; i <= frameIndex; i++)
                {
                    trailPoints.Add(MmToUnityPosition(FrameHistory[i].position));
                }
                UpdateTrailRenderer();
            }
        }

        private void ResetBall()
        {
            Vector3 centerMm = HittingArea.Instance != null ? HittingArea.Instance.CenterMm : Vector3.zero;
            BallPositionMm = new Vector3(
                centerMm.x + hitPosition.xOffsetMm,
                centerMm.y + hitPosition.yOffsetMm,
                BALL_RADIUS_MM
            );
            BallVelocityMmS = Vector3.zero;
            BallRotation = startOrientation.ToQuaternion();
            OmegaRadS = 0f;
            SpinAxis = Vector3.right;

            UpdateBallTransform();
        }

        private void ApplyLaunchImpulse()
        {
            float speedMmS = launchParams.speedMph * MPH_TO_MM_S;
            float vlaRad = launchParams.vlaDeg * Mathf.Deg2Rad;
            float hlaRad = launchParams.hlaDeg * Mathf.Deg2Rad;

            float vz = speedMmS * Mathf.Sin(vlaRad);
            float vPlane = speedMmS * Mathf.Cos(vlaRad);
            float vy = vPlane * Mathf.Cos(hlaRad);
            float vx = vPlane * Mathf.Sin(hlaRad);

            BallVelocityMmS = new Vector3(vx, vy, vz);
            Debug.Log($"ApplyLaunchImpulse: speed={launchParams.speedMph} mph ({speedMmS} mm/s), VLA={launchParams.vlaDeg}°, velocity={BallVelocityMmS}");

            if (launchParams.spinRpm > 0)
            {
                OmegaRadS = (launchParams.spinRpm * 2f * Mathf.PI) / 60f;
                float axisRad = launchParams.spinAxisDeg * Mathf.Deg2Rad;
                SpinAxis = new Vector3(-Mathf.Cos(axisRad), -Mathf.Sin(axisRad), 0f).normalized;
            }
        }

        private void UpdatePhysics(float dt)
        {
            BallPositionMm += BallVelocityMmS * dt;

            if (OmegaRadS > 0)
            {
                float theta = OmegaRadS * dt;
                Quaternion dq = Quaternion.AngleAxis(theta * Mathf.Rad2Deg, SpinAxis);
                BallRotation = dq * BallRotation;
            }

            CheckCameraVisibility();

            UpdateBallTransform();

            if (ShowTrail)
            {
                trailPoints.Add(MmToUnityPosition(BallPositionMm));
                UpdateTrailRenderer();
            }
        }

        private int visibilityCheckCount = 0;

        private void CheckCameraVisibility()
        {
            var stereoRig = StereoRig.Instance;
            if (stereoRig == null)
            {
                if (BallPositionMm.y > 50000f)
                    SetState(SimulationState.Complete);
                return;
            }

            bool isVisible = stereoRig.IsBallVisibleInEitherCamera(BallPositionMm, BALL_RADIUS_MM);

            visibilityCheckCount++;
            if (visibilityCheckCount <= 10)
            {
                Debug.Log($"CheckCameraVisibility #{visibilityCheckCount}: isVisible={isVisible}, wasEverVisible={wasEverVisible}, framesOutsideView={framesOutsideView}, ballPos={BallPositionMm}");
            }

            if (isVisible)
            {
                wasEverVisible = true;
                framesOutsideView = 0;
            }
            else if (wasEverVisible)
            {
                framesOutsideView++;
                Debug.Log($"Ball outside view for {framesOutsideView} frames");
                if (framesOutsideView >= 3)
                {
                    Debug.Log("Ball outside view for 3+ frames, completing simulation");
                    SetState(SimulationState.Complete);
                }
            }
        }

        public void RecordFrame(double timestamp)
        {
            var record = new FrameRecord
            {
                frameIndex = CurrentFrameIndex,
                timestamp = timestamp,
                position = BallPositionMm,
                velocity = BallVelocityMmS,
                rotation = BallRotation,
                omega = OmegaRadS
            };

            FrameHistory.Add(record);
            CurrentFrameIndex++;

            OnFrameCaptured?.Invoke(record);
        }

        private int updateBallTransformCount = 0;

        private void UpdateBallTransform()
        {
            if (ballTransform != null)
            {
                var unityPos = MmToUnityPosition(BallPositionMm);
                ballTransform.position = unityPos;
                ballTransform.rotation = BallRotation;

                updateBallTransformCount++;
                if (updateBallTransformCount <= 3 || updateBallTransformCount % 50 == 0)
                {
                    Debug.Log($"UpdateBallTransform #{updateBallTransformCount}: ballPosMm={BallPositionMm}, unityPos={unityPos}");
                }
            }
            else if (updateBallTransformCount == 0)
            {
                Debug.LogError("UpdateBallTransform called but ballTransform is null!");
                updateBallTransformCount = 1;
            }
        }

        private void UpdateTrailRenderer()
        {
            if (trailRenderer == null)
                return;

            int count = trailPoints.Count;
            if (count > trailArray.Length)
                trailArray = new Vector3[count * 2];

            for (int i = 0; i < count; i++)
                trailArray[i] = trailPoints[i];

            trailRenderer.positionCount = count;
            trailRenderer.SetPositions(trailArray);
        }

        private void SetState(SimulationState newState)
        {
            if (State != newState)
            {
                Debug.Log($"SimulationController state transition: {State} -> {newState}");
                State = newState;
                OnStateChanged?.Invoke(newState);
            }
        }

        public static Vector3 MmToUnityPosition(Vector3 posMm)
        {
            return new Vector3(posMm.x / 1000f, posMm.z / 1000f, -posMm.y / 1000f);
        }

        public static Vector3 UnityToMmPosition(Vector3 posUnity)
        {
            return new Vector3(posUnity.x * 1000f, -posUnity.z * 1000f, posUnity.y * 1000f);
        }
    }
}
