pub mod ball_detector;
pub mod config;
pub mod calibration;
pub mod debug;
pub mod frame_source;
pub mod pipeline;
pub mod spin_detector;
pub mod triangulation;

pub use ball_detector::{BallDetection, BallDetector, DetectionError};
pub use config::StereoRig;
pub use debug::{colors, DebugFrame, DebugMetadata, Overlay};
pub use frame_source::{FrameSource, GroundTruth, RenderedDatasetSource, RigParams, SourceConfig, SourceState, StereoFrame};
pub use pipeline::{process_shot, ProcessingResult};
pub use spin_detector::{FrameSpin, SpinDetector, SpinError, SpinResult};
pub use triangulation::{LaunchData, Point3D, StereoTriangulator, TriangulationError};
