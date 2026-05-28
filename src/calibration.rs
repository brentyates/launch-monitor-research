use image::{GrayImage, Luma};
use image::imageops::FilterType;
use imageproc::corners::corners_fast9;
use imageproc::drawing::draw_cross_mut; // Useful for debugging if we save images
use nalgebra::{Matrix2, SymmetricEigen, Vector2};
use std::collections::HashMap;
use tracing::{info, warn};

// Unity CalibrationBoard default: 9x6 inner corners, 25mm square size
pub const EXPECTED_COLS: usize = 9;
pub const EXPECTED_ROWS: usize = 6;
pub const SQUARE_SIZE_MM: f32 = 25.0;

#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub success: bool,
    pub detected_corners: usize,
    pub corners: Vec<(f64, f64)>,
    pub board_found: bool,
    pub details: String,
}

pub fn detect_calibration_board(image: &GrayImage) -> CalibrationResult {
    let normalized = normalize_contrast(image);
    let mut candidates = Vec::new();

    let inverted = invert_image(&normalized);
    for &threshold in &[5u8, 10u8, 20u8, 30u8] {
        candidates.extend(collect_fast_corners(&normalized, threshold));
        candidates.extend(collect_fast_corners(&inverted, threshold));
    }

    // Upsample to help FAST detect small checkerboard corners
    let upsampled = image::imageops::resize(
        &normalized,
        normalized.width() * 2,
        normalized.height() * 2,
        FilterType::Triangle,
    );
    let upsampled_inverted = invert_image(&upsampled);
    for &threshold in &[5u8, 10u8, 20u8, 30u8] {
        candidates.extend(
            collect_fast_corners(&upsampled, threshold)
                .into_iter()
                .map(|(x, y)| (x / 2, y / 2)),
        );
        candidates.extend(
            collect_fast_corners(&upsampled_inverted, threshold)
                .into_iter()
                .map(|(x, y)| (x / 2, y / 2)),
        );
    }

    let merged = merge_corners(&candidates, 3);
    let expected_min = EXPECTED_COLS * EXPECTED_ROWS;
    let min_needed = expected_min.saturating_sub(8);
    let mut filtered = filter_checker_corners(&normalized, &merged, expected_min, min_needed);
    if filtered.len() < min_needed && merged.len() >= min_needed {
        filtered = merged.clone();
    }
    if filtered.len() < min_needed {
        warn!(
            "Calibration board detection failed: Found {} corners, needed {}",
            filtered.len(),
            expected_min
        );

        if filtered.is_empty() {
            let _ = image.save("/Users/byates/projects/launch-monitor-research/calibration_debug.png");
        }

        return CalibrationResult {
            success: false,
            detected_corners: filtered.len(),
            corners: Vec::new(),
            board_found: false,
            details: format!("Found only {} corners", filtered.len()),
        };
    }

    match extract_grid_corners(&filtered, EXPECTED_ROWS, EXPECTED_COLS) {
        Some(grid) => {
            let det_str = format!(
                "Found {} checker corners, grid extracted: {}x{}",
                filtered.len(),
                EXPECTED_COLS,
                EXPECTED_ROWS
            );
            info!("Calibration board detection: {}", det_str);
            CalibrationResult {
                success: true,
                detected_corners: filtered.len(),
                corners: grid,
                board_found: true,
                details: det_str,
            }
        }
        None => {
            if filtered.len() >= min_needed {
                if let Some(grid) = synthesize_grid_from_pca(&filtered, EXPECTED_ROWS, EXPECTED_COLS) {
                    let det_str = format!(
                        "Found {} checker corners, grid synthesized via PCA ({}x{})",
                        filtered.len(),
                        EXPECTED_COLS,
                        EXPECTED_ROWS
                    );
                    info!("Calibration board detection: {}", det_str);
                    return CalibrationResult {
                        success: true,
                        detected_corners: filtered.len(),
                        corners: grid,
                        board_found: true,
                        details: det_str,
                    };
                }
            }

            warn!(
                "Calibration board detection failed: Found {} corners, but grid fit failed",
                filtered.len()
            );
            CalibrationResult {
                success: false,
                detected_corners: filtered.len(),
                corners: Vec::new(),
                board_found: false,
                details: "Grid fit failed".to_string(),
            }
        }
    }
}

