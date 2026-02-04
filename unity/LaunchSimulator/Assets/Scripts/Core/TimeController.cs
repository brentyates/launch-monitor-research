using UnityEngine;

namespace LaunchMonitor.Core
{
    public class TimeController : MonoBehaviour
    {
        public static TimeController Instance { get; private set; }

        [Header("Time Settings")]
        [SerializeField] private float targetFps = 240f;
        [SerializeField] private float physicsRate = 4000f;

        [Header("Slow Motion")]
        [SerializeField] private float slowMoFactor = 0.1f;

        public float TargetFps
        {
            get => targetFps;
            set
            {
                targetFps = Mathf.Clamp(value, 60f, 2000f);
                UpdateCaptureDeltaTime();
            }
        }

        public float PhysicsRate
        {
            get => physicsRate;
            set
            {
                physicsRate = Mathf.Max(value, 1000f);
                UpdateFixedDeltaTime();
            }
        }

        public bool SlowMoEnabled { get; private set; }

        private float captureAccumulator;
        private float captureDeltaTime;

        void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }
            Instance = this;

            QualitySettings.vSyncCount = 0;
            Application.targetFrameRate = -1;
        }

        void Start()
        {
            UpdateFixedDeltaTime();
            UpdateCaptureDeltaTime();
            Debug.Log($"TimeController initialized: physicsRate={physicsRate}Hz, fixedDeltaTime={Time.fixedDeltaTime}s, targetFps={targetFps}");
        }

        void Update()
        {
            Time.captureDeltaTime = captureDeltaTime;
        }

        public void SetSlowMo(bool enabled)
        {
            SlowMoEnabled = enabled;
            Time.timeScale = enabled ? slowMoFactor : 1f;
        }

        public void ToggleSlowMo()
        {
            SetSlowMo(!SlowMoEnabled);
        }

        public bool ShouldCaptureFrame(float deltaTime)
        {
            captureAccumulator += deltaTime;
            if (captureAccumulator >= captureDeltaTime)
            {
                captureAccumulator -= captureDeltaTime;
                return true;
            }
            return false;
        }

        public void ResetCaptureAccumulator()
        {
            captureAccumulator = 0f;
        }

        private void UpdateFixedDeltaTime()
        {
            Time.fixedDeltaTime = 1f / physicsRate;
        }

        private void UpdateCaptureDeltaTime()
        {
            captureDeltaTime = 1f / targetFps;
        }
    }
}
