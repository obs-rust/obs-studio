//! Ownership and orchestration of the FFmpeg audio resampler. The host retains
//! the SIMD kernels and version-specific FFmpeg types; this core owns the call
//! order, capacity, compensation state and timestamps.
use crate::audio_resampler_drift::{self as drift, Compensation};

/// Public resampling description, independent of the C ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    /// Samples per second.
    pub rate: u32,
    /// OBS audio format discriminant (unknown deliberately falls back to S16).
    pub format: i32,
    /// OBS speaker layout discriminant.
    pub speakers: i32,
}

/// Observable logging events, formatted at the host boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Context allocation failed.
    Allocation,
    /// Mono upmix matrix could not be installed.
    Matrix,
    /// Context initialization failed.
    Init(i32),
    /// Drift compensation has engaged.
    Engage,
    /// FFmpeg rejected compensation.
    Compensation(i32),
    /// FFmpeg rejected conversion.
    Convert(i32),
    /// Final clock drift accounting.
    Drift(u64, u64, f64),
}

/// FFmpeg operations. Implementations own the context and sample allocation;
/// their Drop releases both, including after partial initialization.
pub trait Host {
    /// Input token whose validity is established by the host boundary.
    type Input;
    /// Allocate a context configured for these descriptions.
    fn alloc(&mut self, dst: Info, src: Info) -> bool;
    /// Install a mono upmix row.
    fn matrix(&mut self, row: &[f64; 8]) -> i32;
    /// Initialize the context.
    fn init(&mut self) -> i32;
    /// Set compensation over the given distance.
    fn compensate(&mut self, delta: i32, distance: i32) -> i32;
    /// Buffered delay in the requested time base.
    fn delay(&mut self, base: i64) -> i64;
    /// FFmpeg's round-up rescale, including its overflow semantics.
    fn rescale(&mut self, value: i64, numerator: i64, denominator: i64) -> i64;
    /// Replace the sample allocation, retaining the C allocation-error behavior.
    fn resize(&mut self, channels: u32, frames: i32, format: i32);
    /// Convert into the owned sample allocation.
    fn convert(&mut self, capacity: i32, input: Self::Input, frames: i32) -> i32;
    /// Emit a log event.
    fn log(&mut self, event: Event);
}

/// Resampler state. The opaque public handle has no exposed C layout.
pub struct Resampler<H: Host> {
    /// Kernel host, including the output plane allocation.
    pub host: H,
    /// Drift loop and counters.
    pub compensation: Compensation,
    /// Number of output pointers to publish on success.
    pub planes: u32,
    src: Info,
    dst: Info,
    channels: u32,
    capacity: i32,
}

impl<H: Host> Resampler<H> {
    /// Allocate and initialize, releasing a partially initialized host on error.
    pub fn create(mut host: H, dst: Info, src: Info) -> Option<Self> {
        let channels = drift::output_channels(dst.speakers);
        if !host.alloc(dst, src) {
            host.log(Event::Allocation);
            return None;
        }
        if drift::layout_mask(src.speakers) == drift::ch_layout::MONO
            && drift::upmixes_mono(channels)
            && let Some(row) = drift::upmix_row(channels)
            && host.matrix(&row) < 0
        {
            host.log(Event::Matrix);
        }
        let error = host.init();
        if error != 0 {
            host.log(Event::Init(error));
            return None;
        }
        Some(Self {
            host,
            compensation: Compensation::default(),
            planes: if drift::planar_output(dst.format) {
                channels
            } else {
                1
            },
            src,
            dst,
            channels,
            capacity: 0,
        })
    }

    /// Update the existing lag-lead controller's input error.
    pub fn set_error(&mut self, error_ns: i64) {
        drift::set_error(&mut self.compensation, error_ns);
    }

    /// Convert a packet. Timestamp is written after compensation succeeds,
    /// even when conversion subsequently fails, matching the public C contract.
    pub fn resample(&mut self, input: H::Input, frames: u32, offset: &mut u64) -> Option<u32> {
        if self.compensation.enabled {
            let action = drift::step(&mut self.compensation, self.src.rate, frames);
            if action != drift::Action::Idle {
                if action == drift::Action::Engage {
                    self.host.log(Event::Engage);
                }
                let error = self.host.compensate(
                    drift::delta(&self.compensation, self.src.rate),
                    drift::DISTANCE,
                );
                if error < 0 {
                    self.host.log(Event::Compensation(error));
                    return None;
                }
            }
        }
        let delay = self.host.delay(i64::from(self.src.rate));
        // The C casts FFmpeg's int64 estimate to int before adding padding.
        let estimate = self.host.rescale(
            delay.wrapping_add(i64::from(frames)),
            i64::from(self.dst.rate),
            i64::from(self.src.rate),
        );
        #[allow(clippy::as_conversions)] // original narrowing int cast
        let estimate = drift::output_frames(estimate as i32, self.compensation.engaged);
        *offset = self.host.delay(1_000_000_000).cast_unsigned();
        if estimate > self.capacity {
            self.host.resize(
                self.channels,
                estimate,
                drift::sample_format(self.dst.format),
            );
            self.capacity = estimate;
        }
        let converted = self
            .host
            .convert(self.capacity, input, frames.cast_signed());
        if converted < 0 {
            self.host.log(Event::Convert(converted));
            return None;
        }
        drift::count(&mut self.compensation, frames, converted);
        Some(converted.cast_unsigned())
    }
}

impl<H: Host> Drop for Resampler<H> {
    fn drop(&mut self) {
        if drift::reports_drift(&self.compensation) {
            self.host.log(Event::Drift(
                self.compensation.input_samples,
                self.compensation.output_samples,
                drift::input_rate(&self.compensation, self.dst.rate),
            ));
        }
    }
}
