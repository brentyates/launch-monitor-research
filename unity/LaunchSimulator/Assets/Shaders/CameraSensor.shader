Shader "Hidden/CameraSensor"
{
    SubShader
    {
        Tags { "RenderType"="Opaque" "RenderPipeline"="UniversalPipeline" }
        LOD 100
        ZTest Always ZWrite Off Cull Off

        Pass
        {
            Name "CameraSensor"

            HLSLPROGRAM
            #pragma vertex Vert
            #pragma fragment frag

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.core/Runtime/Utilities/Blit.hlsl"

            // Distortion
            float _DistortionEnabled;
            float _K1;
            float _K2;

            // Noise
            float _NoiseEnabled;
            float _ShotNoiseScale;
            float _ReadNoiseScale;
            float _NoiseSeed;

            // Exposure/Gain simulation
            float _ExposureSimEnabled;
            float _ExposureMs;
            float _SceneBrightness;
            float _TargetBrightness;
            float _MaxGainDb;

            // IR / Grayscale
            float _GrayscaleEnabled;
            float _Contrast;

            // Hash function for random numbers
            float Hash(float2 p)
            {
                float3 p3 = frac(float3(p.xyx) * 0.1031);
                p3 += dot(p3, p3.yzx + 33.33);
                return frac((p3.x + p3.y) * p3.z);
            }

            // Box-Muller transform: convert uniform random to Gaussian
            float2 GaussianRandom(float2 uv, float seed)
            {
                float u1 = Hash(uv + float2(seed, 0.0));
                float u2 = Hash(uv + float2(0.0, seed + 1.7));

                // Clamp to avoid log(0)
                u1 = max(u1, 1e-6);

                float r = sqrt(-2.0 * log(u1));
                float theta = 2.0 * 3.14159265 * u2;

                return float2(r * cos(theta), r * sin(theta));
            }

            half4 frag(Varyings input) : SV_Target
            {
                float2 uv = input.texcoord;

                // === Lens Distortion ===
                if (_DistortionEnabled > 0.5)
                {
                    float2 center = float2(0.5, 0.5);
                    float2 centered = (uv - center);

                    float r2 = dot(centered, centered);
                    float r4 = r2 * r2;

                    float radialFactor = 1.0 + _K1 * r2 + _K2 * r4;
                    uv = centered * radialFactor + center;

                    // Black outside valid range
                    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0)
                    {
                        return half4(0, 0, 0, 1);
                    }
                }

                // Sample the scene
                half4 col = SAMPLE_TEXTURE2D(_BlitTexture, sampler_LinearClamp, uv);

                // === Exposure/Gain Simulation ===
                float gainLinear = 1.0;

                if (_ExposureSimEnabled > 0.5)
                {
                    // Reference: 1ms exposure at brightness 1.0 = properly exposed
                    float referenceExposure = 1.0;

                    // Signal level based on scene brightness and exposure time
                    float signalLevel = _SceneBrightness * (_ExposureMs / referenceExposure);

                    // Clamp to avoid division by zero
                    signalLevel = max(signalLevel, 0.001);

                    // Calculate gain needed to reach target brightness
                    if (signalLevel < _TargetBrightness)
                    {
                        gainLinear = _TargetBrightness / signalLevel;

                        // Cap at maximum gain (convert dB to linear)
                        float maxGainLinear = pow(10.0, _MaxGainDb / 20.0);
                        gainLinear = min(gainLinear, maxGainLinear);
                    }

                    // Apply exposure darkening then gain
                    col.rgb = col.rgb * signalLevel * gainLinear;
                }

                // === Sensor Noise ===
                if (_NoiseEnabled > 0.5)
                {
                    float2 pixelCoord = uv * _ScreenParams.xy;
                    float2 gauss = GaussianRandom(pixelCoord, _NoiseSeed);

                    // Compute luminance for signal-dependent noise
                    float luminance = dot(col.rgb, float3(0.299, 0.587, 0.114));

                    // Shot noise: σ proportional to sqrt(signal)
                    // Read noise: constant σ (but gets amplified by gain!)
                    float shotSigma = sqrt(max(luminance, 0.0)) * _ShotNoiseScale;
                    float readSigma = _ReadNoiseScale * gainLinear; // Key: read noise amplified by gain

                    // Combined noise
                    float3 noise = gauss.x * shotSigma + gauss.y * readSigma;

                    col.rgb = saturate(col.rgb + noise);
                }

                // === Grayscale / IR Mode ===
                if (_GrayscaleEnabled > 0.5)
                {
                    float luminance = dot(col.rgb, float3(0.299, 0.587, 0.114));
                    luminance = saturate((luminance - 0.5) * _Contrast + 0.5);
                    col.rgb = luminance;
                }

                return col;
            }
            ENDHLSL
        }
    }
    Fallback Off
}
