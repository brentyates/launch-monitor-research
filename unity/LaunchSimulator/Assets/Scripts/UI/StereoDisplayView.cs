using UnityEngine;
using UnityEngine.UI;
using LaunchMonitor.Camera;

namespace LaunchMonitor.UI
{
    public class StereoDisplayView : MonoBehaviour
    {
        [Header("References")]
        [SerializeField] private StereoRig stereoRig;
        [SerializeField] private RawImage cam0View;
        [SerializeField] private RawImage cam1View;
        [SerializeField] private RectTransform divider;

        [Header("Layout")]
        [SerializeField] private bool sideBySide = true;
        [SerializeField] private float dividerWidth = 2f;

        private RectTransform cam0Rect;
        private RectTransform cam1Rect;

        void Start()
        {
            if (stereoRig == null)
                stereoRig = StereoRig.Instance;

            SetupViews();
            UpdateLayout();

            if (stereoRig != null)
                stereoRig.OnConfigChanged += OnConfigChanged;
        }

        void OnDestroy()
        {
            if (stereoRig != null)
                stereoRig.OnConfigChanged -= OnConfigChanged;
        }

        private void SetupViews()
        {
            if (cam0View == null)
            {
                var cam0Go = new GameObject("Cam0View");
                cam0Go.transform.SetParent(transform, false);
                cam0View = cam0Go.AddComponent<RawImage>();
                cam0Rect = cam0Go.GetComponent<RectTransform>();
            }
            else
            {
                cam0Rect = cam0View.GetComponent<RectTransform>();
            }

            if (cam1View == null)
            {
                var cam1Go = new GameObject("Cam1View");
                cam1Go.transform.SetParent(transform, false);
                cam1View = cam1Go.AddComponent<RawImage>();
                cam1Rect = cam1Go.GetComponent<RectTransform>();
            }
            else
            {
                cam1Rect = cam1View.GetComponent<RectTransform>();
            }

            if (divider == null)
            {
                var divGo = new GameObject("Divider");
                divGo.transform.SetParent(transform, false);
                var divImage = divGo.AddComponent<Image>();
                divImage.color = new Color(1f, 1f, 1f, 0.3f);
                divider = divGo.GetComponent<RectTransform>();
            }

            UpdateTextures();
        }

        private void UpdateTextures()
        {
            if (stereoRig == null) return;

            if (cam0View != null && stereoRig.Cam0RenderTexture != null)
                cam0View.texture = stereoRig.Cam0RenderTexture;

            if (cam1View != null && stereoRig.Cam1RenderTexture != null)
                cam1View.texture = stereoRig.Cam1RenderTexture;
        }

        private void UpdateLayout()
        {
            var parentRect = GetComponent<RectTransform>();
            if (parentRect == null || stereoRig == null) return;

            float width = parentRect.rect.width;
            float height = parentRect.rect.height;

            if (sideBySide)
            {
                float halfWidth = (width - dividerWidth) / 2f;
                float viewportHeight = height;

                float textureAspect = (float)stereoRig.config.RenderWidth / stereoRig.config.RenderHeight;
                float viewportAspect = halfWidth / viewportHeight;

                float displayWidth, displayHeight;
                if (textureAspect > viewportAspect)
                {
                    displayWidth = halfWidth;
                    displayHeight = halfWidth / textureAspect;
                }
                else
                {
                    displayHeight = viewportHeight;
                    displayWidth = viewportHeight * textureAspect;
                }

                float verticalOffset = (viewportHeight - displayHeight) / 2f;
                float horizontalOffset0 = (halfWidth - displayWidth) / 2f;
                float horizontalOffset1 = halfWidth + dividerWidth + (halfWidth - displayWidth) / 2f;

                cam0Rect.anchorMin = new Vector2(0, 0);
                cam0Rect.anchorMax = new Vector2(0, 0);
                cam0Rect.sizeDelta = new Vector2(displayWidth, displayHeight);
                cam0Rect.anchoredPosition = new Vector2(horizontalOffset0 + displayWidth / 2f, verticalOffset + displayHeight / 2f);

                cam1Rect.anchorMin = new Vector2(0, 0);
                cam1Rect.anchorMax = new Vector2(0, 0);
                cam1Rect.sizeDelta = new Vector2(displayWidth, displayHeight);
                cam1Rect.anchoredPosition = new Vector2(horizontalOffset1 + displayWidth / 2f, verticalOffset + displayHeight / 2f);

                divider.anchorMin = new Vector2(0.5f, 0);
                divider.anchorMax = new Vector2(0.5f, 1);
                divider.sizeDelta = new Vector2(dividerWidth, 0);
                divider.anchoredPosition = Vector2.zero;
            }
        }

        private void OnConfigChanged()
        {
            UpdateTextures();
            UpdateLayout();
        }

        public void SetSideBySide(bool enabled)
        {
            sideBySide = enabled;
            UpdateLayout();
        }
    }
}
