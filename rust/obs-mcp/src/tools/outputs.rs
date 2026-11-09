//! Output tools: `record_start`, `record_stop`, `record_status`,
//! `stream_start`, `stream_stop` and `stream_status` (#3287).
//!
//! The tools talk to OBS through [`OutputBackend`], so they are tested with a
//! fake. Binding it to libobs is a follow-up (#3283).

use std::rc::Rc;

use super::EmptyInput;
use crate::server::ToolError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::server::ToolRegistry;

/// Which output a call is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// The recording output.
    Record,
    /// The streaming output.
    Stream,
}

/// The state of an output as reported by [`OutputBackend::status`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutputStatus {
    /// Whether the output is running.
    pub active: bool,
    /// Milliseconds since the output started, where known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub duration_ms: Option<u64>,
    /// Bytes written or sent so far, where known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bytes: Option<u64>,
}

/// The output operations the tools need from OBS.
///
/// Methods take `&self`: libobs owns the state, so implementations use their
/// own interior mutability.
pub trait OutputBackend {
    /// Typed idempotent start.
    /// # Errors
    /// Reports a processing failure.
    fn start_output(&self, request: OutputRequest) -> Result<ActiveOutput, ToolError> {
        if !self.status(request.kind).active {
            self.start(request.kind).map_err(ToolError::processing)?;
        }
        Ok(ActiveOutput {
            active: self.status(request.kind).active,
        })
    }
    /// Typed idempotent stop.
    /// # Errors
    /// Reports a processing failure.
    fn stop_output(&self, request: OutputRequest) -> Result<ActiveOutput, ToolError> {
        if self.status(request.kind).active {
            self.stop(request.kind).map_err(ToolError::processing)?;
        }
        Ok(ActiveOutput {
            active: self.status(request.kind).active,
        })
    }
    /// Typed status request.
    fn output_status(&self, request: OutputRequest) -> OutputStatus {
        self.status(request.kind)
    }

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

/// State returned by a start or stop operation.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ActiveOutput {
    /// Whether the output is running.
    pub active: bool,
}
typed_tool!(
    RecordStart,
    "record_start",
    OutputBackend,
    EmptyInput,
    ActiveOutput,
    IDEMPOTENT,
    "Start record output.",
    |backend, _input| {
        backend.start_output(OutputRequest {
            kind: OutputKind::Record,
        })
    }
);
typed_tool!(
    RecordStop,
    "record_stop",
    OutputBackend,
    EmptyInput,
    ActiveOutput,
    IDEMPOTENT,
    "Stop record output.",
    |backend, _input| {
        backend.stop_output(OutputRequest {
            kind: OutputKind::Record,
        })
    }
);
typed_tool!(
    RecordStatus,
    "record_status",
    OutputBackend,
    EmptyInput,
    OutputStatus,
    READ_ONLY,
    "Status record output.",
    |backend, _input| {
        Ok(backend.output_status(OutputRequest {
            kind: OutputKind::Record,
        }))
    }
);
typed_tool!(
    StreamStart,
    "stream_start",
    OutputBackend,
    EmptyInput,
    ActiveOutput,
    IDEMPOTENT,
    "Start stream output.",
    |backend, _input| {
        backend.start_output(OutputRequest {
            kind: OutputKind::Stream,
        })
    }
);
typed_tool!(
    StreamStop,
    "stream_stop",
    OutputBackend,
    EmptyInput,
    ActiveOutput,
    IDEMPOTENT,
    "Stop stream output.",
    |backend, _input| {
        backend.stop_output(OutputRequest {
            kind: OutputKind::Stream,
        })
    }
);
typed_tool!(
    StreamStatus,
    "stream_status",
    OutputBackend,
    EmptyInput,
    OutputStatus,
    READ_ONLY,
    "Status stream output.",
    |backend, _input| {
        Ok(backend.output_status(OutputRequest {
            kind: OutputKind::Stream,
        }))
    }
);
/// Registers the six typed output tools.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn OutputBackend>) {
    registry.register(Box::new(RecordStart {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(RecordStop {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(RecordStatus {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(StreamStart {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(StreamStop {
        backend: Rc::clone(backend),
    }));
    registry.register(Box::new(StreamStatus {
        backend: Rc::clone(backend),
    }));
}

/// Generated tool schema contracts.
#[must_use]
pub fn schemas() -> Vec<serde_json::Value> {
    vec![
        crate::server::tool_schema::<RecordStart>(),
        crate::server::tool_schema::<RecordStop>(),
        crate::server::tool_schema::<RecordStatus>(),
        crate::server::tool_schema::<StreamStart>(),
        crate::server::tool_schema::<StreamStop>(),
        crate::server::tool_schema::<StreamStatus>(),
    ]
}

/// Selects an output for backend operations.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputRequest {
    /// Output to operate on.
    pub kind: OutputKind,
}
