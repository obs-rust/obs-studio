//! What the status light and its dialog show, and what obs-websocket's
//! `GetStats` returns as `gpuAcceleration` (obs-rust/obs-studio#846).

use std::fmt::Write as _;

use crate::usage::{Readout, Text, UsageSample};
use crate::{FrameStats, GpuDevice, State, Vendor, classify};

/// The classified state plus the plain-words text the dialog shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub state: State,
    /// The active adapter, `None` when graphics initialization failed.
    pub adapter: Option<String>,
    pub devices: Vec<GpuDevice>,
    /// One or two sentences: what is happening and what it means.
    pub summary: String,
    /// Likely causes to check, empty when there is nothing to fix.
    pub causes: Vec<String>,
    /// OBS log lines that explain the state (#944).
    pub log_lines: Vec<String>,
    /// Locale key of `summary` (frontend/data/locale), filled with
    /// `summary_args` as %1, %2, ... (#945).
    pub summary_key: &'static str,
    pub summary_args: Vec<String>,
    /// Locale keys of `causes`, in order.
    pub cause_keys: Vec<&'static str>,
    /// The render adapter's utilization (#1025), `None` without a source.
    pub usage: Option<UsageSample>,
    /// The last reading is too old to show ([`crate::usage::current`]).
    pub usage_stale: bool,
}

/// Locale keys of [`SOFTWARE_CAUSES`], in the same order (#945).
const SOFTWARE_CAUSE_KEYS: [&str; 3] = [
    "Basic.StatusBar.GpuStatus.Cause.Sandbox",
    "Basic.StatusBar.GpuStatus.Cause.Driver",
    "Basic.StatusBar.GpuStatus.Cause.RenderNode",
];

const SOFTWARE_CAUSES: [&str; 3] = [
    "OBS runs in a sandbox or container (Flatpak, AppImage, a Nix FHS wrapper) \
     that is missing the vendor's EGL/GLX libraries or the egl-wayland \
     external platform config (__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS).",
    "The GPU driver is missing, or its user-space libraries do not match the \
     loaded kernel driver.",
    "The /dev/dri render node is missing or not accessible to OBS \
     (log: \"Unable to open DRM render node.\").",
];

