use image::{GrayImage, Luma};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DetectionError {
    #[error("Ball not found in frame")]
    NotFound,
    #[error("Ball partially clipped at frame edge")]
    Clipped,
}

#[derive(Debug, Clone, Copy)]
pub struct BallDetection {
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
}

const EXPECTED_BALL_RADIUS_PX: u32 = 12;
const PEAK_WINDOW: u32 = EXPECTED_BALL_RADIUS_PX * 3;

pub struct BallDetector {
    diff_threshold: u8,
    min_pixels: usize,
    background: Option<BackgroundModel>,
}

struct BackgroundModel {
    left: GrayImage,
    right: GrayImage,
}

impl BallDetector {
    pub fn new(diff_threshold: u8, min_pixels: usize) -> Self {
        Self {
            diff_threshold,
            min_pixels,
            background: None,
        }
    }

    pub fn set_background(&mut self, left: &GrayImage, right: &GrayImage) {
        self.background = Some(BackgroundModel {
            left: left.clone(),
            right: right.clone(),
        });
    }

    pub fn set_background_from_min(&mut self, frames: &[(GrayImage, GrayImage)]) {
        if frames.is_empty() {
            return;
        }
        let (w, h) = frames[0].0.dimensions();
        let mut left_bg = frames[0].0.clone();
        let mut right_bg = frames[0].1.clone();
        for (left, right) in &frames[1..] {
            for y in 0..h {
                for x in 0..w {
                    let lv = left_bg.get_pixel(x, y).0[0].min(left.get_pixel(x, y).0[0]);
                    left_bg.put_pixel(x, y, Luma([lv]));
                    let rv = right_bg.get_pixel(x, y).0[0].min(right.get_pixel(x, y).0[0]);
                    right_bg.put_pixel(x, y, Luma([rv]));
                }
            }
        }
        self.background = Some(BackgroundModel {
            left: left_bg,
            right: right_bg,
        });
    }

    pub fn has_background(&self) -> bool {
        self.background.is_some()
    }

    pub fn detect_with_background(
        &self,
        gray: &GrayImage,
        is_left: bool,
    ) -> Result<BallDetection, DetectionError> {
        let bg = match &self.background {
            Some(bg) => if is_left { &bg.left } else { &bg.right },
            None => return self.detect(gray),
        };

        let diff = subtract_background(gray, bg);
        self.detect_peak(&diff)
    }

    pub fn get_diff_image(&self, gray: &GrayImage, is_left: bool) -> Option<GrayImage> {
        let bg = match &self.background {
            Some(bg) => if is_left { &bg.left } else { &bg.right },
            None => return None,
        };
        Some(subtract_background(gray, bg))
    }

    pub fn detect(&self, gray: &GrayImage) -> Result<BallDetection, DetectionError> {
        self.detect_peak(gray)
    }

    fn detect_peak(&self, gray: &GrayImage) -> Result<BallDetection, DetectionError> {
        let (width, height) = gray.dimensions();

        let mut peak_val = 0u8;
        let mut peak_x = 0u32;
        let mut peak_y = 0u32;

        for (x, y, pixel) in gray.enumerate_pixels() {
            if pixel.0[0] > peak_val {
                peak_val = pixel.0[0];
                peak_x = x;
                peak_y = y;
            }
        }

        if peak_val < self.diff_threshold {
            return Err(DetectionError::NotFound);
        }

        let x_start = peak_x.saturating_sub(PEAK_WINDOW);
        let y_start = peak_y.saturating_sub(PEAK_WINDOW);
        let x_end = (peak_x + PEAK_WINDOW).min(width - 1);
        let y_end = (peak_y + PEAK_WINDOW).min(height - 1);

        let local_thresh = (peak_val as u16 / 3) as u8;

        let mut weighted_x: f64 = 0.0;
        let mut weighted_y: f64 = 0.0;
        let mut total_weight: f64 = 0.0;
        let mut count = 0usize;
        let mut min_x = u32::MAX;
        let mut max_x = 0u32;
        let mut min_y = u32::MAX;
        let mut max_y = 0u32;

        for y in y_start..=y_end {
            for x in x_start..=x_end {
                let v = gray.get_pixel(x, y).0[0];
                if v >= local_thresh {
                    let w = v as f64;
                    weighted_x += x as f64 * w;
                    weighted_y += y as f64 * w;
                    total_weight += w;
                    count += 1;
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        if count < self.min_pixels {
            return Err(DetectionError::NotFound);
        }

        let center_x = weighted_x / total_weight;
        let center_y = weighted_y / total_weight;

        let x_span = (max_x - min_x) as f64;
        let y_span = (max_y - min_y) as f64;
        let radius = x_span.max(y_span) / 2.0 + 2.0;

        let margin = radius.max(EXPECTED_BALL_RADIUS_PX as f64);
        if center_x - margin < 0.0
            || center_x + margin >= width as f64
            || center_y - margin < 0.0
            || center_y + margin >= height as f64
        {
            return Err(DetectionError::Clipped);
        }

        Ok(BallDetection { center_x, center_y, radius })
    }
}

fn subtract_background(frame: &GrayImage, background: &GrayImage) -> GrayImage {
    let (width, height) = frame.dimensions();
    let mut diff = GrayImage::new(width, height);

    for (x, y, pixel) in frame.enumerate_pixels() {
        let bg_val = background.get_pixel(x, y).0[0] as i16;
        let frame_val = pixel.0[0] as i16;
        let d = (frame_val - bg_val).max(0) as u8;
        diff.put_pixel(x, y, Luma([d]));
    }

    diff
}

impl Default for BallDetector {
    fn default() -> Self {
        Self::new(30, 10)
    }
}
