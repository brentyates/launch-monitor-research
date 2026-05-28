#!/bin/bash
set -e

UNITY_PATH="/Applications/Unity/Hub/Editor/6000.4.3f1/Unity.app/Contents/MacOS/Unity"
PROJECT_PATH="/Users/byates/projects/launch-monitor-research/unity/LaunchSimulator"

echo "Building Unity project..."

"$UNITY_PATH" \
  -quit \
  -batchmode \
  -projectPath "$PROJECT_PATH" \
  -buildTarget StandaloneOSX \
  -executeMethod BuildScript.BuildMacOS \
  -logFile -

echo "Build complete!"
