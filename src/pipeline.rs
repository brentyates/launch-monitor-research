use crate::ball_detector::{BallDetection, BallDetector, DetectionError};
use crate::config::StereoRig;
use crate::debug::{colors, DebugFrame, Overlay};
use crate::frame_source::{GroundTruth, SourceConfig, StereoFrame};
use crate::spin_detector::{FrameSpin, SpinDetector, SpinResult};
use crate::triangulation::{LaunchData, Point3D, StereoTriangulator};
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub launch: Option<LaunchData>,
    pub spin: Option<SpinResult>,
    pub ground_truth: Option<GroundTruth>,
    pub frame_count: u32,
    pub errors: Vec<String>,
}

fn filter_radius_outliers(
    pairs: Vec<(u32, f64, BallDetection, BallDetection)>,
) -> Vec<(u32, f64, BallDetection, BallDetection)> {
    if pairs.len() < 3 {
        return pairs;
    }

    let mut radii: Vec<f64> = pairs
        .iter()
        .flat_map(|(_, _, l, r)| [l.radius, r.radius])
        .collect();
    radii.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = radii[radii.len() / 2];
    let max_radius = median * 2.5;

    let before = pairs.len();
    let filtered: Vec<_> = pairs
        .into_iter()
        .filter(|(_, _, l, r)| l.radius <= max_radius && r.radius <= max_radius)
        .collect();

    if filtered.len() < before {
        info!(
            "Radius filter: removed {} of {} detections (median r={:.1}, max={:.1})",
            before - filtered.len(),
            before,
            median,
            max_radius,
        );
    }

    filtered
}

fn filter_static_start(
    pairs: Vec<(u32, f64, BallDetection, BallDetection)>,
) -> Vec<(u32, f64, BallDetection, BallDetection)> {
    if pairs.len() <= 2 {
        return pairs;
    }

    let threshold = 3.0;
    let first = &pairs[0];

    let first_moving = pairs.iter().position(|p| {
        let dx = (p.2.center_x - first.2.center_x).abs();
        let dy = (p.2.center_y - first.2.center_y).abs();
        dx > threshold || dy > threshold
    });

    match first_moving {
        Some(idx) if idx > 0 => {
            info!("Removed {} static frames at start (render lag)", idx);
            pairs[idx..].to_vec()
        }
        _ => pairs,
    }
}

fn filter_flight_detections(
    pairs: Vec<(u32, f64, BallDetection, BallDetection)>,
) -> Vec<(u32, f64, BallDetection, BallDetection)> {
    if pairs.len() <= 1 {
        return pairs;
    }

    let max_gap = 3;
    let mut best_start = 0;
    let mut best_len = 1;
    let mut cur_start = 0;
    let mut cur_len = 1;

    for i in 1..pairs.len() {
        if pairs[i].0 - pairs[i - 1].0 <= max_gap {
            cur_len += 1;
        } else {
            if cur_len > best_len {
                best_start = cur_start;
                best_len = cur_len;
            }
            cur_start = i;
            cur_len = 1;
        }
    }
    if cur_len > best_len {
        best_start = cur_start;
        best_len = cur_len;
    }

    info!(
        "Filtered detections: kept {} of {} (frames {:?})",
        best_len,
        pairs.len(),
        pairs[best_start..best_start + best_len]
            .iter()
            .map(|(idx, _, _, _)| idx)
            .collect::<Vec<_>>()
    );

    pairs[best_start..best_start + best_len].to_vec()
}

fn project_ball_pixel(
    ball_pos_mm: [f32; 3],
    cam_index: usize,
    triangulator: &StereoTriangulator,
) -> (f64, f64) {
    let pt = Point3D {
        x: ball_pos_mm[0] as f64,
        y: ball_pos_mm[1] as f64,
        z: ball_pos_mm[2] as f64,
    };
    if cam_index == 0 {
        triangulator.project_cam0(&pt)
    } else {
        triangulator.project_cam1(&pt)
    }
}

