use nalgebra::{Matrix3, Vector3};

#[derive(Debug, Clone)]
pub struct StereoRig {
    pub cam0_intrinsic: Matrix3<f64>,
    pub cam1_intrinsic: Matrix3<f64>,
    pub cam0_rotation: Matrix3<f64>,
    pub cam1_rotation: Matrix3<f64>,
    pub cam0_translation: Vector3<f64>,
    pub cam1_translation: Vector3<f64>,
    pub cam0_distortion: [f64; 2],
    pub cam1_distortion: [f64; 2],
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

impl StereoRig {
    pub fn overhead(
        baseline_mm: f64,
        height_mm: f64,
        forward_mm: f64,
        focal_length_mm: f64,
        pixel_pitch_mm: f64,
        render_size: (u32, u32),
        fps: f64,
    ) -> Self {
        let focal_px = focal_length_mm / pixel_pitch_mm;
        let cx = render_size.0 as f64 / 2.0;
        let cy = render_size.1 as f64 / 2.0;

        let intrinsic = Matrix3::new(
            focal_px, 0.0, cx,
            0.0, focal_px, cy,
            0.0, 0.0, 1.0,
        );

        let convergence = Self::compute_convergence(
            height_mm, forward_mm, focal_length_mm, pixel_pitch_mm, render_size.1,
        );

        let cam0_pos = Vector3::new(-baseline_mm / 2.0, forward_mm, height_mm);
        let cam1_pos = Vector3::new(baseline_mm / 2.0, forward_mm, height_mm);

        let cam0_rotation = Self::look_at(&cam0_pos, &convergence);
        let cam1_rotation = Self::look_at(&cam1_pos, &convergence);

        let cam0_translation = -(cam0_rotation * cam0_pos);
        let cam1_translation = -(cam1_rotation * cam1_pos);

        Self {
            cam0_intrinsic: intrinsic,
            cam1_intrinsic: intrinsic,
            cam0_rotation,
            cam1_rotation,
            cam0_translation,
            cam1_translation,
            cam0_distortion: [0.0, 0.0],
            cam1_distortion: [0.0, 0.0],
            width: render_size.0,
            height: render_size.1,
            fps,
        }
    }

    pub fn undistort_pixel(&self, cam_index: usize, u: f64, v: f64) -> (f64, f64) {
        let k = if cam_index == 0 { self.cam0_distortion } else { self.cam1_distortion };
        if k[0] == 0.0 && k[1] == 0.0 {
            return (u, v);
        }

        // Distorted normalized coordinates (0 to 1) -> center-relative (-0.5 to 0.5)
        let du = u - 0.5;
        let dv = v - 0.5;

        // In the Unity shader: output_pixel(uv) = pinhole_pixel(centered * radialFactor + center)
        // So a feature seen at 'uv' in the distorted output was actually at 'uv_warped' in the pinhole render.
        // Pinhole (undistorted) = centered * (1 + k1*r2 + k2*r4) + center
        let r2 = du * du + dv * dv;
        let r4 = r2 * r2;
        let radial_factor = 1.0 + k[0] * r2 + k[1] * r4;

        (du * radial_factor + 0.5, dv * radial_factor + 0.5)
    }

    fn compute_convergence(
        height_mm: f64,
        forward_mm: f64,
        focal_length_mm: f64,
        pixel_pitch_mm: f64,
        render_height: u32,
    ) -> Vector3<f64> {
        let height_m = height_mm / 1000.0;
        let forward_m = forward_mm / 1000.0;

        let render_height_mm = render_height as f64 * pixel_pitch_mm;
        let eff_fov = 2.0 * (render_height_mm / (2.0 * focal_length_mm)).atan();
        let half_fov = eff_fov / 2.0;

        let hitting_half_size_m = 0.075;
        let back_edge_padding_m = 0.025;
        let far_edge = hitting_half_size_m + back_edge_padding_m;

        let horiz_dist = forward_m + far_edge;
        let angle_to_far = height_m.atan2(horiz_dist);
        let convergence_angle = angle_to_far + half_fov;

        let convergence_z_unity = (height_m / convergence_angle.tan()) - forward_m;
        let convergence_y_mm = -convergence_z_unity * 1000.0;

        Vector3::new(0.0, convergence_y_mm, 0.0)
    }

    fn look_at(eye: &Vector3<f64>, target: &Vector3<f64>) -> Matrix3<f64> {
        let forward = (target - eye).normalize();
        let world_up = Vector3::new(0.0, 0.0, 1.0);
        let right = world_up.cross(&forward).normalize();
        let down = right.cross(&forward);

        Matrix3::new(
            right.x, right.y, right.z,
            down.x, down.y, down.z,
            forward.x, forward.y, forward.z,
        )
    }
}