pub fn visualize_corners(image: &GrayImage) -> GrayImage {
    let mut debug_image = image.clone();
    let corners = corners_fast9(image, 30);
    
    for corner in corners {
        draw_cross_mut(&mut debug_image, Luma([255]), corner.x as i32, corner.y as i32);
    }
    
    debug_image
}

fn normalize_contrast(image: &GrayImage) -> GrayImage {
    let mut min_val = u8::MAX;
    let mut max_val = u8::MIN;
    for p in image.pixels() {
        let v = p[0];
        if v < min_val {
            min_val = v;
        }
        if v > max_val {
            max_val = v;
        }
    }

    if max_val <= min_val + 5 {
        return image.clone();
    }

    let scale = 255.0 / (max_val as f32 - min_val as f32);
    let mut out = image.clone();
    for p in out.pixels_mut() {
        let v = p[0] as f32;
        let nv = ((v - min_val as f32) * scale).clamp(0.0, 255.0) as u8;
        *p = Luma([nv]);
    }
    out
}

fn invert_image(image: &GrayImage) -> GrayImage {
    let mut out = image.clone();
    for p in out.pixels_mut() {
        p[0] = 255u8.saturating_sub(p[0]);
    }
    out
}

fn collect_fast_corners(image: &GrayImage, threshold: u8) -> Vec<(u32, u32)> {
    corners_fast9(image, threshold)
        .iter()
        .map(|c| (c.x, c.y))
        .collect()
}

fn merge_corners(points: &[(u32, u32)], radius: u32) -> Vec<(f64, f64)> {
    if points.is_empty() {
        return Vec::new();
    }

    let cell_size = radius.max(1) as i32;
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let mut clusters: Vec<(f64, f64, u32)> = Vec::new(); // sum_x, sum_y, count

    let radius_sq = (radius as f64) * (radius as f64);

    for &(x, y) in points {
        let cx = (x as i32) / cell_size;
        let cy = (y as i32) / cell_size;
        let mut found_idx = None;

        for ny in (cy - 1)..=(cy + 1) {
            for nx in (cx - 1)..=(cx + 1) {
                if let Some(indices) = grid.get(&(nx, ny)) {
                    for &idx in indices {
                        let (sx, sy, count) = clusters[idx];
                        let mx = sx / count as f64;
                        let my = sy / count as f64;
                        let dx = mx - x as f64;
                        let dy = my - y as f64;
                        if dx * dx + dy * dy <= radius_sq {
                            found_idx = Some(idx);
                            break;
                        }
                    }
                }
                if found_idx.is_some() {
                    break;
                }
            }
            if found_idx.is_some() {
                break;
            }
        }

        match found_idx {
            Some(idx) => {
                clusters[idx].0 += x as f64;
                clusters[idx].1 += y as f64;
                clusters[idx].2 += 1;
            }
            None => {
                let idx = clusters.len();
                clusters.push((x as f64, y as f64, 1));
                grid.entry((cx, cy)).or_default().push(idx);
            }
        }
    }

    clusters
        .into_iter()
        .map(|(sx, sy, count)| (sx / count as f64, sy / count as f64))
        .collect()
}

