use super::{FrameSource, GroundTruth, SourceConfig, SourceState, StereoFrame};
use image::GrayImage;
use serde::Deserialize;
use std::io;
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Deserialize)]
struct ManifestGroundTruth {
    speed_mph: f64,
    vla_deg: f64,
    hla_deg: f64,
    spin_rpm: f64,
    spin_axis_deg: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct ManifestFrame {
    index: u32,
    timestamp: f64,
    ball_pos_mm: [f32; 3],
    ball_vel_mm_s: [f32; 3],
    left: String,
    right: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    width: u32,
    height: u32,
    fps: f32,
    ground_truth: ManifestGroundTruth,
    frames: Vec<ManifestFrame>,
}

pub struct RenderedDatasetSource {
    config: SourceConfig,
    ground_truth: GroundTruth,
    frames: Vec<StereoFrame>,
    cursor: usize,
}

unsafe impl Send for RenderedDatasetSource {}

impl RenderedDatasetSource {
    pub fn load(dir: &Path) -> io::Result<Self> {
        let manifest_path = dir.join("manifest.json");
        let manifest_text = std::fs::read_to_string(&manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&manifest_text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let width = manifest.width;
        let height = manifest.height;

        let mut frames = Vec::with_capacity(manifest.frames.len());
        for frame in &manifest.frames {
            let left_rgba = load_rgba(&dir.join(&frame.left))?;
            let right_rgba = load_rgba(&dir.join(&frame.right))?;

            let left = rgba_to_gray(&left_rgba, width, height);
            let right = rgba_to_gray(&right_rgba, width, height);

            frames.push(StereoFrame {
                frame_index: frame.index,
                timestamp: frame.timestamp,
                left,
                right,
                left_rgba,
                right_rgba,
                ball_position_mm: frame.ball_pos_mm,
                ball_velocity_mm_s: frame.ball_vel_mm_s,
            });
        }

        info!(
            "Loaded {} frames from {} ({}x{} @ {} fps)",
            frames.len(),
            dir.display(),
            width,
            height,
            manifest.fps
        );

        Ok(Self {
            config: SourceConfig {
                width,
                height,
                fps: manifest.fps,
            },
            ground_truth: GroundTruth {
                speed_mph: manifest.ground_truth.speed_mph,
                vla_deg: manifest.ground_truth.vla_deg,
                hla_deg: manifest.ground_truth.hla_deg,
                spin_rpm: manifest.ground_truth.spin_rpm,
                spin_axis_deg: manifest.ground_truth.spin_axis_deg,
            },
            frames,
            cursor: 0,
        })
    }

    pub fn read_all_frames(&self) -> Vec<StereoFrame> {
        self.frames.clone()
    }
}

impl FrameSource for RenderedDatasetSource {
    fn config(&self) -> Option<SourceConfig> {
        Some(self.config.clone())
    }

    fn state(&self) -> SourceState {
        SourceState::Complete
    }

    fn poll_frame(&mut self) -> Option<StereoFrame> {
        let frame = self.frames.get(self.cursor).cloned();
        if frame.is_some() {
            self.cursor += 1;
        }
        frame
    }

    fn ground_truth(&self) -> Option<GroundTruth> {
        Some(self.ground_truth.clone())
    }

    fn reset(&mut self) {
        self.cursor = 0;
    }
}

fn load_rgba(path: &Path) -> io::Result<Vec<u8>> {
    let img = image::open(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(img.to_rgba8().into_raw())
}

fn rgba_to_gray(rgba: &[u8], width: u32, height: u32) -> GrayImage {
    let mut gray = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let r = rgba[idx] as f32;
            let g = rgba[idx + 1] as f32;
            let b = rgba[idx + 2] as f32;

            let lum = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            gray.put_pixel(x, y, image::Luma([lum]));
        }
    }

    gray
}