fn save_debug_frame(
    debug_dir: &str,
    frame_idx: u32,
    camera: &str,
    gray: &image::GrayImage,
    detection: &Result<BallDetection, DetectionError>,
    ball_pos_mm: [f32; 3],
    triangulator: &StereoTriangulator,
) {
    let (width, height) = gray.dimensions();
    let mut debug_frame = DebugFrame::new(gray.clone(), "ball_detection");

    let cam_index = if camera == "left" { 0 } else { 1 };
    let (ex, ey) = project_ball_pixel(ball_pos_mm, cam_index, triangulator);

    if ex > 0.0 && ex < width as f64 && ey > 0.0 && ey < height as f64 {
        debug_frame.add_overlay(Overlay::circle((ex, ey), 8.0, colors::CYAN));
    }

    match detection {
        Ok(det) => {
            debug_frame.add_overlay(Overlay::circle(
                (det.center_x, det.center_y),
                det.radius,
                colors::GREEN,
            ));
            info!(
                "Frame {} {}: DETECTED ({:.1},{:.1}) r={:.1} | projected ({:.1},{:.1}) | ball_pos=({:.0},{:.0},{:.0})",
                frame_idx, camera, det.center_x, det.center_y, det.radius,
                ex, ey, ball_pos_mm[0], ball_pos_mm[1], ball_pos_mm[2]
            );
        }
        Err(e) => {
            info!(
                "Frame {} {}: {:?} | projected ({:.1},{:.1}) | ball_pos=({:.0},{:.0},{:.0})",
                frame_idx, camera, e, ex, ey,
                ball_pos_mm[0], ball_pos_mm[1], ball_pos_mm[2]
            );
        }
    }

    let rgb = debug_frame.render_to_rgb();
    let path = format!("{}/frame_{:04}_{}.png", debug_dir, frame_idx, camera);
    rgb.save(&path).ok();
}

