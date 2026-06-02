import sys
import os
import json
import math
import random
import argparse

import bpy
from mathutils import Matrix, Vector, Quaternion

FOCAL_MM = 18.0
PIXEL_PITCH_MM = 0.00508
BALL_RADIUS_MM = 21.335
MPH_TO_MM_S = 447.04
BALL_MESH_RADIUS = 2.0
CAM_POS = (0.0, 1092.0, 3048.0)
CAM_AIM = (0.0, 437.116, 0.0)
WIDTH = 512
HEIGHT = 384
BALL_BLEND = "/Users/byates/projects/launch-monitor-research/blender/tp5_pix_ball.blend"


def look_at_rows(eye, target):
    fwd = (target - eye).normalized()
    world_up = Vector((0.0, 1.0, 0.0)) if abs(fwd.z) > 0.999 else Vector((0.0, 0.0, 1.0))
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


def project(pos_mm, focal_mm):
    eye = Vector(CAM_POS)
    right, down, fwd = look_at_rows(eye, Vector(CAM_AIM))
    rel = Vector(pos_mm) - eye
    xc = right.dot(rel); yc = down.dot(rel); zc = fwd.dot(rel)
    if zc <= 1e-6:
        return None
    f_px = focal_mm / PIXEL_PITCH_MM
    u = f_px * xc / zc + WIDTH / 2.0
    v = f_px * yc / zc + HEIGHT / 2.0
    ball_px = 2.0 * f_px * BALL_RADIUS_MM / rel.length
    return u, v, ball_px


def parse_args():
    argv = sys.argv[sys.argv.index("--") + 1:]
    p = argparse.ArgumentParser()
    p.add_argument("--out", required=True)
    p.add_argument("--shots", type=int, required=True)
    p.add_argument("--seed-base", type=int, default=0)
    p.add_argument("--fps-min", type=float, default=500.0)
    p.add_argument("--fps-max", type=float, default=1000.0)
    p.add_argument("--focal-min", type=float, default=16.0)
    p.add_argument("--focal-max", type=float, default=28.0)
    p.add_argument("--rpm-min", type=float, default=1500.0)
    p.add_argument("--rpm-max", type=float, default=11000.0)
    p.add_argument("--samples", type=int, default=16)
    p.add_argument("--max-frames", type=int, default=12)
    return p.parse_args(argv)


def clear_scene():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    for block in (bpy.data.meshes, bpy.data.cameras, bpy.data.lights):
        for item in list(block):
            block.remove(item)


def setup_scene(scene, samples):
    try:
        scene.render.engine = 'BLENDER_EEVEE_NEXT'
    except Exception:
        scene.render.engine = 'BLENDER_EEVEE'
    try:
        scene.eevee.taa_render_samples = max(samples, 8)
    except Exception:
        pass
    scene.view_settings.view_transform = 'Standard'
    scene.render.image_settings.file_format = 'PNG'
    scene.render.image_settings.color_mode = 'RGB'
    scene.render.image_settings.color_depth = '8'
    scene.render.resolution_x = WIDTH
    scene.render.resolution_y = HEIGHT
    scene.render.resolution_percentage = 100
    scene.render.pixel_aspect_x = 1.0
    scene.render.pixel_aspect_y = 1.0
    scene.render.use_motion_blur = False


def make_camera(scene):
    cam_data = bpy.data.cameras.new("spin")
    cam_data.sensor_fit = 'HORIZONTAL'
    cam_data.sensor_width = WIDTH * PIXEL_PITCH_MM
    cam_data.clip_start = 0.01
    cam_data.clip_end = 1000.0
    cam = bpy.data.objects.new("spin", cam_data)
    cam.matrix_world = cam_matrix_world(CAM_POS, CAM_AIM)
    scene.collection.objects.link(cam)
    scene.camera = cam
    return cam


def load_ball(scene):
    with bpy.data.libraries.load(BALL_BLEND, link=False) as (src, dst):
        dst.objects = list(src.objects)
    appended = [o for o in dst.objects if o is not None]
    for obj in appended:
        scene.collection.objects.link(obj)
    root = bpy.data.objects.new("BallRoot", None)
    scene.collection.objects.link(root)
    for obj in appended:
        if obj.parent is None:
            obj.parent = root
            obj.matrix_parent_inverse = Matrix.Identity(4)
    s = (BALL_RADIUS_MM / 1000.0) / BALL_MESH_RADIUS
    root.scale = (s, s, s)
    root.rotation_mode = 'QUATERNION'
    return root


