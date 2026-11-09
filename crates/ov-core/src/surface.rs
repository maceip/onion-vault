//! The Android surface renderer.
//!
//! CEF renders the page into a memory buffer (windowless / off-screen
//! rendering).  The renderer copies that buffer into the `ANativeWindow` of
//! the Kotlin `SurfaceView`, which is the only way to get the pixels onto
//! the screen without using the Android WebView.

use std::sync::Mutex;

use ov_cef::{FrameSink, PaintBuffer, PixelFormat};

/// The renderer that owns the Android native window.
pub struct SurfaceRenderer {
    /// The native window; `None` while no surface is attached.
    #[cfg(target_os = "android")]
    window: Mutex<Option<ndk::native_window::NativeWindow>>,
    /// The size of the surface in pixels.
    size: Mutex<(i32, i32)>,
    /// The pixel format of the surface.
    format: PixelFormat,
}

impl SurfaceRenderer {
    /// Create a renderer without a surface.
    pub fn new(format: PixelFormat) -> Self {
        Self {
            #[cfg(target_os = "android")]
            window: Mutex::new(None),
            size: Mutex::new((0, 0)),
            format,
        }
    }

    /// Attach the Android native window.
    #[cfg(target_os = "android")]
    pub fn attach(&self, window: ndk::native_window::NativeWindow, width: i32, height: i32) {
        *self.size.lock().unwrap() = (width, height);
        *self.window.lock().unwrap() = Some(window);
    }

    /// Detach the Android native window.
    #[cfg(target_os = "android")]
    pub fn detach(&self) {
        *self.window.lock().unwrap() = None;
        *self.size.lock().unwrap() = (0, 0);
    }

    /// Resize the surface.
    pub fn resize(&self, width: i32, height: i32) {
        *self.size.lock().unwrap() = (width, height);
    }

    /// The current size of the surface.
    pub fn size(&self) -> (i32, i32) {
        *self.size.lock().unwrap()
    }
}

impl FrameSink for SurfaceRenderer {
    fn present(&self, buffer: PaintBuffer<'_>) {
        #[cfg(target_os = "android")]
        self.present_android(buffer);
        #[cfg(not(target_os = "android"))]
        {
            // Without a platform surface there is nothing to draw into; the
            // frame is dropped on purpose (the desktop build has no UI).
            let _ = buffer;
        }
    }

    fn viewport(&self) -> (i32, i32) {
        self.size()
    }

    fn pixel_format(&self) -> PixelFormat {
        self.format
    }
}

#[cfg(target_os = "android")]
impl SurfaceRenderer {
    /// Copy the rendered frame into the native window.
    fn present_android(&self, buffer: PaintBuffer<'_>) {
        let guard = self.window.lock().unwrap();
        let Some(window) = guard.as_ref() else {
            return;
        };

        let Ok(mut locked) = window.lock(None) else {
            log::error!("cannot lock the Android native window");
            return;
        };

        let dst_stride = locked.stride();
        let dst_width = locked.width();
        let dst_height = locked.height();

        let Some(bytes) = locked.bytes() else {
            log::error!("the Android native window has no buffer");
            return;
        };

        let rows = buffer.height.min(dst_height as i32).max(0) as usize;
        let cols = buffer.width.min(dst_width as i32).max(0) as usize;

        // The off-screen buffer of CEF is `CefPixelFormat::N32`; on Linux and
        // Android that is RGBA, on Windows BGRA.  The Android native window
        // always expects RGBA, so a BGRA buffer has to be swizzled.
        let swizzle = self.format.needs_swizzle(PixelFormat::Rgba8888);

        for row in 0..rows {
            let src_start = row * buffer.stride;
            let src = &buffer.pixels[src_start..src_start + cols * 4];
            let dst_start = row * dst_stride;
            if dst_start + cols * 4 > bytes.len() {
                break;
            }
            let dst = &mut bytes[dst_start..dst_start + cols * 4];
            copy_row(dst, src, swizzle);
        }
    }
}

/// Copy one row of pixels, optionally swapping the red and the blue channel.
#[cfg(target_os = "android")]
fn copy_row(dst: &mut [std::mem::MaybeUninit<u8>], src: &[u8], swizzle: bool) {
    for (target, pixel) in dst.chunks_mut(4).zip(src.chunks(4)) {
        if pixel.len() < 4 {
            break;
        }
        if swizzle {
            target[0].write(pixel[2]);
            target[1].write(pixel[1]);
            target[2].write(pixel[0]);
        } else {
            target[0].write(pixel[0]);
            target[1].write(pixel[1]);
            target[2].write(pixel[2]);
        }
        target[3].write(pixel[3]);
    }
}

/// Copy one row of pixels (host build, used by the tests).
#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
fn copy_row(dst: &mut [u8], src: &[u8], swizzle: bool) {
    for (target, pixel) in dst.chunks_mut(4).zip(src.chunks(4)) {
        if pixel.len() < 4 {
            break;
        }
        if swizzle {
            target[0] = pixel[2];
            target[1] = pixel[1];
            target[2] = pixel[0];
        } else {
            target[0] = pixel[0];
            target[1] = pixel[1];
            target[2] = pixel[2];
        }
        target[3] = pixel[3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_copied_verbatim_when_the_formats_match() {
        let src = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut dst = [0u8; 8];
        copy_row(&mut dst, &src, false);
        assert_eq!(dst, src);
    }

    #[test]
    fn rows_are_swizzled_when_the_formats_differ() {
        let src = [1u8, 2, 3, 4];
        let mut dst = [0u8; 4];
        copy_row(&mut dst, &src, true);
        assert_eq!(dst, [3, 2, 1, 4]);
    }
}
