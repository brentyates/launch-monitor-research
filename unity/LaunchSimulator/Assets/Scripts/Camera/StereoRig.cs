using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using System;
using LaunchMonitor.Core;

namespace LaunchMonitor.Camera
{
    public enum SensorCropPreset
    {
        Full,
        Half,
        Quarter,
        Strip,
        Small,
        Tiny,
        Wide
    }

    public static class SensorCropPresets
    {
        public struct PresetData
        {
            public int width;
            public int height;
            public int estimatedFps;

            public PresetData(int w, int h, int fps)
            {
                width = w;
                height = h;
                estimatedFps = fps;
            }
        }

        public static PresetData Get(SensorCropPreset preset)
        {
            return preset switch
            {
                SensorCropPreset.Full => new PresetData(1456, 1088, 60),
                SensorCropPreset.Half => new PresetData(728, 544, 120),
                SensorCropPreset.Quarter => new PresetData(512, 384, 240),
                SensorCropPreset.Strip => new PresetData(688, 136, 400),
                SensorCropPreset.Small => new PresetData(224, 96, 534),
                SensorCropPreset.Tiny => new PresetData(128, 96, 536),
                SensorCropPreset.Wide => new PresetData(320, 96, 536),
                _ => new PresetData(512, 384, 240)
            };
        }

        public static int GetMaxFpsForResolution(int width, int height)
        {
            int pixels = width * height;
            if (pixels >= 1456 * 1088) return 60;
            if (pixels >= 728 * 544) return 120;
            if (pixels >= 512 * 384) return 240;
            if (pixels >= 688 * 136) return 400;
            if (pixels >= 224 * 96) return 534;
            return 536;
        }

        public static string[] GetAllLabels()
        {
            var presets = (SensorCropPreset[])System.Enum.GetValues(typeof(SensorCropPreset));
            var labels = new string[presets.Length];
            for (int i = 0; i < presets.Length; i++)
            {
                var data = Get(presets[i]);
                labels[i] = $"{presets[i]} {data.width}×{data.height} @{data.estimatedFps}";
            }
            return labels;
        }
    }

    [Serializable]
    public class StereoConfig
    {
        public const int SENSOR_WIDTH_PX = 1456;
        public const int SENSOR_HEIGHT_PX = 1088;
        public const float PIXEL_PITCH_MM = 0.00508f;
        public const float SENSOR_WIDTH_MM = SENSOR_WIDTH_PX * PIXEL_PITCH_MM;
        public const float SENSOR_HEIGHT_MM = SENSOR_HEIGHT_PX * PIXEL_PITCH_MM;

        [Header("Rig Position")]
        public float baselineMm = 350f;
        public float heightMm = 3048f;
        public float forwardMm = 1092f;

        [Header("Lens")]
        [Tooltip("Lens focal length in mm. Common values: 4mm (wide), 6mm, 8mm, 12mm (narrow)")]
        public float focalLengthMm = 6f;

        [Header("Sensor Crop")]
        public int width = 512;
        public int height = 384;
        public bool portraitMode = true;

        [Header("Sensor Noise")]
        public bool noiseEnabled = false;
        [Tooltip("Shot noise scale. Realistic range: 0.02-0.1. Simulates photon counting noise (brighter = more noise).")]
        [Range(0f, 0.2f)]
        public float shotNoiseScale = 0.05f;
        [Tooltip("Read noise scale. Realistic range: 0.01-0.05. Constant sensor electronics noise.")]
        [Range(0f, 0.1f)]
        public float readNoiseScale = 0.02f;