pub fn process_shot(
    frames: &[StereoFrame],
    config: &SourceConfig,
    ground_truth: Option<GroundTruth>,
    detector: &mut BallDetector,
    debug_dir: &str,
    rig: &StereoRig,
) -> ProcessingResult {
    let mut errors = Vec::new();
    let frame_count = frames.len() as u32;

    let mut cam0_detections = Vec::new();
    let mut cam1_detections = Vec::new();

    if std::path::Path::new(debug_dir).exists() {
        std::fs::remove_dir_all(debug_dir).ok();
    }
    std::fs::create_dir_all(debug_dir).ok();

    let triangulator = StereoTriangulator::new(rig);

    let gray_pairs: Vec<_> = frames.iter().map(|f| (f.left.clone(), f.right.clone())).collect();
    detector.set_background_from_min(&gray_pairs);
    info!("Set temporal minimum background from {} frames", frames.len());

    for frame in frames {
        let left_result = detector.detect_with_background(&frame.left, true);
        let right_result = detector.detect_with_background(&frame.right, false);

        match &left_result {
            Ok(det) => {
                info!("Frame {} left: center=({:.1},{:.1}) r={:.1}", frame.frame_index, det.center_x, det.center_y, det.radius);
                cam0_detections.push((frame.frame_index, frame.timestamp, *det));
            }
            Err(e) => {
                info!("Frame {} left: {}", frame.frame_index, e);
            }
        }
        match &right_result {
            Ok(det) => {
                info!("Frame {} right: center=({:.1},{:.1}) r={:.1}", frame.frame_index, det.center_x, det.center_y, det.radius);
                cam1_detections.push((frame.frame_index, frame.timestamp, *det));
            }
            Err(e) => {
                info!("Frame {} right: {}", frame.frame_index, e);
            }
        }

        if let Some(diff) = detector.get_diff_image(&frame.left, true) {
            let diff_path = format!("{}/diff_{:04}_left.png", debug_dir, frame.frame_index);
            diff.save(&diff_path).ok();
        }
        if let Some(diff) = detector.get_diff_image(&frame.right, false) {
            let diff_path = format!("{}/diff_{:04}_right.png", debug_dir, frame.frame_index);
            diff.save(&diff_path).ok();
        }

        save_debug_frame(
            debug_dir,
            frame.frame_index,
            "left",
            &frame.left,
            &left_result,
            frame.ball_position_mm,
            &triangulator,
        );
        save_debug_frame(
            debug_dir,
            frame.frame_index,
            "right",
            &frame.right,
            &right_result,
            frame.ball_position_mm,
            &triangulator,
        );
    }

    info!(
        "Detected ball in {} cam0 / {} cam1 frames (out of {} total)",
        cam0_detections.len(),
        cam1_detections.len(),
        frames.len()
    );

    if cam0_detections.is_empty() && cam1_detections.is_empty() {
        info!("No ball detections in any frame!");
        info!("Check {}/ directory for frame images", debug_dir);
    } else {
        info!(
            "Cam0 detections at frames: {:?}",
            cam0_detections.iter().map(|(idx, _, _)| idx).collect::<Vec<_>>()
        );
        info!(
            "Cam1 detections at frames: {:?}",
            cam1_detections.iter().map(|(idx, _, _)| idx).collect::<Vec<_>>()
        );
    }

    let mut launch = None;

    if cam0_detections.len() >= 2 && cam1_detections.len() >= 2 {
        let mut detection_pairs = Vec::new();
        for (idx0, ts0, det0) in &cam0_detections {
            if let Some((_, _, det1)) = cam1_detections.iter().find(|(idx1, _, _)| idx1 == idx0) {
                detection_pairs.push((*idx0, *ts0, *det0, *det1));
            }
        }

        let detection_pairs = filter_radius_outliers(detection_pairs);
        let detection_pairs = filter_static_start(detection_pairs);
        let detection_pairs = filter_flight_detections(detection_pairs);

        if detection_pairs.len() >= 2 {
            let timestamps: Vec<_> = detection_pairs.iter().map(|(idx, ts, _, _)| format!("f{}={:.4}", idx, ts)).collect();
            info!("Frame timestamps: {}", timestamps.join(", "));

            match triangulator.estimate_launch(&detection_pairs) {
                Ok(ld) => {
                    for (i, pos) in ld.positions.iter().enumerate() {
                        let frame_idx = detection_pairs[i].0;
                        let gt_frame = frames.iter().find(|f| f.frame_index == frame_idx);
                        if let Some(gt_f) = gt_frame {
                            let gt = &gt_f.ball_position_mm;
                            info!(
                                "Triangulated[f{}]: ({:.1},{:.1},{:.1}) vs GT ({:.1},{:.1},{:.1})",
                                frame_idx, pos.x, pos.y, pos.z, gt[0], gt[1], gt[2]
                            );
                        } else {
                            info!(
                                "Triangulated[f{}]: ({:.1},{:.1},{:.1})",
                                frame_idx, pos.x, pos.y, pos.z
                            );
                        }
                    }
                    info!(
                        "Estimated launch: {:.1} mph, VLA {:.1}°, HLA {:.1}°",
                        ld.speed_mph, ld.vla_deg, ld.hla_deg
                    );
                    launch = Some(ld);
                }
                Err(e) => {
                    error!("Launch estimation failed: {}", e);
                    errors.push(format!("Launch estimation: {}", e));
                }
            }
        } else {
            errors.push("Not enough matched detections for triangulation".to_string());
        }
    } else {
        errors.push("Not enough ball detections".to_string());
    }

    let spin = if cam0_detections.len() >= 2 {
        let spin_detector = SpinDetector::new(config.fps as f64);

        let spin_frames: Vec<FrameSpin> = cam0_detections
            .iter()
            .filter_map(|(idx, _, det)| {
                frames.iter().find(|f| f.frame_index == *idx).map(|f| {
                    FrameSpin {
                        gray: f.left.clone(),
                        center_x: det.center_x,
                        center_y: det.center_y,
                        radius: det.radius,
                    }
                })
            })
            .collect();

        match spin_detector.detect(&spin_frames) {
            Ok(s) => {
                info!(
                    "Detected spin: {:.0} RPM, axis {:.1}°, confidence {:.2}",
                    s.rpm, s.axis_deg, s.confidence
                );
                Some(s)
            }
            Err(e) => {
                debug!("Spin detection failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    ProcessingResult {
        launch,
        spin,
        ground_truth,
        frame_count,
        errors,
    }
}
