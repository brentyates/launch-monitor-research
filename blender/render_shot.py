import sys
import os
import json
import math
import argparse

import bpy
from mathutils import Matrix, Vector, Quaternion

FOCAL_MM = 6.0
PIXEL_PITCH_MM = 0.00508
BASELINE_MM = 350.0
HEIGHT_MM = 3048.0
FORWARD_MM = 1092.0
BALL_RADIUS_MM = 21.335
MPH_TO_MM_S = 447.04
BALL_MESH_RADIUS = 2.0

BALL_BLEND = "/Users/byates/projects/launch-monitor-research/blender/tp5_pix_ball.blend"


def compute_convergence(render_h):
    # Ported verbatim from src/config.rs StereoRig::compute_convergence
    height_m = HEIGHT_MM / 1000.0
    forward_m = FORWARD_MM / 1000.0
    render_h_mm = render_h * PIXEL_PITCH_MM
    eff_fov = 2.0 * math.atan(render_h_mm / (2.0 * FOCAL_MM))
    half_fov = eff_fov / 2.0
    hitting_half = 0.075
    back_pad = 0.025
    far_edge = hitting_half + back_pad
    horiz = forward_m + far_edge
    angle_to_far = math.atan2(height_m, horiz)
    conv_angle = angle_to_far + half_fov
    conv_z_unity = (height_m / math.tan(conv_angle)) - forward_m
    conv_y_mm = -conv_z_unity * 1000.0
    return Vector((0.0, conv_y_mm, 0.0))


def look_at_rows(eye, target):
    # Ported verbatim from src/config.rs StereoRig::look_at (world_up = +Z)
    fwd = (target - eye).normalized()
    world_up = Vector((0.0, 0.0, 1.0))
    right = world_up.cross(fwd).normalized()
    down = right.cross(fwd)
    return right, down, fwd


def cam_matrix_world(eye_mm, target_mm):
    eye = Vector(eye_mm)
    target = Vector(target_mm)
    right, down, fwd = look_at_rows(eye, target)
    R = Matrix((
        (right.x, -down.x, -fwd.x),
        (right.y, -down.y, -fwd.y),
        (right.z, -down.z, -fwd.z),
    ))
    M = R.to_4x4()
    M.translation = eye / 1000.0
    return M


