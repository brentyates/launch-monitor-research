use launch_monitor_research::{
    process_shot, BallDetector, FrameSource, GroundTruth, ProcessingResult, RenderedDatasetSource,
    RigParams, SourceConfig, StereoRig,
};
use std::path::Path;
use std::process::Command;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

struct TestCase {
    name: &'static str,
    speed_mph: f32,
    vla_deg: f32,
    hla_deg: f32,
    spin_rpm: f32,
    spin_axis_deg: f32,
}

const TEST_CASES: &[TestCase] = &[
    TestCase {
        name: "driver",
        speed_mph: 165.0,
        vla_deg: 10.5,
        hla_deg: -2.5, // Slight draw
        spin_rpm: 2700.0,
        spin_axis_deg: -5.0,
    },
    TestCase {
        name: "7-iron",
        speed_mph: 120.0,
        vla_deg: 16.0,
        hla_deg: 1.2, // Slight fade
        spin_rpm: 7000.0,
        spin_axis_deg: 2.0,
    },
    TestCase {
        name: "wedge",
        speed_mph: 85.0,
        vla_deg: 28.0,
        hla_deg: -0.8, // Straight-ish
        spin_rpm: 9500.0,
        spin_axis_deg: 0.0,
    },
];

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let project_dir = env!("CARGO_MANIFEST_DIR");
    let force_render = std::env::var("RENDER").map(|v| v == "1").unwrap_or(false);

    let mut all_passed = true;
    let mut results_summary: Vec<(String, bool, String)> = Vec::new();

    for test in TEST_CASES {
        println!("\n{}", "=".repeat(60));
        println!("  Test: {}", test.name);
        println!("  Launch: {} mph, VLA {}°, HLA {}°", test.speed_mph, test.vla_deg, test.hla_deg);
        println!("  Spin: {} RPM, axis {}°", test.spin_rpm, test.spin_axis_deg);
        println!("{}\n", "=".repeat(60));

        let dir = format!("{}/renders/{}", project_dir, test.name);
        let needs_render = force_render || !Path::new(&format!("{}/manifest.json", dir)).exists();

        if needs_render {
            if let Err(e) = render_case(test, &dir) {
                let msg = format!("{}: render failed: {}", test.name, e);
                error!("{}", msg);
                results_summary.push((test.name.to_string(), false, msg));
                all_passed = false;
                continue;
            }
        }

        let source = match RenderedDatasetSource::load(Path::new(&dir)) {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("{}: failed to load dataset: {}", test.name, e);
                error!("{}", msg);
                results_summary.push((test.name.to_string(), false, msg));
                all_passed = false;
                continue;
            }
        };

        let config = source.config().unwrap();
        let gt = source.ground_truth();
        let frames = source.read_all_frames();
        info!("Loaded {} frames for {}", frames.len(), test.name);

        let rig = make_rig(&config, &source.rig());
        let mut detector = BallDetector::new(15, 10);
        let debug_dir = format!("{}/debug_frames/{}", project_dir, test.name);
        let result = process_shot(&frames, &config, gt.clone(), &mut detector, &debug_dir, &rig);

        let (passed, summary) = print_results(test, &result, gt.as_ref());
        results_summary.push((test.name.to_string(), passed, summary));
        if !passed {
            all_passed = false;
        }
    }

    println!("\n\n{}", "=".repeat(60));
    println!("  SUMMARY");
    println!("{}", "=".repeat(60));
    for (name, passed, summary) in &results_summary {
        let status = if *passed { "PASS" } else { "FAIL" };
        println!("  [{}] {}: {}", status, name, summary);
    }
    println!("{}\n", "=".repeat(60));

    std::process::exit(if all_passed { 0 } else { 1 });
}

