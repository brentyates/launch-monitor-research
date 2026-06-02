use crate::ball_detector::BallDetection;
use crate::config::{Camera, StereoRig};
use nalgebra::{Matrix3, Matrix4, Vector3, Vector4};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TriangulationError {
    #[error("Insufficient frames for velocity estimation (need at least 2)")]
    InsufficientFrames,
    #[error("Failed to triangulate point")]
    Failed,
}

#[derive(Debug, Clone, Copy)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone)]
pub struct LaunchData {
    pub speed_mph: f64,
    pub vla_deg: f64,
    pub hla_deg: f64,
    pub positions: Vec<Point3D>,
}

pub struct StereoTriangulator {
    proj_cam0: [[f64; 4]; 3],
    proj_cam1: [[f64; 4]; 3],
    rig: Option<StereoRig>,
    cam0_size: (u32, u32),
    cam1_size: (u32, u32),
}

impl StereoTriangulator {
    pub fn project_cam0(&self, point: &Point3D) -> (f64, f64) {
        Self::project(&self.proj_cam0, point)
    }

    pub fn project_cam1(&self, point: &Point3D) -> (f64, f64) {
        Self::project(&self.proj_cam1, point)
    }

    fn project(p: &[[f64; 4]; 3], pt: &Point3D) -> (f64, f64) {
        let x = p[0][0] * pt.x + p[0][1] * pt.y + p[0][2] * pt.z + p[0][3];
        let y = p[1][0] * pt.x + p[1][1] * pt.y + p[1][2] * pt.z + p[1][3];
        let w = p[2][0] * pt.x + p[2][1] * pt.y + p[2][2] * pt.z + p[2][3];
        (x / w, y / w)
    }
}

impl StereoTriangulator {
    pub fn new(rig: &StereoRig) -> Self {
        let proj_cam0 = Self::build_projection(&rig.cam0_intrinsic, &rig.cam0_rotation, &rig.cam0_translation);
        let proj_cam1 = Self::build_projection(&rig.cam1_intrinsic, &rig.cam1_rotation, &rig.cam1_translation);
        Self {
            proj_cam0,
            proj_cam1,
            cam0_size: (rig.width, rig.height),
            cam1_size: (rig.width, rig.height),
            rig: Some(rig.clone()),
        }
    }

    pub fn from_pair(a: &Camera, b: &Camera) -> Self {
        let proj_cam0 = Self::build_projection(&a.intrinsic, &a.rotation, &a.translation);
        let proj_cam1 = Self::build_projection(&b.intrinsic, &b.rotation, &b.translation);
        Self {
            proj_cam0,
            proj_cam1,
            cam0_size: (a.width, a.height),
            cam1_size: (b.width, b.height),
            rig: None,
        }
    }