def project_cv(pos_mm, eye_mm, target_mm, width, height):
    right, down, fwd = look_at_rows(Vector(eye_mm), Vector(target_mm))
    rel = Vector(pos_mm) - Vector(eye_mm)
    xc = right.dot(rel); yc = down.dot(rel); zc = fwd.dot(rel)
    if zc <= 1e-6:
        return None
    f_px = FOCAL_MM / PIXEL_PITCH_MM
    u = f_px * xc / zc + width / 2.0
    v = f_px * yc / zc + height / 2.0
    return (u, v)


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:]
    p = argparse.ArgumentParser()
    p.add_argument("--case", required=True)
    p.add_argument("--speed", type=float, required=True)
    p.add_argument("--vla", type=float, required=True)
    p.add_argument("--hla", type=float, required=True)
    p.add_argument("--spin", type=float, required=True)
    p.add_argument("--axis", type=float, required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--width", type=int, required=True)
    p.add_argument("--height", type=int, required=True)
    p.add_argument("--fps", type=float, required=True)
    p.add_argument("--frames", type=int, required=True)
    p.add_argument("--samples", type=int, required=True)
    p.add_argument("--baseline-mm", type=float, default=BASELINE_MM)
    p.add_argument("--mount-height-mm", type=float, default=HEIGHT_MM)
    p.add_argument("--forward-mm", type=float, default=FORWARD_MM)
    p.add_argument("--focal-mm", type=float, default=FOCAL_MM)
    p.add_argument("--pixel-pitch-mm", type=float, default=PIXEL_PITCH_MM)
    p.add_argument("--exposure-us", type=float, default=0.0,
                   help="exposure time in microseconds; 0 = no motion blur (instantaneous)")
    return p.parse_args(argv)


def clear_scene():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    for block in (bpy.data.meshes, bpy.data.cameras, bpy.data.lights):
        for item in list(block):
            block.remove(item)


def setup_render(scene, args):
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = args.samples
    try:
        prefs = bpy.context.preferences.addons['cycles'].preferences
        prefs.compute_device_type = 'METAL'
        prefs.get_devices()
        for device in prefs.devices:
            device.use = True
        scene.cycles.device = 'GPU'
    except Exception:
        scene.cycles.device = 'CPU'

    scene.view_settings.view_transform = 'Standard'
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGB'
    scene.render.image_settings.color_depth = '8'

    scene.render.resolution_x = args.width
    scene.render.resolution_y = args.height
    scene.render.resolution_percentage = 100
    scene.render.pixel_aspect_x = 1.0
    scene.render.pixel_aspect_y = 1.0


def make_camera(name, eye_mm, target_mm, width):
    cam_data = bpy.data.cameras.new(name)
    cam_data.lens = FOCAL_MM
    cam_data.sensor_fit = 'HORIZONTAL'
    cam_data.sensor_width = width * PIXEL_PITCH_MM
    cam_data.clip_start = 0.01
    cam_data.clip_end = 1000.0
    cam_obj = bpy.data.objects.new(name, cam_data)
    cam_obj.matrix_world = cam_matrix_world(eye_mm, target_mm)
    bpy.context.scene.collection.objects.link(cam_obj)
    return cam_obj


def load_ball(scene):
    with bpy.data.libraries.load(BALL_BLEND, link=False) as (src, dst):
        dst.objects = list(src.objects)

    appended = [o for o in dst.objects if o is not None]
    for obj in appended:
        scene.collection.objects.link(obj)
        try:
            obj.visible_shadow = False
        except Exception:
            pass

    ball_root = bpy.data.objects.new("BallRoot", None)
    scene.collection.objects.link(ball_root)
    ball_root.location = (0.0, 0.0, 0.0)

    for obj in appended:
        if obj.parent is None:
            obj.parent = ball_root
            obj.matrix_parent_inverse = Matrix.Identity(4)

    s = (BALL_RADIUS_MM / 1000.0) / BALL_MESH_RADIUS
    ball_root.scale = (s, s, s)
    ball_root.rotation_mode = 'QUATERNION'
    return ball_root


def make_turf(scene):
    bpy.ops.mesh.primitive_plane_add(size=1.2, location=(0.0, 0.0, 0.0))
    turf = bpy.context.active_object
    turf.name = "Turf"
    mat = bpy.data.materials.new("Turf")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (0.012, 0.025, 0.016, 1.0)
    bsdf.inputs["Roughness"].default_value = 0.95
    turf.data.materials.append(mat)
    return turf


def make_lights(scene):
    sun_data = bpy.data.lights.new("Sun", type='SUN')
    sun_data.energy = 2.5
    sun_data.angle = 0.0
    sun = bpy.data.objects.new("Sun", sun_data)
    sun.rotation_euler = (0.0, 0.0, 0.0)
    scene.collection.objects.link(sun)

    world = scene.world
    if world is None:
        world = bpy.data.worlds.new("World")
        scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg is not None:
        bg.inputs["Color"].default_value = (0.002, 0.002, 0.002, 1.0)


def action_fcurves(obj):
    ad = obj.animation_data
    if not ad or not ad.action:
        return []
    act = ad.action
    fcs = getattr(act, "fcurves", None)
    if fcs:
        return list(fcs)
    out = []
    for layer in getattr(act, "layers", []):
        for strip in getattr(layer, "strips", []):
            for cb in getattr(strip, "channelbags", []):
                out.extend(list(cb.fcurves))
    return out


def main():
    try:
        args = parse_args()

        global FOCAL_MM, PIXEL_PITCH_MM, BASELINE_MM, HEIGHT_MM, FORWARD_MM
        FOCAL_MM = args.focal_mm
        PIXEL_PITCH_MM = args.pixel_pitch_mm
        BASELINE_MM = args.baseline_mm
        HEIGHT_MM = args.mount_height_mm
        FORWARD_MM = args.forward_mm

        os.makedirs(args.out, exist_ok=True)

        scene = bpy.context.scene
        clear_scene()
        setup_render(scene, args)

        conv = compute_convergence(args.height)
        eye0 = (-BASELINE_MM / 2.0, FORWARD_MM, HEIGHT_MM)
        eye1 = (BASELINE_MM / 2.0, FORWARD_MM, HEIGHT_MM)
        cam0 = make_camera("CamLeft", eye0, conv, args.width)
        cam1 = make_camera("CamRight", eye1, conv, args.width)

        ball_root = load_ball(scene)
        make_turf(scene)
        make_lights(scene)

        exposure_fraction = 0.0
        if args.exposure_us > 0.0:
            exposure_fraction = min(args.exposure_us * 1.0e-6 * args.fps, 1.0)
            scene.render.use_motion_blur = True
            scene.render.motion_blur_shutter = exposure_fraction
            try:
                scene.render.motion_blur_position = 'CENTER'
            except Exception:
                pass
        else:
            scene.render.use_motion_blur = False

        speed_mm_s = args.speed * MPH_TO_MM_S
        vla = math.radians(args.vla)
        hla = math.radians(args.hla)
        vplane = speed_mm_s * math.cos(vla)
        v = (
            vplane * math.sin(hla),
            vplane * math.cos(hla),
            speed_mm_s * math.sin(vla),
        )
        p0 = (0.0, 0.0, BALL_RADIUS_MM)
        omega = args.spin * 2.0 * math.pi / 60.0
        axis_rad = math.radians(args.axis)
        spin_axis = Vector((-math.cos(axis_rad), -math.sin(axis_rad), 0.0)).normalized()

        def visible(pos_mm):
            for eye, target in ((eye0, conv), (eye1, conv)):
                proj = project_cv(pos_mm, eye, target, args.width, args.height)
                if proj is None:
                    return False
                u, w = proj
                if not (2 <= u <= args.width - 2 and 2 <= w <= args.height - 2):
                    return False
            return True

        emitted = []
        seen_visible = False
        i = 0
        while len(emitted) < args.frames:
            t = i / args.fps
            pos_mm = (
                p0[0] + v[0] * t,
                p0[1] + v[1] * t,
                p0[2] + v[2] * t,
            )
            vis = visible(pos_mm)
            if vis:
                seen_visible = True
                emitted.append((i, pos_mm))
            elif seen_visible:
                break
            i += 1
            if i > 100000:
                break

        ball_root.rotation_mode = 'QUATERNION'
        for index, (orig_i, pos_mm) in enumerate(emitted):
            phys_t = orig_i / args.fps
            ball_root.location = (pos_mm[0] / 1000.0, pos_mm[1] / 1000.0, pos_mm[2] / 1000.0)
            ball_root.rotation_quaternion = Quaternion(spin_axis, omega * phys_t)
            ball_root.keyframe_insert(data_path="location", frame=index)
            ball_root.keyframe_insert(data_path="rotation_quaternion", frame=index)

        for fc in action_fcurves(ball_root):
            fc.extrapolation = 'LINEAR'
            for kp in fc.keyframe_points:
                kp.interpolation = 'LINEAR'

        scene.frame_start = 0
        scene.frame_end = max(0, len(emitted) - 1)

        manifest_frames = []
        for index, (_, pos_mm) in enumerate(emitted):
            scene.frame_set(index)
            t = index / args.fps
            entry = {
                "index": index,
                "timestamp": t,
                "ball_pos_mm": [pos_mm[0], pos_mm[1], pos_mm[2]],
                "ball_vel_mm_s": [v[0], v[1], v[2]],
            }
            for cam, side in ((cam0, "left"), (cam1, "right")):
                scene.camera = cam
                fname = "frame_%04d_%s.png" % (index, side)
                scene.render.filepath = os.path.join(args.out, fname)
                bpy.ops.render.render(write_still=True)
                entry[side] = fname
            manifest_frames.append(entry)

        manifest = {
            "case": args.case,
            "width": args.width,
            "height": args.height,
            "fps": float(args.fps),
            "ground_truth": {
                "speed_mph": args.speed,
                "vla_deg": args.vla,
                "hla_deg": args.hla,
                "spin_rpm": args.spin,
                "spin_axis_deg": args.axis,
            },
            "rig": {
                "baseline_mm": BASELINE_MM,
                "height_mm": HEIGHT_MM,
                "forward_mm": FORWARD_MM,
                "focal_mm": FOCAL_MM,
                "pixel_pitch_mm": PIXEL_PITCH_MM,
            },
            "sensor": {
                "shutter_type": "global",
                "exposure_us": args.exposure_us,
                "motion_blur_shutter": exposure_fraction,
            },
            "frames": manifest_frames,
        }
        with open(os.path.join(args.out, "manifest.json"), "w") as f:
            json.dump(manifest, f, indent=2)

        print("render_shot: emitted %d frames to %s" % (len(manifest_frames), args.out))
    except Exception as e:
        sys.stderr.write("render_shot fatal: %s\n" % e)
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
