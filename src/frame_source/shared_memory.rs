use super::{FrameSource, GroundTruth, SourceConfig, SourceState, StereoFrame};
use image::GrayImage;
use memmap2::MmapMut;
use std::fs::OpenOptions;
use std::io;
use tracing::{debug, info};

const SHARED_MEMORY_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/LaunchMonitorSharedMemory");
const MAGIC: u32 = 0x474F4C46;
const RING_BUFFER_SIZE: usize = 60;
const HEADER_SIZE: usize = 104;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(i32)]
enum SharedState {
    Idle = 0,
    Ready = 1,
    Streaming = 2,
    Complete = 3,
}

impl From<i32> for SharedState {
    fn from(v: i32) -> Self {
        match v {
            0 => SharedState::Idle,
            1 => SharedState::Ready,
            2 => SharedState::Streaming,
            3 => SharedState::Complete,
            _ => SharedState::Idle,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct GroundTruthData {
    speed_mph: f32,
    vla_deg: f32,
    hla_deg: f32,
    spin_rpm: f32,
    spin_axis_deg: f32,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct LaunchCommand {
    command: i32,
    speed_mph: f32,
    vla_deg: f32,
    hla_deg: f32,
    spin_rpm: f32,
    spin_axis_deg: f32,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct SharedHeader {
    magic: u32,
    state: i32,
    write_head: i32,
    frame_count: i32,
    width: i32,
    height: i32,
    fps: f32,
    ground_truth: GroundTruthData,
    rust_command: LaunchCommand,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
struct FrameHeader {
    frame_index: i32,
    timestamp: f64,
    ball_pos_x: f32,
    ball_pos_y: f32,
    ball_pos_z: f32,
    ball_vel_x: f32,
    ball_vel_y: f32,
    ball_vel_z: f32,
}

pub struct SharedMemorySource {
    mmap: Option<MmapMut>,
    ptr: *mut u8,
    last_read: i32,
    frame_size: usize,
    config: Option<SourceConfig>,
}

unsafe impl Send for SharedMemorySource {}

impl SharedMemorySource {
    pub fn new() -> Self {
        Self {
            mmap: None,
            ptr: std::ptr::null_mut(),
            last_read: -1,
            frame_size: 0,
            config: None,
        }
    }

    pub fn try_connect(&mut self) -> bool {
        if self.mmap.is_some() {
            return true;
        }

        match self.open() {
            Ok(()) => {
                info!("Connected to Unity shared memory");
                true
            }
            Err(e) => {
                debug!("Waiting for Unity shared memory: {}", e);
                false
            }
        }
    }

    #[cfg(unix)]
    fn open(&mut self) -> io::Result<()> {
        let shm_path = SHARED_MEMORY_PATH;

        let file = OpenOptions::new().read(true).write(true).open(&shm_path)?;

        let mmap = unsafe { MmapMut::map_mut(&file)? };
        let ptr = mmap.as_ptr() as *mut u8;

        let header = unsafe { std::ptr::read_unaligned(ptr as *const SharedHeader) };
        if header.magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid shared memory magic number",
            ));
        }

        let pixel_size = (header.width * header.height * 4) as usize;
        let frame_header_size = std::mem::size_of::<FrameHeader>();
        let frame_size = frame_header_size + (pixel_size * 2);

        self.config = Some(SourceConfig {
            width: header.width as u32,
            height: header.height as u32,
            fps: header.fps,
        });

        let width = header.width;
        let height = header.height;
        let fps = header.fps;
        info!("Opened shared memory: {}x{} @ {} fps", width, height, fps);

        self.mmap = Some(mmap);
        self.ptr = ptr;
        self.frame_size = frame_size;
        self.last_read = -1;

        Ok(())
    }

    #[cfg(windows)]
    fn open(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows shared memory not yet implemented",
        ))
    }

    fn read_header(&self) -> Option<SharedHeader> {
        if self.mmap.is_none() {
            return None;
        }
        Some(unsafe { std::ptr::read_volatile(self.ptr as *const SharedHeader) })
    }

    fn read_frame_at_slot(&self, slot_index: usize) -> Option<StereoFrame> {
        let header = self.read_header()?;
        let width = header.width as u32;
        let height = header.height as u32;

        let offset = HEADER_SIZE + (slot_index * self.frame_size);

        unsafe {
            let frame_ptr = self.ptr.add(offset);
            let frame_header = std::ptr::read_volatile(frame_ptr as *const FrameHeader);

            if frame_header.frame_index < 0 {
                return None;
            }

            let header_size = std::mem::size_of::<FrameHeader>();
            let pixel_size = (width * height * 4) as usize;

            let cam0_ptr = frame_ptr.add(header_size);
            let cam1_ptr = frame_ptr.add(header_size + pixel_size);

            let cam0_rgba_raw = std::slice::from_raw_parts(cam0_ptr, pixel_size).to_vec();
            let cam1_rgba_raw = std::slice::from_raw_parts(cam1_ptr, pixel_size).to_vec();

            let cam0_rgba = flip_rgba_y(&cam0_rgba_raw, width, height);
            let cam1_rgba = flip_rgba_y(&cam1_rgba_raw, width, height);

            let left = rgba_to_gray(&cam0_rgba, width, height);
            let right = rgba_to_gray(&cam1_rgba, width, height);

            Some(StereoFrame {
                frame_index: frame_header.frame_index as u32,
                timestamp: frame_header.timestamp,
                left,
                right,
                left_rgba: cam0_rgba,
                right_rgba: cam1_rgba,
                ball_position_mm: [
                    frame_header.ball_pos_x,
                    frame_header.ball_pos_y,
                    frame_header.ball_pos_z,
                ],
                ball_velocity_mm_s: [
                    frame_header.ball_vel_x,
                    frame_header.ball_vel_y,
                    frame_header.ball_vel_z,
                ],
            })
        }
    }

    fn read_frame_internal(&self, frame_index: i32) -> Option<StereoFrame> {
        let slot_index = (frame_index as usize) % RING_BUFFER_SIZE;
        self.read_frame_at_slot(slot_index)
    }
}

impl SharedMemorySource {
    pub fn read_all_frames(&self) -> Vec<StereoFrame> {
        let mut frames = Vec::new();
        let header = match self.read_header() {
            Some(h) => h,
            None => return frames,
        };

        let write_head = header.write_head as usize;
        let frame_count = header.frame_count as usize;

        if frame_count == 0 || write_head == 0 {
            return frames;
        }

        let available = frame_count.min(RING_BUFFER_SIZE).min(write_head);
        let first_frame_idx = write_head - available;

        info!(
            "Reading {} frames from ring buffer (write_head={}, frame_count={}, first_idx={})",
            available, write_head, frame_count, first_frame_idx
        );

        for frame_idx in first_frame_idx..write_head {
            let slot = frame_idx % RING_BUFFER_SIZE;
            if let Some(frame) = self.read_frame_at_slot(slot) {
                frames.push(frame);
            }
        }

        frames.sort_by_key(|f| f.frame_index);
        frames
    }

