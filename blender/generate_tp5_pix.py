#!/usr/bin/env python3
"""
Golf ball with proper hexagonal dimples.

Technique (from Geometry Nodes tutorials):
1. Icosphere (triangles)
2. DUAL MESH - converts triangles to hexagons/pentagons
3. Inset faces
4. Extrude inward
5. Subdivision surface

The dual mesh is the key - it creates the hexagonal pattern seen on real golf balls.
"""

import bpy
import bmesh
import math
from mathutils import Vector
import os

BALL_RADIUS = 2.0
TEXTURE_SIZE = 2048

COLOR_WHITE = (1.0, 1.0, 1.0, 1.0)


def clear_scene():
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.object.delete(use_global=False)
    for block in bpy.data.meshes:
        if block.users == 0:
            bpy.data.meshes.remove(block)
    for block in bpy.data.materials:
        if block.users == 0:
            bpy.data.materials.remove(block)


def create_material_with_texture(name, texture_path, roughness=0.35):
    """Create material with image texture."""
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links

    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = roughness

    # Add image texture node
    tex_node = nodes.new('ShaderNodeTexImage')
    tex_node.location = (-300, 300)
    img = bpy.data.images.load(texture_path)
    tex_node.image = img

    # Connect texture to base color
    links.new(tex_node.outputs['Color'], bsdf.inputs['Base Color'])

    return mat


def create_material(name, color, roughness=0.35):
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = roughness
    return mat


def create_logo_geometry(center, pole_dir, parent, black_mat, orange_mat, white_mat):
    """Create a TP5 Pix logo - diamond within diamond.

    Both outer border and inner center are diamonds (rotated 45°).
    Border split diagonally:
    - Black: left arm + bottom arm
    - Orange: top arm + right arm
    - White diamond center with black dots
    """
    normal = center.normalized()

    tangent = (pole_dir - normal * pole_dir.dot(normal)).normalized()
    bitangent = normal.cross(tangent).normalized()

    offset = BALL_RADIUS * 1.003

    outer_size = 0.19    # outer diamond tip distance
    inner_size = 0.07    # inner diamond tip distance
    gap = 0.008          # gap between border and center

    def pt(t, b):
        p = center + tangent * t + bitangent * b
        return list(p.normalized() * offset)

    # Outer diamond tips (pointing up/down/left/right)
    oTop = pt(outer_size, 0)
    oBottom = pt(-outer_size, 0)
    oLeft = pt(0, -outer_size)
    oRight = pt(0, outer_size)

    # Inner diamond tips (with gap)
    g = inner_size + gap
    iTop = pt(g, 0)
    iBottom = pt(-g, 0)
    iLeft = pt(0, -g)
    iRight = pt(0, g)

    def create_mesh(name, verts, faces, material):
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata(verts, [], faces)
        mesh.update()
        obj = bpy.data.objects.new(name, mesh)
        obj.data.materials.append(material)
        obj.parent = parent
        bpy.context.collection.objects.link(obj)
        return obj

    # Border is 4 quad sections between outer and inner diamonds:
    # Section A: oTop -> oRight -> iRight -> iTop (top-right edge)
    # Section B: oRight -> oBottom -> iBottom -> iRight (bottom-right edge)
    # Section C: oBottom -> oLeft -> iLeft -> iBottom (bottom-left edge)
    # Section D: oLeft -> oTop -> iTop -> iLeft (top-left edge)

    # BLACK: top-left (D) + top-right (A) - TOP half, back-to-back
    black_verts = [
        # Section D (top-left edge)
        oLeft, oTop, iTop,
        oLeft, iTop, iLeft,
        # Section A (top-right edge)
        oTop, oRight, iRight,
        oTop, iRight, iTop,
    ]
    black_faces = [(0,1,2), (3,4,5), (6,7,8), (9,10,11)]
    create_mesh("logo_black", black_verts, black_faces, black_mat)

    # ORANGE: bottom-left (C) + bottom-right (B) - BOTTOM half, back-to-back
    orange_verts = [
        # Section C (bottom-left edge)
        oBottom, oLeft, iLeft,
        oBottom, iLeft, iBottom,
        # Section B (bottom-right edge)
        oRight, oBottom, iBottom,
        oRight, iBottom, iRight,
    ]
    orange_faces = [(0,1,2), (3,4,5), (6,7,8), (9,10,11)]
    create_mesh("logo_orange", orange_verts, orange_faces, orange_mat)

    # WHITE center diamond
    c = inner_size
    white_verts = [pt(c, 0), pt(0, c), pt(-c, 0), pt(0, -c)]
    white_faces = [(0,1,2), (0,2,3)]
    create_mesh("logo_center", white_verts, white_faces, white_mat)

    # Black dots in center (arranged in diamond pattern)
    dot_radius = 0.006
    dot_spacing = 0.016
    for row in range(-3, 4):
        for col in range(-3, 4):
            dt, db = row * dot_spacing, col * dot_spacing
            # Check if inside diamond (|t| + |b| < size)
            if abs(dt) + abs(db) < inner_size - dot_radius * 2:
                dot_verts = [pt(dt, db)]
                for i in range(6):
                    a = i * math.pi / 3
                    dot_verts.append(pt(dt + math.cos(a) * dot_radius, db + math.sin(a) * dot_radius))
                dot_faces = [(0,1,2), (0,2,3), (0,3,4), (0,4,5), (0,5,6), (0,6,1)]
                create_mesh("logo_dot", dot_verts, dot_faces, black_mat)


