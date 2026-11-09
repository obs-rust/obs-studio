//! Bounded typed waits, with a deterministic clock seam (#4447).
use super::control::MediaState;
use crate::server::ToolError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Default auto-wait deadline.
pub const DEFAULT_TIMEOUT_MS: u64 = 5000;
/// Maximum request deadline, keeping control requests bounded.
pub const MAX_TIMEOUT_MS: u64 = 300_000;
/// Default timeout used by serde and fluent clients.
#[must_use]
pub fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}
/// An output controlled by OBS.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputName {
    /// Recording.
    Record,
    /// Streaming.
    Stream,
    /// Virtual Camera.
    Vcam,
    /// Paired virtual microphone.
    Vmic,
}
/// Desired output state.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputState {
    /// Running.
    Active,
    /// Stopped.
    Inactive,
}
/// A supported numeric metric, never an expression.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Average render duration.
    RenderFrameTimeNs,
    /// Render drops since baseline.
    RenderFramesDropped,
    /// Camera maximum measured stage latency.
    VcamLatencyMaxNs,
    /// Microphone maximum measured stage latency.
    VmicLatencyMaxNs,
    /// Microphone xruns, if measured.
    VmicXruns,
    /// Absolute independently measured sync offset.
    AvOffsetAbsMs,
}
/// Explicit condition language, rejecting unknown fields.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// Wait for an output to reach a state.
    OutputState {
        /// Output selector.
        output: OutputName,
        /// Desired state.
        state: OutputState,
    },
    /// Wait until a source exists and is active.
    SourceActive {
        /// Source name.
        name: String,
    },
    /// Wait for a named media source's state.
    MediaState {
        /// Source name.
        name: String,
        /// Desired media state.
        state: MediaState,
    },
    /// Wait until a measured metric is at most the threshold.
    StatAtMost {
        /// Metric selector.
        metric: Metric,
        /// Maximum acceptable value, in the metric's units.
        value: f64,
    },
}
/// One wait request.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitInput {
    /// Condition to observe before waiting.
    pub condition: Condition,
    /// Deadline in milliseconds; zero still observes once.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}
/// Typed final observation, including unavailable measurements.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LastValue {
    /// Output observation.
    Output {
        /// Null if the output state cannot be queried.
        active: Option<bool>,
    },
    /// Source observation.
    Source {
        /// Whether the source exists.
        present: bool,
        /// Whether it is active.
        active: bool,
    },
    /// Media observation.
    Media {
        /// Null if the named source is absent.
        state: Option<MediaState>,
    },
    /// Numeric observation.
    Stat {
        /// Metric selector.
        metric: Metric,
        /// Null if unavailable; never satisfies a threshold.
        value: Option<f64>,
    },
}
/// Result of one observation.
#[derive(Clone, Debug)]
pub struct Observation {
    /// Whether the condition is met.
    pub satisfied: bool,
    /// Typed current value.
    pub value: LastValue,
}
/// Final wait result.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitOutput {
    /// Condition met before the deadline.
    pub satisfied: bool,
    /// Monotonic elapsed time.
    pub elapsed_ms: u64,
    /// Last observed value, including on timeout.
    pub last_value: LastValue,
}
/// Backend request for a single observation.
#[derive(Clone, Debug)]
pub struct ObserveInput {
    /// Condition to evaluate.
    pub condition: Condition,
}
/// Backend seam, reusable for automatic waits.
pub trait WaitBackend {
    /// Observe without waiting.
    /// # Errors
    /// Reports an unavailable subsystem or failed query.
    fn observe(&self, request: &ObserveInput) -> Result<Observation, ToolError>;
    /// Shutdown cancellation is checked before accessing libobs.
    fn cancelled(&self) -> bool {
        false
    }
}
/// Monotonic clock and bounded parking; tests do not sleep.
pub trait WaitClock {
    /// Monotonic time in milliseconds.
    fn now_ms(&self) -> u64;
    /// Wait for at most the supplied interval.
    fn wait_ms(&self, milliseconds: u64);
}
/// Evaluate first, then wait within the deadline.
/// # Errors
/// Reports backend failure, cancellation or an excessive timeout.
pub fn run_wait<B: WaitBackend + ?Sized, C: WaitClock>(
    backend: &B,
    request: WaitInput,
    clock: &C,
) -> Result<WaitOutput, ToolError> {
    validate_timeout(request.timeout_ms)?;
    if let Condition::StatAtMost { value, .. } = &request.condition
        && !value.is_finite()
    {
        return Err(ToolError::wrong_state("metric threshold must be finite"));
    }
    let start = clock.now_ms();
    let observation = ObserveInput {
        condition: request.condition,
    };
    loop {
        if backend.cancelled() {
            return Err(ToolError::not_ready("OBS control is shutting down"));
        }
        let current = backend.observe(&observation)?;
        let elapsed_ms = clock.now_ms().saturating_sub(start);
        if current.satisfied || elapsed_ms >= request.timeout_ms {
            return Ok(WaitOutput {
                satisfied: current.satisfied,
                elapsed_ms,
                last_value: current.value,
            });
        }
        clock.wait_ms((request.timeout_ms - elapsed_ms).min(10));
    }
}

/// Reject excessive deadlines before an operation changes OBS state.
/// # Errors
/// Returns a domain error when the timeout exceeds the bound.
pub fn validate_timeout(timeout_ms: u64) -> Result<(), ToolError> {
    if timeout_ms > MAX_TIMEOUT_MS {
        Err(ToolError::wrong_state("timeout_ms must be at most 300000"))
    } else {
        Ok(())
    }
}
/// Production monotonic clock. Parking is bounded and cancellation is rechecked.
pub struct SystemClock(std::time::Instant);
impl Default for SystemClock {
    fn default() -> Self {
        Self(std::time::Instant::now())
    }
}
impl WaitClock for SystemClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    fn wait_ms(&self, milliseconds: u64) {
        std::thread::park_timeout(std::time::Duration::from_millis(milliseconds));
    }
}
use crate::server::ToolRegistry;
use std::rc::Rc;
typed_tool!(
    WaitTool,
    "wait_for",
    WaitBackend,
    WaitInput,
    WaitOutput,
    READ_ONLY,
    "Observe a typed condition immediately, then wait within timeout_ms. Returns the final typed value on timeout; unavailable metrics never satisfy a threshold.",
    |backend, input| { run_wait(backend.as_ref(), input, &SystemClock::default()) }
);
/// Registers the wait domain.
pub fn register(registry: &mut ToolRegistry, backend: &Rc<dyn WaitBackend>) {
    registry.register(Box::new(WaitTool {
        backend: Rc::clone(backend),
    }));
}
/// Generated tool contract.
#[must_use]
pub fn schemas() -> Vec<serde_json::Value> {
    vec![crate::server::tool_schema::<WaitTool>()]
}

/// Unknown or non-finite metrics never satisfy a numeric condition.
#[must_use]
pub fn metric_at_most(value: Option<f64>, threshold: f64) -> bool {
    threshold.is_finite() && value.is_some_and(|value| value.is_finite() && value <= threshold)
}
