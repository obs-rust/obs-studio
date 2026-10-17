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
//!
//! A slow source must not keep the sampling thread busy or show an old
//! number (#1022): [`next_interval`] stretches the pause after a slow
//! sample, [`current`] drops a reading that is too old, and [`Sampler`]
//! stops asking a source that is slow [`DEMOTE_AFTER`] times in a row.

use std::time::{Duration, Instant};

use crate::{RenderAdapter, Utilization};

/// The pause between samples when they are fast: the status bar refreshes
/// once a second.
pub const BASE_INTERVAL: Duration = Duration::from_secs(1);
/// The longest pause after a slow sample.
pub const MAX_INTERVAL: Duration = Duration::from_secs(30);
/// The pause is this many times the sample's duration, so sampling takes
/// at most about a tenth of the thread's time.
pub const DUTY_FACTOR: u32 = 9;
/// A source call that takes longer than this is slow.
pub const SLOW_SAMPLE: Duration = Duration::from_secs(2);
/// A source that is slow this many times in a row is not asked again.
pub const DEMOTE_AFTER: u32 = 3;
/// A reading older than this many intervals is not shown.
pub const STALE_INTERVALS: u32 = 3;

/// The pause after a sample that took `elapsed`: [`BASE_INTERVAL`], or
/// [`DUTY_FACTOR`] times `elapsed`, at most [`MAX_INTERVAL`].
pub fn next_interval(elapsed: Duration) -> Duration {
    elapsed
        .saturating_mul(DUTY_FACTOR)
        .clamp(BASE_INTERVAL, MAX_INTERVAL)
}

/// `sample` if it is still current at `now`: taken at `taken`, while the
/// thread pauses `interval` between samples. A reading older than
/// [`STALE_INTERVALS`] intervals is `None`, also while a sample hangs.
pub fn current(
    sample: Option<UsageSample>,
    taken: Instant,
    interval: Duration,
    now: Instant,
) -> Option<UsageSample> {
    let limit = interval.saturating_mul(STALE_INTERVALS);
    sample.filter(|_| now.saturating_duration_since(taken) <= limit)
}

/// Where a sample came from (`usage.source` in the JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// NVIDIA, through NVML (#1021).
    Nvml,
    /// amdgpu's `gpu_busy_percent` (#1024).
    Amdgpu,
    /// IOKit's `Device Utilization %` on macOS (#1026).
    Iokit,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nvml => "nvml",
            Self::Amdgpu => "amdgpu",
            Self::Iokit => "iokit",
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

/// A source that [`Sampler`] stopped asking, and its last call times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demotion {
    pub source: Source,
    pub times: Vec<Duration>,
}

/// One source and how slow it has been.
struct Entry {
    source: Box<dyn UsageSource>,
    slow: Vec<Duration>,
    demoted: bool,
}

/// The time now; tests give a fake clock.
pub type Clock = Box<dyn Fn() -> Instant + Send>;

/// Takes a sample from the first source that has a reading, and stops
/// asking a source that is slow [`DEMOTE_AFTER`] times in a row.
pub struct Sampler {
    sources: Vec<Entry>,
    clock: Clock,
    demotions: Vec<Demotion>,
}

impl Sampler {
    pub fn new(sources: Vec<Box<dyn UsageSource>>) -> Self {
        Self::with_clock(sources, Box::new(Instant::now))
    }

    pub fn with_clock(sources: Vec<Box<dyn UsageSource>>, clock: Clock) -> Self {
        let sources = sources
            .into_iter()
            .map(|source| Entry {
                source,
                slow: Vec::new(),
                demoted: false,
            })
            .collect();
        Self {
            sources,
            clock,
            demotions: Vec::new(),
        }
    }

    /// The reading of the first source with one for `adapter`, or `None`.
    pub fn sample(&mut self, adapter: &RenderAdapter) -> Option<UsageSample> {
        for entry in self.sources.iter_mut().filter(|entry| !entry.demoted) {
            let start = (self.clock)();
            let reading = entry.source.whole(adapter);
            let took = (self.clock)().saturating_duration_since(start);
            if took > SLOW_SAMPLE {
                entry.slow.push(took);
                if entry.slow.len() >= DEMOTE_AFTER as usize {
                    entry.demoted = true;
                    self.demotions.push(Demotion {
                        source: entry.source.source(),
                        times: std::mem::take(&mut entry.slow),
                    });
                }
            } else {
                entry.slow.clear();
            }
            if let Utilization::Percent(percent) = reading {
                return Some(UsageSample {
                    percent,
                    source: entry.source.source(),
                });
            }
        }
        None
    }

    /// The sources demoted since the last call, each once.
    pub fn take_demotions(&mut self) -> Vec<Demotion> {
        std::mem::take(&mut self.demotions)
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
