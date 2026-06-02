use image::GrayImage;
use imageproc::edges::canny;
use nalgebra::{Matrix3, Vector3};
use rayon::prelude::*;
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum SpinError {
    #[error("Insufficient frames (need at least 2)")]
    InsufficientFrames,
    #[error("No valid spin hypothesis found")]
    NoValidHypothesis,
}

#[derive(Debug, Clone, Copy)]
pub struct SpinResult {
    pub rpm: f64,
    pub axis_deg: f64,
    pub confidence: f64,
}

pub struct FrameSpin {
    pub gray: GrayImage,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
}

#[derive(Clone, Copy)]
struct Chevron {
    theta: f64,
    phi: f64,
}

pub struct SpinDetector {
    fps: f64,
    chevrons: Vec<Chevron>,
}

impl SpinDetector {
    pub fn new(fps: f64) -> Self {
        Self {
            fps,
            chevrons: Self::tp5_pix_pattern(),
        }
    }

    fn tp5_pix_pattern() -> Vec<Chevron> {
        let lat = (150.0_f64 / 296.0).asin();
        let mut chevrons = Vec::with_capacity(14);

        for i in 0..6 {
            let theta = (i as f64) * std::f64::consts::FRAC_PI_3;
            chevrons.push(Chevron { theta, phi: std::f64::consts::FRAC_PI_2 - lat });
        }
        for i in 0..6 {
            let theta = std::f64::consts::FRAC_PI_6 + (i as f64) * std::f64::consts::FRAC_PI_3;
            chevrons.push(Chevron { theta, phi: std::f64::consts::FRAC_PI_2 + lat });
        }
        chevrons.push(Chevron { theta: 0.0, phi: 0.0 });
        chevrons.push(Chevron { theta: 0.0, phi: std::f64::consts::PI });

        chevrons
    }

    fn rotation_matrix(axis: Vector3<f64>, angle: f64) -> Matrix3<f64> {
        let k = axis.normalize();
        let (sin_a, cos_a) = angle.sin_cos();
        let kx = Matrix3::new(
            0.0, -k.z, k.y,
            k.z, 0.0, -k.x,
            -k.y, k.x, 0.0,
        );
        Matrix3::identity() * cos_a + kx * sin_a + k * k.transpose() * (1.0 - cos_a)
    }

    fn project_chevrons(&self, rot: &Matrix3<f64>, center: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
        self.chevrons
            .iter()
            .filter_map(|c| {
                let pos = Vector3::new(
                    c.phi.sin() * c.theta.cos(),
                    c.phi.sin() * c.theta.sin(),
                    c.phi.cos(),
                );
                let rotated = rot * pos;
                if rotated.z > 0.0 {
                    Some((center.0 + rotated.x * radius, center.1 + rotated.y * radius))
                } else {
                    None
                }
            })
            .collect()
    }

    fn crop_ball(gray: &GrayImage, center: (f64, f64), radius: f64) -> (GrayImage, f64, f64) {
        let margin = (radius * 1.5).ceil() as i32;
        let (w, h) = gray.dimensions();
        let x0 = (center.0 as i32 - margin).max(0) as u32;
        let y0 = (center.1 as i32 - margin).max(0) as u32;
        let x1 = ((center.0 as i32 + margin) as u32).min(w);
        let y1 = ((center.1 as i32 + margin) as u32).min(h);
        let cw = x1 - x0;
        let ch = y1 - y0;

        let mut crop = GrayImage::new(cw, ch);
        for y in 0..ch {
            for x in 0..cw {
                crop.put_pixel(x, y, *gray.get_pixel(x0 + x, y0 + y));
            }
        }
        (crop, x0 as f64, y0 as f64)
    }

    pub fn ball_features(gray: &GrayImage, center: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
        Self::detect_features(gray, center, radius)
    }