impl Report {
    pub fn new(adapter: Option<&str>, devices: Vec<GpuDevice>, frames: Option<FrameStats>) -> Self {
        let state = classify(adapter, &devices, frames);
        let mut causes = Vec::new();
        let mut cause_keys = Vec::new();
        let (summary_key, summary_args): (&'static str, Vec<String>);
        let summary = match adapter {
            None => {
                (summary_key, summary_args) =
                    ("Basic.StatusBar.GpuStatus.Summary.InitFailed", vec![]);
                "Graphics initialization failed, so OBS cannot render video. \
                 Check the log for the graphics error."
                    .to_owned()
            }
            Some(a) if frames.is_some_and(|f| f.median_frame_ns > f.interval_ns) => {
                let f = frames.unwrap_or(FrameStats {
                    median_frame_ns: 0,
                    interval_ns: 0,
                });
                let (frame, budget) = (
                    format!("{:.1}", ms(f.median_frame_ns)),
                    format!("{:.1}", ms(f.interval_ns)),
                );
                (summary_key, summary_args) = (
                    "Basic.StatusBar.GpuStatus.Summary.OverBudget",
                    vec![frame.clone(), budget.clone(), a.to_owned()],
                );
                format!(
                    "Frames take {frame} ms to render against a {budget} ms budget on {a}; expect choppy video."
                )
            }
            Some(a) => match state {
                State::Green => {
                    (summary_key, summary_args) = (
                        "Basic.StatusBar.GpuStatus.Summary.Green",
                        vec![a.to_owned()],
                    );
                    format!("Rendering on {a}.")
                }
                // Software rendering: the text follows the situation (a GPU
                // OBS can't use, or no GPU at all), the color follows #1005.
                _ if !devices.is_empty() => {
                    causes = SOFTWARE_CAUSES.iter().map(|c| (*c).to_owned()).collect();
                    cause_keys = SOFTWARE_CAUSE_KEYS.to_vec();
                    let gpu = devices
                        .iter()
                        .find_map(|d| d.name.clone())
                        .unwrap_or_else(|| devices[0].vendor.display().to_owned());
                    (summary_key, summary_args) = (
                        "Basic.StatusBar.GpuStatus.Summary.Unused",
                        vec![gpu.clone(), a.to_owned()],
                    );
                    format!(
                        "Your GPU ({gpu}) was detected, but OBS is not using its driver. \
                         Rendering on the CPU ({a}); expect choppy video."
                    )
                }
                _ => {
                    causes = SOFTWARE_CAUSES[1..]
                        .iter()
                        .map(|c| (*c).to_owned())
                        .collect();
                    cause_keys = SOFTWARE_CAUSE_KEYS[1..].to_vec();
                    (summary_key, summary_args) = (
                        "Basic.StatusBar.GpuStatus.Summary.NoGpu",
                        vec![a.to_owned()],
                    );
                    format!(
                        "No GPU was detected and OBS is rendering on the CPU ({a}); \
                         expect choppy video."
                    )
                }
            },
        };
        Self {
            state,
            adapter: adapter.map(str::to_owned),
            devices,
            summary,
            causes,
            log_lines: Vec::new(),
            summary_key,
            summary_args,
            cause_keys,
            usage: None,
            usage_stale: false,
        }
    }

    /// Attach the render adapter's utilization ([`crate::usage::Sampler`]).
    pub fn with_usage(self, usage: Option<UsageSample>) -> Self {
        Self { usage, ..self }
    }

    /// Mark the reading as too old: the label is "n/a" and the tooltip
    /// says that the reading is late, not that there is no source.
    pub fn with_usage_stale(self, usage_stale: bool) -> Self {
        Self {
            usage_stale,
            ..self
        }
    }

    /// The status bar label for this state and [`Self::usage`] (#1025).
    /// Software rendering says so instead of a percent: a GPU OBS does not
    /// use can sit at 0 % (#1004). Graphics init failure is "n/a"; its
    /// summary already says why.
    pub fn readout(&self) -> Readout {
        let n_a = Text {
            key: "Basic.StatusBar.Gpu.Unavailable",
            args: vec![],
            english: "GPU: n/a".to_owned(),
        };
        let Some(adapter) = self.adapter.as_deref() else {
            return Readout {
                label: n_a,
                details: vec![],
            };
        };
        if crate::is_software_renderer(adapter) {
            return Readout {
                label: Text {
                    key: "Basic.StatusBar.Gpu.Software",
                    args: vec![],
                    english: "CPU rendering".to_owned(),
                },
                details: vec![],
            };
        }
        let Some(usage) = self.usage else {
            let detail = if self.usage_stale {
                Text {
                    key: "Basic.StatusBar.Gpu.Stale",
                    args: vec![],
                    english: "The GPU reading is late.".to_owned(),
                }
            } else {
                Text {
                    key: "Basic.StatusBar.Gpu.NoSource",
                    args: vec![],
                    english: "No utilization source for this GPU.".to_owned(),
                }
            };
            return Readout {
                label: n_a,
                details: vec![detail],
            };
        };
        Readout {
            label: Text {
                key: "Basic.StatusBar.Gpu.Usage",
                args: vec![usage.percent.to_string()],
                english: format!("GPU: {}%", usage.percent),
            },
            details: vec![],
        }
    }

    /// Attach the log lines that explain the state ([`crate::LogCapture`]).
    pub fn with_log_lines(self, log_lines: Vec<String>) -> Self {
        Self { log_lines, ..self }
    }

    /// The `gpuAcceleration` JSON object:
    /// `{"state","adapter","detectedGpus":[{"vendor","deviceId","name","renderNode"}],"summary","causes",`
    /// `"usage":{"percent","source"}|null,"readout":{"text","key","args","details":[…]},"logLines"}`.
    pub fn to_json(&self) -> String {
        let mut j = String::from("{\"state\":");
        string(&mut j, Some(self.state.as_str()));
        j.push_str(",\"adapter\":");
        string(&mut j, self.adapter.as_deref());
        let _ = write!(
            j,
            ",\"adapterIsApiName\":{}",
            self.adapter
                .as_deref()
                .is_some_and(crate::is_graphics_api_name)
        );
        j.push_str(",\"detectedGpus\":[");
        for (i, d) in self.devices.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push_str("{\"vendor\":");
            string(&mut j, Some(d.vendor.as_str()));
            j.push_str(",\"deviceId\":");
            string(&mut j, d.device_id.as_deref());
            j.push_str(",\"name\":");
            string(&mut j, d.name.as_deref());
            let _ = write!(j, ",\"renderNode\":{}}}", d.render_node);
        }
        j.push_str("],\"summary\":");
        string(&mut j, Some(&self.summary));
        j.push_str(",\"causes\":[");
        for (i, c) in self.causes.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            string(&mut j, Some(c));
        }
        j.push_str("],\"summaryKey\":");
        string(&mut j, Some(self.summary_key));
        j.push_str(",\"summaryArgs\":[");
        for (i, a) in self.summary_args.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            string(&mut j, Some(a));
        }
        j.push_str("],\"causeKeys\":[");
        for (i, k) in self.cause_keys.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            string(&mut j, Some(k));
        }
        j.push_str("],\"usage\":");
        match self.usage {
            Some(u) => {
                let _ = write!(j, "{{\"percent\":{},\"source\":", u.percent);
                string(&mut j, Some(u.source.as_str()));
                j.push('}');
            }
            None => j.push_str("null"),
        }
        let readout = self.readout();
        j.push_str(",\"readout\":{");
        text_fields(&mut j, &readout.label);
        j.push_str(",\"details\":[");
        for (i, t) in readout.details.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            j.push('{');
            text_fields(&mut j, t);
            j.push('}');
        }
        j.push_str("]},\"logLines\":[");
        for (i, line) in self.log_lines.iter().enumerate() {
            if i > 0 {
                j.push(',');
            }
            string(&mut j, Some(line));
        }
        j.push_str("]}");
        j
    }
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

/// Append `"text":…,"key":…,"args":[…]` for a localized text.
fn text_fields(out: &mut String, t: &Text) {
    out.push_str("\"text\":");
    string(out, Some(&t.english));
    out.push_str(",\"key\":");
    string(out, Some(t.key));
    out.push_str(",\"args\":[");
    for (i, a) in t.args.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        string(out, Some(a));
    }
    out.push(']');
}

/// Append `s` as a JSON string, or `null`.
fn string(out: &mut String, s: Option<&str>) {
    let Some(s) = s else {
        out.push_str("null");
        return;
    };
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl State {
    /// `"green"`, `"yellow"` or `"red"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Red => "red",
        }
    }
}

impl Vendor {
    /// `"nvidia"`, `"amd"` or `"intel"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nvidia => "nvidia",
            Self::Amd => "amd",
            Self::Intel => "intel",
        }
    }

    fn display(self) -> &'static str {
        match self {
            Self::Nvidia => "NVIDIA",
            Self::Amd => "AMD",
            Self::Intel => "Intel",
        }
    }
}
