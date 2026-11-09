//! Native implementations of typed control domains.
use super::{ObsBackend, Output, Source};
use crate::server::ToolError;
use crate::tools::EmptyInput;
use crate::tools::control::{
    ControlBackend, MediaInput, MediaStatus, PipelineDump, StageStats, Stages, StartInput,
    StatsSnapshot, SyncStats, VcamStatus, VmicStatus,
};
use crate::tools::scenes::SceneBackend;
use crate::tools::sources::SourceBackend;

const CAMERA: &str = "virtualcam_output";
impl ObsBackend {
    fn camera(&self, create: bool) -> Result<Option<Output>, ToolError> {
        if let Some(output) = Output::by_name(CAMERA) {
            return Ok(Some(output));
        }
        if !create {
            return Ok(None);
        }
        let output =
            Output::create(CAMERA, CAMERA, &serde_json::json!({})).map_err(ToolError::not_ready)?;
        self.camera.replace(Some(output));
        Ok(Output::by_name(CAMERA))
    }
    fn raw_stats(&self) -> Result<StatsSnapshot, ToolError> {
        let render = crate::ffi::render_stats();
        let mut stages = Stages {
            render: StageStats {
                live: crate::ffi::has_video(),
                frames_total: Some(render.render_total_frames),
                frames_dropped: Some(render.render_skipped_frames),
                ..StageStats::default()
            },
            ..Stages::default()
        };
        if let Some(output) = self.camera(false)? {
            if let Some((video, audio)) = output.device_stats() {
                stages.vcam = video;
                stages.vmic = audio;
            } else {
                stages.vcam.live = output.active();
                stages.vmic.live = output.virtual_audio().unwrap_or(false);
            }
        }
        Ok(StatsSnapshot { stages, render, end_to_end: SyncStats { av_offset_ms: None, measured_from: "unavailable: independent capture probe required; stage bounds are libobs timestamp to completed device write".into() } })
    }
    fn media_source(request: &MediaInput) -> Result<Source, ToolError> {
        if request.name.is_empty() {
            return Err(ToolError::wrong_state("media name must not be empty"));
        }
        Source::by_name(&request.name)
            .ok_or_else(|| ToolError::not_found(format!("no source named '{}'", request.name)))
    }
}
impl ControlBackend for ObsBackend {
    fn vcam_start(&self, request: StartInput) -> Result<VcamStatus, ToolError> {
        let output = self
            .camera(true)?
            .ok_or_else(|| ToolError::not_ready("Virtual Camera output unavailable"))?;
        if !output.active() {
            output.start().map_err(ToolError::processing)?;
        }
        let status = self.vcam_status(EmptyInput::default())?;
        if request.wait && !status.active {
            return Err(ToolError::not_ready("Virtual Camera has not become active"));
        }
        Ok(status)
    }
    fn vcam_stop(&self, _request: EmptyInput) -> Result<VcamStatus, ToolError> {
        if let Some(output) = self.camera(false)?
            && output.active()
        {
            output.stop();
        }
        self.vcam_status(EmptyInput::default())
    }
    fn vcam_status(&self, _request: EmptyInput) -> Result<VcamStatus, ToolError> {
        Ok(self
            .camera(false)?
            .map_or_else(VcamStatus::default, |output| VcamStatus {
                configured: true,
                active: output.active(),
                vmic_active: output.virtual_audio(),
            }))
    }
    fn vmic_status(&self, _request: EmptyInput) -> Result<VmicStatus, ToolError> {
        Ok(VmicStatus {
            active: self
                .camera(false)?
                .and_then(|output| output.virtual_audio()),
            source_name: None,
        })
    }
    fn media_restart(&self, request: MediaInput) -> Result<MediaStatus, ToolError> {
        let source = Self::media_source(&request)?;
        source.restart_media().map_err(ToolError::wrong_state)?;
        Ok(source.media())
    }
    fn media_status(&self, request: MediaInput) -> Result<MediaStatus, ToolError> {
        Ok(Self::media_source(&request)?.media())
    }
    fn stats_get(&self, _request: EmptyInput) -> Result<StatsSnapshot, ToolError> {
        let current = self.raw_stats()?;
        Ok(self.baseline.borrow().as_ref().map_or_else(
            || current.clone(),
            |baseline| current.clone().relative_to(baseline),
        ))
    }
    fn stats_reset(&self, _request: EmptyInput) -> Result<StatsSnapshot, ToolError> {
        self.baseline.replace(Some(self.raw_stats()?));
        self.stats_get(EmptyInput::default())
    }
    fn pipeline_dump(&self, _request: EmptyInput) -> Result<PipelineDump, ToolError> {
        Ok(PipelineDump {
            program: self.current(),
            sources: self
                .sources()
                .into_iter()
                .map(|source| source.name)
                .collect(),
            vcam: self.vcam_status(EmptyInput::default())?,
            stats: self.stats_get(EmptyInput::default())?,
        })
    }
}
