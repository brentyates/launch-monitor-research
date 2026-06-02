use image::GrayImage;
use launch_monitor_research::{
    load_rig_dataset, BallDetector, FrameSpin, SpinDetector,
};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::process::Command;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
struct BaseShot {
    speed_mph: f64,
    vla_deg: f64,
    hla_deg: f64,
}

#[derive(Debug, Deserialize)]
struct SpinSweepSpec {
    base_shot: BaseShot,
    spin_axis_deg: f64,
    fps_list: Vec<f64>,
    focal_list: Vec<f64>,
    spin_rpm_list: Vec<f64>,
}

struct Row {
    fps: f64,
    focal_mm: f64,
    rpm: f64,
    rot_per_frame: f64,
    frames: usize,
    ball_px: f64,
    rate_err_pct: f64,
    axis_err_deg: f64,
    tier: &'static str,
}

const AIM_Y_MM: f64 = 437.116;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let project_dir = env!("CARGO_MANIFEST_DIR");
    let force_render = std::env::var("RENDER").map(|v| v == "1").unwrap_or(false);
    let spec_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("{}/configs/spin_sweep.json", project_dir));

    let spec: SpinSweepSpec =
        serde_json::from_str(&std::fs::read_to_string(&spec_path).expect("read spec")).expect("parse spec");

    let total = spec.fps_list.len() * spec.focal_list.len() * spec.spin_rpm_list.len();
    println!("Spin sweep: {} combinations\n", total);

    let mut rows: Vec<Row> = Vec::new();

    for &fps in &spec.fps_list {
        for &focal in &spec.focal_list {
            for &rpm in &spec.spin_rpm_list {
                let id = format!("f{}_fps{}_rpm{}", focal as i64, fps as i64, rpm as i64);
                let dir = format!("{}/renders/spin_sweep/{}", project_dir, id);
                let manifest = format!("{}/manifest.json", dir);

                if force_render || !Path::new(&manifest).exists() {
                    write_rig(&dir, fps, focal);
                    if let Err(e) = render(project_dir, &dir, &spec, rpm) {
                        eprintln!("{}: render failed: {}", id, e);
                        continue;
                    }
                }

                let dataset = match load_rig_dataset(Path::new(&dir)) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("{}: load failed: {}", id, e);
                        continue;
                    }
                };

                let grays: Vec<GrayImage> = {
                    let mut seq = dataset.gray_seq("spin0");
                    seq.sort_by_key(|(idx, _, _)| *idx);
                    seq.into_iter().map(|(_, _, g)| g).collect()
                };

                let rot_per_frame = 6.0 * rpm / fps;
                let row = match detect_spin(&grays, fps) {
                    Some((rpm_est, axis_est, ball_px, n)) => {
                        let rate_err = ((rpm_est - rpm) / rpm * 100.0).abs();
                        let axis_err = (axis_est - spec.spin_axis_deg).abs();
                        Row {
                            fps, focal_mm: focal, rpm, rot_per_frame, frames: n, ball_px,
                            rate_err_pct: rate_err, axis_err_deg: axis_err,
                            tier: tier(rate_err, axis_err),
                        }
                    }
                    None => Row {
                        fps, focal_mm: focal, rpm, rot_per_frame, frames: grays.len(), ball_px: 0.0,
                        rate_err_pct: f64::NAN, axis_err_deg: f64::NAN, tier: "NODETECT",
                    },
                };
                println!(
                    "fps={:<5} f={:<3}mm rpm={:<6} rot/frame={:>5.1}° frames={:<2} px={:>5.1} rate_err={:>7.2}% axis_err={:>5.2}° [{}]",
                    row.fps, row.focal_mm, row.rpm, row.rot_per_frame, row.frames, row.ball_px,
                    row.rate_err_pct, row.axis_err_deg, row.tier
                );
                rows.push(row);
            }
        }
    }

    write_csv(project_dir, &rows);
    print_summary(&rows);
}

fn detect_spin(grays: &[GrayImage], fps: f64) -> Option<(f64, f64, f64, usize)> {
    if grays.len() < 2 {
        return None;
    }
    let pairs: Vec<(GrayImage, GrayImage)> = grays.iter().map(|g| (g.clone(), g.clone())).collect();
    let mut det = BallDetector::new(15, 10);
    det.set_background_from_min(&pairs);

    let mut frames = Vec::new();
    let mut radii = Vec::new();
    for g in grays {
        if let Ok(d) = det.detect_with_background(g, true) {
            radii.push(d.radius);
            frames.push(FrameSpin {
                gray: g.clone(),
                center_x: d.center_x,
                center_y: d.center_y,
                radius: d.radius,
            });
        }
    }
    if frames.len() < 2 {
        return None;
    }
    radii.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let ball_px = 2.0 * radii[radii.len() / 2];

    let detector = SpinDetector::new(fps);
    match detector.detect(&frames) {
        Ok(s) => Some((s.rpm, s.axis_deg, ball_px, frames.len())),
        Err(_) => None,
    }
}

