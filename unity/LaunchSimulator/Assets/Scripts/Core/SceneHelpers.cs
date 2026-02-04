using UnityEngine;

namespace LaunchMonitor.Core
{
    public class SceneHelpers : MonoBehaviour
    {
        [Header("Visual Helpers")]
        [SerializeField] private bool showGrid = false;
        [SerializeField] private bool showAxes = false;
        [SerializeField] private bool showBallMarker = false;

        private GameObject grid;
        private GameObject axes;
        private GameObject ballMarker;

        void Start()
        {
            CreateHelpers();
        }

        void CreateHelpers()
        {
            if (showGrid)
                CreateGrid();
            if (showAxes)
                CreateAxes();
            if (showBallMarker)
                CreateBallMarker();
        }

        void CreateGrid()
        {
            grid = new GameObject("Grid");
            grid.transform.SetParent(transform);

            float gridSize = 2f;
            int divisions = 20;
            float spacing = gridSize / divisions;

            var lineRenderer = grid.AddComponent<LineRenderer>();
            lineRenderer.useWorldSpace = true;
            lineRenderer.startWidth = 0.002f;
            lineRenderer.endWidth = 0.002f;
            lineRenderer.material = new Material(Shader.Find("Sprites/Default"));
            lineRenderer.startColor = new Color(0.25f, 0.25f, 0.25f, 0.5f);
            lineRenderer.endColor = new Color(0.25f, 0.25f, 0.25f, 0.5f);

            var points = new System.Collections.Generic.List<Vector3>();
            float half = gridSize / 2f;

            for (int i = 0; i <= divisions; i++)
            {
                float pos = -half + i * spacing;
                points.Add(new Vector3(pos, 0.001f, -half));
                points.Add(new Vector3(pos, 0.001f, half));
                points.Add(new Vector3(pos, 0.001f, half));
            }

            for (int i = 0; i <= divisions; i++)
            {
                float pos = -half + i * spacing;
                points.Add(new Vector3(-half, 0.001f, pos));
                points.Add(new Vector3(half, 0.001f, pos));
                points.Add(new Vector3(half, 0.001f, pos));
            }

            lineRenderer.positionCount = points.Count;
            lineRenderer.SetPositions(points.ToArray());
        }

        void CreateAxes()
        {
            axes = new GameObject("Axes");
            axes.transform.SetParent(transform);
            axes.transform.position = new Vector3(0, 0.002f, 0);

            float length = 0.5f;

            CreateAxisLine(axes, Vector3.right * length, Color.red);
            CreateAxisLine(axes, Vector3.up * length, Color.green);
            CreateAxisLine(axes, Vector3.forward * length, Color.blue);
        }

        void CreateAxisLine(GameObject parent, Vector3 direction, Color color)
        {
            var line = new GameObject("Axis");
            line.transform.SetParent(parent.transform);
            var lr = line.AddComponent<LineRenderer>();
            lr.useWorldSpace = false;
            lr.startWidth = 0.005f;
            lr.endWidth = 0.005f;
            lr.material = new Material(Shader.Find("Sprites/Default"));
            lr.startColor = color;
            lr.endColor = color;
            lr.positionCount = 2;
            lr.SetPosition(0, Vector3.zero);
            lr.SetPosition(1, direction);
        }

        void CreateBallMarker()
        {
            ballMarker = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
            ballMarker.name = "BallMarker";
            ballMarker.transform.SetParent(transform);

            Vector3 center = HittingArea.Instance != null ? HittingArea.Instance.Center : Vector3.zero;
            ballMarker.transform.position = new Vector3(center.x, 0.001f, center.z);
            ballMarker.transform.localScale = new Vector3(0.04f, 0.001f, 0.04f);

            Destroy(ballMarker.GetComponent<Collider>());

            var renderer = ballMarker.GetComponent<Renderer>();
            renderer.material = new Material(Shader.Find("Sprites/Default"));
            renderer.material.color = new Color(1f, 0f, 0f, 0.5f);
        }

        public void UpdateBallMarkerPosition(Vector3 positionMm)
        {
            if (ballMarker != null)
            {
                Vector3 center = HittingArea.Instance != null ? HittingArea.Instance.Center : Vector3.zero;
                ballMarker.transform.position = new Vector3(
                    center.x + positionMm.x / 1000f,
                    0.001f,
                    center.z + positionMm.y / 1000f
                );
            }
        }
    }
}
