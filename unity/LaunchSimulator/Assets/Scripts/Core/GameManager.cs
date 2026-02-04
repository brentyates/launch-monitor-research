using UnityEngine;
using LaunchMonitor.Camera;
using LaunchMonitor.Transport;

namespace LaunchMonitor.Core
{
    public class GameManager : MonoBehaviour
    {
        public static GameManager Instance { get; private set; }

        [Header("Prefabs")]
        [SerializeField] private GameObject ballPrefab;

        [Header("Scene References")]
        [SerializeField] private Transform groundPlane;

        [Header("Components")]
        public SimulationController Simulation { get; private set; }
        public TimeController Time { get; private set; }
        public StereoRig StereoRig { get; private set; }
        public SensorCapture Capture { get; private set; }
        public SharedMemoryWriter SharedMemory { get; private set; }

        private GameObject ballInstance;
        private bool isCapturing;

        void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }
            Instance = this;

            // Allow Unity to run even when window is unfocused
            Application.runInBackground = true;

            InitializeComponents();
        }

        void Start()
        {
            SetupScene();
            SetupBall();

            Simulation.OnStateChanged += OnSimulationStateChanged;
        }

        void Update()
        {
            if (isCapturing && Simulation.State == SimulationState.Flight)
            {
                if (Capture.ShouldCaptureFrame(UnityEngine.Time.deltaTime))
                {
                    Capture.CaptureFrame();
                    Simulation.RecordFrame(UnityEngine.Time.timeAsDouble);
                }
            }
        }

        void OnDestroy()
        {
            if (Simulation != null)
                Simulation.OnStateChanged -= OnSimulationStateChanged;
        }

        private void InitializeComponents()
        {
            Simulation = gameObject.GetComponent<SimulationController>();
            if (Simulation == null)
                Simulation = gameObject.AddComponent<SimulationController>();

            Time = gameObject.GetComponent<TimeController>();
            if (Time == null)
                Time = gameObject.AddComponent<TimeController>();

            var stereoRigGo = new GameObject("StereoRig");
            stereoRigGo.transform.SetParent(transform);
            StereoRig = stereoRigGo.AddComponent<StereoRig>();

            Capture = gameObject.GetComponent<SensorCapture>();
            if (Capture == null)
                Capture = gameObject.AddComponent<SensorCapture>();

            SharedMemory = gameObject.GetComponent<SharedMemoryWriter>();
            if (SharedMemory == null)
                SharedMemory = gameObject.AddComponent<SharedMemoryWriter>();
        }

        private void SetupScene()
        {
            if (groundPlane == null)
            {
                var ground = GameObject.CreatePrimitive(PrimitiveType.Plane);
                ground.name = "Ground";
                ground.transform.position = Vector3.zero;
                ground.transform.localScale = new Vector3(10, 1, 10);

                var renderer = ground.GetComponent<Renderer>();
                if (renderer != null)
                {
                    var material = new Material(Shader.Find("Universal Render Pipeline/Lit"));
                    material.color = new Color(0.3f, 0.5f, 0.3f);
                    renderer.material = material;
                }

                groundPlane = ground.transform;
            }

            CreateHittingZoneMarker();
        }

        private void CreateHittingZoneMarker()
        {
            var testCube = GameObject.CreatePrimitive(PrimitiveType.Cube);
            testCube.name = "TEST_HITTING_ZONE";
            testCube.transform.position = new Vector3(0, 0.1f, 0);
            testCube.transform.localScale = new Vector3(0.15f, 0.05f, 0.15f);
            var mat = new Material(Shader.Find("Universal Render Pipeline/Lit"));
            mat.color = Color.red;
            testCube.GetComponent<Renderer>().material = mat;
            Destroy(testCube.GetComponent<Collider>());
        }

        private void SetupBall()
        {
            if (ballInstance != null)
                Destroy(ballInstance);

            if (ballPrefab != null)
            {
                ballInstance = Instantiate(ballPrefab);
            }
            else
            {
                ballInstance = new GameObject("GolfBall");
                ballInstance.AddComponent<GolfBall>();
            }

            ballInstance.name = "GolfBall";
        }

        private void OnSimulationStateChanged(SimulationState state)
        {
            switch (state)
            {
                case SimulationState.Idle:
                    isCapturing = false;
                    break;

                case SimulationState.Flight:
                    if (!isCapturing)
                    {
                        isCapturing = true;
                        Capture.StartCapture();
                        Time.ResetCaptureAccumulator();
                    }
                    break;

                case SimulationState.Complete:
                    if (isCapturing)
                    {
                        isCapturing = false;
                        Capture.StopCapture();
                    }
                    break;
            }
        }

        public void ConnectToBackend()
        {
            SharedMemory.Connect(StereoRig.config.RenderWidth, StereoRig.config.RenderHeight);
        }

        public void ProcessCapturedFrames()
        {
            if (!SharedMemory.IsConnected)
                ConnectToBackend();

            var frames = Capture.GetCapturedFrames();
            if (frames.Count == 0)
            {
                Debug.LogWarning("No frames captured");
                return;
            }

            SharedMemory.SetReady(Simulation.launchParams, frames.Count);
            SharedMemory.SetStreaming();

            foreach (var frame in frames)
            {
                SharedMemory.WriteFrame(frame);
            }

            SharedMemory.SetComplete(frames.Count);

            Debug.Log($"Sent {frames.Count} frames to Rust backend");
        }
    }
}
