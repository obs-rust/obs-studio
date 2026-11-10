//! Wait observations over unchanged libobs queries.
use super::{ObsBackend, Source};
use crate::server::ToolError;
use crate::tools::EmptyInput;
use crate::tools::control::ControlBackend;
use crate::tools::outputs::{OutputBackend, OutputKind};
use crate::tools::waiting::{
    Condition, LastValue, Metric, Observation, ObserveInput, OutputName, OutputState, WaitBackend,
};
impl ObsBackend {
    /// Cancels pending waits when an in-app host shuts down.
    #[must_use]
    pub fn with_cancellation(
        mut self,
        flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        self.cancellation = Some(flag);
        self
    }
}
impl WaitBackend for ObsBackend {
    fn cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "interactive stage measurements and xrun counts are far below 2^53"
    )]
    fn observe(&self, request: &ObserveInput) -> Result<Observation, ToolError> {
        let (satisfied, value) = match &request.condition {
            Condition::OutputState { output, state } => {
                let active = match output {
                    OutputName::Record => Some(self.status(OutputKind::Record).active),
                    OutputName::Stream => Some(self.status(OutputKind::Stream).active),
                    OutputName::Vcam => Some(self.vcam_status(EmptyInput {})?.active),
                    OutputName::Vmic => self.vmic_status(EmptyInput {})?.active,
                };
                (
                    active == Some(matches!(state, OutputState::Active)),
                    LastValue::Output { active },
                )
            }
            Condition::SourceActive { name } => {
                let source = Source::by_name(name);
                let active = source.as_ref().is_some_and(Source::active);
                (
                    active,
                    LastValue::Source {
                        present: source.is_some(),
                        active,
                    },
                )
            }
            Condition::MediaState { name, state } => {
                let current = Source::by_name(name).map(|source| source.media().state);
                (current == Some(*state), LastValue::Media { state: current })
            }
            Condition::StatAtMost { metric, value } => {
                let stats = self.stats_get(EmptyInput {})?;
                let current = match metric {
                    Metric::RenderFrameTimeNs => (stats.stages.render.live
                        && stats.render.average_frame_time_ns > 0)
                        .then_some(stats.render.average_frame_time_ns as f64),
                    Metric::RenderFramesDropped => Some(stats.render.render_skipped_frames as f64),
                    Metric::VcamLatencyMaxNs => stats.stages.vcam.latency_max_ns.map(|v| v as f64),
                    Metric::VmicLatencyMaxNs => stats.stages.vmic.latency_max_ns.map(|v| v as f64),
                    Metric::VmicXruns => stats.stages.vmic.xruns.map(|v| v as f64),
                    Metric::AvOffsetAbsMs => stats.end_to_end.av_offset_ms.map(f64::abs),
                };
                (
                    crate::tools::waiting::metric_at_most(current, *value),
                    LastValue::Stat {
                        metric: *metric,
                        value: current,
                    },
                )
            }
        };
        Ok(Observation { satisfied, value })
    }
}