def make_turf(scene):
    bpy.ops.mesh.primitive_plane_add(size=1.2, location=(0.0, 0.0, 0.0))
    turf = bpy.context.active_object
    mat = bpy.data.materials.new("Turf")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (0.012, 0.025, 0.016, 1.0)
    bsdf.inputs["Roughness"].default_value = 0.95
    turf.data.materials.append(mat)


def make_lights(scene):
    sun_data = bpy.data.lights.new("Sun", type='SUN')
    sun_data.energy = 2.5
    sun_data.angle = 0.0
    sun = bpy.data.objects.new("Sun", sun_data)
    scene.collection.objects.link(sun)
    world = scene.world
    if world is None:
        world = bpy.data.worlds.new("World")
        scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg is not None:
        bg.inputs["Color"].default_value = (0.002, 0.002, 0.002, 1.0)


def main():
    args = parse_args()
    os.makedirs(os.path.join(args.out, "raw"), exist_ok=True)
    labels_path = os.path.join(args.out, "labels.jsonl")

    scene = bpy.context.scene
    clear_scene()
    setup_scene(scene, args.samples)
    cam = make_camera(scene)
    ball = load_ball(scene)
    make_turf(scene)
    make_lights(scene)

    rng = random.Random(args.seed_base)
    written = 0
    with open(labels_path, "a") as lf:
        for shot in range(args.shots):
            sid = args.seed_base + shot
            rpm = rng.uniform(args.rpm_min, args.rpm_max)
            axis_deg = rng.uniform(-20.0, 20.0)
            speed = rng.uniform(70.0, 180.0)
            vla = rng.uniform(5.0, 35.0)
            hla = rng.uniform(-5.0, 5.0)
            fps = rng.uniform(args.fps_min, args.fps_max)
            focal = rng.uniform(args.focal_min, args.focal_max)
            cam.data.lens = focal
            scene.cycles.seed = rng.randint(0, 1_000_000)

            speed_mm_s = speed * MPH_TO_MM_S
            vr = math.radians(vla); hr = math.radians(hla)
            vplane = speed_mm_s * math.cos(vr)
            v = (vplane * math.sin(hr), vplane * math.cos(hr), speed_mm_s * math.sin(vr))
            p0 = (0.0, 0.0, BALL_RADIUS_MM)
            omega = rpm * 2.0 * math.pi / 60.0
            ar = math.radians(axis_deg)
            spin_axis = Vector((-math.cos(ar), -math.sin(ar), 0.0)).normalized()

            visible = []
            i = 0
            seen = False
            while len(visible) < args.max_frames:
                t = i / fps
                pos = (p0[0] + v[0] * t, p0[1] + v[1] * t, p0[2] + v[2] * t)
                pr = project(pos, focal)
                ok = pr is not None and 2 <= pr[0] <= WIDTH - 2 and 2 <= pr[1] <= HEIGHT - 2
                if ok:
                    seen = True
                    visible.append((i, pos, pr))
                elif seen:
                    break
                i += 1
                if i > 100000:
                    break

            if len(visible) < 2:
                continue

            shot_dir = os.path.join(args.out, "raw", "%06d" % sid)
            os.makedirs(shot_dir, exist_ok=True)
            frames = []
            for idx, (orig_i, pos, pr) in enumerate(visible):
                t = orig_i / fps
                ball.location = (pos[0] / 1000.0, pos[1] / 1000.0, pos[2] / 1000.0)
                ball.rotation_quaternion = Quaternion(spin_axis, omega * t)
                fname = "frame_%04d.png" % idx
                scene.render.filepath = os.path.join(shot_dir, fname)
                bpy.ops.render.render(write_still=True)
                frames.append({"file": "%06d/%s" % (sid, fname), "u": pr[0], "v": pr[1], "ball_px": pr[2]})

            lf.write(json.dumps({
                "id": sid, "rpm": rpm, "axis_deg": axis_deg, "fps": fps,
                "focal_mm": focal, "speed_mph": speed, "vla_deg": vla, "hla_deg": hla,
                "frames": frames,
            }) + "\n")
            lf.flush()
            written += 1
            if written % 25 == 0:
                print("gen_dataset: %d/%d shots written" % (written, args.shots))

    print("gen_dataset: done, %d shots" % written)


if __name__ == "__main__":
    main()