    fn build_projection(k: &Matrix3<f64>, r: &Matrix3<f64>, t: &Vector3<f64>) -> [[f64; 4]; 3] {
        let mut p = [[0.0; 4]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for l in 0..3 {
                    p[i][j] += k[(i, l)] * r[(l, j)];
                }
            }
            for l in 0..3 {
                p[i][3] += k[(i, l)] * t[l];
            }
        }
        p
    }

    pub fn triangulate(&self, cam0: &BallDetection, cam1: &BallDetection) -> Result<Point3D, TriangulationError> {
        let (u1, v1) = match &self.rig {
            Some(rig) => {
                let n_u = cam0.center_x / self.cam0_size.0 as f64;
                let n_v = cam0.center_y / self.cam0_size.1 as f64;
                let (un_u, un_v) = rig.undistort_pixel(0, n_u, n_v);
                (un_u * self.cam0_size.0 as f64, un_v * self.cam0_size.1 as f64)
            }
            None => (cam0.center_x, cam0.center_y),
        };

        let (u2, v2) = match &self.rig {
            Some(rig) => {
                let n_u = cam1.center_x / self.cam1_size.0 as f64;
                let n_v = cam1.center_y / self.cam1_size.1 as f64;
                let (un_u, un_v) = rig.undistort_pixel(1, n_u, n_v);
                (un_u * self.cam1_size.0 as f64, un_v * self.cam1_size.1 as f64)
            }
            None => (cam1.center_x, cam1.center_y),
        };

        let p1 = &self.proj_cam0;
        let p2 = &self.proj_cam1;

        let a = Matrix4::new(
            u1 * p1[2][0] - p1[0][0], u1 * p1[2][1] - p1[0][1], u1 * p1[2][2] - p1[0][2], u1 * p1[2][3] - p1[0][3],
            v1 * p1[2][0] - p1[1][0], v1 * p1[2][1] - p1[1][1], v1 * p1[2][2] - p1[1][2], v1 * p1[2][3] - p1[1][3],
            u2 * p2[2][0] - p2[0][0], u2 * p2[2][1] - p2[0][1], u2 * p2[2][2] - p2[0][2], u2 * p2[2][3] - p2[0][3],
            v2 * p2[2][0] - p2[1][0], v2 * p2[2][1] - p2[1][1], v2 * p2[2][2] - p2[1][2], v2 * p2[2][3] - p2[1][3],
        );

        let svd = a.svd(true, true);
        let v_t = svd.v_t.ok_or(TriangulationError::Failed)?;
        let x: Vector4<f64> = v_t.row(3).transpose();

        if x.w.abs() < 1e-10 {
            return Err(TriangulationError::Failed);
        }

        Ok(Point3D {
            x: x.x / x.w,
            y: x.y / x.w,
            z: x.z / x.w,
        })
    }

    pub fn estimate_launch(
        &self,
        detections: &[(u32, f64, BallDetection, BallDetection)],
    ) -> Result<LaunchData, TriangulationError> {
        if detections.len() < 2 {
            return Err(TriangulationError::InsufficientFrames);
        }

        let all_positions: Result<Vec<_>, _> = detections
            .iter()
            .map(|(_, _, l, r)| self.triangulate(l, r))
            .collect();
        let all_positions = all_positions?;

        let all_times: Vec<f64> = detections
            .iter()
            .map(|(_, ts, _, _)| *ts)
            .collect();

        let (vx, vy, vz, positions) = if all_positions.len() >= 4 {
            let (vx0, vy0, vz0) = Self::fit_velocity(&all_times, &all_positions)?;

            let residuals: Vec<f64> = all_times.iter().zip(all_positions.iter()).map(|(t, p)| {
                let t_mean = all_times.iter().sum::<f64>() / all_times.len() as f64;
                let x_mean = all_positions.iter().map(|p| p.x).sum::<f64>() / all_positions.len() as f64;
                let y_mean = all_positions.iter().map(|p| p.y).sum::<f64>() / all_positions.len() as f64;
                let z_mean = all_positions.iter().map(|p| p.z).sum::<f64>() / all_positions.len() as f64;
                let dt = t - t_mean;
                let ex = p.x - (x_mean + vx0 * dt);
                let ey = p.y - (y_mean + vy0 * dt);
                let ez = p.z - (z_mean + vz0 * dt);
                (ex * ex + ey * ey + ez * ez).sqrt()
            }).collect();

            let mean_res = residuals.iter().sum::<f64>() / residuals.len() as f64;
            let std_res = (residuals.iter().map(|r| (r - mean_res).powi(2)).sum::<f64>() / residuals.len() as f64).sqrt();
            let threshold = mean_res + 2.0 * std_res;

            let mut filtered_times = Vec::new();
            let mut filtered_positions = Vec::new();
            for (i, res) in residuals.iter().enumerate() {
                if *res <= threshold {
                    filtered_times.push(all_times[i]);
                    filtered_positions.push(all_positions[i]);
                } else {
                    tracing::info!(
                        "Rejected frame {} as outlier (residual {:.1}mm, threshold {:.1}mm)",
                        detections[i].0, res, threshold
                    );
                }
            }

            if filtered_positions.len() >= 2 && filtered_positions.len() < all_positions.len() {
                let (vx, vy, vz) = Self::fit_velocity(&filtered_times, &filtered_positions)?;
                (vx, vy, vz, filtered_positions)
            } else {
                (vx0, vy0, vz0, all_positions)
            }
        } else {
            let (vx, vy, vz) = Self::fit_velocity(&all_times, &all_positions)?;
            (vx, vy, vz, all_positions)
        };

        let speed_mm_s = (vx * vx + vy * vy + vz * vz).sqrt();
        let speed_mph = speed_mm_s * 0.00223694;

        let horizontal_speed = (vx * vx + vy * vy).sqrt();
        let vla_deg = vz.atan2(horizontal_speed).to_degrees();
        let hla_deg = vx.atan2(vy).to_degrees();

        Ok(LaunchData { speed_mph, vla_deg, hla_deg, positions })
    }

    fn fit_velocity(times: &[f64], positions: &[Point3D]) -> Result<(f64, f64, f64), TriangulationError> {
        let n = times.len() as f64;
        let t_mean = times.iter().sum::<f64>() / n;
        let x_mean = positions.iter().map(|p| p.x).sum::<f64>() / n;
        let y_mean = positions.iter().map(|p| p.y).sum::<f64>() / n;
        let z_mean = positions.iter().map(|p| p.z).sum::<f64>() / n;

        let t_var: f64 = times.iter().map(|t| (t - t_mean).powi(2)).sum();
        if t_var < 1e-20 {
            return Err(TriangulationError::Failed);
        }

        let vx = times.iter().zip(positions.iter()).map(|(t, p)| (t - t_mean) * (p.x - x_mean)).sum::<f64>() / t_var;
        let vy = times.iter().zip(positions.iter()).map(|(t, p)| (t - t_mean) * (p.y - y_mean)).sum::<f64>() / t_var;
        let vz = times.iter().zip(positions.iter()).map(|(t, p)| (t - t_mean) * (p.z - z_mean)).sum::<f64>() / t_var;

        Ok((vx, vy, vz))
    }
}