        [Header("Exposure Simulation")]
        public bool exposureSimEnabled = false;
        [Tooltip("Ambient light level in lux. Dark room ~50, indoor ~300, bright indoor ~500.")]
        [Range(0f, 1000f)]
        public float ambientLux = 300f;
        [Tooltip("IR strobe pulse duration in microseconds.")]
        [Range(1f, 1000f)]
        public float strobePulseMicroseconds = 20f;
        [Tooltip("Maximum gain in dB. 40dB = 100x amplification.")]
        [Range(0f, 60f)]
        public float maxGainDb = 40f;
        [Tooltip("Target mid-tone brightness (0-1).")]
        [Range(0.2f, 0.8f)]
        public float targetBrightness = 0.5f;

        public float CalculatedExposureMs => 1000f / (TimeController.Instance?.TargetFps ?? 500f);

        public float CalculateStrobeIrradianceAtBall()
        {
            float distanceM = Mathf.Sqrt(heightMm * heightMm + forwardMm * forwardMm) / 1000f;

            float ledEfficiency = 0.30f;
            float radiantPowerW = strobePower * ledEfficiency;

            float beamSteradians = 2f * Mathf.PI * (1f - Mathf.Cos(strobeBeamAngle * 0.5f * Mathf.Deg2Rad));
            float radiantIntensity = radiantPowerW / Mathf.Max(beamSteradians, 0.1f);

            float irradianceWm2 = radiantIntensity / (distanceM * distanceM);
            return irradianceWm2;
        }

        public float CalculateEffectiveSignal()
        {
            float exposureMs = CalculatedExposureMs;

            float strobeIrradiance = irFilterEnabled ? CalculateStrobeIrradianceAtBall() : 0f;
            float effectiveStrobeExposureMs = Mathf.Min(strobePulseMicroseconds / 1000f, exposureMs);

            float strobeEnergy = strobeIrradiance * effectiveStrobeExposureMs;

            float ambientIrradiance = ambientLux * 0.005f;
            if (irFilterEnabled) ambientIrradiance *= 0.02f;
            float ambientEnergy = ambientIrradiance * exposureMs;

            float totalEnergy = strobeEnergy + ambientEnergy;

            float sensorQE = 0.30f;
            float wellCapacityNormalized = 1.0f;
            float signalLevel = totalEnergy * sensorQE / wellCapacityNormalized;

            return Mathf.Clamp01(signalLevel);
        }

        public float CalculateSNR()
        {
            float signal = CalculateEffectiveSignal();
            float gain = CalculateRequiredGain();

            float readNoiseElectrons = 2.5f;
            float shotNoiseElectrons = Mathf.Sqrt(signal * 10000f);

            float totalNoise = Mathf.Sqrt(
                (readNoiseElectrons * gain) * (readNoiseElectrons * gain) +
                shotNoiseElectrons * shotNoiseElectrons
            );

            float signalElectrons = signal * 10000f * gain;
            return signalElectrons / Mathf.Max(totalNoise, 1f);
        }

        public float CalculateRequiredGain()
        {
            float signal = CalculateEffectiveSignal();
            if (signal >= targetBrightness) return 1f;
            float gainNeeded = targetBrightness / Mathf.Max(signal, 0.001f);
            float maxGain = Mathf.Pow(10f, maxGainDb / 20f);
            return Mathf.Min(gainNeeded, maxGain);
        }

        public float CalculateRequiredGainDb()
        {
            float gain = CalculateRequiredGain();
            if (gain <= 1f) return 0f;
            return 20f * Mathf.Log10(gain);
        }

        [Header("Lens Distortion (Brown-Conrady Radial)")]
        [Tooltip("First radial distortion coefficient. Negative = barrel, positive = pincushion. Typical: -0.1 to -0.3")]
        public float distortionK1 = -0.15f;
        [Tooltip("Second radial distortion coefficient. Corrects higher-order distortion. Typical: 0.01 to 0.05")]
        public float distortionK2 = 0.02f;
        public bool distortionEnabled = false;

        [Header("IR Simulation")]
        public bool irFilterEnabled = false;
        public float strobePower = 50f;
        public float strobeBeamAngle = 60f;

