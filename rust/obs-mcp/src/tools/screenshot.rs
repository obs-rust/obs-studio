//! The `screenshot` tool: captures a source or the program output as a PNG
//! image (#3288).
//!
//! The tool talks to OBS through [`CaptureBackend`], so it is tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::io::Write as _;
use std::rc::Rc;

use crate::server::{ToolAnnotations, ToolError, TypedTool};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::server::{Content, ToolRegistry};

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
    /// Typed frame capture request.
    /// # Errors
    /// Reports a missing source or unavailable graphics pipeline.
    fn capture_frame(&self, request: CaptureInput) -> Result<Frame, ToolError> {
        self.capture(request.source.as_deref(), request.width, request.height)
            .map_err(ToolError::processing)
    }

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

/// Screenshot selection and requested size.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureInput {
    /// Source to capture; omit for program output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}
/// Captured PNG and its dimensions.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CaptureOutput {
    /// PNG bytes encoded in base64.
    pub data: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Source name, or null for program output.
    pub source: Option<String>,
}
struct ScreenshotTool {
    backend: Rc<dyn CaptureBackend>,
}
impl TypedTool for ScreenshotTool {
    type Input = CaptureInput;
    type Output = CaptureOutput;
    const NAME: &'static str = "screenshot";
    const ANNOTATIONS: ToolAnnotations = ToolAnnotations::READ_ONLY;
    fn description(&self) -> &str {
        "Capture a PNG screenshot of a source, or of the program output when no source is named. Returns the image plus a text item {width, height, source} (source is null for the program output). The PNG has an alpha channel (RGBA): pixels nothing was drawn on, such as an empty scene, are fully transparent, which a viewer may show as white. A source only appears in the program output once it is an item of the program scene."
    }
    fn call(&self, input: CaptureInput) -> Result<CaptureOutput, ToolError> {
        if input.source.as_ref().is_some_and(String::is_empty) {
            return Err(ToolError::processing(
                "argument `source` must be a non-empty string",
            ));
        }
        if input.width == Some(0) || input.height == Some(0) {
            return Err(ToolError::processing(
                "width and height must be positive integers",
            ));
        }
        let frame = self.backend.capture_frame(input.clone())?;
        let data = STANDARD.encode(encode_png(&frame).map_err(ToolError::processing)?);
        Ok(CaptureOutput {
            data,
            width: frame.width,
            height: frame.height,
            source: input.source,
        })
    }
    fn content(output: &CaptureOutput) -> Result<Vec<Content>, ToolError> {
        let size = serde_json::json!({ "width": output.width, "height": output.height, "source": output.source });
        Ok(vec![
            Content::Image {
                data: output.data.clone(),
                mime_type: "image/png".to_owned(),
            },
            Content::Text(size.to_string()),
        ])
    }
}
/// Registers the typed screenshot tool.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn CaptureBackend>) {
    registry.register(Box::new(ScreenshotTool {
        backend: Rc::clone(backend),
    }));
}

/// Generated tool schema contracts.
#[must_use]
pub fn schemas() -> Vec<serde_json::Value> {
    vec![crate::server::tool_schema::<ScreenshotTool>()]
}
