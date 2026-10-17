//! The status bar's `GPU: x%` label (obs-rust/obs-studio#1025).
//!
//! A [`Sampler`] asks its [`UsageSource`]s in order for the render
//! adapter's whole-GPU utilization and takes the first reading (#1004).
//! [`crate::report::Report::readout`] turns the report's state and the
//! sample into the label text.
//!
//! OBS's own share is not shown. NVML's per-process samples are sparse
//! and short-window peaks: on an RTX 3060 an OBS that rendered 4K at
//! 25 fps the whole time had no sample in 4 of 7 two-second windows, and
//! 45 % in one while the device average was 20 %.

use crate::{RenderAdapter, Utilization};

/// Where a sample came from (`usage.source` in the JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// NVIDIA, through NVML (#1021).
    Nvml,
    /// amdgpu's `gpu_busy_percent` (#1024).
    Amdgpu,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nvml => "nvml",
            Self::Amdgpu => "amdgpu",
        }
    }
}

/// One utilization source. The platform code gives the real ones
/// ([`crate::platform::usage_sources`]); tests give fakes.
pub trait UsageSource: Send {
    fn source(&self) -> Source;
    /// Whole-adapter utilization of `adapter`.
    fn whole(&mut self, adapter: &RenderAdapter) -> Utilization;
}

/// One reading for the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageSample {
    /// Whole-adapter percent, 0 to 100.
    pub percent: u8,
    pub source: Source,
}

/// Takes a sample from the first source that has a reading.
pub struct Sampler {
    sources: Vec<Box<dyn UsageSource>>,
}

impl Sampler {
    pub fn new(sources: Vec<Box<dyn UsageSource>>) -> Self {
        Self { sources }
    }

    /// The reading of the first source with one for `adapter`, or `None`.
    pub fn sample(&mut self, adapter: &RenderAdapter) -> Option<UsageSample> {
        self.sources
            .iter_mut()
            .find_map(|source| match source.whole(adapter) {
                Utilization::Percent(percent) => Some(UsageSample {
                    percent,
                    source: source.source(),
                }),
                Utilization::Unavailable => None,
            })
    }
}

/// A localized text: the frontend looks `key` up and fills `args` as %1,
/// %2, ...; `english` is the same text for logs and a missing key (#945).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub key: &'static str,
    pub args: Vec<String>,
    pub english: String,
}

/// What the label shows: its text, and lines for the tooltip below the
/// report's summary. The dot's color is the report's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readout {
    pub label: Text,
    pub details: Vec<Text>,
}
