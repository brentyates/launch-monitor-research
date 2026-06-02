use crate::spin_detector::{FrameSpin, SpinDetector};
use nalgebra::{Matrix3, Vector3};

pub struct InterframeSpin {
    pub rpm: f64,
    pub axis_deg: f64,
    pub confidence: f64,
    pub pairs: usize,
}

pub fn estimate_spin_interframe(frames: &[FrameSpin], fps: f64) -> Option<InterframeSpin> {
    if frames.len() < 2 {
        return None;
    }

    let clouds: Vec<Vec<Vector3<f64>>> = frames
        .iter()
        .map(|f| {
            let feats = SpinDetector::ball_features(&f.gray, (f.center_x, f.center_y), f.radius);
            backproject(&feats, (f.center_x, f.center_y), f.radius)
        })
        .collect();

    let mut rates = Vec::new();
    let mut axes = Vec::new();
    for w in clouds.windows(2) {
        if let Some(r) = solve_rotation(&w[0], &w[1]) {
            let (angle, axis) = rotation_angle_axis(&r);
            if angle > 1e-4 {
                rates.push(angle * fps * 60.0 / (2.0 * std::f64::consts::PI));
                axes.push(axis);
            }
        }
    }

    if rates.is_empty() {
        return None;
    }

    rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rpm = rates[rates.len() / 2];

    let axis_mean = axes.iter().fold(Vector3::zeros(), |acc, a| acc + a) / axes.len() as f64;
    let axis_deg = axis_mean.y.atan2(axis_mean.x).to_degrees();

    let spread = rates.last().unwrap() - rates.first().unwrap();
    let confidence = (1.0 / (1.0 + spread / rpm.max(1.0))).clamp(0.0, 1.0);

    Some(InterframeSpin { rpm, axis_deg, confidence, pairs: rates.len() })
}

fn backproject(features: &[(f64, f64)], center: (f64, f64), radius: f64) -> Vec<Vector3<f64>> {
    features
        .iter()
        .filter_map(|&(fx, fy)| {
            let u = (fx - center.0) / radius;
            let v = (fy - center.1) / radius;
            let r2 = u * u + v * v;
            if r2 > 0.95 {
                return None;
            }
            let z = (1.0 - r2).sqrt();
            Some(Vector3::new(u, v, z))
        })
        .collect()
}

fn solve_rotation(a: &[Vector3<f64>], b: &[Vector3<f64>]) -> Option<Matrix3<f64>> {
    if a.len() < 3 || b.len() < 3 {
        return None;
    }

    let gate2 = 0.6_f64 * 0.6;
    let mut pairs: Vec<(Vector3<f64>, Vector3<f64>)> = Vec::new();
    for &pa in a {
        let mut best: Option<Vector3<f64>> = None;
        let mut bd = gate2;
        for &pb in b {
            let d = (pa - pb).norm_squared();
            if d < bd {
                bd = d;
                best = Some(pb);
            }
        }
        if let Some(pb) = best {
            pairs.push((pa, pb));
        }
    }
    if pairs.len() < 3 {
        return None;
    }

    let mut r = kabsch(&pairs);
    let resid: Vec<f64> = pairs.iter().map(|(pa, pb)| (r * pa - pb).norm()).collect();
    let mut sorted = resid.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let med = sorted[sorted.len() / 2];
    let thresh = (med * 2.0).max(0.1);
    let inliers: Vec<(Vector3<f64>, Vector3<f64>)> = pairs
        .iter()
        .cloned()
        .zip(resid.iter())
        .filter(|(_, &res)| res <= thresh)
        .map(|(p, _)| p)
        .collect();
    if inliers.len() >= 3 {
        r = kabsch(&inliers);
    }
    Some(r)
}

fn kabsch(pairs: &[(Vector3<f64>, Vector3<f64>)]) -> Matrix3<f64> {
    let mut h = Matrix3::zeros();
    for (a, b) in pairs {
        h += a * b.transpose();
    }
    let svd = h.svd(true, true);
    let u = svd.u.unwrap();
    let v = svd.v_t.unwrap().transpose();
    let mut d = Matrix3::identity();
    d[(2, 2)] = (v * u.transpose()).determinant().signum();
    v * d * u.transpose()
}

fn rotation_angle_axis(r: &Matrix3<f64>) -> (f64, Vector3<f64>) {
    let trace = r[(0, 0)] + r[(1, 1)] + r[(2, 2)];
    let cos = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0);
    let angle = cos.acos();
    let axis = Vector3::new(
        r[(2, 1)] - r[(1, 2)],
        r[(0, 2)] - r[(2, 0)],
        r[(1, 0)] - r[(0, 1)],
    );
    let n = axis.norm();
    let axis = if n > 1e-9 { axis / n } else { Vector3::new(0.0, 0.0, 1.0) };
    (angle, axis)
}
