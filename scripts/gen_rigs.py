#!/usr/bin/env python3
"""Generate rig definition files under rigs/.

A rig is the first-class unit of experimentation: a named device setup with a
list of explicitly-placed cameras (position + aim + intrinsics) and what it
measures. Cameras are placed anywhere in the world frame (X lateral, Y
downrange, Z up, millimeters). The overhead-converging stereo geometry is
emitted here as one generator; other rigs are placed explicitly.
"""

import json
import math
import os

PROJECT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RIGS_DIR = os.path.join(PROJECT_DIR, "rigs")

PIXEL_PITCH_MM = 0.00508


def overhead_convergence_aim_y(height_mm, forward_mm, focal_mm, pitch_mm, render_h):
    height_m = height_mm / 1000.0
    forward_m = forward_mm / 1000.0
    render_h_mm = render_h * pitch_mm
    eff_fov = 2.0 * math.atan(render_h_mm / (2.0 * focal_mm))
    half_fov = eff_fov / 2.0
    far_edge = 0.075 + 0.025
    horiz = forward_m + far_edge
    angle_to_far = math.atan2(height_m, horiz)
    conv_angle = angle_to_far + half_fov
    conv_z_unity = (height_m / math.tan(conv_angle)) - forward_m
    return -conv_z_unity * 1000.0


def overhead_stereo():
    baseline_mm, height_mm, forward_mm, focal_mm = 350.0, 3048.0, 1092.0, 6.0
    width, height = 512, 384
    aim_y = overhead_convergence_aim_y(height_mm, forward_mm, focal_mm, PIXEL_PITCH_MM, height)
    aim = [0.0, round(aim_y, 3), 0.0]
    cam = lambda cid, x: {
        "id": cid, "role": "position",
        "position_mm": [x, forward_mm, height_mm], "aim_mm": aim,
        "focal_mm": focal_mm, "pixel_pitch_mm": PIXEL_PITCH_MM,
        "width": width, "height": height,
    }
    return {
        "name": "overhead_stereo",
        "measures": ["position"],
        "fps": 240.0,
        "strobe_us": 50.0,
        "samples": 32,
        "cameras": [cam("left", -baseline_mm / 2.0), cam("right", baseline_mm / 2.0)],
    }


def spin_overhead_mono():
    height_mm, forward_mm, focal_mm = 3048.0, 1092.0, 18.0
    aim_y = overhead_convergence_aim_y(height_mm, forward_mm, 6.0, PIXEL_PITCH_MM, 384)
    return {
        "name": "spin_overhead_mono",
        "measures": ["spin"],
        "fps": 480.0,
        "strobe_us": 20.0,
        "samples": 32,
        "cameras": [
            {
                "id": "spin0", "role": "spin",
                "position_mm": [0.0, forward_mm, height_mm], "aim_mm": [0.0, round(aim_y, 3), 0.0],
                "focal_mm": focal_mm, "pixel_pitch_mm": PIXEL_PITCH_MM,
                "width": 512, "height": 384,
            }
        ],
    }


def main():
    os.makedirs(RIGS_DIR, exist_ok=True)
    for rig in (overhead_stereo(), spin_overhead_mono()):
        path = os.path.join(RIGS_DIR, rig["name"] + ".json")
        with open(path, "w") as f:
            json.dump(rig, f, indent=2)
        print("wrote", path)


if __name__ == "__main__":
    main()
