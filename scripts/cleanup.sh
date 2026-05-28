#!/bin/bash

echo "Cleaning up launch monitor processes..."
pkill -f "LaunchSimulator" 2>/dev/null || true
pkill -f "lm-gui" 2>/dev/null || true
rm -f /tmp/LaunchMonitorSharedMemory
echo "Done"
