//! Clipboard attachments for desktop apps: a file list, or an image as its
//! original encoded bytes (PNG, TIFF or JPEG). Reading only snapshots the
//! bytes; decoding happens later, bounded, off the UI thread ([`to_png`]).
//! Never ask AppKit or arboard to decode an unbounded image on paste.
//! With `decoding-fallback` off macOS, arboard decodes before limits can be
//! checked; opt in only when this tradeoff is acceptable.
//!
//! ```no_run
//! use desktop_clipboard::{ClipboardAttachment, clipboard_attachment, to_png};
//! if let Some(ClipboardAttachment::Image(encoded)) = clipboard_attachment()? {
//!     std::thread::spawn(move || {
//!         let png = to_png(encoded)?;
//!         // Send these bytes back to the UI through a channel.
//!         Ok::<_, desktop_clipboard::Error>(png)
//!     });
//! }
//! # Ok::<(), desktop_clipboard::Error>(())
//! ```
//!
//! Extracted from yapper (`crates/yapper-ui/src/attachment_input.rs` and
//! `attachments.rs`).

use std::path::PathBuf;

pub use image::ImageFormat;

/// Clipboard acquisition or image conversion failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Clipboard {
        operation: &'static str,
        source: arboard::Error,
    },
    Image {
        operation: &'static str,
        source: image::ImageError,
    },
    EncodedLimit,
    PixelLimit,
    Unreadable,
    Changed,
    InvalidPixels,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clipboard { operation, source } => write!(f, "Clipboard {operation}: {source}"),
            Self::Image { operation, source } => write!(f, "Clipboard image {operation}: {source}"),
            Self::EncodedLimit => f.write_str("Clipboard image exceeds the 32 MiB encoded limit"),
            Self::PixelLimit => {
                f.write_str("Clipboard image exceeds the pixel or decoded allocation limit")
            }
            Self::Unreadable => f.write_str("Clipboard image is unreadable"),
            Self::Changed => f.write_str("Clipboard changed during paste; try again"),
            Self::InvalidPixels => f.write_str("Clipboard image has an unexpected size"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Clipboard { source, .. } => Some(source),
            Self::Image { source, .. } => Some(source),
            _ => None,
        }
    }
}

trait ClipboardSource {
    fn files(&mut self) -> Result<Vec<PathBuf>, Error>;
    fn image(&mut self) -> Result<Option<EncodedImage>, Error>;
}
struct NativeSource(arboard::Clipboard);
impl ClipboardSource for NativeSource {
    fn files(&mut self) -> Result<Vec<PathBuf>, Error> {
        match self.0.get().file_list() {
            Ok(files) => Ok(files),
            Err(arboard::Error::ContentNotAvailable) => Ok(Vec::new()),
            Err(source) => Err(Error::Clipboard {
                operation: "reading files",
                source,
            }),
        }
    }
    fn image(&mut self) -> Result<Option<EncodedImage>, Error> {
        clipboard_image()
    }
}
fn attachment(source: &mut impl ClipboardSource) -> Result<Option<ClipboardAttachment>, Error> {
    let files = source.files()?;
    if !files.is_empty() {
        return Ok(Some(ClipboardAttachment::Files(files)));
    }
    source
        .image()
        .map(|image| image.map(ClipboardAttachment::Image))
}

/// Alternate clipboard representations are not independent attachments.
#[derive(Debug)]
pub enum ClipboardAttachment {
    Files(Vec<PathBuf>),
    Image(EncodedImage),
}

