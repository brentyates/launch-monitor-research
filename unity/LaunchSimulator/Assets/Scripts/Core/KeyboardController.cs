using UnityEngine;
using UnityEngine.InputSystem;
using LaunchMonitor.Camera;

namespace LaunchMonitor.Core
{
    public class KeyboardController : MonoBehaviour
    {
        private SimulationController sim;
        private SimulationFlowController flow;
        private InspectorController inspector;
        private StereoRig stereoRig;

        void Start()
        {
            sim = SimulationController.Instance;
            flow = GetComponent<SimulationFlowController>();
            if (flow == null)
                flow = FindFirstObjectByType<SimulationFlowController>();

            inspector = FindFirstObjectByType<InspectorController>();
            stereoRig = StereoRig.Instance;
        }

        void Update()
        {
            var keyboard = Keyboard.current;
            if (keyboard == null) return;

            if (keyboard.spaceKey.wasPressedThisFrame)
            {
                if (sim != null && (sim.State == SimulationState.Idle || sim.State == SimulationState.Complete))
                {
                    sim.Launch();
                    Debug.Log("Launched ball!");
                }
            }

            if (keyboard.rKey.wasPressedThisFrame)
            {
                sim?.ResetSimulation();
                Debug.Log("Reset simulation");
            }

            if (keyboard.pKey.wasPressedThisFrame)
            {
                if (sim != null)
                {
                    if (sim.State == SimulationState.Flight)
                        sim.Pause();
                    else if (sim.State == SimulationState.Armed)
                        sim.Resume();
                }
            }

            if (keyboard.enterKey.wasPressedThisFrame)
            {
                flow?.ProcessFrames();
                Debug.Log("Processing frames...");
            }

            if (keyboard.sKey.wasPressedThisFrame && keyboard.leftShiftKey.isPressed)
            {
                var timeCtrl = TimeController.Instance;
                if (timeCtrl != null)
                {
                    timeCtrl.ToggleSlowMo();
                    Debug.Log($"Slow-mo: {timeCtrl.SlowMoEnabled}");
                }
            }

            if (keyboard.iKey.wasPressedThisFrame)
            {
                ToggleInspectorMode();
            }

            if (sim != null && sim.FrameHistory.Count > 0 &&
                (sim.State == SimulationState.Complete || sim.State == SimulationState.Armed))
            {
                int maxFrame = sim.FrameHistory.Count - 1;
                if (keyboard.leftArrowKey.wasPressedThisFrame)
                    sim.SeekToFrame(Mathf.Max(0, sim.CurrentFrameIndex - 1));
                if (keyboard.rightArrowKey.wasPressedThisFrame)
                    sim.SeekToFrame(Mathf.Min(maxFrame, sim.CurrentFrameIndex + 1));
                if (keyboard.homeKey.wasPressedThisFrame)
                    sim.SeekToFrame(0);
                if (keyboard.endKey.wasPressedThisFrame)
                    sim.SeekToFrame(maxFrame);
            }
        }

        private void ToggleInspectorMode()
        {
            if (inspector == null)
            {
                Debug.LogWarning("InspectorController not found");
                return;
            }

            bool newState = !inspector.IsActive;
            inspector.IsActive = newState;

            if (stereoRig != null)
            {
                stereoRig.stereoDisplayMode = !newState;
                stereoRig.UpdateDisplayMode();
            }

            if (newState)
            {
                var ball = FindFirstObjectByType<GolfBall>();
                if (ball != null)
                {
                    inspector.SetTarget(ball.transform);
                }
                inspector.ResetView();
            }

            Debug.Log($"Inspector mode: {(newState ? "ON" : "OFF")} (Press I to toggle)");
        }
    }
}
