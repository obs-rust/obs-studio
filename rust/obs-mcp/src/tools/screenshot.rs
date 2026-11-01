//! The `screenshot` tool: captures a source or the program output as a PNG
//! image (#3288).
//!
//! The tool talks to OBS through [`CaptureBackend`], so it is tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::io::Write as _;
use std::rc::Rc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};
use serde_json::{Value, json};

use crate::server::{Content, Tool, ToolRegistry};

/// Raw pixels from [`CaptureBackend::capture`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, row-major RGBA, top row first.
    pub rgba: Vec<u8>,
}

/// The capture operation the tool needs from OBS.
pub trait CaptureBackend {
    /// Renders `source` (or the program output when `None`) and returns its
    /// pixels. `width` and `height` are the requested size; a side left `None`
    /// keeps the aspect ratio or the native size.
    ///
    /// # Errors
    ///
    /// Returns a message when the source does not exist or cannot be captured.
    fn capture(
        &self,
        source: Option<&str>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<Frame, String>;
}

/// Encodes `frame` as a PNG file.
///
/// # Errors
///
/// Returns a message when the dimensions are zero or do not match the buffer.
pub fn encode_png(frame: &Frame) -> Result<Vec<u8>, String> {
    let Frame {
        width,
        height,
        rgba,
    } = frame;
    if *width == 0 || *height == 0 {
        return Err(format!("cannot encode a {width}x{height} image"));
    }
    let stride = (*width as usize)
        .checked_mul(4)
        .ok_or("image is too wide")?;
    let expected = stride
        .checked_mul(*height as usize)
        .ok_or("image is too large")?;
    if rgba.len() != expected {
        return Err(format!(
            "frame has {} bytes, expected {expected} for {width}x{height} RGBA",
            rgba.len()
        ));
    }

    // Every scanline is prefixed with filter type 0 (none).
    let mut raw = Vec::with_capacity(expected + *height as usize);
    for row in rgba.chunks_exact(stride) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = ZlibEncoder::new(Vec::new(), Compression::fast());
    zlib.write_all(&raw).map_err(|e| e.to_string())?;
    let idat = zlib.finish().map_err(|e| e.to_string())?;

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // 8 bits per channel, colour type 6 (RGBA), deflate, adaptive filtering, no interlace.
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = Vec::with_capacity(idat.len() + 57);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    push_chunk(&mut png, b"IHDR", &ihdr)?;
    push_chunk(&mut png, b"IDAT", &idat)?;
    push_chunk(&mut png, b"IEND", &[])?;
    Ok(png)
}

/// Appends a length-prefixed, CRC-suffixed PNG chunk.
fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) -> Result<(), String> {
    let len = u32::try_from(data.len()).map_err(|_| "PNG chunk is too large".to_owned())?;
    png.extend_from_slice(&len.to_be_bytes());
    let start = png.len();
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc = Crc::new();
    crc.update(&png[start..]);
    png.extend_from_slice(&crc.sum().to_be_bytes());
    Ok(())
}

/// An optional positive pixel dimension argument.
fn dimension_arg(arguments: &Value, key: &str) -> Result<Option<u32>, String> {
    match arguments.get(key) {
        None => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|&n| n > 0)
            .map(Some)
            .ok_or_else(|| format!("argument `{key}` must be a positive integer")),
    }
}

/// The `screenshot` tool bound to a backend.
struct ScreenshotTool {
    backend: Rc<dyn CaptureBackend>,
}

impl ScreenshotTool {
    /// Captures and returns the base64 PNG.
    fn base64_png(&self, arguments: &Value) -> Result<String, String> {
        let source = match arguments.get("source") {
            None => None,
            Some(Value::String(s)) if !s.is_empty() => Some(s.as_str()),
            Some(_) => return Err("argument `source` must be a non-empty string".into()),
        };
        let width = dimension_arg(arguments, "width")?;
        let height = dimension_arg(arguments, "height")?;
        let frame = self.backend.capture(source, width, height)?;
        Ok(STANDARD.encode(encode_png(&frame)?))
    }
}

impl Tool for ScreenshotTool {
    fn name(&self) -> &str {
        "screenshot"
    }

    fn description(&self) -> &str {
        "Capture a PNG screenshot of a source, or of the program output when no source is named."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "minLength": 1, "description": "Source name; omit for the program output." },
                "width": { "type": "integer", "minimum": 1, "description": "Image width in pixels." },
                "height": { "type": "integer", "minimum": 1, "description": "Image height in pixels." },
            },
            "additionalProperties": false,
        })
    }

    fn call(&self, arguments: &Value) -> Result<String, String> {
        self.base64_png(arguments)
    }

    fn call_content(&self, arguments: &Value) -> Result<Vec<Content>, String> {
        Ok(vec![Content::Image {
            data: self.base64_png(arguments)?,
            mime_type: "image/png".to_owned(),
        }])
    }
}

/// Registers the `screenshot` tool on `registry`, backed by `backend`.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn CaptureBackend>) {
    registry.register(Box::new(ScreenshotTool {
        backend: Rc::clone(backend),
    }));
}
