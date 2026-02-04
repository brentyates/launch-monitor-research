using UnityEngine;
using LaunchMonitor.Camera;

namespace LaunchMonitor.Core
{
    public class HittingArea : MonoBehaviour
    {
        public static HittingArea Instance { get; private set; }

        [Header("Hitting Area Size")]
        [Tooltip("Width and depth of the hitting area in mm")]
        public float sizeMm = 150f;

        [Header("Gizmo")]
        public Color gizmoColor = new Color(0f, 1f, 0f, 0.5f);
        public bool showGizmo = true;
        public bool showConvergencePoint = true;

        public Vector3 Center => transform.position;
        public Vector3 CenterMm => new Vector3(
            transform.position.x * 1000f,
            -transform.position.z * 1000f,
            transform.position.y * 1000f
        );

        void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }
            Instance = this;
        }

        void OnDrawGizmos()
        {
            if (!showGizmo) return;

            Gizmos.color = gizmoColor;

            float halfSize = (sizeMm / 1000f) / 2f;
            Vector3 center = transform.position;

            Vector3[] corners = new Vector3[]
            {
                center + new Vector3(-halfSize, 0.002f, -halfSize),
                center + new Vector3(halfSize, 0.002f, -halfSize),
                center + new Vector3(halfSize, 0.002f, halfSize),
                center + new Vector3(-halfSize, 0.002f, halfSize)
            };

            Gizmos.DrawLine(corners[0], corners[1]);
            Gizmos.DrawLine(corners[1], corners[2]);
            Gizmos.DrawLine(corners[2], corners[3]);
            Gizmos.DrawLine(corners[3], corners[0]);

            Gizmos.DrawLine(corners[0], corners[2]);
            Gizmos.DrawLine(corners[1], corners[3]);

            Gizmos.color = Color.yellow;
            Gizmos.DrawWireSphere(center, 0.01f);

            if (showConvergencePoint && StereoRig.Instance != null)
            {
                Vector3 convergence = StereoRig.Instance.CalculateConvergencePoint();
                Gizmos.color = Color.cyan;
                Gizmos.DrawWireSphere(convergence, 0.02f);
                Gizmos.DrawLine(center, convergence);
            }
        }
    }
}
