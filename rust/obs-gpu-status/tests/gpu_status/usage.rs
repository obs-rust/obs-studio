//! Tier 1 tests for obs-rust/obs-studio#1025: the `GPU: x%` label. An
//! injected sampler (fake sources) feeds the report, and the readout gives
//! the text and the report the color for the hardware, software and n/a
//! cases. New code with no C original, so Tiers 2 and 3 do not apply.

use obs_gpu_status::report::Report;
use obs_gpu_status::usage::{Sampler, Source, UsageSample, UsageSource};
use obs_gpu_status::{FrameStats, GpuDevice, RenderAdapter, State, Utilization, Vendor};

const RTX: &str = "NVIDIA GeForce RTX 3060/PCIe/SSE2";
const LLVMPIPE: &str = "Mesa llvmpipe (LLVM 21.1.8, 256 bits)";

/// A source that reads `whole` for `pci` only.
struct Fake {
    source: Source,
    pci: &'static str,
    whole: Utilization,
}

impl Fake {
    fn new(source: Source, pci: &'static str, whole: Utilization) -> Box<Self> {
        Box::new(Self { source, pci, whole })
    }
}

impl UsageSource for Fake {
    fn source(&self) -> Source {
        self.source
    }

    fn whole(&mut self, adapter: &RenderAdapter) -> Utilization {
        match adapter {
            RenderAdapter::Pci(a) if a == self.pci => self.whole,
            _ => Utilization::Unavailable,
        }
    }
}

const NV: &str = "0000:2b:00.0";
const AMD: &str = "0000:0c:00.0";

fn pci(a: &str) -> RenderAdapter {
    RenderAdapter::Pci(a.to_owned())
}

fn rtx3060() -> GpuDevice {
    GpuDevice {
        vendor: Vendor::Nvidia,
        device_id: Some("2504".into()),
        name: Some("NVIDIA GeForce RTX 3060".into()),
        render_node: true,
    }
}

fn sample(percent: u8) -> Option<UsageSample> {
    Some(UsageSample {
        percent,
        source: Source::Nvml,
    })
}

#[test]
fn first_source_with_a_reading_wins() {
    let mut s = Sampler::new(vec![
        Fake::new(Source::Nvml, NV, Utilization::Percent(37)),
        Fake::new(Source::Amdgpu, AMD, Utilization::Percent(40)),
    ]);
    let amd = s.sample(&pci(AMD)).unwrap();
    assert_eq!((amd.percent, amd.source), (40, Source::Amdgpu));
    let nv = s.sample(&pci(NV)).unwrap();
    assert_eq!((nv.percent, nv.source), (37, Source::Nvml));
}

#[test]
fn earlier_source_wins_when_both_read() {
    let mut s = Sampler::new(vec![
        Fake::new(Source::Nvml, NV, Utilization::Percent(37)),
        Fake::new(Source::Amdgpu, NV, Utilization::Percent(40)),
    ]);
    assert_eq!(s.sample(&pci(NV)), sample(37));
}

#[test]
fn no_reading_is_none() {
    let mut s = Sampler::new(vec![Fake::new(Source::Nvml, NV, Utilization::Percent(37))]);
    assert_eq!(s.sample(&pci(AMD)), None);
    assert_eq!(s.sample(&RenderAdapter::Unknown), None);
    assert_eq!(Sampler::new(vec![]).sample(&pci(NV)), None);
}

#[test]
fn hardware_with_a_sample_shows_the_percent_only() {
    let r = Report::new(Some(RTX), vec![rtx3060()], None).with_usage(sample(37));
    assert_eq!(r.state, State::Green);
    let out = r.readout();
    assert_eq!(out.label.english, "GPU: 37%");
    assert_eq!(out.label.key, "Basic.StatusBar.Gpu.Usage");
    assert_eq!(out.label.args, ["37"]);
    // No "OBS's share" line: NVML's per-process samples are not a share
    // of the headline (usage.rs).
    assert!(out.details.is_empty(), "{:?}", out.details);
}

