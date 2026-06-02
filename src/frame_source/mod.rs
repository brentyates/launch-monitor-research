mod rendered_dataset;

pub use rendered_dataset::{RenderedDatasetSource, RigParams};

use image::GrayImage;

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    Disconnected,
    Idle,
    Ready,
    Streaming,
    Complete,
}

#[derive(Debug, Clone)]
pub struct GroundTruth {
    pub speed_mph: f64,
    pub vla_deg: f64,
    pub hla_deg: f64,
    pub spin_rpm: f64,
    pub spin_axis_deg: f64,
}

#[derive(Debug, Clone)]
pub struct StereoFrame {
    pub frame_index: u32,
    pub timestamp: f64,
    pub left: GrayImage,
    pub right: GrayImage,
    pub left_rgba: Vec<u8>,
    pub right_rgba: Vec<u8>,
    pub ball_position_mm: [f32; 3],
    pub ball_velocity_mm_s: [f32; 3],
}

pub trait FrameSource: Send {
    fn config(&self) -> Option<SourceConfig>;
    fn state(&self) -> SourceState;
    fn poll_frame(&mut self) -> Option<StereoFrame>;
    fn ground_truth(&self) -> Option<GroundTruth>;
    fn reset(&mut self);
}
