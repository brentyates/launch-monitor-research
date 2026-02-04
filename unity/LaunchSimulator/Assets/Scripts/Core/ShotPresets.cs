using UnityEngine;
using System;
using System.Collections.Generic;

namespace LaunchMonitor.Core
{
    [Serializable]
    public class ShotPreset
    {
        public string name;
        public LaunchParameters launchParams;
        public BallOrientation orientation;
        public string description;
    }

    [CreateAssetMenu(fileName = "ShotPresets", menuName = "Launch Monitor/Shot Presets")]
    public class ShotPresets : ScriptableObject
    {
        public List<ShotPreset> presets = new List<ShotPreset>();

        public static ShotPreset Driver => new ShotPreset
        {
            name = "Driver",
            description = "Standard driver shot with mid spin",
            launchParams = new LaunchParameters
            {
                speedMph = 165f,
                vlaDeg = 10.5f,
                hlaDeg = 0f,
                spinRpm = 2500f,
                spinAxisDeg = 0f
            },
            orientation = new BallOrientation()
        };

        public static ShotPreset Iron7 => new ShotPreset
        {
            name = "7 Iron",
            description = "Standard 7 iron shot",
            launchParams = new LaunchParameters
            {
                speedMph = 130f,
                vlaDeg = 16f,
                hlaDeg = 0f,
                spinRpm = 7000f,
                spinAxisDeg = 0f
            },
            orientation = new BallOrientation()
        };

        public static ShotPreset Wedge => new ShotPreset
        {
            name = "Wedge",
            description = "Pitching wedge with high spin",
            launchParams = new LaunchParameters
            {
                speedMph = 95f,
                vlaDeg = 25f,
                hlaDeg = 0f,
                spinRpm = 10000f,
                spinAxisDeg = 0f
            },
            orientation = new BallOrientation()
        };

        public static ShotPreset Draw => new ShotPreset
        {
            name = "Draw",
            description = "Driver with draw spin",
            launchParams = new LaunchParameters
            {
                speedMph = 165f,
                vlaDeg = 10.5f,
                hlaDeg = -2f,
                spinRpm = 2800f,
                spinAxisDeg = -15f
            },
            orientation = new BallOrientation()
        };

        public static ShotPreset Fade => new ShotPreset
        {
            name = "Fade",
            description = "Driver with fade spin",
            launchParams = new LaunchParameters
            {
                speedMph = 165f,
                vlaDeg = 10.5f,
                hlaDeg = 2f,
                spinRpm = 2800f,
                spinAxisDeg = 15f
            },
            orientation = new BallOrientation()
        };

        public static ShotPreset LowSpin => new ShotPreset
        {
            name = "Low Spin",
            description = "Test case with minimal spin",
            launchParams = new LaunchParameters
            {
                speedMph = 130f,
                vlaDeg = 12f,
                hlaDeg = 0f,
                spinRpm = 500f,
                spinAxisDeg = 0f
            },
            orientation = new BallOrientation()
        };

        public static ShotPreset HighSpin => new ShotPreset
        {
            name = "High Spin",
            description = "Test case with maximum spin",
            launchParams = new LaunchParameters
            {
                speedMph = 95f,
                vlaDeg = 30f,
                hlaDeg = 0f,
                spinRpm = 14000f,
                spinAxisDeg = 0f
            },
            orientation = new BallOrientation()
        };

        public static List<ShotPreset> DefaultPresets => new List<ShotPreset>
        {
            Driver, Iron7, Wedge, Draw, Fade, LowSpin, HighSpin
        };
    }
}
