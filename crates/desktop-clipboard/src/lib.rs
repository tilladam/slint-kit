//! Clipboard attachments for desktop apps: a file list, or an image as its
//! original encoded bytes (PNG, TIFF or JPEG). Reading only snapshots the
//! bytes; decoding happens later, bounded, off the UI thread ([`to_png`]).
//! Never ask AppKit or arboard to decode an unbounded image on paste.
//!
//! Extracted from yapper (`crates/yapper-ui/src/attachment_input.rs` and
//! `attachments.rs`).

use std::path::PathBuf;

pub use image::ImageFormat;

/// Alternate clipboard representations are not independent attachments.
#[derive(Debug)]
pub enum ClipboardAttachment {
    Files(Vec<PathBuf>),
    Image(EncodedImage),
}

/// `None` lets a text input handle ordinary text with its native
/// selection/undo. A recognized but unreadable representation is an error,
/// not a text fallback.
pub fn clipboard_attachment() -> Result<Option<ClipboardAttachment>, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    match clipboard.get().file_list() {
        Ok(files) if !files.is_empty() => return Ok(Some(ClipboardAttachment::Files(files))),
        Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
        Err(e) => return Err(e.to_string()),
    }
    clipboard_image().map(|image| image.map(ClipboardAttachment::Image))
}

/// Clipboard image bytes as they were on the clipboard.
#[derive(Debug)]
pub struct EncodedImage {
    pub bytes: Vec<u8>,
    pub format: ImageFormat,
}

impl EncodedImage {
    /// MIME type for the bytes as they are.
    pub fn mime_type(&self) -> &'static str {
        self.format.to_mime_type()
    }
}

/// Largest encoded clipboard image accepted.
pub const MAX_CLIPBOARD_BYTES: usize = 32 * 1024 * 1024;
/// Largest width or height [`to_png`] decodes.
pub const MAX_DIMENSION: u32 = 8192;
/// Largest pixel count [`to_png`] decodes.
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// Largest decoder allocation [`to_png`] allows.
pub const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;

/// The clipboard's image, if it has one. macOS reads the encoded pasteboard
/// data; elsewhere see the `decoding-fallback` feature.
pub fn clipboard_image() -> Result<Option<EncodedImage>, String> {
    platform::clipboard_image()
}

/// Decodes `encoded` within the limits above and re-encodes it as PNG.
/// Meant for a worker thread, not the UI thread.
pub fn to_png(encoded: EncodedImage) -> Result<Vec<u8>, String> {
    use image::{ImageDecoder, ImageReader};
    if encoded.bytes.len() > MAX_CLIPBOARD_BYTES {
        return Err("Clipboard image exceeds the 32 MiB encoded limit".into());
    }
    let mut reader = ImageReader::with_format(std::io::Cursor::new(encoded.bytes), encoded.format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|e| format!("Clipboard image: {e}"))?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS || decoder.total_bytes() > MAX_DECODE_BYTES {
        return Err("Clipboard image exceeds the 16 megapixel limit".into());
    }
    let image = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png.into_inner())
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{EncodedImage, ImageFormat, MAX_CLIPBOARD_BYTES};

    pub fn clipboard_image() -> Result<Option<EncodedImage>, String> {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::NSString;
        let board = NSPasteboard::generalPasteboard();
        let change = board.changeCount();
        let types = board.types();
        for (kind, format) in [
            ("public.png", ImageFormat::Png),
            ("public.tiff", ImageFormat::Tiff),
            ("public.jpeg", ImageFormat::Jpeg),
        ] {
            let kind = NSString::from_str(kind);
            if !types
                .as_ref()
                .is_some_and(|types| types.containsObject(&kind))
            {
                continue;
            }
            let data = board
                .dataForType(&kind)
                .ok_or("Clipboard image is unreadable")?;
            if data.length() > MAX_CLIPBOARD_BYTES {
                return Err("Clipboard image exceeds the 32 MiB encoded limit".into());
            }
            // SAFETY: immutable retained NSData is alive during the copy. No
            // mutable alias is made and no pointer escapes this call.
            let bytes = unsafe { data.as_bytes_unchecked() }.to_vec();
            if board.changeCount() != change {
                return Err("Clipboard changed during paste; try again".into());
            }
            return Ok(Some(EncodedImage { bytes, format }));
        }
        Ok(None)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::EncodedImage;

    #[cfg(not(feature = "decoding-fallback"))]
    pub fn clipboard_image() -> Result<Option<EncodedImage>, String> {
        // No pre-decode native snapshot adapter yet. File lists and native text
        // paste remain supported; do not invoke an unbounded image decoder.
        Ok(None)
    }

    /// arboard has already decoded the image to RGBA by the time the size
    /// can be checked; this only refuses to keep or re-encode a huge one.
    #[cfg(feature = "decoding-fallback")]
    pub fn clipboard_image() -> Result<Option<EncodedImage>, String> {
        use super::{ImageFormat, MAX_DIMENSION, MAX_PIXELS};
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        let data = match clipboard.get_image() {
            Ok(data) => data,
            Err(arboard::Error::ContentNotAvailable) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let (w, h) = (data.width as u32, data.height as u32);
        if w > MAX_DIMENSION || h > MAX_DIMENSION || u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err("Clipboard image exceeds the 16 megapixel limit".into());
        }
        let image = image::RgbaImage::from_raw(w, h, data.bytes.into_owned())
            .ok_or("Clipboard image has an unexpected size")?;
        let mut png = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(Some(EncodedImage {
            bytes: png.into_inner(),
            format: ImageFormat::Png,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(width: u32, height: u32) -> EncodedImage {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([20, 40, 60, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Tiff).unwrap();
        EncodedImage {
            bytes: bytes.into_inner(),
            format: ImageFormat::Tiff,
        }
    }

    #[test]
    fn clipboard_png_preserves_pixels_and_rejects_limits() {
        let png = to_png(encoded(2, 1)).unwrap();
        let image = image::load_from_memory_with_format(&png, ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        assert_eq!(image.dimensions(), (2, 1));
        assert_eq!(image.get_pixel(1, 0).0, [20, 40, 60, 255]);
        assert!(to_png(encoded(8193, 1)).is_err());
        let mut invalid = encoded(1, 1);
        invalid.bytes.truncate(10);
        assert!(to_png(invalid).is_err());
    }

    #[test]
    fn mime_types_follow_the_format() {
        let mime = |format| {
            EncodedImage {
                bytes: Vec::new(),
                format,
            }
            .mime_type()
        };
        assert_eq!(mime(ImageFormat::Png), "image/png");
        assert_eq!(mime(ImageFormat::Jpeg), "image/jpeg");
        assert_eq!(mime(ImageFormat::Tiff), "image/tiff");
    }
}