fn render_case(test: &TestCase, dir: &str) -> std::io::Result<()> {
    let project_dir = env!("CARGO_MANIFEST_DIR");
    let blender = std::env::var("BLENDER")
        .unwrap_or_else(|_| "/Applications/Blender.app/Contents/MacOS/Blender".to_string());
    let script = format!("{}/blender/render_shot.py", project_dir);

    info!("Rendering {} via Blender into {}", test.name, dir);

    let status = Command::new(blender)
        .args([
            "--background",
            "--factory-startup",
            "--python",
            &script,
            "--",
            "--case",
            test.name,
            "--speed",
            &test.speed_mph.to_string(),
            "--vla",
            &test.vla_deg.to_string(),
            "--hla",
            &test.hla_deg.to_string(),
            "--spin",
            &test.spin_rpm.to_string(),
            "--axis",
            &test.spin_axis_deg.to_string(),
            "--out",
            dir,
            "--width",
            "512",
            "--height",
            "384",
            "--fps",
            "240",
            "--frames",
            "14",
            "--samples",
            "64",
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

fn make_rig(config: &SourceConfig, rig: &RigParams) -> StereoRig {
    StereoRig::overhead(
        rig.baseline_mm,
        rig.height_mm,
        rig.forward_mm,
        rig.focal_mm,
        rig.pixel_pitch_mm,
        (config.width, config.height),
        config.fps as f64,
    )
}

fn print_results(
    test: &TestCase,
    result: &ProcessingResult,
    ground_truth: Option<&GroundTruth>,
) -> (bool, String) {
    let mut passed = true;

    if let Some(launch) = &result.launch {
        let speed_err = if test.speed_mph > 0.0 {
            ((launch.speed_mph - test.speed_mph as f64) / test.speed_mph as f64 * 100.0).abs()
        } else {
            0.0
        };
        let vla_err = (launch.vla_deg - test.vla_deg as f64).abs();
        let hla_err = (launch.hla_deg - test.hla_deg as f64).abs();

        println!("  Launch: {:.1} mph, VLA {:.1}°, HLA {:.1}°", launch.speed_mph, launch.vla_deg, launch.hla_deg);
        println!("  Errors: speed {:.2}%, VLA {:.2}°, HLA {:.2}°", speed_err, vla_err, hla_err);

        if let Some(gt) = ground_truth {
            let gt_speed_err = ((launch.speed_mph - gt.speed_mph) / gt.speed_mph * 100.0).abs();
            let gt_vla_err = (launch.vla_deg - gt.vla_deg).abs();
            let gt_hla_err = (launch.hla_deg - gt.hla_deg).abs();
            println!(
                "  vs GT:  speed {:.2}% ({:.1} mph), VLA {:.2}° ({:.1}°), HLA {:.2}° ({:.1}°)",
                gt_speed_err, gt.speed_mph, gt_vla_err, gt.vla_deg, gt_hla_err, gt.hla_deg
            );
        }

        if let Some(spin) = &result.spin {
            let spin_err = if test.spin_rpm > 0.0 {
                ((spin.rpm - test.spin_rpm as f64) / test.spin_rpm as f64 * 100.0).abs()
            } else {
                0.0
            };
            println!(
                "  Spin: {:.0} RPM (err {:.1}%), axis {:.1}°, confidence {:.2}",
                spin.rpm, spin_err, spin.axis_deg, spin.confidence
            );
        }

        if launch.speed_mph <= 0.0 || result.frame_count < 2 {
            passed = false;
        }

        let max_err_pct = 2.0;
        let max_err_deg = 2.0;
        if speed_err > max_err_pct || vla_err > max_err_deg || hla_err > max_err_deg {
            passed = false;
        }

        let summary = format!(
            "speed {:.2}%, VLA {:.2}°, HLA {:.2}° ({} frames)",
            speed_err, vla_err, hla_err, result.frame_count
        );
        (passed, summary)
    } else {
        println!("  No launch data computed");
        for err in &result.errors {
            println!("  Error: {}", err);
        }
        (false, "no launch data".to_string())
    }
}
