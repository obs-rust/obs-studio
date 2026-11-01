//! Output tools: `record_start`, `record_stop`, `record_status`,
//! `stream_start`, `stream_stop` and `stream_status` (#3287).
//!
//! The tools talk to OBS through [`OutputBackend`], so they are tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::rc::Rc;

use serde_json::{Map, Value, json};

use crate::server::{Tool, ToolRegistry};

/// Which output a call is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKind {
    /// The recording output.
    Record,
    /// The streaming output.
    Stream,
}

/// The state of an output as reported by [`OutputBackend::status`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputStatus {
    /// Whether the output is running.
    pub active: bool,
    /// Milliseconds since the output started, where known.
    pub duration_ms: Option<u64>,
    /// Bytes written or sent so far, where known.
    pub bytes: Option<u64>,
}

/// The output operations the tools need from OBS.
///
/// Methods take `&self`: libobs owns the state, so implementations use their
/// own interior mutability.
pub trait OutputBackend {
    /// Starts the output of `kind`.
    ///
    /// # Errors
    ///
    /// Returns a message when it is already running or cannot start.
    fn start(&self, kind: OutputKind) -> Result<(), String>;
    /// Stops the output of `kind`.
    ///
    /// # Errors
    ///
    /// Returns a message when it is not running.
    fn stop(&self, kind: OutputKind) -> Result<(), String>;
    /// The current state of the output of `kind`.
    fn status(&self, kind: OutputKind) -> OutputStatus;
}

/// Which operation an [`OutputTool`] performs.
#[derive(Clone, Copy)]
enum Action {
    Start,
    Stop,
    Status,
}

/// One output tool bound to a backend.
struct OutputTool {
    kind: OutputKind,
    action: Action,
    backend: Rc<dyn OutputBackend>,
}

impl Tool for OutputTool {
    fn name(&self) -> &str {
        match (self.kind, self.action) {
            (OutputKind::Record, Action::Start) => "record_start",
            (OutputKind::Record, Action::Stop) => "record_stop",
            (OutputKind::Record, Action::Status) => "record_status",
            (OutputKind::Stream, Action::Start) => "stream_start",
            (OutputKind::Stream, Action::Stop) => "stream_stop",
            (OutputKind::Stream, Action::Status) => "stream_status",
        }
    }

    fn description(&self) -> &str {
        match (self.kind, self.action) {
            (OutputKind::Record, Action::Start) => "Start recording.",
            (OutputKind::Record, Action::Stop) => "Stop recording.",
            (OutputKind::Record, Action::Status) => {
                "Report whether recording is active, with duration_ms and bytes where known."
            }
            (OutputKind::Stream, Action::Start) => "Start streaming.",
            (OutputKind::Stream, Action::Stop) => "Stop streaming.",
            (OutputKind::Stream, Action::Status) => {
                "Report whether streaming is active, with duration_ms and bytes where known."
            }
        }
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
        })
    }

    fn call(&self, _arguments: &Value) -> Result<String, String> {
        let result = match self.action {
            Action::Start => {
                self.backend.start(self.kind)?;
                json!({ "active": true })
            }
            Action::Stop => {
                self.backend.stop(self.kind)?;
                json!({ "active": false })
            }
            Action::Status => {
                let status = self.backend.status(self.kind);
                let mut out = Map::new();
                out.insert("active".into(), status.active.into());
                if let Some(ms) = status.duration_ms {
                    out.insert("duration_ms".into(), ms.into());
                }
                if let Some(bytes) = status.bytes {
                    out.insert("bytes".into(), bytes.into());
                }
                Value::Object(out)
            }
        };
        Ok(result.to_string())
    }
}

/// Registers the six output tools on `registry`, backed by `backend`.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn OutputBackend>) {
    for kind in [OutputKind::Record, OutputKind::Stream] {
        for action in [Action::Start, Action::Stop, Action::Status] {
            registry.register(Box::new(OutputTool {
                kind,
                action,
                backend: Rc::clone(backend),
            }));
        }
    }
}
