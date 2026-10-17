//! Tier 1 tests for the slow-source backoff of the `GPU: x%` label
//! (obs-rust/obs-studio#1022, follow-up of #1264): the pause after a sample
//! grows with its duration and recovers, an old reading is not shown, and a
//! source that is slow three times in a row is no longer asked. A fake
//! clock stands in for time; each fake source advances it by its cost.
//! New code with no C original, so Tiers 2 and 3 do not apply.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use obs_gpu_status::report::Report;
use obs_gpu_status::usage::{
    BASE_INTERVAL, Demotion, MAX_INTERVAL, Sampler, Source, UsageSample, UsageSource, current,
    next_interval,
};
use obs_gpu_status::{GpuDevice, RenderAdapter, Utilization, Vendor};

const NV: &str = "0000:2b:00.0";
const RTX: &str = "NVIDIA GeForce RTX 3060/PCIe/SSE2";

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn the_pause_grows_with_the_sample_time_and_recovers() {
    assert_eq!(next_interval(ms(0)), BASE_INTERVAL);
    assert_eq!(next_interval(ms(100)), BASE_INTERVAL);
    assert_eq!(next_interval(ms(200)), ms(1800));
    assert_eq!(next_interval(ms(3000)), ms(27000));
    assert_eq!(next_interval(ms(10_000)), MAX_INTERVAL);
    // A fast sample after a slow one gives the base pause again.
    assert_eq!(next_interval(ms(1)), BASE_INTERVAL);
}

fn sample(percent: u8) -> Option<UsageSample> {
    Some(UsageSample {
        percent,
        source: Source::Iokit,
    })
}

#[test]
fn a_reading_older_than_three_intervals_is_not_shown() {
    let t = Instant::now();
    assert_eq!(current(sample(37), t, ms(1000), t + ms(2900)), sample(37));
    assert_eq!(current(sample(37), t, ms(1000), t + ms(3100)), None);
    // After a slow sample the pause is longer, and so is the limit.
    assert_eq!(
        current(sample(37), t, ms(27_000), t + ms(60_000)),
        sample(37)
    );
    assert_eq!(current(sample(37), t, ms(27_000), t + ms(82_000)), None);
    assert_eq!(current(None, t, ms(1000), t), None);
}

/// A fake clock that the fake sources move forward.
#[derive(Clone)]
struct FakeClock(Arc<Mutex<Instant>>);

impl FakeClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Instant::now())))
    }
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
    fn boxed(&self) -> obs_gpu_status::usage::Clock {
        let c = self.0.clone();
        Box::new(move || *c.lock().unwrap())
    }
}

/// A source that takes `costs[n]` on its `n`-th call (the last one after
/// that) and reads `reading`; counts its calls.
struct Timed {
    source: Source,
    clock: FakeClock,
    costs: Vec<Duration>,
    reading: Utilization,
    calls: Arc<AtomicU32>,
}

impl UsageSource for Timed {
    fn source(&self) -> Source {
        self.source
    }
    fn whole(&mut self, _adapter: &RenderAdapter) -> Utilization {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) as usize;
        self.clock.advance(self.costs[n.min(self.costs.len() - 1)]);
        self.reading
    }
}

fn timed(
    source: Source,
    clock: &FakeClock,
    costs: &[u64],
    reading: Utilization,
) -> (Box<dyn UsageSource>, Arc<AtomicU32>) {
    let calls = Arc::new(AtomicU32::new(0));
    let s = Timed {
        source,
        clock: clock.clone(),
        costs: costs.iter().map(|&c| ms(c)).collect(),
        reading,
        calls: calls.clone(),
    };
    (Box::new(s), calls)
}

fn pci() -> RenderAdapter {
    RenderAdapter::Pci(NV.to_owned())
}

#[test]
fn a_source_slow_three_times_in_a_row_is_demoted() {
    let clock = FakeClock::new();
    let (slow, slow_calls) = timed(
        Source::Iokit,
        &clock,
        &[2500, 3000, 2100],
        Utilization::Percent(50),
    );
    let (next, _) = timed(Source::Nvml, &clock, &[5], Utilization::Percent(20));
    let mut s = Sampler::with_clock(vec![slow, next], clock.boxed());
    // Slow, but still the first source with a reading.
    for _ in 0..3 {
        assert_eq!(s.sample(&pci()).map(|u| u.percent), Some(50));
    }
    assert_eq!(
        s.take_demotions(),
        [Demotion {
            source: Source::Iokit,
            times: vec![ms(2500), ms(3000), ms(2100)],
        }]
    );
    // Demoted: the next source answers, and the slow one is not called.
    assert_eq!(
        s.sample(&pci()),
        Some(UsageSample {
            percent: 20,
            source: Source::Nvml,
        })
    );
    assert_eq!(slow_calls.load(Ordering::SeqCst), 3);
    // Each demotion is reported once.
    s.sample(&pci());
    assert!(s.take_demotions().is_empty());
}

#[test]
fn a_fast_sample_resets_the_count() {
    let clock = FakeClock::new();
    let (src, calls) = timed(
        Source::Iokit,
        &clock,
        &[2500, 2500, 10, 2500, 2500, 10],
        Utilization::Percent(30),
    );
    let mut s = Sampler::with_clock(vec![src], clock.boxed());
    for _ in 0..6 {
        assert_eq!(s.sample(&pci()).map(|u| u.percent), Some(30));
    }
    assert!(s.take_demotions().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 6);
}

#[test]
fn a_slow_source_without_a_reading_is_demoted_too() {
    // For example a call that times out: slow, and no value.
    let clock = FakeClock::new();
    let (slow, slow_calls) = timed(Source::Iokit, &clock, &[2500], Utilization::Unavailable);
    let mut s = Sampler::with_clock(vec![slow], clock.boxed());
    for _ in 0..3 {
        assert_eq!(s.sample(&pci()), None);
    }
    assert_eq!(s.take_demotions().len(), 1);
    assert_eq!(s.sample(&pci()), None);
    assert_eq!(slow_calls.load(Ordering::SeqCst), 3);
}

fn rtx3060() -> GpuDevice {
    GpuDevice {
        vendor: Vendor::Nvidia,
        device_id: Some("2504".into()),
        name: Some("NVIDIA GeForce RTX 3060".into()),
        render_node: true,
    }
}

#[test]
fn a_stale_reading_shows_n_a_and_says_it_is_late() {
    let r = Report::new(Some(RTX), vec![rtx3060()], None)
        .with_usage(None)
        .with_usage_stale(true);
    let out = r.readout();
    assert_eq!(out.label.english, "GPU: n/a");
    assert_eq!(out.details.len(), 1);
    assert_eq!(out.details[0].key, "Basic.StatusBar.Gpu.Stale");
    assert_eq!(out.details[0].english, "The GPU reading is late.");
}