        [Header("Convergence")]
        [Tooltip("Padding beyond the back edge of the hitting zone to keep visible (mm)")]
        public float backEdgePaddingMm = 25f;

        public int RenderWidth => portraitMode ? height : width;
        public int RenderHeight => portraitMode ? width : height;

        public float FullSensorHorizontalFov => 2f * Mathf.Atan(SENSOR_WIDTH_MM / (2f * focalLengthMm)) * Mathf.Rad2Deg;
        public float FullSensorVerticalFov => 2f * Mathf.Atan(SENSOR_HEIGHT_MM / (2f * focalLengthMm)) * Mathf.Rad2Deg;

        public float EffectiveFov
        {
            get
            {
                float physicalHeightMm = RenderHeight * PIXEL_PITCH_MM;
                return 2f * Mathf.Atan(physicalHeightMm / (2f * focalLengthMm)) * Mathf.Rad2Deg;
            }
        }

        public float EffectiveHorizontalFov
        {
            get
            {
                float physicalWidthMm = RenderWidth * PIXEL_PITCH_MM;
                return 2f * Mathf.Atan(physicalWidthMm / (2f * focalLengthMm)) * Mathf.Rad2Deg;
            }
        }
    }

    public class StereoRig : MonoBehaviour
    {
        public static StereoRig Instance { get; private set; }

        [Header("Configuration")]
        public StereoConfig config = new StereoConfig();

        [Header("Camera References")]
        [SerializeField] private UnityEngine.Camera cam0;
        [SerializeField] private UnityEngine.Camera cam1;

        [Header("Render Targets")]
        public RenderTexture Cam0RenderTexture { get; private set; }
        public RenderTexture Cam1RenderTexture { get; private set; }

        [Header("Display Mode")]
        public bool stereoDisplayMode = true;

        public UnityEngine.Camera Cam0 => cam0;
        public UnityEngine.Camera Cam1 => cam1;
        public Light Cam0Strobe => cam0Strobe;
        public Light Cam1Strobe => cam1Strobe;

        public event Action OnConfigChanged;

        private Light cam0Strobe;
        private Light cam1Strobe;
        private UnityEngine.Camera backgroundCamera;
        private IRSimulation irSimulation;
        private bool irWasEnabled;
        private bool distortionWasEnabled;

        private Light sceneDirectionalLight;
        private float originalDirectionalIntensity;
        private Color originalAmbientLight;
        private AmbientMode originalAmbientMode;
        private float originalAmbientIntensity;

        private System.Collections.Generic.List<Light> sceneSpotLights = new System.Collections.Generic.List<Light>();
        private System.Collections.Generic.Dictionary<Light, float> originalSpotIntensities = new System.Collections.Generic.Dictionary<Light, float>();

