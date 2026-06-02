use crate::spin_detector::{FrameSpin, SpinDetector};
use image::GrayImage;
use nalgebra::{Matrix3, Vector3};
use rayon::prelude::*;

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

struct HpImage {
    w: i64,
    h: i64,
    data: Vec<f64>,
}

impl HpImage {
    fn from_gray(gray: &GrayImage, sigma: f32) -> HpImage {
        let blurred = imageproc::filter::gaussian_blur_f32(gray, sigma.max(0.5));
        let (w, h) = gray.dimensions();
        let mut data = vec![0.0; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                data[i] = gray.get_pixel(x, y).0[0] as f64 - blurred.get_pixel(x, y).0[0] as f64;
            }
        }
        HpImage { w: w as i64, h: h as i64, data }
    }

    fn sample(&self, x: f64, y: f64) -> Option<f64> {
        let x0 = x.floor() as i64;
        let y0 = y.floor() as i64;
        if x0 < 0 || y0 < 0 || x0 + 1 >= self.w || y0 + 1 >= self.h {
            return None;
        }
        let fx = x - x0 as f64;
        let fy = y - y0 as f64;
        let idx = |xx: i64, yy: i64| -> f64 { self.data[(yy * self.w + xx) as usize] };
        let top = idx(x0, y0) * (1.0 - fx) + idx(x0 + 1, y0) * fx;
        let bot = idx(x0, y0 + 1) * (1.0 - fx) + idx(x0 + 1, y0 + 1) * fx;
        Some(top * (1.0 - fy) + bot * fy)
    }
}

pub fn estimate_spin_dense(frames: &[FrameSpin], fps: f64) -> Option<InterframeSpin> {
    if frames.len() < 2 {
        return None;
    }

    let hps: Vec<HpImage> = frames
        .iter()
        .map(|f| HpImage::from_gray(&f.gray, (0.30 * f.radius) as f32))
        .collect();
    let samples: Vec<Vec<(Vector3<f64>, f64)>> = frames
        .iter()
        .zip(hps.iter())
        .map(|(f, hp)| ball_samples(hp, (f.center_x, f.center_y), f.radius))
        .collect();

    let coarse_axes = fib_sphere(96);
    let coarse_thetas: Vec<f64> = (1..=13).map(|i| (i as f64 * 10.0).to_radians()).collect();

    let mut rates = Vec::new();
    let mut axes = Vec::new();
    let mut nccs = Vec::new();

    for i in 0..frames.len() - 1 {
        let a = &samples[i];
        let hp_b = &hps[i + 1];
        let cb = (frames[i + 1].center_x, frames[i + 1].center_y);
        let rb = frames[i + 1].radius;
        if a.len() < 30 {
            continue;
        }

        let mut hyps: Vec<(Vector3<f64>, f64)> = Vec::new();
        for &ax in &coarse_axes {
            for &th in &coarse_thetas {
                hyps.push((ax, th));
            }
        }
        let coarse = best_hypothesis(&hyps, a, hp_b, cb, rb);

        let mut fine: Vec<(Vector3<f64>, f64)> = Vec::new();
        let neighbors = perturbed_axes(coarse.0, 8.0_f64.to_radians());
        let mut th = (coarse.1 - 12.0_f64.to_radians()).max(0.5_f64.to_radians());
        while th <= coarse.1 + 12.0_f64.to_radians() {
            for &ax in &neighbors {
                fine.push((ax, th));
            }
            th += 0.5_f64.to_radians();
        }
        let best = best_hypothesis(&fine, a, hp_b, cb, rb);

        let r = rot_axis_angle(best.0, best.1);
        let (angle, axis) = rotation_angle_axis(&r);
        if angle > 1e-4 && best.2 > 0.0 {
            rates.push(angle * fps * 60.0 / (2.0 * std::f64::consts::PI));
            axes.push(axis);
            nccs.push(best.2);
        }
    }

    if rates.is_empty() {
        return None;
    }

    rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rpm = rates[rates.len() / 2];
    let axis_mean = axes.iter().fold(Vector3::zeros(), |acc, a| acc + a) / axes.len() as f64;
    let axis_deg = axis_mean.y.atan2(axis_mean.x).to_degrees();
    let confidence = nccs.iter().sum::<f64>() / nccs.len() as f64;

    Some(InterframeSpin { rpm, axis_deg, confidence, pairs: rates.len() })
}

fn ball_samples(hp: &HpImage, center: (f64, f64), radius: f64) -> Vec<(Vector3<f64>, f64)> {
    let mut out = Vec::new();
    let rmax = 0.85_f64;
    let step = 0.05_f64;
    let mut v = -rmax;
    while v <= rmax {
        let mut u = -rmax;
        while u <= rmax {
            let r2 = u * u + v * v;
            if r2 <= rmax * rmax {
                let z = (1.0 - r2).sqrt();
                if let Some(val) = hp.sample(center.0 + u * radius, center.1 + v * radius) {
                    out.push((Vector3::new(u, v, z), val));
                }
            }
            u += step;
        }
        v += step;
    }
    out
}

fn best_hypothesis(
    hyps: &[(Vector3<f64>, f64)],
    a: &[(Vector3<f64>, f64)],
    hp_b: &HpImage,
    cb: (f64, f64),
    rb: f64,
) -> (Vector3<f64>, f64, f64) {
    hyps.par_iter()
        .map(|&(axis, theta)| {
            let r = rot_axis_angle(axis, theta);
            let mut va = Vec::with_capacity(a.len());
            let mut vb = Vec::with_capacity(a.len());
            for (p, ia) in a {
                let p2 = r * p;
                if p2.z > 0.1 {
                    if let Some(ib) = hp_b.sample(cb.0 + p2.x * rb, cb.1 + p2.y * rb) {
                        va.push(*ia);
                        vb.push(ib);
                    }
                }
            }
            (axis, theta, ncc(&va, &vb))
        })
        .max_by(|x, y| x.2.partial_cmp(&y.2).unwrap())
        .unwrap()
}

fn ncc(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    if a.len() < 40 {
        return -2.0;
    }
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..a.len() {
        let x = a[i] - ma;
        let y = b[i] - mb;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da <= 1e-9 || db <= 1e-9 {
        return -2.0;
    }
    num / (da.sqrt() * db.sqrt())
}

fn fib_sphere(n: usize) -> Vec<Vector3<f64>> {
    let phi = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let t = phi * i as f64;
            Vector3::new(r * t.cos(), y, r * t.sin())
        })
        .collect()
}

fn perturbed_axes(axis: Vector3<f64>, ang: f64) -> Vec<Vector3<f64>> {
    let mut out = vec![axis];
    let helper = if axis.x.abs() < 0.9 {
        Vector3::new(1.0, 0.0, 0.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    let e1 = axis.cross(&helper).normalize();
    let e2 = axis.cross(&e1).normalize();
    for k in 0..6 {
        let a = k as f64 * std::f64::consts::FRAC_PI_3;
        let dir = e1 * a.cos() + e2 * a.sin();
        out.push((axis * ang.cos() + dir * ang.sin()).normalize());
    }
    out
}

fn rot_axis_angle(axis: Vector3<f64>, theta: f64) -> Matrix3<f64> {
    let k = axis.normalize();
    let (s, c) = theta.sin_cos();
    let kx = Matrix3::new(0.0, -k.z, k.y, k.z, 0.0, -k.x, -k.y, k.x, 0.0);
    Matrix3::identity() + kx * s + kx * kx * (1.0 - c)
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