fn filter_checker_corners(
    image: &GrayImage,
    corners: &[(f64, f64)],
    expected_min: usize,
    min_needed: usize,
) -> Vec<(f64, f64)> {
    if corners.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<((f64, f64), f64)> = Vec::with_capacity(corners.len());
    for &(x, y) in corners {
        let score = checker_response(image, x as f32, y as f32, 3);
        scored.push(((x, y), score));
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let threshold = if scored.len() >= expected_min {
        scored
            .get(expected_min.saturating_sub(1))
            .map(|(_, s)| *s)
            .unwrap_or(0.0)
    } else {
        scored
            .get(min_needed.saturating_sub(1))
            .map(|(_, s)| *s)
            .unwrap_or(0.0)
    };

    let mut filtered: Vec<(f64, f64)> = scored
        .iter()
        .filter(|(_, s)| *s >= threshold.max(0.05))
        .map(|(p, _)| *p)
        .collect();

    if filtered.len() < min_needed {
        filtered = scored
            .into_iter()
            .take(min_needed.max(1))
            .map(|(p, _)| p)
            .collect();
    }

    filtered
}

fn checker_response(image: &GrayImage, x: f32, y: f32, radius: i32) -> f64 {
    let (w, h) = image.dimensions();
    let xi = x.round() as i32;
    let yi = y.round() as i32;
    let r = radius;
    if xi - r < 1 || yi - r < 1 || xi + r + 1 >= w as i32 || yi + r + 1 >= h as i32 {
        return 0.0;
    }

    let mut sum_q = [0.0f64; 4];
    let mut count_q = [0u32; 4];
    for dy in -r..=r {
        for dx in -r..=r {
            let px = xi + dx;
            let py = yi + dy;
            let v = image.get_pixel(px as u32, py as u32)[0] as f64 / 255.0;
            let quad = match (dx >= 0, dy >= 0) {
                (false, false) => 0,
                (true, false) => 1,
                (false, true) => 2,
                (true, true) => 3,
            };
            sum_q[quad] += v;
            count_q[quad] += 1;
        }
    }

    let mut mean = [0.0f64; 4];
    for i in 0..4 {
        if count_q[i] == 0 {
            return 0.0;
        }
        mean[i] = sum_q[i] / count_q[i] as f64;
    }

    let d1 = (mean[0] - mean[3]).abs();
    let d2 = (mean[1] - mean[2]).abs();
    let checker = (d1 + d2) * 0.5;
    let contrast = (mean.iter().cloned().fold(f64::MIN, f64::max)
        - mean.iter().cloned().fold(f64::MAX, f64::min))
        .max(0.0);

    (checker * contrast).min(1.0)
}

fn extract_grid_corners(
    corners: &[(f64, f64)],
    rows: usize,
    cols: usize,
) -> Option<Vec<(f64, f64)>> {
    if corners.len() < rows * cols {
        return None;
    }

    let mean = corners
        .iter()
        .fold(Vector2::new(0.0, 0.0), |acc, &(x, y)| acc + Vector2::new(x, y))
        / (corners.len() as f64);

    let mut cov = Matrix2::zeros();
    for &(x, y) in corners {
        let v = Vector2::new(x, y) - mean;
        cov[(0, 0)] += v.x * v.x;
        cov[(0, 1)] += v.x * v.y;
        cov[(1, 0)] += v.y * v.x;
        cov[(1, 1)] += v.y * v.y;
    }
    cov /= corners.len() as f64;

    let eig = SymmetricEigen::new(cov);
    let mut axes = [eig.eigenvectors.column(0).into_owned(), eig.eigenvectors.column(1).into_owned()];
    if eig.eigenvalues[1] > eig.eigenvalues[0] {
        axes.swap(0, 1);
    }

    let axis_u = axes[0];
    let axis_v = axes[1];

    let mut projected: Vec<(f64, f64, (f64, f64))> = Vec::with_capacity(corners.len());
    for &(x, y) in corners {
        let v = Vector2::new(x, y) - mean;
        let u = v.dot(&axis_u);
        let w = v.dot(&axis_v);
        projected.push((u, w, (x, y)));
    }

    let min_u = projected.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_u = projected.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_v = projected.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_v = projected.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    let spacing_u = (max_u - min_u) / (cols as f64 - 1.0);
    let spacing_v = (max_v - min_v) / (rows as f64 - 1.0);

    if spacing_u.abs() < 1.0 || spacing_v.abs() < 1.0 {
        return None;
    }

    let mut grid: Vec<Vec<Option<((f64, f64), f64)>>> = vec![vec![None; cols]; rows];
    let max_dist_u = spacing_u.abs() * 0.6;
    let max_dist_v = spacing_v.abs() * 0.6;

    for (u, v, orig) in projected {
        let col_f = (u - min_u) / spacing_u;
        let row_f = (v - min_v) / spacing_v;
        let col = col_f.round() as isize;
        let row = row_f.round() as isize;
        if row < 0 || row >= rows as isize || col < 0 || col >= cols as isize {
            continue;
        }

        let du = (col_f - col as f64).abs() * spacing_u.abs();
        let dv = (row_f - row as f64).abs() * spacing_v.abs();
        if du > max_dist_u || dv > max_dist_v {
            continue;
        }

        let row = row as usize;
        let col = col as usize;
        let dist = du * du + dv * dv;
        if let Some((_, best_dist)) = grid[row][col].as_mut() {
            if dist < *best_dist {
                *best_dist = dist;
                grid[row][col] = Some((orig, dist));
            }
        } else {
            grid[row][col] = Some((orig, dist));
        }
    }

    let mut ordered = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        for col in 0..cols {
            match grid[row][col] {
                Some((p, _)) => ordered.push(p),
                None => return None,
            }
        }
    }

    Some(ordered)
}

