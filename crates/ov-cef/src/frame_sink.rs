//! The pixel buffer that the off-screen renderer hands to the platform.
//!
//! CEF renders into a plain memory buffer when windowless (off-screen)
//! rendering is enabled.  On Android that buffer is copied into the
//! `ANativeWindow` of a `SurfaceView` by the `ov-core` crate.

/// A rendered frame.
pub struct PaintBuffer<'a> {
    /// The BGRA (or RGBA, see [`crate::PixelFormat`]) pixel data.
    pub pixels: &'a [u8],
    /// The width of the frame in pixels.
    pub width: i32,
    /// The height of the frame in pixels.
    pub height: i32,
    /// The number of bytes per row.
    pub stride: usize,
    /// The rectangles that have to be repainted.
    pub dirty_rects: &'a [cef::Rect],
}

/// The pixel format of the off-screen buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// Blue, green, red, alpha - the layout of `CefPixelFormat::N32` on
    /// Windows.
    Bgra8888,
    /// Red, green, blue, alpha - the layout of `CefPixelFormat::N32` on
    /// Linux and Android, and the layout of `ANativeWindow`'s
    /// `WINDOW_FORMAT_RGBA_8888`.
    Rgba8888,
}

impl PixelFormat {
    /// `true` when the format has to be swizzled before it is written into an
    /// `ANativeWindow`.
    pub fn needs_swizzle(&self, target: PixelFormat) -> bool {
        *self != target
    }
}

/// Receives the rendered frames.
///
/// The implementation lives in `ov-core`: it locks the Android native window
/// and copies the pixels into it.
pub trait FrameSink: Send + Sync {
    /// Present a frame.
    fn present(&self, buffer: PaintBuffer<'_>);
    /// The size of the surface the browser has to render into.
    fn viewport(&self) -> (i32, i32);
    /// The pixel format of the surface.
    fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgba8888
    }
}