        void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }
            Instance = this;
        }

        private int lastScreenWidth;
        private int lastScreenHeight;

        void Start()
        {
            CacheSceneLighting();
            SetupCameras();
            SetupIRSimulation();
            ApplyConfiguration();
            lastScreenWidth = Screen.width;
            lastScreenHeight = Screen.height;
        }

        void Update()
        {
            if (Screen.width != lastScreenWidth || Screen.height != lastScreenHeight)
            {
                lastScreenWidth = Screen.width;
                lastScreenHeight = Screen.height;
                if (stereoDisplayMode)
                    UpdateDisplayMode();
            }
        }

        void OnDestroy()
        {
            ReleaseRenderTextures();
        }

        private void CacheSceneLighting()
        {
            var lights = FindObjectsByType<Light>(FindObjectsSortMode.None);
            foreach (var light in lights)
            {
                if (light.type == LightType.Directional && sceneDirectionalLight == null)
                {
                    sceneDirectionalLight = light;
                    originalDirectionalIntensity = light.intensity;
                }
                else if (light.type == LightType.Spot && !IsStrobeLight(light))
                {
                    sceneSpotLights.Add(light);
                    originalSpotIntensities[light] = light.intensity;
                }
            }

            originalAmbientLight = RenderSettings.ambientLight;
            originalAmbientMode = RenderSettings.ambientMode;
            originalAmbientIntensity = RenderSettings.ambientIntensity;
        }

        private bool IsStrobeLight(Light light)
        {
            return light == cam0Strobe || light == cam1Strobe;
        }

        private void SetupIRSimulation()
        {
            var irGo = new GameObject("IRSimulation");
            irGo.transform.SetParent(transform);
            irSimulation = irGo.AddComponent<IRSimulation>();
        }

        public void ApplyConfiguration()
        {
            UpdateCameraPositions();
            UpdateCameraSettings();
            UpdateRenderTextures();
            UpdateIRMode();
            UpdateSensorEffects();
            UpdateDisplayMode();
            InvalidateFrustumPlanes();
            OnConfigChanged?.Invoke();
        }

        private void UpdateIRMode()
        {
            if (config.irFilterEnabled && !irWasEnabled)
            {
                EnableIRMode();
                irWasEnabled = true;
            }
            else if (!config.irFilterEnabled && irWasEnabled)
            {
                DisableIRMode();
                irWasEnabled = false;
            }
        }

        private void UpdateSensorEffects()
        {
            CameraSensorFeature.DistortionEnabled = config.distortionEnabled;
            CameraSensorFeature.K1 = config.distortionK1;
            CameraSensorFeature.K2 = config.distortionK2;

            CameraSensorFeature.NoiseEnabled = config.noiseEnabled;
            CameraSensorFeature.ShotNoiseScale = config.shotNoiseScale;
            CameraSensorFeature.ReadNoiseScale = config.readNoiseScale;

            CameraSensorFeature.ExposureSimEnabled = config.exposureSimEnabled;
            CameraSensorFeature.ExposureMs = config.CalculatedExposureMs;
            CameraSensorFeature.SceneBrightness = config.CalculateEffectiveSignal();
            CameraSensorFeature.TargetBrightness = config.targetBrightness;
            CameraSensorFeature.MaxGainDb = config.maxGainDb;

            CameraSensorFeature.GrayscaleEnabled = config.irFilterEnabled;
            CameraSensorFeature.Contrast = 1.0f;

            distortionWasEnabled = config.distortionEnabled;
        }

        public void UpdateSensorSettings()
        {
            CameraSensorFeature.K1 = config.distortionK1;
            CameraSensorFeature.K2 = config.distortionK2;
            CameraSensorFeature.ShotNoiseScale = config.shotNoiseScale;
            CameraSensorFeature.ReadNoiseScale = config.readNoiseScale;
            CameraSensorFeature.ExposureMs = config.CalculatedExposureMs;
            CameraSensorFeature.SceneBrightness = config.CalculateEffectiveSignal();
            CameraSensorFeature.TargetBrightness = config.targetBrightness;
            CameraSensorFeature.MaxGainDb = config.maxGainDb;
        }

        private void EnableIRMode()
        {
            if (sceneDirectionalLight != null)
                sceneDirectionalLight.intensity = 0f;

            foreach (var light in sceneSpotLights)
            {
                if (light != null)
                    light.intensity = 0f;
            }

            RenderSettings.ambientMode = AmbientMode.Flat;
            RenderSettings.ambientLight = Color.black;
            RenderSettings.ambientIntensity = 0f;

            ConfigureStrobeForIR(cam0Strobe);
            ConfigureStrobeForIR(cam1Strobe);

            cam0.backgroundColor = Color.black;
            cam1.backgroundColor = Color.black;

            if (irSimulation != null)
                irSimulation.EnableIR();
        }

        private void DisableIRMode()
        {
            if (sceneDirectionalLight != null)
                sceneDirectionalLight.intensity = originalDirectionalIntensity;

            foreach (var light in sceneSpotLights)
            {
                if (light != null && originalSpotIntensities.TryGetValue(light, out float intensity))
                    light.intensity = intensity;
            }

            RenderSettings.ambientMode = originalAmbientMode;
            RenderSettings.ambientLight = originalAmbientLight;
            RenderSettings.ambientIntensity = originalAmbientIntensity;

            RestoreStrobe(cam0Strobe);
            RestoreStrobe(cam1Strobe);

            cam0.backgroundColor = new Color(0.1f, 0.1f, 0.15f);
            cam1.backgroundColor = new Color(0.1f, 0.1f, 0.15f);

            if (irSimulation != null)
                irSimulation.DisableIR();
        }

        private void ConfigureStrobeForIR(Light strobe)
        {
            if (strobe == null) return;

            strobe.type = LightType.Spot;
            strobe.spotAngle = config.strobeBeamAngle;
            strobe.innerSpotAngle = config.strobeBeamAngle * 0.7f;
            strobe.intensity = config.strobePower;
            strobe.range = 50f;
            strobe.color = Color.white;
            strobe.shadows = LightShadows.None;
        }

        public void UpdateStrobeSettings()
        {
            if (!config.irFilterEnabled) return;
            if (cam0Strobe != null)
            {
                cam0Strobe.intensity = config.strobePower;
                cam0Strobe.spotAngle = config.strobeBeamAngle;
                cam0Strobe.innerSpotAngle = config.strobeBeamAngle * 0.7f;
            }
            if (cam1Strobe != null)
            {
                cam1Strobe.intensity = config.strobePower;
                cam1Strobe.spotAngle = config.strobeBeamAngle;
                cam1Strobe.innerSpotAngle = config.strobeBeamAngle * 0.7f;
            }
        }

        private void RestoreStrobe(Light strobe)
        {
            if (strobe == null) return;

            strobe.type = LightType.Point;
            strobe.intensity = 0f;
            strobe.range = 20f;
            strobe.color = Color.white;
        }

        private void SetupCameras()
        {
            if (cam0 == null)
            {
                var cam0Go = new GameObject("Cam0");
                cam0Go.transform.SetParent(transform);
                cam0 = cam0Go.AddComponent<UnityEngine.Camera>();
                cam0Go.AddComponent<UniversalAdditionalCameraData>();
            }

            if (cam1 == null)
            {
                var cam1Go = new GameObject("Cam1");
                cam1Go.transform.SetParent(transform);
                cam1 = cam1Go.AddComponent<UnityEngine.Camera>();
                cam1Go.AddComponent<UniversalAdditionalCameraData>();
            }

            cam0.clearFlags = CameraClearFlags.SolidColor;
            cam0.backgroundColor = new Color(0.1f, 0.1f, 0.15f);
            cam1.clearFlags = CameraClearFlags.SolidColor;
            cam1.backgroundColor = new Color(0.1f, 0.1f, 0.15f);

            if (backgroundCamera == null)
            {
                var bgCamGo = new GameObject("BackgroundCamera");
                bgCamGo.transform.SetParent(transform);
                backgroundCamera = bgCamGo.AddComponent<UnityEngine.Camera>();
                backgroundCamera.clearFlags = CameraClearFlags.SolidColor;
                backgroundCamera.backgroundColor = Color.black;
                backgroundCamera.cullingMask = 0;
                backgroundCamera.depth = -100;
            }
        }

        public void UpdateDisplayMode()
        {
            if (stereoDisplayMode)
            {
                cam0.targetTexture = null;
                cam1.targetTexture = null;
                cam0.enabled = true;
                cam1.enabled = true;

                float textureAspect = (float)config.RenderWidth / config.RenderHeight;
                float screenAspect = (float)Screen.width / Screen.height;
                float halfScreenAspect = screenAspect / 2f;

                float rectWidth, rectHeight, rectX, rectY;

                if (textureAspect > halfScreenAspect)
                {
                    rectWidth = 0.5f;
                    rectHeight = halfScreenAspect / textureAspect;
                    rectX = 0f;
                    rectY = (1f - rectHeight) / 2f;
                }
                else
                {
                    rectHeight = 1f;
                    rectWidth = (textureAspect / halfScreenAspect) * 0.5f;
                    rectX = (0.5f - rectWidth) / 2f;
                    rectY = 0f;
                }

                cam0.rect = new Rect(rectX, rectY, rectWidth, rectHeight);
                cam1.rect = new Rect(0.5f + rectX, rectY, rectWidth, rectHeight);
                cam0.depth = 1;
                cam1.depth = 1;

                if (backgroundCamera != null)
                    backgroundCamera.enabled = true;
            }
            else
            {
                cam0.enabled = false;
                cam1.enabled = false;
                if (backgroundCamera != null)
                    backgroundCamera.enabled = false;
            }
        }

        private void UpdateCameraPositions()
        {
            float baselineM = config.baselineMm / 1000f;
            float heightM = config.heightMm / 1000f;
            float forwardM = config.forwardMm / 1000f;

            cam0.transform.localPosition = new Vector3(-baselineM / 2f, heightM, -forwardM);
            cam1.transform.localPosition = new Vector3(baselineM / 2f, heightM, -forwardM);

            Vector3 convergence = CalculateConvergencePoint();
            cam0.transform.LookAt(convergence);
            cam1.transform.LookAt(convergence);

            SetupStrobeLights();
        }

        public Vector3 CalculateConvergencePoint()
        {
            var hitting = HittingArea.Instance;
            if (hitting == null)
                return Vector3.zero;

            float heightM = config.heightMm / 1000f;
            float forwardM = config.forwardMm / 1000f;
            float halfSizeM = (hitting.sizeMm / 1000f) / 2f;
            float paddingM = config.backEdgePaddingMm / 1000f;

            float verticalFovRad = config.EffectiveFov * Mathf.Deg2Rad;
            float halfFov = verticalFovRad / 2f;

            float farEdgeZ = hitting.Center.z + halfSizeM + paddingM;
            float horizontalDistToFarEdge = forwardM + farEdgeZ;

            if (horizontalDistToFarEdge <= 0)
                return hitting.Center;

            float angleToFarEdge = Mathf.Atan2(heightM, horizontalDistToFarEdge);
            float convergenceAngle = angleToFarEdge + halfFov;

            float convergenceZ = (heightM / Mathf.Tan(convergenceAngle)) - forwardM;

            return new Vector3(hitting.Center.x, 0, convergenceZ);
        }

        private void SetupStrobeLights()
        {
            if (cam0Strobe == null)
            {
                var cam0StrobeGo = new GameObject("Cam0Strobe");
                cam0StrobeGo.transform.SetParent(cam0.transform);
                cam0StrobeGo.transform.localPosition = Vector3.zero;
                cam0StrobeGo.transform.localRotation = Quaternion.identity;
                cam0Strobe = cam0StrobeGo.AddComponent<Light>();
                cam0Strobe.type = LightType.Point;
                cam0Strobe.color = Color.white;
                cam0Strobe.intensity = 0f;
                cam0Strobe.range = 20f;
            }

            if (cam1Strobe == null)
            {
                var cam1StrobeGo = new GameObject("Cam1Strobe");
                cam1StrobeGo.transform.SetParent(cam1.transform);
                cam1StrobeGo.transform.localPosition = Vector3.zero;
                cam1StrobeGo.transform.localRotation = Quaternion.identity;
                cam1Strobe = cam1StrobeGo.AddComponent<Light>();
                cam1Strobe.type = LightType.Point;
                cam1Strobe.color = Color.white;
                cam1Strobe.intensity = 0f;
                cam1Strobe.range = 20f;
            }
        }

        private void UpdateCameraSettings()
        {
            float aspect = (float)config.RenderWidth / config.RenderHeight;
            float fov = config.EffectiveFov;

            cam0.fieldOfView = fov;
            cam0.aspect = aspect;
            cam0.nearClipPlane = 0.1f;
            cam0.farClipPlane = 100f;

            cam1.fieldOfView = fov;
            cam1.aspect = aspect;
            cam1.nearClipPlane = 0.1f;
            cam1.farClipPlane = 100f;
        }

        private void UpdateRenderTextures()
        {
            ReleaseRenderTextures();

            Cam0RenderTexture = new RenderTexture(config.RenderWidth, config.RenderHeight, 24, RenderTextureFormat.ARGB32);
            Cam0RenderTexture.Create();

            Cam1RenderTexture = new RenderTexture(config.RenderWidth, config.RenderHeight, 24, RenderTextureFormat.ARGB32);
            Cam1RenderTexture.Create();
        }

        private void ReleaseRenderTextures()
        {
            if (Cam0RenderTexture != null)
            {
                Cam0RenderTexture.Release();
                Destroy(Cam0RenderTexture);
                Cam0RenderTexture = null;
            }

            if (Cam1RenderTexture != null)
            {
                Cam1RenderTexture.Release();
                Destroy(Cam1RenderTexture);
                Cam1RenderTexture = null;
            }
        }

        public void RenderBothCameras()
        {
            var sim = LaunchMonitor.Core.SimulationController.Instance;
            bool trailWasActive = false;
            GameObject trailObj = null;

            if (sim != null)
            {
                trailObj = GameObject.Find("BallTrail");
                if (trailObj != null)
                {
                    trailWasActive = trailObj.activeSelf;
                    trailObj.SetActive(false);
                }
            }

            var cam0PrevTarget = cam0.targetTexture;
            var cam1PrevTarget = cam1.targetTexture;
            var cam0PrevRect = cam0.rect;
            var cam1PrevRect = cam1.rect;

            cam0.targetTexture = Cam0RenderTexture;
            cam1.targetTexture = Cam1RenderTexture;
            cam0.rect = new Rect(0, 0, 1, 1);
            cam1.rect = new Rect(0, 0, 1, 1);

            cam0.Render();
            cam1.Render();

            cam0.targetTexture = cam0PrevTarget;
            cam1.targetTexture = cam1PrevTarget;
            cam0.rect = cam0PrevRect;
            cam1.rect = cam1PrevRect;

            if (trailObj != null && trailWasActive)
            {
                trailObj.SetActive(true);
            }
        }

        public float CalculateBallPixelSize()
        {
            float ballRadiusMm = 21.335f;
            float distanceToOrigin = Mathf.Sqrt(
                config.heightMm * config.heightMm +
                config.forwardMm * config.forwardMm
            );

            float fovRad = config.EffectiveFov * Mathf.Deg2Rad;
            float viewHeightMm = 2f * distanceToOrigin * Mathf.Tan(fovRad / 2f);
            float pixelsPerMm = config.RenderHeight / viewHeightMm;

            return ballRadiusMm * 2f * pixelsPerMm;
        }

        public (float expectedPx, float actualPx, float distanceMm, float unityFov) GetBallSizeDiagnostics(Vector3 ballPosMm)
        {
            float ballRadiusMm = 21.335f;

            Vector3 ballPosUnity = new Vector3(
                ballPosMm.x / 1000f,
                ballPosMm.z / 1000f,
                -ballPosMm.y / 1000f
            );

            Vector3 camPos = cam0.transform.position;
            float distanceM = Vector3.Distance(camPos, ballPosUnity);
            float distanceMm = distanceM * 1000f;

            float fovRad = cam0.fieldOfView * Mathf.Deg2Rad;
            float viewHeightM = 2f * distanceM * Mathf.Tan(fovRad / 2f);
            float viewHeightMm = viewHeightM * 1000f;
            float pixelsPerMm = config.RenderHeight / viewHeightMm;
            float expectedPx = ballRadiusMm * 2f * pixelsPerMm;

            Vector3 ballTop = ballPosUnity + cam0.transform.up * (ballRadiusMm / 1000f);
            Vector3 ballBottom = ballPosUnity - cam0.transform.up * (ballRadiusMm / 1000f);
            Vector3 topScreen = cam0.WorldToScreenPoint(ballTop);
            Vector3 bottomScreen = cam0.WorldToScreenPoint(ballBottom);
            float actualPx = Vector3.Distance(topScreen, bottomScreen);

            return (expectedPx, actualPx, distanceMm, cam0.fieldOfView);
        }

        public float CalculateLookAheadDistance()
        {
            Vector3 convergence = CalculateConvergencePoint();
            Vector3 hittingCenter = HittingArea.Instance != null ? HittingArea.Instance.Center : Vector3.zero;

            float heightM = config.heightMm / 1000f;
            float forwardM = config.forwardMm / 1000f;
            float verticalFovRad = config.EffectiveFov * Mathf.Deg2Rad;
            float halfFov = verticalFovRad / 2f;

            float convergenceAngle = Mathf.Atan2(heightM, forwardM + convergence.z);
            float bottomOfFrameAngle = convergenceAngle - halfFov;

            if (bottomOfFrameAngle <= 0)
                return config.heightMm * 10f;

            float nearEdgeZ = (heightM / Mathf.Tan(bottomOfFrameAngle)) - forwardM;
            return (hittingCenter.z - nearEdgeZ) * 1000f;
        }

        public (Vector2 cam0, Vector2 cam1) ProjectPoint(Vector3 worldPosMm)
        {
            Vector3 worldPosM = new Vector3(
                worldPosMm.x / 1000f,
                worldPosMm.z / 1000f,
                worldPosMm.y / 1000f
            );

            Vector3 cam0Viewport = this.cam0.WorldToViewportPoint(worldPosM);
            Vector3 cam1Viewport = this.cam1.WorldToViewportPoint(worldPosM);

            Vector2 cam0Pixel = new Vector2(
                cam0Viewport.x * config.RenderWidth,
                (1f - cam0Viewport.y) * config.RenderHeight
            );
            Vector2 cam1Pixel = new Vector2(
                cam1Viewport.x * config.RenderWidth,
                (1f - cam1Viewport.y) * config.RenderHeight
            );

            return (cam0Pixel, cam1Pixel);
        }

        private Plane[] cam0FrustumPlanes = new Plane[6];
        private Plane[] cam1FrustumPlanes = new Plane[6];
        private bool frustumPlanesDirty = true;

        public void InvalidateFrustumPlanes()
        {
            frustumPlanesDirty = true;
        }

        private void UpdateFrustumPlanesIfNeeded()
        {
            if (!frustumPlanesDirty) return;
            GeometryUtility.CalculateFrustumPlanes(cam0, cam0FrustumPlanes);
            GeometryUtility.CalculateFrustumPlanes(cam1, cam1FrustumPlanes);
            frustumPlanesDirty = false;
        }

        public bool IsBallVisibleInEitherCamera(Vector3 ballPosMm, float ballRadiusMm)
        {
            UpdateFrustumPlanesIfNeeded();

            Vector3 worldPosM = new Vector3(
                ballPosMm.x / 1000f,
                ballPosMm.z / 1000f,
                -ballPosMm.y / 1000f
            );
            float radiusM = ballRadiusMm / 1000f;

            Bounds ballBounds = new Bounds(worldPosM, Vector3.one * radiusM * 2f);

            return GeometryUtility.TestPlanesAABB(cam0FrustumPlanes, ballBounds) ||
                   GeometryUtility.TestPlanesAABB(cam1FrustumPlanes, ballBounds);
        }
    }
}
