//! The `screenshot` tool: captures a source or the program output as a PNG
//! image (#3288).
//!
//! The tool talks to OBS through [`CaptureBackend`], so it is tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::rc::Rc;

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
    let _ = frame;
    todo!("RED: PNG encoder (#3288)")
}

/// The `screenshot` tool bound to a backend.
struct ScreenshotTool {
    backend: Rc<dyn CaptureBackend>,
}

impl ScreenshotTool {
    /// Captures and returns the base64 PNG.
    fn base64_png(&self, arguments: &Value) -> Result<String, String> {
        let _ = (&self.backend, arguments);
        todo!("RED: screenshot (#3288)")
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
