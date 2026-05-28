use launch_monitor_research::{
    process_shot, BallDetector, FrameSource, GroundTruth, ProcessingResult, SharedMemorySource,
    SourceConfig, SourceState, StereoRig,
};
use std::process::Command;
use std::time::{Duration, Instant};
use tracing::{error, info, debug};
use tracing_subscriber::EnvFilter;
use nalgebra::Vector3;
use launch_monitor_research::calibration::CalibrationResult;

const TIMEOUT_SECS: u64 = 30;

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

    let mut source = SharedMemorySource::new();

    info!("Waiting for Unity shared memory...");
    let start = Instant::now();
    while !source.try_connect() {
        if start.elapsed() > Duration::from_secs(TIMEOUT_SECS) {
            error!("Timeout waiting for Unity shared memory");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    info!("Connected to Unity shared memory");

    activate_unity_window();
    
    let config = source.config().unwrap_or(launch_monitor_research::SourceConfig { width: 512, height: 384, fps: 240.0 });
    let mut rig = make_rig(&config);

    // --- Calibration Phase ---
    info!("Starting Camera Calibration...");
    source.send_calibrate_command();
    std::thread::sleep(Duration::from_millis(500)); // Give Unity time to show board

    let mut calibration_passed = false;
    let calibration_timeout = Instant::now();
    let mut last_frame = None;
    
    while calibration_timeout.elapsed() < Duration::from_secs(5) {
        if let Some(frame) = source.poll_frame() {
            last_frame = Some(frame.clone());
            // Check left camera
            let left_res = launch_monitor_research::calibration::detect_calibration_board(&frame.left);
            // Check right camera
            let right_res = launch_monitor_research::calibration::detect_calibration_board(&frame.right);

            if left_res.success && right_res.success {
                info!("Calibration Successful!");
                info!("Left Camera: {}", left_res.details);
                info!("Right Camera: {}", right_res.details);
                
                // --- BASICS: CALIBRATE THE RIG ---
                // We use a Linear Least Squares solver to find k1, k2 such that:
                // u_dist = (u_pin - 0.5) * (1 + k1*r2 + k2*r4) + 0.5
                // Rewritten as: (u_dist - 0.5)/(u_pin - 0.5) - 1 = k1*r2 + k2*r4
                
                let solve_k = |corners: &[(f64, f64)], w: u32, h: u32| -> [f64; 2] {
                    if corners.len() < 54 { return [0.0, 0.0]; }
                    
                    // Sort corners into grid order (top-to-bottom, left-to-right)
                    let mut sorted = corners.to_vec();
                    // First sort by Y (rows)
                    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                    
                    // Within each block of 9, sort by X
                    for chunk in sorted.chunks_mut(9) {
                        chunk.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                    }

                    // Use detected bounding box to define "ideal" pinhole grid
                    let min_x = sorted.iter().take(54).map(|c| c.0).fold(f64::INFINITY, f64::min) / w as f64;
                    let max_x = sorted.iter().take(54).map(|c| c.0).fold(f64::NEG_INFINITY, f64::max) / w as f64;
                    let min_y = sorted.iter().take(54).map(|c| c.1).fold(f64::INFINITY, f64::min) / h as f64;
                    let max_y = sorted.iter().take(54).map(|c| c.1).fold(f64::NEG_INFINITY, f64::max) / h as f64;
                    
                    let bw = max_x - min_x;
                    let bh = max_y - min_y;

                    let mut ata = [[0.0; 2]; 2];
                    let mut atb = [0.0; 2];
                    
                    for (i, &det_p) in sorted.iter().take(54).enumerate() {
                        let u_dist = det_p.0 / w as f64;
                        let v_dist = det_p.1 / h as f64;
                        
                        let col = (i % 9) as f64;
                        let row = (i / 9) as f64;
                        
                        let u_pin = min_x + (col / 8.0) * bw;
                        let v_pin = min_y + (row / 5.0) * bh;
                        
                        let du_pin = u_pin - 0.5;
                        let dv_pin = v_pin - 0.5;
                        let r2 = du_pin * du_pin + dv_pin * dv_pin;
                        let r4 = r2 * r2;
                        
                        let du_dist = u_dist - 0.5;
                        let dv_dist = v_dist - 0.5;
                        
                        let x1 = du_pin * r2;
                        let x2 = du_pin * r4;
                        let y = du_dist - du_pin;
                        
                        ata[0][0] += x1 * x1;
                        ata[0][1] += x1 * x2;
                        ata[1][0] += x1 * x2;
                        ata[1][1] += x2 * x2;
                        atb[0] += x1 * y;
                        atb[1] += x2 * y;

                        let x1_v = dv_pin * r2;
                        let x2_v = dv_pin * r4;
                        let y_v = dv_dist - dv_pin;
                        ata[0][0] += x1_v * x1_v;
                        ata[0][1] += x1_v * x2_v;
                        ata[1][0] += x1_v * x2_v;
                        ata[1][1] += x2_v * x2_v;
                        atb[0] += x1_v * y_v;
                        atb[1] += x2_v * y_v;
                    }
                    
                    let det: f64 = ata[0][0] * ata[1][1] - ata[0][1] * ata[1][0];
                    if det.abs() > 1e-12 {
                        let k1 = (atb[0] * ata[1][1] - atb[1] * ata[0][1]) / det;
                        let k2 = (ata[0][0] * atb[1] - ata[1][0] * atb[0]) / det;
                        [k1, k2]
                    } else {
                        [0.0, 0.0]
                    }
                };

                rig.cam0_distortion = solve_k(&left_res.corners, rig.width, rig.height); 
                rig.cam1_distortion = solve_k(&right_res.corners, rig.width, rig.height);
                info!("Calibrated Rig - Cam0 K1: {:.4}, K2: {:.4}", rig.cam0_distortion[0], rig.cam0_distortion[1]);
                info!("Calibrated Rig - Cam1 K1: {:.4}, K2: {:.4}", rig.cam1_distortion[0], rig.cam1_distortion[1]);
                // ---------------------------------

                calibration_passed = true;
                
                // Save calibration images
                let debug_dir = format!("{}/debug_frames", env!("CARGO_MANIFEST_DIR"));
                std::fs::create_dir_all(&debug_dir).ok();
                let _ = frame.left.save(format!("{}/calibration_left.png", debug_dir));
                let _ = frame.right.save(format!("{}/calibration_right.png", debug_dir));
                info!("Saved calibration images to {}", debug_dir);
                
                break;
            } else {
                info!("Calibration searching... Left: {}, Right: {}", 
                    left_res.detected_corners, right_res.detected_corners);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if !calibration_passed {
        error!("Calibration FAILED: Could not detect calibration board in both cameras within timeout.");
        
        // Save failure images for debugging
        if let Some(frame) = last_frame {
            let debug_dir = format!("{}/debug_frames", env!("CARGO_MANIFEST_DIR"));
            std::fs::create_dir_all(&debug_dir).ok();
            let _ = frame.left.save(format!("{}/calibration_left.png", debug_dir));
            let _ = frame.right.save(format!("{}/calibration_right.png", debug_dir));
            info!("Saved failure calibration images to {}", debug_dir);
        }
    // Decide if we want to hard fail or just warn. 
        // For E2E tests, maybe we should fail? 
        // For now, let's hard fail to ensure we fix it if it's broken.
        std::process::exit(1);
    }

    // Reset to clear board
    source.send_reset_command();
    std::thread::sleep(Duration::from_millis(500)); // Give Unity time to hide board
    // -------------------------

    let project_dir = env!("CARGO_MANIFEST_DIR");
    let mut all_passed = true;
    let mut results_summary: Vec<(String, bool, String)> = Vec::new();

    for test in TEST_CASES {
        println!("\n{}", "=".repeat(60));
        println!("  Test: {}", test.name);
        println!("  Launch: {} mph, VLA {}°, HLA {}°", test.speed_mph, test.vla_deg, test.hla_deg);
        println!("  Spin: {} RPM, axis {}°", test.spin_rpm, test.spin_axis_deg);
        println!("{}\n", "=".repeat(60));

        activate_unity_window();
        source.send_reset_command();
        
        if !wait_for_state(&mut source, SourceState::Idle, "reset") {
             let msg = format!("{}: timeout waiting for reset (idle)", test.name);
             error!("{}", msg);
             results_summary.push((test.name.to_string(), false, msg));
             all_passed = false;
             continue;
        }

        activate_unity_window();
        source.send_launch_command(
            test.speed_mph,
            test.vla_deg,
            test.hla_deg,
            test.spin_rpm,
            test.spin_axis_deg,
        );

        if !wait_for_state(&mut source, SourceState::Complete, "flight complete") {
             let msg = format!("{}: timeout waiting for flight to complete", test.name);
             error!("{}", msg);
             results_summary.push((test.name.to_string(), false, msg));
             all_passed = false;
             continue;
        }

        let frames = source.read_all_frames();
        info!("Collected {} frames for {}", frames.len(), test.name);

        if frames.is_empty() {
            let msg = format!("{}: no frames received", test.name);
            error!("{}", msg);
            results_summary.push((test.name.to_string(), false, msg));
            all_passed = false;
            continue;
        }

        let config = match source.config() {
            Some(c) => c,
            None => {
                let msg = format!("{}: could not read source config", test.name);
                error!("{}", msg);
                results_summary.push((test.name.to_string(), false, msg));
                all_passed = false;
                continue;
            }
        };
        let ground_truth = source.ground_truth();

        let debug_dir = format!("{}/debug_frames/{}", project_dir, test.name);
        let rig = make_rig(&config);
        let mut detector = BallDetector::new(15, 10);
        let result = process_shot(&frames, &config, ground_truth.clone(), &mut detector, &debug_dir, &rig);

        let (passed, summary) = print_results(test, &result, ground_truth.as_ref());
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

fn make_rig(config: &SourceConfig) -> StereoRig {
    StereoRig::overhead(
        350.0,
        3048.0,
        1092.0,
        6.0,
        0.00508,
        (config.width, config.height),
        config.fps as f64,
    )
}

fn activate_unity_window() {
    let _ = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to set frontmost of first process whose name contains \"LaunchSimulator\" to true",
        ])
        .output();
    std::thread::sleep(Duration::from_millis(200));
}

fn wait_for_state(source: &mut SharedMemorySource, target: SourceState, label: &str) -> bool {
    let start = Instant::now();
    let mut last_log = Instant::now();
    loop {
        let state = source.state();
        if state == target {
            return true;
        }
        if start.elapsed() > Duration::from_secs(TIMEOUT_SECS) {
            error!("Timeout waiting for {}: current state {:?}", label, state);
            return false;
        }
        if last_log.elapsed() > Duration::from_secs(5) {
            info!("Still waiting for {} (current: {:?}, elapsed: {:.0}s)", label, state, start.elapsed().as_secs_f64());
            last_log = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
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
