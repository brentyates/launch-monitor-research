use launch_monitor_research::{
    process_shot, BallDetector, FrameSource, RenderedDatasetSource, StereoRig,
};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, Deserialize)]
struct Shot {
    name: String,
    speed_mph: f64,
    vla_deg: f64,
    hla_deg: f64,
    spin_rpm: f64,
    spin_axis_deg: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct RigConfig {
    name: String,
    baseline_mm: f64,
    height_mm: f64,
    forward_mm: f64,
    focal_mm: f64,
    pixel_pitch_mm: f64,
    width: u32,
    height: u32,
    fps: f64,
    samples: u32,
}

#[derive(Debug, Deserialize)]
struct SweepSpec {
    shots: Vec<Shot>,
    configs: Vec<RigConfig>,
}

struct ShotResult {
    shot: String,
    frames: u32,
    detected: bool,
    speed_err_pct: f64,
    vla_err_deg: f64,
    hla_err_deg: f64,
}

const USEFUL: (f64, f64) = (2.0, 1.0);
const GOOD: (f64, f64) = (1.0, 0.5);

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let project_dir = env!("CARGO_MANIFEST_DIR");
    let force_render = std::env::var("RENDER").map(|v| v == "1").unwrap_or(false);
    let spec_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("{}/configs/sweep.json", project_dir));

    let spec_text = std::fs::read_to_string(&spec_path).expect("read sweep spec");
    let spec: SweepSpec = serde_json::from_str(&spec_text).expect("parse sweep spec");

    println!(
        "Sweep: {} configs x {} shots = {} datasets\n",
        spec.configs.len(),
        spec.shots.len(),
        spec.configs.len() * spec.shots.len()
    );

    let mut csv = String::from(
        "config,fps,width,height,focal_mm,baseline_mm,mount_height_mm,cost_usd,shot,frames,detected,speed_err_pct,vla_err_deg,hla_err_deg\n",
    );
    let mut summary: Vec<(String, f64, f64, f64, f64, u32, &'static str)> = Vec::new();

    for cfg in &spec.configs {
        let cost = estimate_cost(cfg);
        let mut worst_speed = 0.0_f64;
        let mut worst_vla = 0.0_f64;
        let mut worst_hla = 0.0_f64;
        let mut min_frames = u32::MAX;
        let mut any_fail = false;

        println!(
            "=== {} | {}fps {}x{} f{}mm base{}mm h{}mm | ~${:.0} ===",
            cfg.name, cfg.fps, cfg.width, cfg.height, cfg.focal_mm, cfg.baseline_mm, cfg.height_mm, cost
        );

        for shot in &spec.shots {
            let r = run_one(project_dir, cfg, shot, force_render);
            println!(
                "  {:<8} frames={:<2} detected={:<5} speed={:>6.2}% vla={:>5.2}° hla={:>5.2}°",
                r.shot, r.frames, r.detected, r.speed_err_pct, r.vla_err_deg, r.hla_err_deg
            );
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{:.0},{},{},{},{:.3},{:.3},{:.3}\n",
                cfg.name, cfg.fps, cfg.width, cfg.height, cfg.focal_mm, cfg.baseline_mm,
                cfg.height_mm, cost, r.shot, r.frames, r.detected, r.speed_err_pct,
                r.vla_err_deg, r.hla_err_deg
            ));

            if !r.detected {
                any_fail = true;
            }
            worst_speed = worst_speed.max(r.speed_err_pct);
            worst_vla = worst_vla.max(r.vla_err_deg);
            worst_hla = worst_hla.max(r.hla_err_deg);
            min_frames = min_frames.min(r.frames);
        }

        let worst_angle = worst_vla.max(worst_hla);
        let tier = if any_fail {
            "FAIL"
        } else if worst_speed <= GOOD.0 && worst_angle <= GOOD.1 {
            "GOOD"
        } else if worst_speed <= USEFUL.0 && worst_angle <= USEFUL.1 {
            "USEFUL"
        } else {
            "FAIL"
        };
        println!(
            "  -> worst: speed {:.2}% angle {:.2}° | min frames {} | {}\n",
            worst_speed, worst_angle, min_frames, tier
        );
        summary.push((cfg.name.clone(), cost, worst_speed, worst_vla, worst_hla, min_frames, tier));
    }

    let csv_path = format!("{}/results/sweep.csv", project_dir);
    std::fs::create_dir_all(format!("{}/results", project_dir)).ok();
    std::fs::write(&csv_path, &csv).expect("write csv");

    summary.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    println!("{}", "=".repeat(78));
    println!("  FRONTIER (cheapest first)");
    println!("{}", "=".repeat(78));
    println!(
        "  {:<18} {:>8} {:>10} {:>9} {:>7}  {}",
        "config", "cost", "speed%", "angle°", "frames", "tier"
    );
    for (name, cost, ws, wv, wh, frames, tier) in &summary {
        println!(
            "  {:<18} {:>7.0}$ {:>9.2} {:>8.2} {:>7}  {}",
            name, cost, ws, wv.max(*wh), frames, tier
        );
    }
    let cheapest_useful = summary.iter().find(|s| s.6 == "USEFUL" || s.6 == "GOOD");
    if let Some(s) = cheapest_useful {
        println!("\n  Cheapest config meeting the budget: {} (~${:.0}, {})", s.0, s.1, s.6);
    } else {
        println!("\n  No config in this grid met the budget.");
    }
    println!("\n  Full results: {}", csv_path);
}

