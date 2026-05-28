use image::{GrayImage, Rgb, RgbImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugMetadata {
    pub algorithm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_time_ms: Option<f64>,
    #[serde(default)]
    pub extra: serde_json::Value,
}

impl DebugMetadata {
    pub fn new(algorithm: &str) -> Self {
        Self {
            algorithm: algorithm.to_string(),
            confidence: None,
            processing_time_ms: None,
            extra: serde_json::Value::Null,
        }
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence);
        self
    }

    pub fn with_timing(mut self, ms: f64) -> Self {
        self.processing_time_ms = Some(ms);
        self
    }

    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.extra = extra;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Overlay {
    Circle {
        center: (f64, f64),
        radius: f64,
        color: [u8; 3],
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Line {
        start: (f64, f64),
        end: (f64, f64),
        color: [u8; 3],
    },
    Points {
        coords: Vec<(f64, f64)>,
        color: [u8; 3],
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Text {
        position: (f64, f64),
        text: String,
        color: [u8; 3],
    },
    Rect {
        top_left: (f64, f64),
        width: f64,
        height: f64,
        color: [u8; 3],
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

impl Overlay {
    pub fn circle(center: (f64, f64), radius: f64, color: [u8; 3]) -> Self {
        Self::Circle {
            center,
            radius,
            color,
            label: None,
        }
    }

    pub fn circle_labeled(center: (f64, f64), radius: f64, color: [u8; 3], label: &str) -> Self {
        Self::Circle {
            center,
            radius,
            color,
            label: Some(label.to_string()),
        }
    }

    pub fn line(start: (f64, f64), end: (f64, f64), color: [u8; 3]) -> Self {
        Self::Line { start, end, color }
    }

    pub fn points(coords: Vec<(f64, f64)>, color: [u8; 3]) -> Self {
        Self::Points {
            coords,
            color,
            label: None,
        }
    }

    pub fn text(position: (f64, f64), text: &str, color: [u8; 3]) -> Self {
        Self::Text {
            position,
            text: text.to_string(),
            color,
        }
    }

    pub fn rect(top_left: (f64, f64), width: f64, height: f64, color: [u8; 3]) -> Self {
        Self::Rect {
            top_left,
            width,
            height,
            color,
            label: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DebugFrame {
    pub image: GrayImage,
    pub overlays: Vec<Overlay>,
    pub metadata: DebugMetadata,
}

impl DebugFrame {
    pub fn new(image: GrayImage, algorithm: &str) -> Self {
        Self {
            image,
            overlays: Vec::new(),
            metadata: DebugMetadata::new(algorithm),
        }
    }

    pub fn add_overlay(&mut self, overlay: Overlay) {
        self.overlays.push(overlay);
    }

    pub fn with_overlay(mut self, overlay: Overlay) -> Self {
        self.overlays.push(overlay);
        self
    }

    pub fn with_metadata(mut self, metadata: DebugMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn render_to_rgb(&self) -> RgbImage {
        let (width, height) = self.image.dimensions();
        let mut rgb = RgbImage::new(width, height);

        for (x, y, gray) in self.image.enumerate_pixels() {
            rgb.put_pixel(x, y, Rgb([gray.0[0], gray.0[0], gray.0[0]]));
        }

        for overlay in &self.overlays {
            draw_overlay(&mut rgb, overlay);
        }

        rgb
    }
}

fn draw_overlay(img: &mut RgbImage, overlay: &Overlay) {
    match overlay {
        Overlay::Circle {
            center,
            radius,
            color,
            ..
        } => {
            draw_circle(img, *center, *radius, *color);
        }
        Overlay::Line { start, end, color } => {
            draw_line(img, *start, *end, *color);
        }
        Overlay::Points { coords, color, .. } => {
            for &(x, y) in coords {
                draw_cross(img, (x, y), 3.0, *color);
            }
        }
        Overlay::Text { .. } => {
            // Text rendering requires a font - skip for now
        }
        Overlay::Rect {
            top_left,
            width,
            height,
            color,
            ..
        } => {
            draw_rect(img, *top_left, *width, *height, *color);
        }
    }
}

fn draw_circle(img: &mut RgbImage, center: (f64, f64), radius: f64, color: [u8; 3]) {
    let (cx, cy) = center;
    let (w, h) = (img.width() as i32, img.height() as i32);
    let steps = (radius * 8.0).max(32.0) as usize;

    for i in 0..steps {
        let angle = (i as f64 / steps as f64) * std::f64::consts::TAU;
        let x = (cx + radius * angle.cos()).round() as i32;
        let y = (cy + radius * angle.sin()).round() as i32;

        if x >= 0 && x < w && y >= 0 && y < h {
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
}

fn draw_line(img: &mut RgbImage, start: (f64, f64), end: (f64, f64), color: [u8; 3]) {
    let (x0, y0) = start;
    let (x1, y1) = end;
    let (w, h) = (img.width() as i32, img.height() as i32);

    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let steps = dx.max(dy).max(1.0) as usize;

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = (x0 + t * (x1 - x0)).round() as i32;
        let y = (y0 + t * (y1 - y0)).round() as i32;

        if x >= 0 && x < w && y >= 0 && y < h {
            img.put_pixel(x as u32, y as u32, Rgb(color));
        }
    }
}

fn draw_cross(img: &mut RgbImage, center: (f64, f64), size: f64, color: [u8; 3]) {
    let (cx, cy) = center;
    draw_line(img, (cx - size, cy), (cx + size, cy), color);
    draw_line(img, (cx, cy - size), (cx, cy + size), color);
}

fn draw_rect(img: &mut RgbImage, top_left: (f64, f64), width: f64, height: f64, color: [u8; 3]) {
    let (x0, y0) = top_left;
    let x1 = x0 + width;
    let y1 = y0 + height;

    draw_line(img, (x0, y0), (x1, y0), color);
    draw_line(img, (x1, y0), (x1, y1), color);
    draw_line(img, (x1, y1), (x0, y1), color);
    draw_line(img, (x0, y1), (x0, y0), color);
}

pub mod colors {
    pub const RED: [u8; 3] = [255, 0, 0];
    pub const GREEN: [u8; 3] = [0, 255, 0];
    pub const BLUE: [u8; 3] = [0, 0, 255];
    pub const YELLOW: [u8; 3] = [255, 255, 0];
    pub const CYAN: [u8; 3] = [0, 255, 255];
    pub const MAGENTA: [u8; 3] = [255, 0, 255];
    pub const ORANGE: [u8; 3] = [255, 165, 0];
    pub const WHITE: [u8; 3] = [255, 255, 255];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_frame_creation() {
        let img = GrayImage::new(100, 100);
        let frame = DebugFrame::new(img, "test");
        assert_eq!(frame.metadata.algorithm, "test");
        assert!(frame.overlays.is_empty());
    }

    #[test]
    fn test_overlay_circle() {
        let overlay = Overlay::circle((50.0, 50.0), 10.0, colors::RED);
        if let Overlay::Circle {
            center,
            radius,
            color,
            label,
        } = overlay
        {
            assert_eq!(center, (50.0, 50.0));
            assert_eq!(radius, 10.0);
            assert_eq!(color, colors::RED);
            assert!(label.is_none());
        } else {
            panic!("Expected Circle overlay");
        }
    }

    #[test]
    fn test_render_to_rgb() {
        let mut img = GrayImage::new(50, 50);
        for p in img.pixels_mut() {
            p.0[0] = 128;
        }

        let frame = DebugFrame::new(img, "test")
            .with_overlay(Overlay::circle((25.0, 25.0), 10.0, colors::GREEN));

        let rgb = frame.render_to_rgb();
        assert_eq!(rgb.dimensions(), (50, 50));
    }
}
