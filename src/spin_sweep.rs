use image::GrayImage;
use launch_monitor_research::{
    estimate_spin_dense, estimate_spin_interframe, load_rig_dataset, BallDetector, FrameSpin,
    SpinDetector,
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

fn one() -> usize {
    1
}

#[derive(Debug, Deserialize)]
struct SpinSweepSpec {
    base_shot: BaseShot,
    spin_axis_deg: f64,
    fps_list: Vec<f64>,
    focal_list: Vec<f64>,
    spin_rpm_list: Vec<f64>,
    #[serde(default = "one")]
    seeds: usize,
}

struct Cell {
    fps: f64,
    focal_mm: f64,
    rpm: f64,
    rot_per_frame: f64,
    frames: usize,
    ball_px: f64,
    n: usize,
    mean_err: f64,
    std_err: f64,
    min_err: f64,
    max_err: f64,
    perfect: usize,
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
    let seeds = spec.seeds.max(1);

    println!(
        "Spin sweep: {} cells x {} seeds = {} renders\n",
        spec.fps_list.len() * spec.focal_list.len() * spec.spin_rpm_list.len(),
        seeds,
        spec.fps_list.len() * spec.focal_list.len() * spec.spin_rpm_list.len() * seeds
    );

    let mut cells: Vec<Cell> = Vec::new();
    let mut csv = String::from("fps,focal_mm,rpm,seed,frames,ball_px,rate_err_pct,axis_err_deg,tier\n");

    for &fps in &spec.fps_list {
        for &focal in &spec.focal_list {
            for &rpm in &spec.spin_rpm_list {
                let cell_id = format!("f{}_fps{}_rpm{}", focal as i64, fps as i64, rpm as i64);
                let cell_dir = format!("{}/renders/spin_sweep/{}", project_dir, cell_id);
                write_rig(&cell_dir, fps, focal);

                let mut errs: Vec<f64> = Vec::new();
                let mut frames = 0usize;
                let mut ball_px = 0.0;
                let mut perfect = 0usize;

                for seed in 0..seeds {
                    let out = format!("{}/seed{}", cell_dir, seed);
                    if force_render || !Path::new(&format!("{}/manifest.json", out)).exists() {
                        if let Err(e) = render(project_dir, &cell_dir, &out, &spec, rpm, seed) {
                            eprintln!("{} seed{}: render failed: {}", cell_id, seed, e);
                            continue;
                        }
                    }
                    let dataset = match load_rig_dataset(Path::new(&out)) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let grays: Vec<GrayImage> = {
                        let mut seq = dataset.gray_seq("spin0");
                        seq.sort_by_key(|(idx, _, _)| *idx);
                        seq.into_iter().map(|(_, _, g)| g).collect()
                    };
                    let (rate_err, axis_err, px, n) = match detect_spin(&grays, fps) {
                        Some((rpm_est, axis_est, px, n)) => (
                            ((rpm_est - rpm) / rpm * 100.0).abs(),
                            (axis_est - spec.spin_axis_deg).abs(),
                            px,
                            n,
                        ),
                        None => (f64::NAN, f64::NAN, 0.0, grays.len()),
                    };
                    let t = if rate_err.is_nan() { "NODETECT" } else { tier(rate_err, axis_err) };
                    csv.push_str(&format!(
                        "{},{},{},{},{},{:.1},{:.3},{:.3},{}\n",
                        fps, focal, rpm, seed, n, px, rate_err, axis_err, t
                    ));
                    if !rate_err.is_nan() {
                        errs.push(rate_err);
                        frames = n;
                        ball_px = px;
                        if t == "PERFECT" {
                            perfect += 1;
                        }
                    }
                }

                let cell = summarize(fps, focal, rpm, frames, ball_px, perfect, &errs);
                println!(
                    "fps={:<5} f={:<3}mm rpm={:<6} rot/frame={:>5.1}° frames={:<2} px={:>5.1} | rate_err mean={:>6.1}% std={:>6.1}% [{:.1}–{:.1}] perfect={}/{}",
                    cell.fps, cell.focal_mm, cell.rpm, cell.rot_per_frame, cell.frames, cell.ball_px,
                    cell.mean_err, cell.std_err, cell.min_err, cell.max_err, cell.perfect, seeds
                );
                cells.push(cell);
            }
        }
    }

    write_csv(project_dir, &csv);
    print_summary(&cells, seeds);
}

fn summarize(
    fps: f64, focal_mm: f64, rpm: f64, frames: usize, ball_px: f64, perfect: usize, errs: &[f64],
) -> Cell {
    let rot_per_frame = 6.0 * rpm / fps;
    let n = errs.len();
    let (mean_err, std_err, min_err, max_err) = if n == 0 {
        (f64::NAN, f64::NAN, f64::NAN, f64::NAN)
    } else {
        let mean = errs.iter().sum::<f64>() / n as f64;
        let var = errs.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / n as f64;
        let min = errs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = errs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        (mean, var.sqrt(), min, max)
    };
    Cell { fps, focal_mm, rpm, rot_per_frame, frames, ball_px, n, mean_err, std_err, min_err, max_err, perfect }
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

    let method = std::env::var("SPIN_METHOD").unwrap_or_else(|_| "search".to_string());
    if method == "dense" {
        let s = estimate_spin_dense(&frames, fps)?;
        Some((s.rpm, s.axis_deg, ball_px, frames.len()))
    } else if method == "interframe" {
        let s = estimate_spin_interframe(&frames, fps)?;
        Some((s.rpm, s.axis_deg, ball_px, frames.len()))
    } else {
        let detector = SpinDetector::new(fps);
        match detector.detect(&frames) {
            Ok(s) => Some((s.rpm, s.axis_deg, ball_px, frames.len())),
            Err(_) => None,
        }
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

fn render(project_dir: &str, cell_dir: &str, out: &str, spec: &SpinSweepSpec, rpm: f64, seed: usize) -> std::io::Result<()> {
    let blender = std::env::var("BLENDER")
        .unwrap_or_else(|_| "/Applications/Blender.app/Contents/MacOS/Blender".to_string());
    let status = Command::new(blender)
        .args([
            "--background", "--factory-startup", "--python",
            &format!("{}/blender/render_shot.py", project_dir), "--",
            "--rig", &format!("{}/rig.json", cell_dir),
            "--case", "spin",
            "--speed", &spec.base_shot.speed_mph.to_string(),
            "--vla", &spec.base_shot.vla_deg.to_string(),
            "--hla", &spec.base_shot.hla_deg.to_string(),
            "--spin", &rpm.to_string(),
            "--axis", &spec.spin_axis_deg.to_string(),
            "--out", out,
            "--frames", "24",
            "--seed", &seed.to_string(),
        ])
        .status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "blender failed"));
    }
    Ok(())
}