def add_all_logos(ball):
    """Add all 14 diamond logos to the ball."""
    # Create materials
    black_mat = create_material("LogoBlack", (0.15, 0.15, 0.15, 1.0))
    orange_mat = create_material("LogoOrange", (0.9, 0.36, 0.0, 1.0))
    white_mat = create_material("LogoWhite", (1.0, 1.0, 1.0, 1.0))

    # Latitude angle from ball_viewer: asin(150/296) ≈ 30.5 degrees
    lat_angle = math.asin(150/296)
    circle_z_upper = math.sin(lat_angle)
    circle_z_lower = -math.sin(lat_angle)
    circle_radius = math.cos(lat_angle)

    # Upper latitude circle: 6 logos
    for i in range(6):
        theta = math.radians(i * 60)
        center = Vector((
            circle_radius * math.cos(theta),
            circle_radius * math.sin(theta),
            circle_z_upper
        )).normalized()
        create_logo_geometry(center, Vector((0, 0, 1)), ball, black_mat, orange_mat, white_mat)

    # Lower latitude circle: 6 logos offset 30°
    for i in range(6):
        theta = math.radians(30 + i * 60)
        center = Vector((
            circle_radius * math.cos(theta),
            circle_radius * math.sin(theta),
            circle_z_lower
        )).normalized()
        create_logo_geometry(center, Vector((0, 0, -1)), ball, black_mat, orange_mat, white_mat)

    # Pole logos
    create_logo_geometry(Vector((0, 0, 1)), Vector((1, 0, 0)), ball, black_mat, orange_mat, white_mat)
    create_logo_geometry(Vector((0, 0, -1)), Vector((-1, 0, 0)), ball, black_mat, orange_mat, white_mat)


def dual_mesh(bm):
    """
    Convert mesh to its dual.
    Each face becomes a vertex, each vertex becomes a face.
    Triangles → hexagons/pentagons
    """
    bm.verts.ensure_lookup_table()
    bm.faces.ensure_lookup_table()
    bm.edges.ensure_lookup_table()

    # Store face centers (will become new vertices)
    face_centers = {f.index: f.calc_center_median().copy() for f in bm.faces}

    # For each vertex, find surrounding faces in order
    # These will become the new faces
    new_faces_data = []

    for vert in bm.verts:
        # Get all faces connected to this vertex
        connected_faces = list(vert.link_faces)

        if len(connected_faces) < 3:
            continue

        # Sort faces by angle around the vertex
        # Use the vertex normal as reference
        vert_normal = vert.normal
        vert_pos = vert.co

        # Create a consistent reference frame
        # Pick any edge from vertex as reference direction
        if not vert.link_edges:
            continue

        ref_edge = vert.link_edges[0]
        other_vert = ref_edge.other_vert(vert)
        ref_dir = (other_vert.co - vert_pos).normalized()

        # Cross product gives perpendicular in the tangent plane
        perp_dir = vert_normal.cross(ref_dir).normalized()

        def angle_around_vertex(face):
            fc = face_centers[face.index]
            to_fc = (fc - vert_pos).normalized()
            # Project onto tangent plane
            x = to_fc.dot(ref_dir)
            y = to_fc.dot(perp_dir)
            return math.atan2(y, x)

        connected_faces.sort(key=angle_around_vertex)

        # The new face vertices are the face centers in order
        new_faces_data.append([face_centers[f.index] for f in connected_faces])

    # Clear old geometry
    bm.clear()

    # Create new vertices from face centers
    center_to_vert = {}
    for fc in face_centers.values():
        key = (round(fc.x, 6), round(fc.y, 6), round(fc.z, 6))
        if key not in center_to_vert:
            center_to_vert[key] = bm.verts.new(fc)

    bm.verts.ensure_lookup_table()

    # Create new faces
    for face_positions in new_faces_data:
        try:
            verts = []
            for pos in face_positions:
                key = (round(pos.x, 6), round(pos.y, 6), round(pos.z, 6))
                verts.append(center_to_vert[key])
            if len(verts) >= 3:
                bm.faces.new(verts)
        except ValueError:
            pass  # Face already exists or degenerate

    bm.normal_update()


