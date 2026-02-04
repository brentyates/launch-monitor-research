using UnityEngine;
using LaunchMonitor.Camera;

namespace LaunchMonitor.Core
{
    public enum LookAtTarget
    {
        HittingAreaCenter,
        CameraConvergencePoint
    }

    [ExecuteAlways]
    public class LookAtHittingArea : MonoBehaviour
    {
        [Tooltip("What point to aim at")]
        public LookAtTarget target = LookAtTarget.HittingAreaCenter;

        [Tooltip("Offset from the target point")]
        public Vector3 targetOffset = Vector3.zero;

        void Start()
        {
            UpdateLookAt();
        }

        void Update()
        {
            if (!Application.isPlaying)
                UpdateLookAt();
        }

        void OnValidate()
        {
            UpdateLookAt();
        }

        void UpdateLookAt()
        {
            Vector3 lookTarget = GetTargetPoint() + targetOffset;

            if (transform.position != lookTarget)
                transform.LookAt(lookTarget);
        }

        Vector3 GetTargetPoint()
        {
            switch (target)
            {
                case LookAtTarget.CameraConvergencePoint:
                    if (StereoRig.Instance != null)
                        return StereoRig.Instance.CalculateConvergencePoint();
                    goto case LookAtTarget.HittingAreaCenter;

                case LookAtTarget.HittingAreaCenter:
                default:
                    return HittingArea.Instance != null ? HittingArea.Instance.Center : Vector3.zero;
            }
        }
    }
}
