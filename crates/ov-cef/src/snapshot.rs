//! The snapshot of a visited page.
//!
//! A hidden service is slow: a back navigation over a six hop circuit can
//! take seconds, and a page that was reached through a form POST cannot be
//! restored by the network stack at all.  PlugTalk therefore keeps a
//! bounded ring of snapshots: the metadata of the page, a downsampled copy of
//! the rendered frame and the state of the PGP fields.
//!
//! The snapshot is assembled in the browser process, which sees every piece
//! of it:
//!
//!   * the URL comes from the load handler,
//!   * the title, the markup and the PGP fields come from the DOM scan of the
//!     render process,
//!   * the scroll offset comes from the render handler, and
//!   * the frame comes from the paint callback.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::frame_sink::PixelFormat;
use crate::ipc::DomFieldMetadata;

/// The maximum number of snapshots that are kept.
pub const MAX_DEPTH: usize = 12;

/// The factor the rendered frame is reduced by.
///
/// A full frame of a phone is about ten megabytes; a snapshot of a quarter of
/// the size is enough for the preview of the predictive back gesture and
/// keeps the ring below ten megabytes.
pub const FRAME_SCALE: usize = 4;

/// The maximum size of the markup that is kept (the bigger pages are dropped).
pub const MAX_MARKUP_BYTES: usize = 512 * 1024;

/// A downsampled copy of a rendered frame.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameSnapshot {
    /// The width of the snapshot in pixels.
    pub width: u32,
    /// The height of the snapshot in pixels.
    pub height: u32,
    /// The pixels in the `R`, `G`, `B`, `A` order.
    pub pixels: Vec<u8>,
}

impl FrameSnapshot {
    /// `true` when the snapshot carries pixels.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.pixels.is_empty()
    }
}

/// The snapshot of a page.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PageSnapshot {
    /// The URL of the page.
    pub url: String,
    /// The title of the page.
    pub title: String,
    /// The horizontal scroll offset of the document.
    pub scroll_x: f64,
    /// The vertical scroll offset of the document.
    pub scroll_y: f64,
    /// The markup of the document.
    pub markup: Vec<u8>,
    /// The PGP fields of the page.
    pub fields: Vec<DomFieldMetadata>,
    /// The last rendered frame of the page.
    pub frame: Option<FrameSnapshot>,
}

impl PageSnapshot {
    /// `true` when the snapshot carries a usable frame.
    pub fn has_frame(&self) -> bool {
        self.frame.as_ref().is_some_and(|frame| !frame.is_empty())
    }
}

/// The state that the collector gathers.
#[derive(Default)]
struct CollectorState {
    /// The entry that is being built (the page that is currently shown).
    current: Option<PageSnapshot>,
    /// The pages that were left (the newest first).
    finished: Vec<PageSnapshot>,
    /// The scroll offset of the document.
    scroll_x: f64,
    /// The scroll offset of the document.
    scroll_y: f64,
}

/// The collector of the snapshot data.
///
/// The collector is shared between the render handler (which delivers the
/// frames and the scroll offset), the load handler (which starts a new page)
/// and the client (which receives the DOM scan of the render process).
#[derive(Default)]
pub struct SnapshotCollector {
    inner: Mutex<CollectorState>,
}

impl SnapshotCollector {
    /// Create an empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// A new page was loaded.
    ///
    /// The page that was shown until now becomes part of the ring, and a new
    /// entry is started for the page that was just loaded.  The snapshot of
    /// the page that was left is returned: it is complete now and the native
    /// UI needs it for the preview of the predictive back gesture.
    pub fn begin_page(&self, url: &str) -> Option<PageSnapshot> {
        let mut state = self.inner.lock().unwrap();
        let finished = state.current.take();
        if let Some(previous) = &finished {
            state.finished.insert(0, previous.clone());
            // The ring is bounded: the oldest page falls out of it.
            state.finished.truncate(MAX_DEPTH - 1);
        }
        state.current = Some(PageSnapshot {
            url: url.to_string(),
            ..PageSnapshot::default()
        });
        finished
    }

    /// The render process reported the state of the document.
    pub fn update_scan(&self, title: &str, markup: String, fields: Vec<DomFieldMetadata>) {
        let mut state = self.inner.lock().unwrap();
        let Some(current) = state.current.as_mut() else {
            return;
        };
        if !title.is_empty() {
            current.title = title.to_string();
        }
        if !markup.is_empty() && markup.len() <= MAX_MARKUP_BYTES {
            current.markup = markup.into_bytes();
        }
        current.fields = fields;
    }

