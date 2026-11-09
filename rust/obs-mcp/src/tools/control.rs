//! Device, media and measured pipeline control (#4446).
use std::rc::Rc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::server::{ToolError, ToolRegistry};
use super::EmptyInput;

/// Bounded start options, extended by the wait domain.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartInput {
    /// Wait until the output reports active.
    #[serde(default)]
    pub wait: bool,
}
/// Virtual Camera and its paired audio source.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VcamStatus {
    /// Whether the registered output exists.
    pub configured: bool,
    /// Video output is active.
    pub active: bool,
    /// Paired virtual microphone is active, when the plugin supports the query.
    pub vmic_active: Option<bool>,
}
/// Virtual microphone state.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct VmicStatus {
    /// Active according to virtualcam_audio; null means unsupported.
    pub active: Option<bool>,
    /// Runtime source name, when known.
    pub source_name: Option<String>,
}
/// A media source selection.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MediaInput {
    /// Name of an existing media source.
    pub name: String,
}
/// libobs media state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaState {
    /// Not a controllable media source.
    #[default]
    None,
    /// Playing.
    Playing,
    /// Opening.
    Opening,
    /// Buffering.
    Buffering,
    /// Paused.
    Paused,
    /// Stopped.
    Stopped,
    /// Playback reached the end.
    Ended,
    /// Playback failed.
    Error,
}
/// Media playback snapshot, in milliseconds.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MediaStatus {
    /// Source name.
    pub name: String,
    /// Current media state.
    pub state: MediaState,
    /// Total duration, if known.
    pub duration_ms: Option<i64>,
    /// Playback position, if known.
    pub position_ms: Option<i64>,
}
/// One live pipeline stage; null values are unmeasured, never synthetic zeroes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StageStats {
    /// Currently running.
    pub live: bool,
    /// Observed minimum stage residence time, if measured.
    pub latency_min_ns: Option<u64>,
    /// Observed maximum stage residence time, if measured.
    pub latency_max_ns: Option<u64>,
    /// Frames handled since the counter baseline.
    pub frames_total: Option<u64>,
    /// Dropped frames since baseline, if measured.
    pub frames_dropped: Option<u64>,
    /// Underruns/overruns since baseline, if measured.
    pub xruns: Option<u64>,
}
/// Render and device stages.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Stages {
    /// OBS render stage.
    pub render: StageStats,
    /// Virtual Camera write stage.
    pub vcam: StageStats,
    /// Virtual microphone stage.
    pub vmic: StageStats,
}
/// End-to-end measurement provenance.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct SyncStats {
    /// Measured audio minus video offset; null until independently measured.
    pub av_offset_ms: Option<f64>,
    /// Measurement source; queue estimates are not end-to-end measurements.
    pub measured_from: String,
}
/// GetStats-style render counters.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct RenderStats {
    /// Actual render FPS.
    pub active_fps: f64,
    /// Average render duration, not end-to-end latency.
    pub average_frame_time_ns: u64,
    /// Frames rendered since baseline.
    pub render_total_frames: u64,
    /// Frames lagged since baseline.
    pub render_skipped_frames: u64,
}
/// Typed pipeline statistics.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StatsSnapshot {
    /// Per-stage measured counters and bounds.
    pub stages: Stages,
    /// End-to-end sync, with explicit provenance.
    pub end_to_end: SyncStats,
    /// Render counters.
    pub render: RenderStats,
}
/// Graph snapshot useful for diagnosing missing sources and outputs.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PipelineDump {
    /// Program source name.
    pub program: Option<String>,
    /// Source names in stable order.
    pub sources: Vec<String>,
    /// Virtual output state.
    pub vcam: VcamStatus,
    /// Current pipeline measurements.
    pub stats: StatsSnapshot,
}
/// Backend operations take typed requests and return typed responses.
pub trait ControlBackend {
    /// Starts or observes an already-running camera.
    /// # Errors
    /// Reports missing plugins, unavailable devices or processing failures.
    fn vcam_start(&self, request: StartInput) -> Result<VcamStatus, ToolError>;
    /// Stops or observes an already-stopped camera.
    /// # Errors
    /// Reports unavailable output configuration.
    fn vcam_stop(&self, request: EmptyInput) -> Result<VcamStatus, ToolError>;
    /// Camera status.
    /// # Errors
    /// Reports query failures.
    fn vcam_status(&self, request: EmptyInput) -> Result<VcamStatus, ToolError>;
    /// Microphone status.
    /// # Errors
    /// Reports query failures.
    fn vmic_status(&self, request: EmptyInput) -> Result<VmicStatus, ToolError>;
    /// Restarts a media source.
    /// # Errors
    /// Reports missing or non-media sources.
    fn media_restart(&self, request: MediaInput) -> Result<MediaStatus, ToolError>;
    /// Media status.
    /// # Errors
    /// Reports missing sources.
    fn media_status(&self, request: MediaInput) -> Result<MediaStatus, ToolError>;
    /// Statistics since the current baseline.
    /// # Errors
    /// Reports query failures.
    fn stats_get(&self, request: EmptyInput) -> Result<StatsSnapshot, ToolError>;
    /// Sets a new baseline and returns the resulting snapshot.
    /// # Errors
    /// Reports query failures.
    fn stats_reset(&self, request: EmptyInput) -> Result<StatsSnapshot, ToolError>;
    /// Graph and measurements.
    /// # Errors
    /// Reports query failures.
    fn pipeline_dump(&self, request: EmptyInput) -> Result<PipelineDump, ToolError>;
}
/// Difference from a baseline; a recreated counter begins a new generation.
#[must_use]
pub fn counter_delta(current: u64, baseline: u64) -> u64 { todo!("counter baseline") }
