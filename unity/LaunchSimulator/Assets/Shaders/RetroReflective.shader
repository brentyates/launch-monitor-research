Shader "LaunchMonitor/RetroReflective"
{
    Properties
    {
        _BaseColor ("Base Color", Color) = (0.1, 0.1, 0.1, 1)
        _RetroColor ("Retro-Reflective Color", Color) = (1, 1, 1, 1)
        _RetroIntensity ("Retro Intensity", Range(0, 50)) = 20
        _RetroSharpness ("Retro Sharpness", Range(0.9, 1.0)) = 0.98
        _Smoothness ("Smoothness", Range(0, 1)) = 0.3
    }
    SubShader
    {
        Tags
        {
            "RenderType" = "Opaque"
            "RenderPipeline" = "UniversalPipeline"
            "Queue" = "Geometry"
        }
        LOD 100

        Pass
        {
            Name "ForwardLit"
            Tags { "LightMode" = "UniversalForward" }

            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #pragma multi_compile _ _MAIN_LIGHT_SHADOWS
            #pragma multi_compile _ _ADDITIONAL_LIGHTS
            #pragma multi_compile_fragment _ _SHADOWS_SOFT

            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Lighting.hlsl"

            struct Attributes
            {
                float4 positionOS : POSITION;
                float3 normalOS : NORMAL;
            };

            struct Varyings
            {
                float4 positionCS : SV_POSITION;
                float3 positionWS : TEXCOORD0;
                float3 normalWS : TEXCOORD1;
            };

            CBUFFER_START(UnityPerMaterial)
                float4 _BaseColor;
                float4 _RetroColor;
                float _RetroIntensity;
                float _RetroSharpness;
                float _Smoothness;
            CBUFFER_END

            Varyings vert(Attributes input)
            {
                Varyings output;
                output.positionWS = TransformObjectToWorld(input.positionOS.xyz);
                output.positionCS = TransformWorldToHClip(output.positionWS);
                output.normalWS = TransformObjectToWorldNormal(input.normalOS);
                return output;
            }

            half4 frag(Varyings input) : SV_Target
            {
                float3 normalWS = normalize(input.normalWS);
                float3 viewDirWS = normalize(GetCameraPositionWS() - input.positionWS);

                half3 finalColor = _BaseColor.rgb * 0.1;

                Light mainLight = GetMainLight();
                float3 lightDirWS = normalize(mainLight.direction);

                float retroMain = saturate(dot(viewDirWS, reflect(-lightDirWS, normalWS)));
                retroMain = smoothstep(_RetroSharpness, 1.0, retroMain);

                float facingCamera = saturate(dot(normalWS, viewDirWS));
                facingCamera = pow(facingCamera, 2);

                float nDotL = saturate(dot(normalWS, lightDirWS));
                finalColor += mainLight.color * _BaseColor.rgb * nDotL * 0.3;
                finalColor += mainLight.color * _RetroColor.rgb * retroMain * _RetroIntensity * facingCamera;

                #ifdef _ADDITIONAL_LIGHTS
                uint additionalLightsCount = GetAdditionalLightsCount();
                for (uint i = 0; i < additionalLightsCount; i++)
                {
                    Light light = GetAdditionalLight(i, input.positionWS);
                    float3 additionalLightDir = normalize(light.direction);

                    float retroAdd = saturate(dot(viewDirWS, reflect(-additionalLightDir, normalWS)));
                    retroAdd = smoothstep(_RetroSharpness, 1.0, retroAdd);

                    float nDotLAdd = saturate(dot(normalWS, additionalLightDir));
                    float attenuation = light.distanceAttenuation * light.shadowAttenuation;

                    finalColor += light.color * _BaseColor.rgb * nDotLAdd * attenuation * 0.3;
                    finalColor += light.color * _RetroColor.rgb * retroAdd * _RetroIntensity * attenuation * facingCamera;
                }
                #endif

                return half4(finalColor, 1.0);
            }
            ENDHLSL
        }

        Pass
        {
            Name "ShadowCaster"
            Tags { "LightMode" = "ShadowCaster" }

            ZWrite On
            ZTest LEqual
            ColorMask 0

            HLSLPROGRAM
            #pragma vertex ShadowPassVertex
            #pragma fragment ShadowPassFragment
            #include "Packages/com.unity.render-pipelines.universal/Shaders/ShadowCasterPass.hlsl"
            ENDHLSL
        }
    }
}