#[test]
fn hardware_without_a_source_is_n_a_and_green() {
    let r = Report::new(Some(RTX), vec![rtx3060()], None).with_usage(None);
    assert_eq!(r.state, State::Green);
    let out = r.readout();
    assert_eq!(out.label.english, "GPU: n/a");
    assert_eq!(out.label.key, "Basic.StatusBar.Gpu.Unavailable");
    assert_eq!(out.details.len(), 1);
    assert_eq!(out.details[0].key, "Basic.StatusBar.Gpu.NoSource");
}

#[test]
fn software_rendering_never_shows_a_percent() {
    // An idle RTX at 0 % while OBS renders on llvmpipe (#1004): red.
    let r = Report::new(Some(LLVMPIPE), vec![rtx3060()], None).with_usage(sample(0));
    assert_eq!(r.state, State::Red);
    let out = r.readout();
    assert_eq!(out.label.english, "CPU rendering");
    assert_eq!(out.label.key, "Basic.StatusBar.Gpu.Software");
    assert!(out.details.is_empty(), "{:?}", out.details);
    // No GPU at all: the same text, yellow (#1005).
    let r = Report::new(Some(LLVMPIPE), vec![], None);
    assert_eq!(r.state, State::Yellow);
    assert_eq!(r.readout().label.english, "CPU rendering");
}

#[test]
fn init_failure_is_n_a_and_red() {
    let r = Report::new(None, vec![rtx3060()], None);
    assert_eq!(r.state, State::Red);
    let out = r.readout();
    assert_eq!(out.label.english, "GPU: n/a");
    // The summary already says why; no "no source" line.
    assert!(out.details.is_empty(), "{:?}", out.details);
}

#[test]
fn over_budget_keeps_the_percent_and_is_red() {
    let slow = FrameStats {
        median_frame_ns: 95_000_000,
        interval_ns: 33_333_333,
    };
    let r = Report::new(Some(RTX), vec![rtx3060()], Some(slow)).with_usage(sample(99));
    assert_eq!(r.state, State::Red);
    assert_eq!(r.readout().label.english, "GPU: 99%");
}

#[test]
fn injected_sampler_drives_text_and_color() {
    let mut s = Sampler::new(vec![Fake::new(Source::Nvml, NV, Utilization::Percent(23))]);
    let usage = s.sample(&pci(NV));
    let r = Report::new(Some(RTX), vec![rtx3060()], None).with_usage(usage);
    assert_eq!(
        (r.state, r.readout().label.english.as_str()),
        (State::Green, "GPU: 23%")
    );
    let r = Report::new(Some(LLVMPIPE), vec![rtx3060()], None).with_usage(usage);
    assert_eq!(
        (r.state, r.readout().label.english.as_str()),
        (State::Red, "CPU rendering")
    );
    let usage = s.sample(&RenderAdapter::Unknown);
    let r = Report::new(Some(RTX), vec![rtx3060()], None).with_usage(usage);
    assert_eq!(
        (r.state, r.readout().label.english.as_str()),
        (State::Green, "GPU: n/a")
    );
}

#[test]
fn json_carries_usage_and_readout() {
    let j = Report::new(Some(RTX), vec![rtx3060()], None)
        .with_usage(sample(37))
        .to_json();
    assert!(
        j.contains("\"usage\":{\"percent\":37,\"source\":\"nvml\"}"),
        "{j}"
    );
    assert!(
        j.contains(
            "\"readout\":{\"text\":\"GPU: 37%\",\"key\":\"Basic.StatusBar.Gpu.Usage\",\"args\":[\"37\"],\"details\":[]}"
        ),
        "{j}"
    );
    assert!(j.ends_with("]}"), "{j}");
}

#[test]
fn json_without_usage_is_null() {
    let j = Report::new(Some(RTX), vec![rtx3060()], None).to_json();
    assert!(j.contains("\"usage\":null"), "{j}");
    assert!(
        j.contains(
            "\"readout\":{\"text\":\"GPU: n/a\",\"key\":\"Basic.StatusBar.Gpu.Unavailable\",\"args\":[],\
             \"details\":[{\"text\":\"No utilization source for this GPU.\",\"key\":\"Basic.StatusBar.Gpu.NoSource\",\"args\":[]}]}"
        ),
        "{j}"
    );
}

#[test]
fn source_names() {
    assert_eq!(Source::Nvml.as_str(), "nvml");
    assert_eq!(Source::Amdgpu.as_str(), "amdgpu");
}
