//! Control API domains (#4446); no additional test executable.
use obs_mcp::tools::control::counter_delta;
#[test]
fn counters_are_relative_to_baseline_and_survive_recreation() {
    assert_eq!(counter_delta(20, 5), 15);
    assert_eq!(counter_delta(5, 5), 0);
    assert_eq!(counter_delta(3, 20), 3);
}
use obs_mcp::server::{Server, ToolError, ToolRegistry};
use obs_mcp::tools::{
    EmptyInput,
    control::{
        ControlBackend, MediaInput, MediaState, MediaStatus, PipelineDump, StartInput,
        StatsSnapshot, VcamStatus, VmicStatus, register,
    },
};
use serde_json::{Value, json};
use std::cell::Cell;
use std::rc::Rc;
#[derive(Default)]
struct Fake {
    active: Cell<bool>,
}
impl ControlBackend for Fake {
    fn vcam_start(&self, _: StartInput) -> Result<VcamStatus, ToolError> {
        self.active.set(true);
        self.vcam_status(EmptyInput {})
    }
    fn vcam_stop(&self, _: EmptyInput) -> Result<VcamStatus, ToolError> {
        self.active.set(false);
        self.vcam_status(EmptyInput {})
    }
    fn vcam_status(&self, _: EmptyInput) -> Result<VcamStatus, ToolError> {
        Ok(VcamStatus {
            configured: true,
            active: self.active.get(),
            vmic_active: Some(self.active.get()),
        })
    }
    fn vmic_status(&self, _: EmptyInput) -> Result<VmicStatus, ToolError> {
        Ok(VmicStatus {
            active: Some(self.active.get()),
            source_name: None,
        })
    }
    fn media_restart(&self, request: MediaInput) -> Result<MediaStatus, ToolError> {
        self.media_status(request)
    }
    fn media_status(&self, request: MediaInput) -> Result<MediaStatus, ToolError> {
        if request.name != "clip" {
            return Err(ToolError::not_found("missing media"));
        }
        Ok(MediaStatus {
            name: request.name,
            state: MediaState::Playing,
            duration_ms: Some(1000),
            position_ms: Some(0),
        })
    }
    fn stats_get(&self, _: EmptyInput) -> Result<StatsSnapshot, ToolError> {
        Ok(StatsSnapshot::default())
    }
    fn stats_reset(&self, _: EmptyInput) -> Result<StatsSnapshot, ToolError> {
        self.stats_get(EmptyInput {})
    }
    fn pipeline_dump(&self, _: EmptyInput) -> Result<PipelineDump, ToolError> {
        Ok(PipelineDump::default())
    }
}
fn rpc(server: &mut Server, name: &str, arguments: Value) -> Value {
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":arguments}}).to_string()+"\n";
    let mut output = Vec::new();
    server.serve(request.as_bytes(), &mut output).unwrap();
    serde_json::from_slice(&output).unwrap()
}
#[test]
fn device_media_tools_have_typed_results_and_strict_arguments() {
    let mut tools = ToolRegistry::new();
    let backend: Rc<dyn ControlBackend> = Rc::new(Fake::default());
    register(&mut tools, &backend);
    assert_eq!(tools.list()["tools"].as_array().unwrap().len(), 9);
    let mut server = Server::with_tools(tools);
    for name in ["vcam_start", "vcam_start", "vcam_stop", "vcam_stop"] {
        let response = rpc(&mut server, name, json!({}));
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(
            response["result"]["structuredContent"]["active"],
            name.ends_with("start")
        );
    }
    assert_eq!(
        rpc(&mut server, "media_restart", json!({"name":"clip"}))["result"]["structuredContent"]["state"],
        "playing"
    );
    assert_eq!(
        rpc(&mut server, "media_status", json!({"name":"missing"}))["result"]["structuredContent"]
            ["code"],
        600
    );
    assert_eq!(
        rpc(&mut server, "vcam_start", json!({"typo":true}))["error"]["code"],
        -32602
    );
    let stats = rpc(&mut server, "stats_get", json!({}));
    assert!(stats["result"]["structuredContent"]["end_to_end"]["av_offset_ms"].is_null());
    assert!(stats["result"]["structuredContent"]["stages"]["vmic"]["xruns"].is_null());
}
#[test]
fn reset_changes_counters_but_preserves_gauges_and_unknown_values() {
    let mut now = StatsSnapshot::default();
    now.stages.vcam.frames_total = Some(30);
    now.stages.vcam.latency_max_ns = Some(800);
    let mut baseline = StatsSnapshot::default();
    baseline.stages.vcam.frames_total = Some(10);
    let result = now.relative_to(&baseline);
    assert_eq!(result.stages.vcam.frames_total, Some(20));
    assert_eq!(result.stages.vcam.latency_max_ns, Some(800));
    assert_eq!(result.stages.vmic.xruns, None);
}
