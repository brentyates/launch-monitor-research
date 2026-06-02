use super::GroundTruth;
use crate::config::CameraDef;
use image::GrayImage;
use serde::Deserialize;
use std::collections::HashMap;
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
    images: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    rig: String,
    measures: Vec<String>,
    fps: f32,
    ground_truth: ManifestGroundTruth,
    cameras: Vec<CameraDef>,
    frames: Vec<ManifestFrame>,
}

pub struct RigFrame {
    pub index: u32,
    pub timestamp: f64,
    pub ball_pos_mm: [f32; 3],
    pub ball_vel_mm_s: [f32; 3],
    pub images: HashMap<String, GrayImage>,
}

pub struct RigDataset {
    pub rig_name: String,
    pub measures: Vec<String>,
    pub fps: f32,
    pub ground_truth: GroundTruth,
    pub cameras: Vec<CameraDef>,
    pub frames: Vec<RigFrame>,
}

impl RigDataset {
    pub fn camera(&self, id: &str) -> Option<&CameraDef> {
        self.cameras.iter().find(|c| c.id == id)
    }

    pub fn gray_seq(&self, id: &str) -> Vec<(u32, f64, GrayImage)> {
        self.frames
            .iter()
            .filter_map(|f| f.images.get(id).map(|img| (f.index, f.timestamp, img.clone())))
            .collect()
    }
}

pub fn load(dir: &Path) -> io::Result<RigDataset> {
    let manifest_path = dir.join("manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_text)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut frames = Vec::with_capacity(manifest.frames.len());
    for frame in &manifest.frames {
        let mut images = HashMap::new();
        for (cam_id, fname) in &frame.images {
            let gray = load_gray(&dir.join(fname))?;
            images.insert(cam_id.clone(), gray);
        }
        frames.push(RigFrame {
            index: frame.index,
            timestamp: frame.timestamp,
            ball_pos_mm: frame.ball_pos_mm,
            ball_vel_mm_s: frame.ball_vel_mm_s,
            images,
        });
    }

    info!(
        "Loaded rig '{}' ({} frames, {} cameras) from {} @ {} fps",
        manifest.rig,
        frames.len(),
        manifest.cameras.len(),
        dir.display(),
        manifest.fps
    );

    Ok(RigDataset {
        rig_name: manifest.rig,
        measures: manifest.measures,
        fps: manifest.fps,
        ground_truth: GroundTruth {
            speed_mph: manifest.ground_truth.speed_mph,
            vla_deg: manifest.ground_truth.vla_deg,
            hla_deg: manifest.ground_truth.hla_deg,
            spin_rpm: manifest.ground_truth.spin_rpm,
            spin_axis_deg: manifest.ground_truth.spin_axis_deg,
        },
        cameras: manifest.cameras,
        frames,
    })
}

fn load_gray(path: &Path) -> io::Result<GrayImage> {
    let img = image::open(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let raw = rgba.into_raw();
    let mut gray = GrayImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let r = raw[idx] as f32;
            let g = raw[idx + 1] as f32;
            let b = raw[idx + 2] as f32;
            let lum = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
            gray.put_pixel(x, y, image::Luma([lum]));
        }
    }
    Ok(gray)
}
