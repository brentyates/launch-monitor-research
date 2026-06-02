pub mod ball_detector;
pub mod config;
pub mod calibration;
pub mod debug;
pub mod frame_source;
pub mod pipeline;
pub mod spin_detector;
pub mod spin_tracker;
pub mod triangulation;

pub use ball_detector::{BallDetection, BallDetector, DetectionError};
pub use config::{Camera, CameraDef, StereoRig};
pub use debug::{colors, DebugFrame, DebugMetadata, Overlay};
pub use frame_source::{load_rig_dataset, FrameSource, GroundTruth, RenderedDatasetSource, RigDataset, RigFrame, RigParams, SourceConfig, SourceState, StereoFrame};
pub use pipeline::{process_shot, ProcessingResult};
pub use spin_detector::{FrameSpin, SpinDetector, SpinError, SpinResult};
pub use spin_tracker::{estimate_spin_dense, estimate_spin_interframe, InterframeSpin};
pub use triangulation::{LaunchData, Point3D, StereoTriangulator, TriangulationError};
