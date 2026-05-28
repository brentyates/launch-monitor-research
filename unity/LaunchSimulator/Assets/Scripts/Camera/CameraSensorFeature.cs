using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using UnityEngine.Rendering.RenderGraphModule;

namespace LaunchMonitor.Camera
{
    public class CameraSensorFeature : ScriptableRendererFeature
    {
        public static bool DistortionEnabled { get; set; } = false;
        public static float K1 { get; set; } = 0f;
        public static float K2 { get; set; } = 0f;

        public static bool NoiseEnabled { get; set; } = false;
        public static float ShotNoiseScale { get; set; } = 0.05f;
        public static float ReadNoiseScale { get; set; } = 0.02f;

        public static bool ExposureSimEnabled { get; set; } = false;
        public static float ExposureMs { get; set; } = 1.0f;
        public static float SceneBrightness { get; set; } = 1.0f;
        public static float TargetBrightness { get; set; } = 0.5f;
        public static float MaxGainDb { get; set; } = 40f;

        public static bool GrayscaleEnabled { get; set; } = false;
        public static float Contrast { get; set; } = 1.4f;

        [System.Serializable]
        public class Settings
        {
            public RenderPassEvent renderPassEvent = RenderPassEvent.AfterRenderingPostProcessing;
            public Material sensorMaterial;
        }

        public Settings settings = new Settings();
        private CameraSensorPass sensorPass;
        private static int frameCount = 0;

        public override void Create()
        {
            sensorPass = new CameraSensorPass(settings);
            sensorPass.renderPassEvent = settings.renderPassEvent;
        }

        public override void AddRenderPasses(ScriptableRenderer renderer, ref RenderingData renderingData)
        {
            if (!DistortionEnabled && !NoiseEnabled && !GrayscaleEnabled && !ExposureSimEnabled) return;
            if (renderingData.cameraData.cameraType != CameraType.Game) return;

            Debug.Log($"[CameraSensorFeature] Adding pass for {renderingData.cameraData.camera.name}, distortion={DistortionEnabled}, grayscale={GrayscaleEnabled}");

            if (settings.sensorMaterial == null)
            {
                var shader = Shader.Find("Hidden/CameraSensor");
                if (shader != null)
                    settings.sensorMaterial = CoreUtils.CreateEngineMaterial(shader);
            }

            if (settings.sensorMaterial == null) return;

            frameCount++;
            renderer.EnqueuePass(sensorPass);
        }

        protected override void Dispose(bool disposing)
        {
            sensorPass?.Dispose();
        }

        class CameraSensorPass : ScriptableRenderPass
        {
            private Settings settings;
            private const string PassName = "Camera Sensor";

            private class PassData
            {
                public TextureHandle source;
                public Material material;

                public bool distortionEnabled;
                public float k1;
                public float k2;

                public bool noiseEnabled;
                public float shotNoiseScale;
                public float readNoiseScale;
                public float noiseSeed;

                public bool exposureSimEnabled;
                public float exposureMs;
                public float sceneBrightness;
                public float targetBrightness;
                public float maxGainDb;

                public bool grayscaleEnabled;
                public float contrast;
            }

            public CameraSensorPass(Settings settings)
            {
                this.settings = settings;
                requiresIntermediateTexture = true;
            }

            public override void RecordRenderGraph(RenderGraph renderGraph, ContextContainer frameData)
            {
                if (settings.sensorMaterial == null)
                {
                    Debug.LogWarning("[CameraSensorFeature] Sensor material is null, skipping pass");
                    return;
                }

                var resourceData = frameData.Get<UniversalResourceData>();

                if (resourceData.isActiveTargetBackBuffer) return;

                var source = resourceData.activeColorTexture;

                var desc = renderGraph.GetTextureDesc(source);
                desc.name = "_SensorTemp";
                desc.clearBuffer = false;

                var tempTexture = renderGraph.CreateTexture(desc);

                using (var builder = renderGraph.AddRasterRenderPass<PassData>(PassName, out var passData))
                {
                    passData.source = source;
                    passData.material = settings.sensorMaterial;

                    passData.distortionEnabled = DistortionEnabled;
                    passData.k1 = K1;
                    passData.k2 = K2;

                    passData.noiseEnabled = NoiseEnabled;
                    passData.shotNoiseScale = ShotNoiseScale;
                    passData.readNoiseScale = ReadNoiseScale;
                    passData.noiseSeed = Time.frameCount * 0.1f + Random.value * 100f;

                    passData.exposureSimEnabled = ExposureSimEnabled;
                    passData.exposureMs = ExposureMs;
                    passData.sceneBrightness = SceneBrightness;
                    passData.targetBrightness = TargetBrightness;
                    passData.maxGainDb = MaxGainDb;

                    passData.grayscaleEnabled = GrayscaleEnabled;
                    passData.contrast = Contrast;

                    builder.UseTexture(source, AccessFlags.Read);
                    builder.SetRenderAttachment(tempTexture, 0, AccessFlags.Write);

                    builder.SetRenderFunc((PassData data, RasterGraphContext ctx) =>
                    {
                        data.material.SetFloat("_DistortionEnabled", data.distortionEnabled ? 1f : 0f);
                        data.material.SetFloat("_K1", data.k1);
                        data.material.SetFloat("_K2", data.k2);

                        data.material.SetFloat("_NoiseEnabled", data.noiseEnabled ? 1f : 0f);
                        data.material.SetFloat("_ShotNoiseScale", data.shotNoiseScale);
                        data.material.SetFloat("_ReadNoiseScale", data.readNoiseScale);
                        data.material.SetFloat("_NoiseSeed", data.noiseSeed);

                        data.material.SetFloat("_ExposureSimEnabled", data.exposureSimEnabled ? 1f : 0f);
                        data.material.SetFloat("_ExposureMs", data.exposureMs);
                        data.material.SetFloat("_SceneBrightness", data.sceneBrightness);
                        data.material.SetFloat("_TargetBrightness", data.targetBrightness);
                        data.material.SetFloat("_MaxGainDb", data.maxGainDb);

                        data.material.SetFloat("_GrayscaleEnabled", data.grayscaleEnabled ? 1f : 0f);
                        data.material.SetFloat("_Contrast", data.contrast);

                        Blitter.BlitTexture(ctx.cmd, data.source, new Vector4(1, 1, 0, 0), data.material, 0);
                    });
                }

                using (var builder = renderGraph.AddRasterRenderPass<PassData>("Copy Back", out var passData))
                {
                    passData.source = tempTexture;

                    builder.UseTexture(tempTexture, AccessFlags.Read);
                    builder.SetRenderAttachment(source, 0, AccessFlags.Write);

                    builder.SetRenderFunc((PassData data, RasterGraphContext ctx) =>
                    {
                        Blitter.BlitTexture(ctx.cmd, data.source, new Vector4(1, 1, 0, 0), 0, false);
                    });
                }
            }

            public void Dispose()
            {
            }
        }
    }
}