fn write_csv(project_dir: &str, csv: &str) {
    std::fs::create_dir_all(format!("{}/results", project_dir)).ok();
    let path = format!("{}/results/spin_sweep.csv", project_dir);
    std::fs::write(&path, csv).expect("write csv");
    println!("\nPer-seed results: {}", path);
}

fn print_summary(cells: &[Cell], seeds: usize) {
    println!("\n{}", "=".repeat(78));
    println!("  ROBUSTNESS (mean error and spread across {} noise seeds per cell)", seeds);
    println!("{}", "=".repeat(78));
    let robust: Vec<&Cell> = cells
        .iter()
        .filter(|c| c.n > 0 && c.mean_err <= 5.0 && c.std_err <= 5.0)
        .collect();
    if robust.is_empty() {
        println!("  No cell is robustly accurate (mean<=5% AND std<=5%) across seeds.");
        println!("  => instability is NOT removed by these conditions; the detector itself is the limit.");
    } else {
        println!("  Robustly accurate cells (mean<=5% AND std<=5% across seeds):");
        for c in &robust {
            println!(
                "    fps={:<5} f={:<3}mm rpm={:<6} frames={} px={:.0} mean={:.1}% std={:.1}%",
                c.fps, c.focal_mm, c.rpm, c.frames, c.ball_px, c.mean_err, c.std_err
            );
        }
    }
    println!("\n  Key question: does std shrink as fps/frames rise? (hardware lever) or stay high? (algorithm lever)");
}