fn run_one(project_dir: &str, cfg: &RigConfig, shot: &Shot, force_render: bool) -> ShotResult {
    let dir = format!("{}/renders/sweep/{}/{}", project_dir, cfg.name, shot.name);
    let manifest = format!("{}/manifest.json", dir);

    if force_render || !Path::new(&manifest).exists() {
        if let Err(e) = render(project_dir, cfg, shot, &dir) {
            eprintln!("  render failed for {}/{}: {}", cfg.name, shot.name, e);
            return ShotResult {
                shot: shot.name.clone(),
                frames: 0,
                detected: false,
                speed_err_pct: f64::NAN,
                vla_err_deg: f64::NAN,
                hla_err_deg: f64::NAN,
            };
        }
    }

    let source = match RenderedDatasetSource::load(Path::new(&dir)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  load failed for {}/{}: {}", cfg.name, shot.name, e);
            return ShotResult {
                shot: shot.name.clone(),
                frames: 0,
                detected: false,
                speed_err_pct: f64::NAN,
                vla_err_deg: f64::NAN,
                hla_err_deg: f64::NAN,
            };
        }
    };

    let config = source.config().unwrap();
    let gt = source.ground_truth();
    let frames = source.read_all_frames();
    let rp = source.rig();
    let rig = StereoRig::overhead(
        rp.baseline_mm,
        rp.height_mm,
        rp.forward_mm,
        rp.focal_mm,
        rp.pixel_pitch_mm,
        (config.width, config.height),
        config.fps as f64,
    );

    let mut detector = BallDetector::new(15, 10);
    let debug_dir = format!("{}/debug", dir);
    let result = process_shot(&frames, &config, gt, &mut detector, &debug_dir, &rig);

    match result.launch {
        Some(l) => ShotResult {
            shot: shot.name.clone(),
            frames: result.frame_count,
            detected: true,
            speed_err_pct: ((l.speed_mph - shot.speed_mph) / shot.speed_mph * 100.0).abs(),
            vla_err_deg: (l.vla_deg - shot.vla_deg).abs(),
            hla_err_deg: (l.hla_deg - shot.hla_deg).abs(),
        },
        None => ShotResult {
            shot: shot.name.clone(),
            frames: result.frame_count,
            detected: false,
            speed_err_pct: f64::NAN,
            vla_err_deg: f64::NAN,
            hla_err_deg: f64::NAN,
        },
    }
}

fn render(project_dir: &str, cfg: &RigConfig, shot: &Shot, dir: &str) -> std::io::Result<()> {
    let blender = std::env::var("BLENDER")
        .unwrap_or_else(|_| "/Applications/Blender.app/Contents/MacOS/Blender".to_string());
    let script = format!("{}/blender/render_shot.py", project_dir);

    let status = Command::new(blender)
        .args([
            "--background", "--factory-startup", "--python", &script, "--",
            "--case", &shot.name,
            "--speed", &shot.speed_mph.to_string(),
            "--vla", &shot.vla_deg.to_string(),
            "--hla", &shot.hla_deg.to_string(),
            "--spin", &shot.spin_rpm.to_string(),
            "--axis", &shot.spin_axis_deg.to_string(),
            "--out", dir,
            "--width", &cfg.width.to_string(),
            "--height", &cfg.height.to_string(),
            "--fps", &cfg.fps.to_string(),
            "--frames", "20",
            "--samples", &cfg.samples.to_string(),
            "--baseline-mm", &cfg.baseline_mm.to_string(),
            "--mount-height-mm", &cfg.height_mm.to_string(),
            "--forward-mm", &cfg.forward_mm.to_string(),
            "--focal-mm", &cfg.focal_mm.to_string(),
            "--pixel-pitch-mm", &cfg.pixel_pitch_mm.to_string(),
        ])
        .status()?;

    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Blender exited with {}", status),
        ));
    }
    Ok(())
}

fn estimate_cost(cfg: &RigConfig) -> f64 {
    let mp = (cfg.width as f64 * cfg.height as f64) / 1.0e6;
    let per_cam = 40.0 + (cfg.fps / 120.0) * 70.0 + mp * 120.0;
    per_cam * 2.0 + 70.0
}