    const OFFSET_RUST_COMMAND: usize = 48;

    fn write_command(&self, cmd: LaunchCommand) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            let cmd_ptr = self.ptr.add(Self::OFFSET_RUST_COMMAND) as *mut LaunchCommand;
            std::ptr::write_volatile(cmd_ptr, cmd);
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    }

    pub fn write_head(&self) -> i32 {
        self.read_header().map(|h| h.write_head).unwrap_or(0)
    }

    pub fn send_launch_command(
        &mut self,
        speed_mph: f32,
        vla_deg: f32,
        hla_deg: f32,
        spin_rpm: f32,
        spin_axis_deg: f32,
    ) -> bool {
        if self.mmap.is_none() {
            return false;
        }

        self.write_command(LaunchCommand {
            command: 1,
            speed_mph,
            vla_deg,
            hla_deg,
            spin_rpm,
            spin_axis_deg,
        });

        info!(
            "Sent launch command: {} mph, VLA {}°, HLA {}°",
            speed_mph, vla_deg, hla_deg
        );
        true
    }

    pub fn send_reset_command(&mut self) -> bool {
        if self.mmap.is_none() {
            return false;
        }

        self.write_command(LaunchCommand {
            command: 2,
            ..Default::default()
        });

        info!("Sent reset command");
        true
    }

    pub fn send_calibrate_command(&mut self) -> bool {
        if self.mmap.is_none() {
            return false;
        }

        self.write_command(LaunchCommand {
            command: 3,
            ..Default::default()
        });

        info!("Sent calibrate command");
        true
    }
}

impl Default for SharedMemorySource {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameSource for SharedMemorySource {
    fn config(&self) -> Option<SourceConfig> {
        self.config.clone()
    }

    fn state(&self) -> SourceState {
        if self.mmap.is_none() {
            return SourceState::Disconnected;
        }

        match self.read_header() {
            Some(header) => match SharedState::from(header.state) {
                SharedState::Idle => SourceState::Idle,
                SharedState::Ready => SourceState::Ready,
                SharedState::Streaming => SourceState::Streaming,
                SharedState::Complete => SourceState::Complete,
            },
            None => SourceState::Disconnected,
        }
    }

    fn poll_frame(&mut self) -> Option<StereoFrame> {
        let header = self.read_header()?;
        let write_head = header.write_head;

        if write_head <= 0 {
            return None;
        }

        let oldest_available = if write_head > RING_BUFFER_SIZE as i32 {
            write_head - RING_BUFFER_SIZE as i32
        } else {
            0
        };

        if self.last_read < oldest_available - 1 {
            self.last_read = oldest_available - 1;
        }

        if self.last_read >= write_head - 1 {
            return None;
        }

        let next_frame = self.last_read + 1;

        if let Some(frame) = self.read_frame_internal(next_frame) {
            self.last_read = next_frame;
            Some(frame)
        } else {
            None
        }
    }

    fn ground_truth(&self) -> Option<GroundTruth> {
        let header = self.read_header()?;
        Some(GroundTruth {
            speed_mph: header.ground_truth.speed_mph as f64,
            vla_deg: header.ground_truth.vla_deg as f64,
            hla_deg: header.ground_truth.hla_deg as f64,
            spin_rpm: header.ground_truth.spin_rpm as f64,
            spin_axis_deg: header.ground_truth.spin_axis_deg as f64,
        })
    }

    fn reset(&mut self) {
        self.last_read = -1;
    }
}

fn flip_rgba_y(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let row_bytes = (width * 4) as usize;
    let mut flipped = vec![0u8; rgba.len()];
    for y in 0..height as usize {
        let src_y = (height as usize) - 1 - y;
        let dst_start = y * row_bytes;
        let src_start = src_y * row_bytes;
        flipped[dst_start..dst_start + row_bytes].copy_from_slice(&rgba[src_start..src_start + row_bytes]);
    }
    flipped
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
