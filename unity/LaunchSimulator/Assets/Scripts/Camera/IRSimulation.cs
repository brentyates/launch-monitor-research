using UnityEngine;

namespace LaunchMonitor.Camera
{
    public class IRSimulation : MonoBehaviour
    {
        public static IRSimulation Instance { get; private set; }

        void Awake()
        {
            Instance = this;
        }

        public void EnableIR()
        {
            CameraSensorFeature.GrayscaleEnabled = true;
        }

        public void DisableIR()
        {
            CameraSensorFeature.GrayscaleEnabled = false;
        }
    }
}
