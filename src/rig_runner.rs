use launch_monitor_research::{
    load_rig_dataset, BallDetector, Camera, FrameSpin, RigDataset, SpinDetector,
    StereoTriangulator,
};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tracing::info;

struct Shot {
    name: &'static str,
    speed_mph: f64,
    vla_deg: f64,
    hla_deg: f64,
    spin_rpm: f64,
    spin_axis_deg: f64,
}

const ENVELOPE: &[Shot] = &[
    Shot { name: "driver", speed_mph: 165.0, vla_deg: 10.5, hla_deg: -2.5, spin_rpm: 2700.0, spin_axis_deg: -5.0 },
    Shot { name: "7-iron", speed_mph: 120.0, vla_deg: 16.0, hla_deg: 1.2, spin_rpm: 7000.0, spin_axis_deg: 2.0 },
    Shot { name: "wedge", speed_mph: 85.0, vla_deg: 28.0, hla_deg: -0.8, spin_rpm: 9500.0, spin_axis_deg: 0.0 },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Good,
    Useful,
    Fail,
}

impl Tier {
    fn label(&self) -> &'static str {
        match self {
            Tier::Good => "GOOD",
            Tier::Useful => "USEFUL",
            Tier::Fail => "FAIL",
        }
    }
}

fn position_tier(speed_pct: f64, max_angle: f64) -> Tier {
    if speed_pct <= 1.0 && max_angle <= 0.5 {
        Tier::Good
    } else if speed_pct <= 2.0 && max_angle <= 1.0 {
        Tier::Useful
    } else {
        Tier::Fail
    }
}

fn spin_tier(rate_pct: f64, axis: f64) -> Tier {
    if rate_pct <= 5.0 && axis <= 1.5 {
        Tier::Good
    } else if rate_pct <= 10.0 && axis <= 3.0 {
        Tier::Useful
    } else {
        Tier::Fail
    }
}

struct ResultRow {
    shot: String,
    measure: &'static str,
    frames: usize,
    ball_px: f64,
    err_a: f64,
    err_b: f64,
    tier: Tier,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let project_dir = env!("CARGO_MANIFEST_DIR");
    let rig_name = std::env::args().nth(1).unwrap_or_else(|| "overhead_stereo".to_string());

    let mut rows: Vec<ResultRow> = Vec::new();

    for shot in ENVELOPE {
        let dir = format!("{}/renders/{}/{}", project_dir, rig_name, shot.name);
        let manifest = format!("{}/manifest.json", dir);

        let need_render = std::env::var("RENDER").map(|v| v == "1").unwrap_or(false)
            || !Path::new(&manifest).exists();

        if need_render {
            if let Err(e) = render_shot(project_dir, &rig_name, shot, &dir) {
                eprintln!("render failed for {}/{}: {}", rig_name, shot.name, e);
                continue;
            }
        }

        let dataset = match load_rig_dataset(Path::new(&dir)) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("load failed for {}/{}: {}", rig_name, shot.name, e);
                continue;
            }
        };

        if dataset.measures.iter().any(|m| m == "position") {
            if let Some(row) = run_position(&dataset, shot) {
                print_row(&row);
                rows.push(row);
            }
        }

        if dataset.measures.iter().any(|m| m == "spin") {
            if let Some(row) = run_spin(&dataset, shot) {
                print_row(&row);
                rows.push(row);
            }
        }
    }

    print_summary(&rows);
    if let Err(e) = write_csv(project_dir, &rig_name, &rows) {
        eprintln!("csv write failed: {}", e);
    }
}

