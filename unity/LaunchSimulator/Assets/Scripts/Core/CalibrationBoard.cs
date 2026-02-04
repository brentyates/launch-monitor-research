using UnityEngine;

namespace LaunchMonitor.Core
{
    public class CalibrationBoard : MonoBehaviour
    {
        public static CalibrationBoard Instance { get; private set; }

        [Header("Board Dimensions")]
        [Tooltip("Number of inner corners horizontally (squares - 1)")]
        public int innerCornersX = 9;

        [Tooltip("Number of inner corners vertically (squares - 1)")]
        public int innerCornersY = 6;

        [Tooltip("Size of each square in millimeters")]
        public float squareSizeMm = 25f;

        [Header("Appearance")]
        public Color lightColor = Color.white;
        public Color darkColor = Color.black;

        [Header("Positioning")]
        [Tooltip("Height above ground in mm")]
        public float heightMm = 0f;

        [Header("Visibility")]
        public bool startVisible = false;

        private MeshRenderer meshRenderer;
        private MeshFilter meshFilter;
        private Material boardMaterial;
        private Texture2D boardTexture;
        private bool isVisible;

        public int SquaresX => innerCornersX + 1;
        public int SquaresY => innerCornersY + 1;
        public float BoardWidthMm => SquaresX * squareSizeMm;
        public float BoardHeightMm => SquaresY * squareSizeMm;
        public bool IsVisible => isVisible;

        void Awake()
        {
            Instance = this;
        }

        void Start()
        {
            CreateBoard();
            SetVisible(startVisible);
        }

        public void SetVisible(bool visible)
        {
            isVisible = visible;
            if (meshRenderer != null)
                meshRenderer.enabled = visible;

            var ball = FindFirstObjectByType<GolfBall>();
            if (ball != null)
                ball.gameObject.SetActive(!visible);
        }

        public void Toggle()
        {
            SetVisible(!isVisible);
        }

        void OnDestroy()
        {
            if (boardTexture != null)
                Destroy(boardTexture);
            if (boardMaterial != null)
                Destroy(boardMaterial);
        }

        public void CreateBoard()
        {
            meshFilter = GetComponent<MeshFilter>();
            if (meshFilter == null)
                meshFilter = gameObject.AddComponent<MeshFilter>();

            meshRenderer = GetComponent<MeshRenderer>();
            if (meshRenderer == null)
                meshRenderer = gameObject.AddComponent<MeshRenderer>();

            CreateMesh();
            CreateTexture();
            ApplyMaterial();
            UpdatePosition();
        }

        private void CreateMesh()
        {
            float widthM = BoardWidthMm / 1000f;
            float heightM = BoardHeightMm / 1000f;

            Mesh mesh = new Mesh();
            mesh.name = "CalibrationBoard";

            Vector3[] vertices = new Vector3[4]
            {
                new Vector3(-widthM / 2f, 0, -heightM / 2f),
                new Vector3(widthM / 2f, 0, -heightM / 2f),
                new Vector3(-widthM / 2f, 0, heightM / 2f),
                new Vector3(widthM / 2f, 0, heightM / 2f)
            };

            int[] triangles = new int[6] { 0, 2, 1, 2, 3, 1 };

            Vector2[] uvs = new Vector2[4]
            {
                new Vector2(0, 0),
                new Vector2(1, 0),
                new Vector2(0, 1),
                new Vector2(1, 1)
            };

            Vector3[] normals = new Vector3[4]
            {
                Vector3.up,
                Vector3.up,
                Vector3.up,
                Vector3.up
            };

            mesh.vertices = vertices;
            mesh.triangles = triangles;
            mesh.uv = uvs;
            mesh.normals = normals;

            meshFilter.mesh = mesh;
        }

        private void CreateTexture()
        {
            int pixelsPerSquare = 64;
            int texWidth = SquaresX * pixelsPerSquare;
            int texHeight = SquaresY * pixelsPerSquare;

            boardTexture = new Texture2D(texWidth, texHeight, TextureFormat.RGB24, false);
            boardTexture.filterMode = FilterMode.Point;
            boardTexture.wrapMode = TextureWrapMode.Clamp;

            Color[] pixels = new Color[texWidth * texHeight];

            for (int y = 0; y < texHeight; y++)
            {
                for (int x = 0; x < texWidth; x++)
                {
                    int squareX = x / pixelsPerSquare;
                    int squareY = y / pixelsPerSquare;

                    bool isLight = (squareX + squareY) % 2 == 0;
                    pixels[y * texWidth + x] = isLight ? lightColor : darkColor;
                }
            }

            boardTexture.SetPixels(pixels);
            boardTexture.Apply();
        }

        private void ApplyMaterial()
        {
            boardMaterial = new Material(Shader.Find("Universal Render Pipeline/Unlit"));
            boardMaterial.mainTexture = boardTexture;
            meshRenderer.material = boardMaterial;
        }

        private void UpdatePosition()
        {
            Vector3 pos = transform.position;
            pos.y = heightMm / 1000f;
            transform.position = pos;
        }

        public void SetPosition(Vector3 positionMm)
        {
            transform.position = new Vector3(
                positionMm.x / 1000f,
                positionMm.z / 1000f,
                positionMm.y / 1000f
            );
        }

        public void SetRotation(float pitchDeg, float yawDeg)
        {
            transform.rotation = Quaternion.Euler(pitchDeg, yawDeg, 0);
        }

        void OnValidate()
        {
            if (Application.isPlaying && meshFilter != null)
            {
                CreateBoard();
            }
        }

#if UNITY_EDITOR
        void OnDrawGizmos()
        {
            float widthM = BoardWidthMm / 1000f;
            float heightM = BoardHeightMm / 1000f;

            Gizmos.color = Color.yellow;
            Gizmos.matrix = transform.localToWorldMatrix;
            Gizmos.DrawWireCube(Vector3.zero, new Vector3(widthM, 0.001f, heightM));

            Gizmos.color = Color.red;
            float cornerSize = squareSizeMm / 1000f * 0.2f;
            for (int y = 0; y < innerCornersY; y++)
            {
                for (int x = 0; x < innerCornersX; x++)
                {
                    float cx = (x + 1) * squareSizeMm / 1000f - widthM / 2f;
                    float cy = (y + 1) * squareSizeMm / 1000f - heightM / 2f;
                    Gizmos.DrawWireSphere(new Vector3(cx, 0, cy), cornerSize);
                }
            }
        }
#endif
    }
}