def create_golf_ball():
    """Create golf ball with hexagonal dimples."""

    print("  Creating icosphere...")
    bpy.ops.mesh.primitive_ico_sphere_add(
        subdivisions=4,  # More subdivisions = more dimples
        radius=BALL_RADIUS,
        location=(0, 0, 0)
    )
    ball = bpy.context.active_object
    ball.name = "GolfBall"

    bpy.ops.object.mode_set(mode='EDIT')
    bm = bmesh.from_edit_mesh(ball.data)

    print(f"  Icosphere: {len(bm.faces)} triangular faces, {len(bm.verts)} vertices")

    # Step 1: DUAL MESH - converts triangles to hexagons
    print("  Converting to dual mesh (hexagons)...")
    dual_mesh(bm)

    bm.verts.ensure_lookup_table()
    bm.faces.ensure_lookup_table()
    print(f"  Dual mesh: {len(bm.faces)} faces (hexagons/pentagons)")

    # Project all vertices to sphere surface
    for v in bm.verts:
        v.co = v.co.normalized() * BALL_RADIUS

    bmesh.update_edit_mesh(ball.data)

    # Step 2: Extrude hexagons directly (NO inset - that creates the rim)
    print("  Extruding dimples...")
    bm = bmesh.from_edit_mesh(ball.data)
    bm.faces.ensure_lookup_table()

    all_faces = list(bm.faces)
    print(f"  Extruding {len(all_faces)} hexagonal faces")

    # Extrude all faces inward
    extruded = bmesh.ops.extrude_discrete_faces(bm, faces=all_faces)

    # Move extruded faces inward AND scale down for bowl shape
    for geom in extruded['faces']:
        face_center = geom.calc_center_median()
        sphere_inward = -face_center.normalized()

        # Move inward (dimple depth)
        for v in geom.verts:
            v.co += sphere_inward * 0.045

        # Scale down toward face center (bowl shape)
        new_center = geom.calc_center_median()
        for v in geom.verts:
            to_center = new_center - v.co
            v.co += to_center * 0.55  # Shrink 55%

    bmesh.update_edit_mesh(ball.data)

    bpy.ops.object.mode_set(mode='OBJECT')

    # Step 4: Subdivision surface for smoothing
    print("  Smoothing with subdivision surface...")
    subsurf = ball.modifiers.new(name="Subsurf", type='SUBSURF')
    subsurf.levels = 3
    subsurf.render_levels = 3
    bpy.ops.object.modifier_apply(modifier="Subsurf")

    bpy.ops.object.shade_smooth()

    # UV unwrap for text texture
    print("  UV unwrapping (manual spherical)...")
    mesh = ball.data
    if not mesh.uv_layers:
        mesh.uv_layers.new(name="UVMap")
    uv_layer = mesh.uv_layers.active.data

    for poly in mesh.polygons:
        for loop_idx in poly.loop_indices:
            vert_idx = mesh.loops[loop_idx].vertex_index
            co = mesh.vertices[vert_idx].co.normalized()
            lon = math.atan2(co.y, co.x)
            lat = math.asin(max(-1, min(1, co.z)))
            u = lon / (2 * math.pi) + 0.5
            v = 0.5 + lat / math.pi
            uv_layer[loop_idx].uv = (u, v)

    # Load text texture
    script_dir = os.path.dirname(os.path.abspath(__file__))
    texture_path = os.path.join(script_dir, "tp5_pix_texture.png")
    print(f"  Loading text texture: {texture_path}")

    # Material with texture (text on white background)
    mat = create_material_with_texture("TP5_Pix", texture_path)
    ball.data.materials.append(mat)

    # Add logo geometry
    print("  Adding logo geometry...")
    add_all_logos(ball)

    print("  Complete!")
    return ball


def export_gltf(filepath):
    bpy.ops.export_scene.gltf(
        filepath=filepath,
        export_format='GLB',
        use_selection=False,
        export_apply=True,
        export_materials='EXPORT',
    )


def export_fbx(filepath):
    bpy.ops.export_scene.fbx(
        filepath=filepath,
        use_selection=False,
        apply_scale_options='FBX_SCALE_ALL',
        path_mode='COPY',
        embed_textures=True,
        mesh_smooth_type='FACE',
        use_mesh_modifiers=True,
    )


def main():
    print("=" * 50)
    print("Golf Ball - Dual Mesh + Inset/Extrude")
    print("=" * 50)

    clear_scene()

    print("\nBuilding golf ball...")
    ball = create_golf_ball()

    script_dir = os.path.dirname(os.path.abspath(__file__))

    blend_path = os.path.join(script_dir, "tp5_pix_ball.blend")
    bpy.ops.wm.save_as_mainfile(filepath=blend_path)
    print(f"\nSaved: {blend_path}")

    glb_path = os.path.join(script_dir, "tp5_pix_ball.glb")
    export_gltf(glb_path)
    print(f"Exported: {glb_path}")

    fbx_path = os.path.join(script_dir, "tp5_pix_ball.fbx")
    export_fbx(fbx_path)
    print(f"Exported: {fbx_path}")

    print("=" * 50)


if __name__ == "__main__":
    main()