fn tier(rate_pct: f64, axis: f64) -> &'static str {
    if rate_pct <= 1.0 && axis <= 1.0 {
        "PERFECT"
    } else if rate_pct <= 5.0 && axis <= 1.5 {
        "GOOD"
    } else if rate_pct <= 10.0 && axis <= 3.0 {
        "USEFUL"
    } else {
        "FAIL"
    }
}

fn write_rig(dir: &str, fps: f64, focal: f64) {
    std::fs::create_dir_all(dir).ok();
    let rig = json!({
        "name": "spin_sweep",
        "measures": ["spin"],
        "fps": fps,
        "strobe_us": 20.0,
        "samples": 32,
        "cameras": [{
            "id": "spin0", "role": "spin",
            "position_mm": [0.0, 1092.0, 3048.0],
            "aim_mm": [0.0, AIM_Y_MM, 0.0],
            "focal_mm": focal, "pixel_pitch_mm": 0.00508,
            "width": 512, "height": 384
        }]
    });
    std::fs::write(format!("{}/rig.json", dir), serde_json::to_string_pretty(&rig).unwrap()).ok();
}

fn render(project_dir: &str, dir: &str, spec: &SpinSweepSpec, rpm: f64) -> std::io::Result<()> {
    let blender = std::env::var("BLENDER")
        .unwrap_or_else(|_| "/Applications/Blender.app/Contents/MacOS/Blender".to_string());
    let status = Command::new(blender)
        .args([
            "--background", "--factory-startup", "--python",
            &format!("{}/blender/render_shot.py", project_dir), "--",
            "--rig", &format!("{}/rig.json", dir),
            "--case", "spin",
            "--speed", &spec.base_shot.speed_mph.to_string(),
            "--vla", &spec.base_shot.vla_deg.to_string(),
            "--hla", &spec.base_shot.hla_deg.to_string(),
            "--spin", &rpm.to_string(),
            "--axis", &spec.spin_axis_deg.to_string(),
            "--out", dir,
            "--frames", "24",
        ])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "blender failed"));
    }
    Ok(())
}

fn write_csv(project_dir: &str, rows: &[Row]) {
    let mut csv = String::from("fps,focal_mm,rpm,rot_per_frame_deg,frames,ball_px,rate_err_pct,axis_err_deg,tier\n");
    for r in rows {
        csv.push_str(&format!(
            "{},{},{},{:.2},{},{:.1},{:.3},{:.3},{}\n",
            r.fps, r.focal_mm, r.rpm, r.rot_per_frame, r.frames, r.ball_px,
            r.rate_err_pct, r.axis_err_deg, r.tier
        ));
    }
    std::fs::create_dir_all(format!("{}/results", project_dir)).ok();
    let path = format!("{}/results/spin_sweep.csv", project_dir);
    std::fs::write(&path, csv).expect("write csv");
    println!("\nFull results: {}", path);
}

fn print_summary(rows: &[Row]) {
    let perfect: Vec<&Row> = rows.iter().filter(|r| r.tier == "PERFECT").collect();
    println!("\n{}", "=".repeat(70));
    println!("  PERFECT spin detection ({} of {} combos):", perfect.len(), rows.len());
    println!("{}", "=".repeat(70));
    for r in &perfect {
        println!(
            "  fps={:<5} f={:<3}mm rpm={:<6} rot/frame={:>5.1}° frames={} px={:.0}",
            r.fps, r.focal_mm, r.rpm, r.rot_per_frame, r.frames, r.ball_px
        );
    }
    let ok: Vec<&Row> = rows.iter().filter(|r| r.tier == "PERFECT" || r.tier == "GOOD").collect();
    if !ok.is_empty() {
        let lo = ok.iter().map(|r| r.rot_per_frame).fold(f64::INFINITY, f64::min);
        let hi = ok.iter().map(|r| r.rot_per_frame).fold(f64::NEG_INFINITY, f64::max);
        println!("\n  PERFECT/GOOD spans rotation-per-frame ~{:.1}° to ~{:.1}°", lo, hi);
        println!("  (below ~{:.1}° = too little rotation/too few frames; above = aliasing)", lo);
    }
}