/// `None` lets a text input handle ordinary text with its native
/// selection/undo. A recognized but unreadable representation is an error,
/// not a text fallback.
pub fn clipboard_attachment() -> Result<Option<ClipboardAttachment>, Error> {
    let clipboard = arboard::Clipboard::new().map_err(|source| Error::Clipboard {
        operation: "opening",
        source,
    })?;
    attachment(&mut NativeSource(clipboard))
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
pub fn clipboard_image() -> Result<Option<EncodedImage>, Error> {
    platform::clipboard_image()
}

/// Decodes `encoded` within the limits above and re-encodes it as PNG.
/// Meant for a worker thread, not the UI thread.
pub fn to_png(encoded: EncodedImage) -> Result<Vec<u8>, Error> {
    use image::{ImageDecoder, ImageReader};
    if encoded.bytes.len() > MAX_CLIPBOARD_BYTES {
        return Err(Error::EncodedLimit);
    }
    let mut reader = ImageReader::with_format(std::io::Cursor::new(encoded.bytes), encoded.format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|source| Error::Image {
        operation: "reading header",
        source,
    })?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > MAX_PIXELS || decoder.total_bytes() > MAX_DECODE_BYTES {
        return Err(Error::PixelLimit);
    }
    let image = image::DynamicImage::from_decoder(decoder).map_err(|source| Error::Image {
        operation: "decoding",
        source,
    })?;
    let mut png = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|source| Error::Image {
            operation: "encoding PNG",
            source,
        })?;
    Ok(png.into_inner())
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{EncodedImage, Error, ImageFormat, MAX_CLIPBOARD_BYTES};

    pub fn clipboard_image() -> Result<Option<EncodedImage>, Error> {
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
            let data = board.dataForType(&kind).ok_or(Error::Unreadable)?;
            if data.length() > MAX_CLIPBOARD_BYTES {
                return Err(Error::EncodedLimit);
            }
            // SAFETY: immutable retained NSData is alive during the copy. No
            // mutable alias is made and no pointer escapes this call.
            let bytes = unsafe { data.as_bytes_unchecked() }.to_vec();
            if board.changeCount() != change {
                return Err(Error::Changed);
            }
            return Ok(Some(EncodedImage { bytes, format }));
        }
        Ok(None)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{EncodedImage, Error};

    #[cfg(not(feature = "decoding-fallback"))]
    pub fn clipboard_image() -> Result<Option<EncodedImage>, Error> {
        // No pre-decode native snapshot adapter yet. File lists and native text
        // paste remain supported; do not invoke an unbounded image decoder.
        Ok(None)
    }

    /// arboard has already decoded the image to RGBA by the time the size
    /// can be checked; this only refuses to keep or re-encode a huge one.
    #[cfg(feature = "decoding-fallback")]
    pub fn clipboard_image() -> Result<Option<EncodedImage>, Error> {
        use super::{ImageFormat, MAX_DIMENSION, MAX_PIXELS};
        let mut clipboard = arboard::Clipboard::new().map_err(|source| Error::Clipboard {
            operation: "opening",
            source,
        })?;
        let data = match clipboard.get_image() {
            Ok(data) => data,
            Err(arboard::Error::ContentNotAvailable) => return Ok(None),
            Err(source) => {
                return Err(Error::Clipboard {
                    operation: "reading image",
                    source,
                });
            }
        };
        let w = u32::try_from(data.width).map_err(|_| Error::PixelLimit)?;
        let h = u32::try_from(data.height).map_err(|_| Error::PixelLimit)?;
        if w > MAX_DIMENSION || h > MAX_DIMENSION || u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err(Error::PixelLimit);
        }
        let image = image::RgbaImage::from_raw(w, h, data.bytes.into_owned())
            .ok_or(Error::InvalidPixels)?;
        let mut png = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|source| Error::Image {
                operation: "encoding PNG",
                source,
            })?;
        Ok(Some(EncodedImage {
            bytes: png.into_inner(),
            format: ImageFormat::Png,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Source {
        files: Result<Vec<PathBuf>, Error>,
        image: Option<EncodedImage>,
        image_reads: usize,
    }
    impl ClipboardSource for Source {
        fn files(&mut self) -> Result<Vec<PathBuf>, Error> {
            std::mem::replace(&mut self.files, Ok(Vec::new()))
        }
        fn image(&mut self) -> Result<Option<EncodedImage>, Error> {
            self.image_reads += 1;
            Ok(self.image.take())
        }
    }
    #[test]
    fn file_priority_and_errors_do_not_read_the_image() {
        let mut source = Source {
            files: Ok(vec![PathBuf::from("file.txt")]),
            image: Some(encoded(1, 1)),
            image_reads: 0,
        };
        assert!(matches!(
            attachment(&mut source),
            Ok(Some(ClipboardAttachment::Files(_)))
        ));
        assert_eq!(source.image_reads, 0);
        source.files = Err(Error::Unreadable);
        assert!(matches!(attachment(&mut source), Err(Error::Unreadable)));
        assert_eq!(source.image_reads, 0);
        assert!(matches!(
            attachment(&mut source),
            Ok(Some(ClipboardAttachment::Image(_)))
        ));
        assert!(attachment(&mut source).unwrap().is_none());
    }

    #[test]
    fn conversion_error_preserves_its_source() {
        let err = to_png(EncodedImage {
            bytes: vec![],
            format: ImageFormat::Png,
        })
        .unwrap_err();
        assert!(std::error::Error::source(&err).is_some());
    }

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

    /// A 1x1 PNG whose header claims `width` x `height`: enough for the
    /// decoder to report dimensions, without allocating the pixels.
    fn png_claiming(width: u32, height: u32) -> EncodedImage {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = !0u32;
            for &b in bytes {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0xedb8_8320 & (crc & 1).wrapping_neg());
                }
            }
            !crc
        }
        let image = image::RgbaImage::new(1, 1);
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let mut bytes = bytes.into_inner();
        // Signature (8), IHDR length (4), "IHDR" (4), width, height, ...
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        let crc = crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        EncodedImage {
            bytes,
            format: ImageFormat::Png,
        }
    }

    #[test]
    fn oversized_encoded_bytes_are_rejected_before_decoding() {
        let huge = EncodedImage {
            bytes: vec![0; MAX_CLIPBOARD_BYTES + 1],
            format: ImageFormat::Png,
        };
        assert!(matches!(to_png(huge), Err(Error::EncodedLimit)));
    }

    #[test]
    fn pixel_count_is_bounded_even_within_the_dimension_limit() {
        // 8192 x 2049 stays within MAX_DIMENSION per side but exceeds MAX_PIXELS.
        assert!(matches!(
            to_png(png_claiming(MAX_DIMENSION, 2049)),
            Err(Error::PixelLimit)
        ));
        // Exactly MAX_PIXELS passes the check; decoding then fails on the
        // missing pixel data, not on the limit.
        let err = to_png(png_claiming(4096, 4096)).unwrap_err();
        assert!(matches!(err, Error::Image { .. }), "{err}");
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