fn run_position(dataset: &RigDataset, shot: &Shot) -> Option<ResultRow> {
    let pos_cams: Vec<&launch_monitor_research::CameraDef> =
        dataset.cameras.iter().filter(|c| c.role == "position").collect();
    if pos_cams.len() < 2 {
        eprintln!("{}: rig has fewer than 2 position cameras", shot.name);
        return None;
    }

    let cam_a = Camera::from_def(pos_cams[0]);
    let cam_b = Camera::from_def(pos_cams[1]);
    let triangulator = StereoTriangulator::from_pair(&cam_a, &cam_b);

    let seq_a = dataset.gray_seq(&cam_a.id);
    let seq_b = dataset.gray_seq(&cam_b.id);

    let det_a = detect_sequence(&seq_a);
    let det_b = detect_sequence(&seq_b);

    let mut detections = Vec::new();
    for (idx, ts, da) in &det_a {
        if let Some((_, _, db)) = det_b.iter().find(|(i, _, _)| i == idx) {
            detections.push((*idx, *ts, *da, *db));
        }
    }

    let mut radii: Vec<f64> = det_a.iter().map(|(_, _, d)| d.radius).collect();
    radii.extend(det_b.iter().map(|(_, _, d)| d.radius));
    let ball_px = 2.0 * median(&mut radii);

    let launch = match triangulator.estimate_launch(&detections) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}: launch estimation failed: {}", shot.name, e);
            return None;
        }
    };

    let gt = &dataset.ground_truth;
    let speed_pct = pct_error(launch.speed_mph, gt.speed_mph);
    let vla_err = (launch.vla_deg - gt.vla_deg).abs();
    let hla_err = (launch.hla_deg - gt.hla_deg).abs();
    let max_angle = vla_err.max(hla_err);
    let tier = position_tier(speed_pct, max_angle);

    info!(
        "{} position: speed {:.1} mph (gt {:.1}), vla {:.2}° (gt {:.2}), hla {:.2}° (gt {:.2})",
        shot.name, launch.speed_mph, gt.speed_mph, launch.vla_deg, gt.vla_deg, launch.hla_deg, gt.hla_deg
    );

    Some(ResultRow {
        shot: shot.name.to_string(),
        measure: "position",
        frames: detections.len(),
        ball_px,
        err_a: speed_pct,
        err_b: max_angle,
        tier,
    })
}

fn run_spin(dataset: &RigDataset, shot: &Shot) -> Option<ResultRow> {
    let spin_cam = match dataset.cameras.iter().find(|c| c.role == "spin") {
        Some(c) => c,
        None => {
            eprintln!("{}: rig has no spin camera", shot.name);
            return None;
        }
    };

    let seq = dataset.gray_seq(&spin_cam.id);
    let detections = detect_sequence(&seq);

    if detections.len() < 2 {
        eprintln!("{}: insufficient spin detections", shot.name);
        return None;
    }

    let mut radii: Vec<f64> = detections.iter().map(|(_, _, d)| d.radius).collect();
    let ball_px = 2.0 * median(&mut radii);

    let mut seq_map = std::collections::HashMap::new();
    for (idx, _, gray) in &seq {
        seq_map.insert(*idx, gray.clone());
    }

    let spin_frames: Vec<FrameSpin> = detections
        .iter()
        .filter_map(|(idx, _, det)| {
            seq_map.get(idx).map(|gray| FrameSpin {
                gray: gray.clone(),
                center_x: det.center_x,
                center_y: det.center_y,
                radius: det.radius,
            })
        })
        .collect();

    let detector = SpinDetector::new(dataset.fps as f64);
    let spin = match detector.detect(&spin_frames) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: spin detection failed: {}", shot.name, e);
            return None;
        }
    };

    let gt = &dataset.ground_truth;
    let rate_pct = pct_error(spin.rpm, gt.spin_rpm);
    let axis_err = (spin.axis_deg - gt.spin_axis_deg).abs();
    let tier = spin_tier(rate_pct, axis_err);

    info!(
        "{} spin: {:.0} rpm (gt {:.0}), axis {:.2}° (gt {:.2}), ball {:.1} px, {} frames",
        shot.name, spin.rpm, gt.spin_rpm, spin.axis_deg, gt.spin_axis_deg, ball_px, detections.len()
    );

    Some(ResultRow {
        shot: shot.name.to_string(),
        measure: "spin",
        frames: detections.len(),
        ball_px,
        err_a: rate_pct,
        err_b: axis_err,
        tier,
    })
}

