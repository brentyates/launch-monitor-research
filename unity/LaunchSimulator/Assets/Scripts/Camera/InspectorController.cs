using UnityEngine;
using UnityEngine.InputSystem;
using LaunchMonitor.Core;
using LaunchMonitor.UI;

namespace LaunchMonitor.Camera
{
    public class InspectorController : MonoBehaviour
    {
        [Header("Settings")]
        [SerializeField] private float rotationSpeed = 0.3f;
        [SerializeField] private float zoomSpeed = 0.005f;
        [SerializeField] private float minDistance = 0.022f;
        [SerializeField] private float maxDistance = 0.5f;

        [Header("Display")]
        [SerializeField] private bool showWireframe;
        [SerializeField] private bool showAxes = true;
        [SerializeField] private bool spinPreview = false;

        [Header("References")]
        [SerializeField] private UnityEngine.Camera inspectorCamera;
        [SerializeField] private Transform ballTarget;
        [SerializeField] private GameObject axesHelper;

        private float currentDistance = 0.08f;
        private float rotationX;
        private float rotationY;
        private bool isActive;
        private Vector2 lastMousePos;
        private bool isDragging;

        public bool IsActive
        {
            get => isActive;
            set
            {
                isActive = value;
                if (inspectorCamera != null)
                    inspectorCamera.gameObject.SetActive(value);
                if (axesHelper != null)
                    axesHelper.SetActive(value && showAxes);
            }
        }

        public bool ShowWireframe
        {
            get => showWireframe;
            set
            {
                showWireframe = value;
                UpdateWireframe();
            }
        }

        public bool ShowAxes
        {
            get => showAxes;
            set
            {
                showAxes = value;
                if (axesHelper != null)
                    axesHelper.SetActive(isActive && value);
            }
        }

        public bool SpinPreview
        {
            get => spinPreview;
            set => spinPreview = value;
        }

        void Start()
        {
            SetupInspectorCamera();
            SetupAxesHelper();
            IsActive = false;
        }

        void Update()
        {
            if (!isActive) return;

            HandleInput();
            UpdateCameraPosition();

            if (spinPreview && ballTarget != null)
            {
                PreviewSpin();
            }
        }

        private void SetupInspectorCamera()
        {
            if (inspectorCamera == null)
            {
                var camGo = new GameObject("InspectorCamera");
                camGo.transform.SetParent(transform);
                inspectorCamera = camGo.AddComponent<UnityEngine.Camera>();
                inspectorCamera.clearFlags = CameraClearFlags.SolidColor;
                inspectorCamera.backgroundColor = new Color(0.1f, 0.1f, 0.1f);
                inspectorCamera.fieldOfView = 45f;
                inspectorCamera.nearClipPlane = 0.001f;
                inspectorCamera.farClipPlane = 10f;
            }

            inspectorCamera.gameObject.SetActive(false);
        }

        private void SetupAxesHelper()
        {
            if (axesHelper != null) return;

            axesHelper = new GameObject("AxesHelper");
            axesHelper.transform.SetParent(transform);

            CreateAxis(Color.red, Vector3.right, "X");
            CreateAxis(Color.green, Vector3.up, "Y");
            CreateAxis(Color.blue, Vector3.forward, "Z");

            axesHelper.SetActive(false);
        }

        private void CreateAxis(Color color, Vector3 direction, string name)
        {
            var axis = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
            axis.name = $"Axis{name}";
            axis.transform.SetParent(axesHelper.transform);

            float length = 0.1f;
            float thickness = 0.001f;

            axis.transform.localScale = new Vector3(thickness, length / 2f, thickness);
            axis.transform.localPosition = direction * (length / 2f);
            axis.transform.localRotation = Quaternion.FromToRotation(Vector3.up, direction);

            var renderer = axis.GetComponent<Renderer>();
            if (renderer != null)
            {
                var material = new Material(Shader.Find("Universal Render Pipeline/Lit"));
                material.color = color;
                renderer.material = material;
            }

            var collider = axis.GetComponent<Collider>();
            if (collider != null)
                Destroy(collider);
        }

        private void HandleInput()
        {
            var mouse = Mouse.current;
            if (mouse == null) return;

            bool overUI = SimulatorUI.Instance != null && SimulatorUI.Instance.IsPointerOverUI();

            if (mouse.leftButton.wasPressedThisFrame && !overUI)
            {
                isDragging = true;
                lastMousePos = mouse.position.ReadValue();
            }

            if (mouse.leftButton.wasReleasedThisFrame)
            {
                isDragging = false;
            }

            if (isDragging && mouse.leftButton.isPressed)
            {
                Vector2 currentPos = mouse.position.ReadValue();
                Vector2 delta = currentPos - lastMousePos;

                rotationX += delta.x * rotationSpeed;
                rotationY -= delta.y * rotationSpeed;
                rotationY = Mathf.Clamp(rotationY, -89f, 89f);

                lastMousePos = currentPos;
            }

            if (!overUI)
            {
                Vector2 scroll = mouse.scroll.ReadValue();
                if (Mathf.Abs(scroll.y) > 0.01f)
                {
                    currentDistance -= scroll.y * zoomSpeed;
                    currentDistance = Mathf.Clamp(currentDistance, minDistance, maxDistance);
                }
            }
        }

        private void UpdateCameraPosition()
        {
            if (inspectorCamera == null || ballTarget == null) return;

            Quaternion rotation = Quaternion.Euler(rotationY, rotationX, 0);
            Vector3 offset = rotation * new Vector3(0, 0, -currentDistance);

            inspectorCamera.transform.position = ballTarget.position + offset;
            inspectorCamera.transform.LookAt(ballTarget.position);

            if (axesHelper != null)
            {
                axesHelper.transform.position = ballTarget.position;
            }
        }

        private void PreviewSpin()
        {
            var sim = SimulationController.Instance;
            if (sim == null || sim.State != SimulationState.Idle) return;

            float previewOmega = (sim.launchParams.spinRpm * 2f * Mathf.PI) / 60f;
            float axisRad = sim.launchParams.spinAxisDeg * Mathf.Deg2Rad;
            Vector3 axis = new Vector3(-Mathf.Cos(axisRad), -Mathf.Sin(axisRad), 0f).normalized;

            float theta = previewOmega * Time.deltaTime;
            Quaternion dq = Quaternion.AngleAxis(theta * Mathf.Rad2Deg, axis);

            ballTarget.rotation = dq * ballTarget.rotation;
        }

        private void UpdateWireframe()
        {
            if (ballTarget == null) return;

            var renderers = ballTarget.GetComponentsInChildren<Renderer>();
            foreach (var renderer in renderers)
            {
                foreach (var mat in renderer.materials)
                {
                    if (showWireframe)
                    {
                        mat.SetFloat("_Wireframe", 1f);
                    }
                    else
                    {
                        mat.SetFloat("_Wireframe", 0f);
                    }
                }
            }
        }

        public void SetTarget(Transform target)
        {
            ballTarget = target;
        }

        public void ResetView()
        {
            rotationX = 0;
            rotationY = 0;
            currentDistance = 0.08f;
            UpdateCameraPosition();
        }
    }
}