fn synthesize_grid_from_pca(
    corners: &[(f64, f64)],
    rows: usize,
    cols: usize,
) -> Option<Vec<(f64, f64)>> {
    if corners.len() < rows * cols / 2 {
        return None;
    }

    let mean = corners
        .iter()
        .fold(Vector2::new(0.0, 0.0), |acc, &(x, y)| acc + Vector2::new(x, y))
        / (corners.len() as f64);

    let mut cov = Matrix2::zeros();
    for &(x, y) in corners {
        let v = Vector2::new(x, y) - mean;
        cov[(0, 0)] += v.x * v.x;
        cov[(0, 1)] += v.x * v.y;
        cov[(1, 0)] += v.y * v.x;
        cov[(1, 1)] += v.y * v.y;
    }
    cov /= corners.len() as f64;

    let eig = SymmetricEigen::new(cov);
    let mut axes = [eig.eigenvectors.column(0).into_owned(), eig.eigenvectors.column(1).into_owned()];
    if eig.eigenvalues[1] > eig.eigenvalues[0] {
        axes.swap(0, 1);
    }

    let axis_u = axes[0];
    let axis_v = axes[1];

    let mut u_vals = Vec::with_capacity(corners.len());
    let mut v_vals = Vec::with_capacity(corners.len());
    for &(x, y) in corners {
        let v = Vector2::new(x, y) - mean;
        u_vals.push(v.dot(&axis_u));
        v_vals.push(v.dot(&axis_v));
    }

    u_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min_u = percentile_sorted(&u_vals, 0.05);
    let max_u = percentile_sorted(&u_vals, 0.95);
    let min_v = percentile_sorted(&v_vals, 0.05);
    let max_v = percentile_sorted(&v_vals, 0.95);

    let spacing_u = (max_u - min_u) / (cols as f64 - 1.0);
    let spacing_v = (max_v - min_v) / (rows as f64 - 1.0);

    if spacing_u.abs() < 1.0 || spacing_v.abs() < 1.0 {
        return None;
    }

    let mut ordered = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        let v = min_v + row as f64 * spacing_v;
        for col in 0..cols {
            let u = min_u + col as f64 * spacing_u;
            let point = mean + axis_u * u + axis_v * v;
            ordered.push((point.x, point.y));
        }
    }

    Some(ordered)
}

fn percentile_sorted(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let idx = ((values.len() - 1) as f64 * p.clamp(0.0, 1.0)).round() as usize;
    values[idx]
}
