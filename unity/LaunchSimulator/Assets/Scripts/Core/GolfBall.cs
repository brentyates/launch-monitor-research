using UnityEngine;

namespace LaunchMonitor.Core
{
    public class GolfBall : MonoBehaviour
    {
        [Header("Ball Model")]
        [SerializeField] private GameObject ballModelPrefab;

        [Header("Ball Properties")]
        [SerializeField] private float radiusMm = 21.335f;

        private GameObject ballModelInstance;

        void Start()
        {
            CreateBall();
        }

        public void CreateBall()
        {
            if (ballModelInstance != null)
            {
                Destroy(ballModelInstance);
            }

            if (ballModelPrefab == null)
            {
                ballModelPrefab = Resources.Load<GameObject>("tp5_pix_ball");
            }

            if (ballModelPrefab == null)
            {
                Debug.LogError("GolfBall: No ball model prefab assigned and couldn't load from Resources");
                CreateFallbackSphere();
                return;
            }

            ballModelInstance = Instantiate(ballModelPrefab, transform);
            ballModelInstance.name = "BallModel";
            ballModelInstance.transform.localPosition = Vector3.zero;
            ballModelInstance.transform.localRotation = Quaternion.identity;

            float blenderRadius = 2.0f;
            float targetRadius = radiusMm / 1000f;
            float scale = targetRadius / blenderRadius;
            ballModelInstance.transform.localScale = Vector3.one * scale;
        }

        private void CreateFallbackSphere()
        {
            Debug.LogWarning("GolfBall: Using fallback white sphere");

            var meshFilter = gameObject.AddComponent<MeshFilter>();
            var meshRenderer = gameObject.AddComponent<MeshRenderer>();

            var sphere = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            meshFilter.mesh = sphere.GetComponent<MeshFilter>().mesh;
            Destroy(sphere);

            float scale = radiusMm / 1000f / 0.5f;
            transform.localScale = Vector3.one * scale;

            meshRenderer.material = new Material(Shader.Find("Universal Render Pipeline/Lit"));
            meshRenderer.material.color = Color.white;
        }
    }
}