    /// The document was scrolled.
    pub fn update_scroll(&self, x: f64, y: f64) {
        let mut state = self.inner.lock().unwrap();
        state.scroll_x = x;
        state.scroll_y = y;
        if let Some(current) = state.current.as_mut() {
            current.scroll_x = x;
            current.scroll_y = y;
        }
    }

    /// A frame was rendered.
    pub fn update_frame(
        &self,
        pixels: &[u8],
        width: i32,
        height: i32,
        stride: usize,
        format: PixelFormat,
    ) {
        let mut state = self.inner.lock().unwrap();
        let Some(current) = state.current.as_mut() else {
            return;
        };
        if width <= 0 || height <= 0 {
            return;
        }
        let frame = downsample(pixels, width as usize, height as usize, stride, format);
        current.frame = Some(frame);
    }

    /// The snapshots of the current page and the pages that were left.
    ///
    /// The first entry is the page that is currently shown.
    pub fn stack(&self) -> Vec<PageSnapshot> {
        let state = self.inner.lock().unwrap();
        let mut stack = Vec::with_capacity(state.finished.len() + 1);
        if let Some(current) = state.current.clone() {
            stack.push(current);
        }
        stack.extend(state.finished.iter().cloned());
        stack.truncate(MAX_DEPTH);
        stack
    }

    /// The number of snapshots.
    pub fn len(&self) -> usize {
        self.stack().len()
    }

    /// `true` when there is no snapshot at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Reduce a rendered frame to a quarter of its size.
///
/// The buffer of CEF is in the `N32` format: on Linux and Android that is
/// `R`, `G`, `B`, `A`, on Windows `B`, `G`, `R`, `A`.  The snapshot always
/// uses the `R`, `G`, `B`, `A` order, because that is what the Android
/// bitmap of the preview expects.
pub fn downsample(
    pixels: &[u8],
    width: usize,
    height: usize,
    stride: usize,
    format: PixelFormat,
) -> FrameSnapshot {
    let swizzle = format.needs_swizzle(PixelFormat::Rgba8888);
    let columns = width.div_ceil(FRAME_SCALE).max(1);
    let rows = height.div_ceil(FRAME_SCALE).max(1);
    let mut out = Vec::with_capacity(columns * rows * 4);

    for y in (0..height).step_by(FRAME_SCALE) {
        for x in (0..width).step_by(FRAME_SCALE) {
            let start = y * stride + x * 4;
            if start + 4 > pixels.len() {
                break;
            }
            let pixel = &pixels[start..start + 4];
            if swizzle {
                out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            } else {
                out.extend_from_slice(pixel);
            }
        }
    }

    FrameSnapshot {
        width: columns as u32,
        height: rows as u32,
        pixels: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_is_reduced() {
        // A two by two image in the RGBA order.
        let pixels: Vec<u8> = (0..16).collect();
        let frame = downsample(&pixels, 2, 2, 8, PixelFormat::Rgba8888);
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.pixels, vec![0, 1, 2, 3]);
    }

    #[test]
    fn the_channels_are_swizzled_when_the_format_differs() {
        let pixels: Vec<u8> = (0..16).collect();
        let frame = downsample(&pixels, 2, 2, 8, PixelFormat::Bgra8888);
        assert_eq!(frame.pixels, vec![2, 1, 0, 3]);
    }

    #[test]
    fn the_ring_is_bounded() {
        let collector = SnapshotCollector::new();
        for index in 0..(MAX_DEPTH + 5) {
            collector.begin_page(&format!("https://page-{index}.onion"));
        }
        let stack = collector.stack();
        assert_eq!(stack.len(), MAX_DEPTH);
        assert_eq!(
            stack[0].url,
            format!("https://page-{}.onion", MAX_DEPTH + 4)
        );
    }

    #[test]
    fn the_scan_fills_the_current_page() {
        let collector = SnapshotCollector::new();
        collector.begin_page("https://example.onion");
        collector.update_scan("The title", "<html></html>".to_string(), Vec::new());
        let stack = collector.stack();
        assert_eq!(stack[0].title, "The title");
        assert_eq!(stack[0].markup, b"<html></html>");
    }
}