    fn detect_features(gray: &GrayImage, center: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
        let (crop, ox, oy) = Self::crop_ball(gray, center, radius);
        let lc = (center.0 - ox, center.1 - oy);

        let inner_r2 = (radius * 0.70).powi(2);
        let outer_r2 = (radius * 0.85).powi(2);

        let thresh_pts = Self::detect_dark_spots(&crop, lc, outer_r2, radius);
        let edge_pts = Self::detect_edge_points(&crop, lc, inner_r2);

        let dedup_dist = (0.15 * radius).max(2.0);
        let dedup_d2 = dedup_dist * dedup_dist;

        let n_thresh = thresh_pts.len();
        let mut all: Vec<(f64, f64)> = thresh_pts;

        for pt in &edge_pts {
            let is_dup = all.iter().any(|&(ax, ay)| {
                (ax - pt.0).powi(2) + (ay - pt.1).powi(2) < dedup_d2
            });
            if !is_dup {
                all.push(*pt);
            }
        }

        debug!(
            "Features r={:.1}: thresh={} edge_new={} total={}",
            radius, n_thresh, all.len() - n_thresh, all.len()
        );

        all.into_iter().map(|(x, y)| (x + ox, y + oy)).collect()
    }

    fn detect_edge_points(
        crop: &GrayImage,
        center: (f64, f64),
        inner_r2: f64,
    ) -> Vec<(f64, f64)> {
        let blurred = imageproc::filter::gaussian_blur_f32(crop, 0.5);
        let edges = canny(&blurred, 15.0, 60.0);
        let (w, h) = edges.dimensions();

        let mut points = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if edges.get_pixel(x, y).0[0] > 0 {
                    let dx = x as f64 - center.0;
                    let dy = y as f64 - center.1;
                    if dx * dx + dy * dy < inner_r2 {
                        points.push((x as f64, y as f64));
                    }
                }
            }
        }

        Self::cluster_points(&points, 3.0)
    }

    fn detect_dark_spots(
        crop: &GrayImage,
        center: (f64, f64),
        mask_r2: f64,
        radius: f64,
    ) -> Vec<(f64, f64)> {
        let blurred = imageproc::filter::gaussian_blur_f32(crop, 0.5);
        let (cx, cy) = center;
        let (cw, ch) = blurred.dimensions();

        let mut sum = 0u64;
        let mut count = 0u64;
        let mut values = Vec::new();
        for y in 0..ch {
            for x in 0..cw {
                let dx = x as f64 - cx;
                let dy = y as f64 - cy;
                if dx * dx + dy * dy < mask_r2 {
                    let v = blurred.get_pixel(x, y).0[0];
                    sum += v as u64;
                    count += 1;
                    values.push(v);
                }
            }
        }
        if count == 0 {
            return Vec::new();
        }

        let mean = sum as f64 / count as f64;
        let variance: f64 = values.iter()
            .map(|&v| (v as f64 - mean).powi(2))
            .sum::<f64>() / count as f64;
        let stddev = variance.sqrt();

        let threshold = (mean - 1.5 * stddev.max(8.0)) as u8;

        let mut dark_points = Vec::new();
        for y in 0..ch {
            for x in 0..cw {
                let dx = x as f64 - cx;
                let dy = y as f64 - cy;
                if dx * dx + dy * dy < mask_r2 {
                    let v = blurred.get_pixel(x, y).0[0];
                    if v < threshold {
                        dark_points.push((x as f64, y as f64));
                    }
                }
            }
        }

        Self::cluster_points(&dark_points, radius * 0.2)
    }

    fn cluster_points(points: &[(f64, f64)], merge_dist: f64) -> Vec<(f64, f64)> {
        if points.is_empty() {
            return Vec::new();
        }

        let merge_d2 = merge_dist * merge_dist;
        let mut assigned = vec![false; points.len()];
        let mut centroids = Vec::new();

        for i in 0..points.len() {
            if assigned[i] {
                continue;
            }
            assigned[i] = true;
            let mut sx = points[i].0;
            let mut sy = points[i].1;
            let mut n = 1.0;

            for j in (i + 1)..points.len() {
                if assigned[j] {
                    continue;
                }
                let dx = points[j].0 - points[i].0;
                let dy = points[j].1 - points[i].1;
                if dx * dx + dy * dy < merge_d2 {
                    assigned[j] = true;
                    sx += points[j].0;
                    sy += points[j].1;
                    n += 1.0;
                }
            }

            centroids.push((sx / n, sy / n));
        }

        centroids
    }

    fn score_frame(
        detected: &[(f64, f64)],
        predicted: &[(f64, f64)],
        threshold: f64,
    ) -> f64 {
        if detected.is_empty() || predicted.is_empty() {
            return 0.0;
        }

        let gauss = |dist: f64| -> f64 {
            (-2.0 * (dist / threshold).powi(2)).exp()
        };

        let mut forward_score = 0.0;
        let mut forward_matches = 0;
        for &(dx, dy) in detected {
            let min_dist = predicted
                .iter()
                .map(|&(px, py)| ((dx - px).powi(2) + (dy - py).powi(2)).sqrt())
                .fold(f64::MAX, f64::min);
            let s = gauss(min_dist);
            forward_score += s;
            if min_dist < threshold {
                forward_matches += 1;
            }
        }
        forward_score /= detected.len() as f64;

        let mut backward_score = 0.0;
        for &(px, py) in predicted {
            let min_dist = detected
                .iter()
                .map(|&(dx, dy)| ((dx - px).powi(2) + (dy - py).powi(2)).sqrt())
                .fold(f64::MAX, f64::min);
            backward_score += gauss(min_dist);
        }
        backward_score /= predicted.len() as f64;

        let combined = 0.7 * forward_score + 0.3 * backward_score;

        let max_possible = detected.len().max(predicted.len()) as f64;
        let match_bonus = 0.2 * (forward_matches as f64 / max_possible);

        combined + match_bonus
    }

    pub fn detect(&self, frames: &[FrameSpin]) -> Result<SpinResult, SpinError> {
        if frames.len() < 2 {
            return Err(SpinError::InsufficientFrames);
        }

        let features: Vec<_> = frames
            .iter()
            .map(|f| Self::detect_features(&f.gray, (f.center_x, f.center_y), f.radius))
            .collect();

        let frame_weights: Vec<f64> = features
            .iter()
            .map(|f| (f.len() as f64 / 3.0).min(1.0))
            .collect();

        let avg_radius = frames.iter().map(|f| f.radius).sum::<f64>() / frames.len() as f64;
        let threshold = (0.15 * avg_radius).max(2.0);
        let rad_per_rpm = (360.0_f64).to_radians() / (60.0 * self.fps);

        let best_coarse = self.search_coarse(frames, &features, &frame_weights, threshold, rad_per_rpm);

        let best_medium = self.search_refine(
            frames, &features, &frame_weights, threshold, rad_per_rpm,
            best_coarse.0, best_coarse.1, best_coarse.2,
            50.0, 5.0, 0.5, 12,
        );

        let best_fine = self.search_refine(
            frames, &features, &frame_weights, threshold, rad_per_rpm,
            best_medium.0, best_medium.1, best_medium.2,
            10.0, 1.0, 0.15, 8,
        );

        if best_fine.3 < 0.1 {
            return Err(SpinError::NoValidHypothesis);
        }

        Ok(SpinResult {
            rpm: best_fine.0,
            axis_deg: best_fine.1,
            confidence: best_fine.3,
        })
    }

    fn search_coarse(
        &self,
        frames: &[FrameSpin],
        features: &[Vec<(f64, f64)>],
        weights: &[f64],
        threshold: f64,
        rad_per_rpm: f64,
    ) -> (f64, f64, f64, f64) {
        let rpm_values: Vec<f64> = (0..=600).map(|i| i as f64 * 25.0).collect();

        let axis_angles: Vec<f64> = (-45i32..=45)
            .step_by(5)
            .map(|a| a as f64)
            .collect();

        let initial_rots: Vec<f64> = (0..24).map(|i| (i as f64 * 15.0).to_radians()).collect();

        let mut hypotheses = Vec::with_capacity(rpm_values.len() * axis_angles.len() * initial_rots.len());
        for &rpm in &rpm_values {
            for &axis_deg in &axis_angles {
                for &init_rot in &initial_rots {
                    hypotheses.push((rpm, axis_deg, init_rot));
                }
            }
        }

        hypotheses
            .par_iter()
            .map(|&(rpm, axis_deg, init_rot)| {
                let score = self.evaluate_hypothesis(
                    frames, features, weights, threshold, rad_per_rpm,
                    rpm, axis_deg, init_rot,
                );
                (rpm, axis_deg, init_rot, score)
            })
            .max_by(|a, b| a.3.partial_cmp(&b.3).unwrap())
            .unwrap()
    }

    fn search_refine(
        &self,
        frames: &[FrameSpin],
        features: &[Vec<(f64, f64)>],
        weights: &[f64],
        threshold: f64,
        rad_per_rpm: f64,
        center_rpm: f64,
        center_axis: f64,
        center_rot: f64,
        rpm_range: f64,
        rpm_step: f64,
        rot_range: f64,
        rot_steps: usize,
    ) -> (f64, f64, f64, f64) {
        let rpm_min = (center_rpm - rpm_range).max(0.0);
        let rpm_max = center_rpm + rpm_range;
        let rpm_count = ((rpm_max - rpm_min) / rpm_step) as usize + 1;

        let rot_step = 2.0 * rot_range / rot_steps as f64;

        let mut hypotheses = Vec::new();
        for i in 0..rpm_count {
            let rpm = rpm_min + i as f64 * rpm_step;
            let axis_angles = [center_axis - 2.0, center_axis - 1.0, center_axis, center_axis + 1.0, center_axis + 2.0];
            for &axis in &axis_angles {
                for j in 0..=rot_steps {
                    let rot = center_rot - rot_range + j as f64 * rot_step;
                    hypotheses.push((rpm, axis, rot));
                }
            }
        }

        hypotheses
            .par_iter()
            .map(|&(rpm, axis_deg, init_rot)| {
                let score = self.evaluate_hypothesis(
                    frames, features, weights, threshold, rad_per_rpm,
                    rpm, axis_deg, init_rot,
                );
                (rpm, axis_deg, init_rot, score)
            })
            .max_by(|a, b| a.3.partial_cmp(&b.3).unwrap())
            .unwrap()
    }

    fn evaluate_hypothesis(
        &self,
        frames: &[FrameSpin],
        features: &[Vec<(f64, f64)>],
        weights: &[f64],
        threshold: f64,
        rad_per_rpm: f64,
        rpm: f64,
        axis_deg: f64,
        init_rot: f64,
    ) -> f64 {
        let axis_rad = axis_deg.to_radians();
        let spin_axis = Vector3::new(axis_rad.sin(), 0.0, axis_rad.cos());
        let angle_per_frame = rpm * rad_per_rpm;

        let init_axis = Vector3::new(0.0, 0.0, 1.0);
        let mut rot = Self::rotation_matrix(init_axis, init_rot);

        let mut total = 0.0;
        let mut total_weight = 0.0;

        for (i, frame) in frames.iter().enumerate() {
            let center = (frame.center_x, frame.center_y);
            let radius = frame.radius;
            let predicted = self.project_chevrons(&rot, center, radius);
            let score = Self::score_frame(&features[i], &predicted, threshold);
            total += score * weights[i];
            total_weight += weights[i];
            rot = Self::rotation_matrix(spin_axis, angle_per_frame) * rot;
        }

        if total_weight > 0.0 { total / total_weight } else { 0.0 }
    }
}