fn detect_sequence(
    seq: &[(u32, f64, image::GrayImage)],
) -> Vec<(u32, f64, launch_monitor_research::BallDetection)> {
    if seq.is_empty() {
        return Vec::new();
    }

    let mut detector = BallDetector::new(15, 10);
    let pairs: Vec<(image::GrayImage, image::GrayImage)> =
        seq.iter().map(|(_, _, g)| (g.clone(), g.clone())).collect();
    detector.set_background_from_min(&pairs);

    let mut out = Vec::new();
    for (idx, ts, gray) in seq {
        if let Ok(det) = detector.detect_with_background(gray, true) {
            out.push((*idx, *ts, det));
        }
    }
    out
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}

fn pct_error(measured: f64, truth: f64) -> f64 {
    if truth.abs() < 1e-9 {
        return 0.0;
    }
    ((measured - truth) / truth).abs() * 100.0
}

fn print_row(row: &ResultRow) {
    if row.measure == "position" {
        println!(
            "{:<8} position  frames={:<3} ball_px={:>6.1}  speed_err={:>6.2}%  angle_err={:>5.2}°  [{}]",
            row.shot, row.frames, row.ball_px, row.err_a, row.err_b, row.tier.label()
        );
    } else {
        println!(
            "{:<8} spin      frames={:<3} ball_px={:>6.1}  rate_err={:>7.2}%  axis_err={:>5.2}°  [{}]",
            row.shot, row.frames, row.ball_px, row.err_a, row.err_b, row.tier.label()
        );
    }
}

fn print_summary(rows: &[ResultRow]) {
    println!("\n=== summary ===");
    for row in rows {
        println!("{:<8} {:<9} {}", row.shot, row.measure, row.tier.label());
    }
}

fn write_csv(project_dir: &str, rig_name: &str, rows: &[ResultRow]) -> std::io::Result<()> {
    let dir = format!("{}/results", project_dir);
    fs::create_dir_all(&dir)?;
    let path = format!("{}/{}.csv", dir, rig_name);
    let mut file = fs::File::create(&path)?;
    writeln!(file, "rig,shot,measure,frames,ball_px,err_a,err_b,tier")?;
    for row in rows {
        writeln!(
            file,
            "{},{},{},{},{:.2},{:.4},{:.4},{}",
            rig_name, row.shot, row.measure, row.frames, row.ball_px, row.err_a, row.err_b, row.tier.label()
        )?;
    }
    Ok(())
}

fn render_shot(project_dir: &str, rig_name: &str, shot: &Shot, dir: &str) -> std::io::Result<()> {
    let blender = std::env::var("BLENDER")
        .unwrap_or_else(|_| "/Applications/Blender.app/Contents/MacOS/Blender".to_string());
    let script = format!("{}/blender/render_shot.py", project_dir);
    let rig_path = format!("{}/rigs/{}.json", project_dir, rig_name);

    info!("Rendering {}/{} via Blender into {}", rig_name, shot.name, dir);

    let status = Command::new(blender)
        .args([
            "--background",
            "--factory-startup",
            "--python",
            &script,
            "--",
            "--rig",
            &rig_path,
            "--case",
            shot.name,
            "--speed",
            &shot.speed_mph.to_string(),
            "--vla",
            &shot.vla_deg.to_string(),
            "--hla",
            &shot.hla_deg.to_string(),
            "--spin",
            &shot.spin_rpm.to_string(),
            "--axis",
            &shot.spin_axis_deg.to_string(),
            "--out",
            dir,
            "--frames",
            "24",
        ])
        .status()?;

    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Blender exited with status {}", status),
        ));
    }

    Ok(())
}
