//! จับภาพและวาด in the Browser block (Interface, Phase C: Browser): a capture of the page that
//! the person marks with strokes. The marks are drawn over the capture while they are made and
//! burned into it when it is sent, as one PNG the composer attaches for the model to see.

use std::{path::PathBuf, sync::Arc};

use base64::Engine as _;
use gpui::RenderImage;
use image::{Rgba, RgbaImage};

/// How thick a mark is, in CSS pixels.
pub const STROKE: f32 = 3.;
/// The marks' colour: the red a reviewer circles with.
pub const RED: [u8; 4] = [0xE5, 0x48, 0x4D, 0xFF];

/// A point on the capture, in CSS pixels from its top left.
pub type Spot = (f32, f32);

pub struct Marks {
    /// The capture as Chromium took it, in device pixels.
    shot: RgbaImage,
    /// The capture as GPUI draws it (BGRA).
    pub shown: Arc<RenderImage>,
    /// The page's size in CSS pixels when it was taken.
    pub size: (f32, f32),
    pub strokes: Vec<Vec<Spot>>,
}

impl Marks {
    /// Reads `Page.captureScreenshot`'s base64 PNG, taken of a page `size` CSS pixels big.
    pub fn from_capture(data: &str, size: (f32, f32)) -> anyhow::Result<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
        let shot =
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.into_rgba8();
        Ok(Self::new(shot, size))
    }

    pub fn new(shot: RgbaImage, size: (f32, f32)) -> Self {
        let mut bgra = shot.clone();
        for pixel in bgra.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        Self {
            shot,
            shown: Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])),
            size,
            strokes: Vec::new(),
        }
    }

    /// Starts a stroke at `spot`.
    pub fn begin(&mut self, spot: Spot) {
        self.strokes.push(vec![spot]);
    }

    /// Carries the last stroke on to `spot`.
    pub fn extend(&mut self, spot: Spot) {
        if let Some(stroke) = self.strokes.last_mut() {
            stroke.push(spot);
        }
    }

    pub fn undo(&mut self) {
        self.strokes.pop();
    }

    /// The capture with the marks burned in.
    pub fn flatten(&self) -> RgbaImage {
        let mut image = self.shot.clone();
        let scale = if self.size.0 > 0. {
            image.width() as f32 / self.size.0
        } else {
            1.
        };
        let radius = STROKE * scale / 2.;
        for stroke in &self.strokes {
            let mut points = stroke.iter().map(|&(x, y)| (x * scale, y * scale));
            let Some(mut from) = points.next() else {
                continue;
            };
            dot(&mut image, from, radius);
            for to in points {
                line(&mut image, from, to, radius);
                from = to;
            }
        }
        image
    }

    /// Writes the marked capture to a PNG of its own, for `--image`.
    pub fn save(&self) -> anyhow::Result<PathBuf> {
        let dir = std::env::temp_dir().join("cedian-capture");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!(
            "จับภาพ-{}.png",
            chrono::Local::now().format("%Y%m%d-%H%M%S%3f")
        ));
        self.flatten()
            .save_with_format(&path, image::ImageFormat::Png)?;
        Ok(path)
    }
}

/// Stamps dots along a segment, half a pixel apart, so the line has no gaps.
fn line(image: &mut RgbaImage, from: (f32, f32), to: (f32, f32), radius: f32) {
    let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
    let steps = (length * 2.).ceil().max(1.) as u32;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let at = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        dot(image, at, radius);
    }
}

fn dot(image: &mut RgbaImage, (cx, cy): (f32, f32), radius: f32) {
    let (width, height) = (image.width() as i64, image.height() as i64);
    let left = ((cx - radius).floor() as i64).max(0);
    let right = ((cx + radius).ceil() as i64).min(width - 1);
    let top = ((cy - radius).floor() as i64).max(0);
    let bottom = ((cy + radius).ceil() as i64).min(height - 1);
    for y in top..=bottom {
        for x in left..=right {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dx * dx + dy * dy <= radius * radius {
                image.put_pixel(x as u32, y as u32, Rgba(RED));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_is_burned_in_at_the_capture_scale() {
        // A page 100 by 50 CSS pixels taken at 2x.
        let mut marks = Marks::new(RgbaImage::from_pixel(200, 100, Rgba([255; 4])), (100., 50.));
        marks.begin((10., 10.));
        marks.extend((90., 10.));
        let flat = marks.flatten();
        assert_eq!(flat.get_pixel(20, 20).0, RED);
        assert_eq!(flat.get_pixel(100, 20).0, RED);
        assert_eq!(flat.get_pixel(179, 20).0, RED);
        assert_eq!(flat.get_pixel(100, 60).0, [255; 4]);
        marks.undo();
        assert_eq!(marks.flatten().get_pixel(100, 20).0, [255; 4]);
    }

    #[test]
    fn a_capture_is_read_and_saved_as_a_png() {
        let mut png = std::io::Cursor::new(Vec::new());
        RgbaImage::from_pixel(4, 2, Rgba([1, 2, 3, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("encode");
        let data = base64::engine::general_purpose::STANDARD.encode(png.into_inner());
        let mut marks = Marks::from_capture(&data, (4., 2.)).expect("capture");
        marks.begin((0., 0.));
        let path = marks.save().expect("save");
        let saved = image::open(&path).expect("open").into_rgba8();
        assert_eq!(saved.dimensions(), (4, 2));
        assert_eq!(saved.get_pixel(0, 0).0, RED);
        assert_eq!(saved.get_pixel(3, 1).0, [1, 2, 3, 255]);
        std::fs::remove_file(path).ok();
    }
}
