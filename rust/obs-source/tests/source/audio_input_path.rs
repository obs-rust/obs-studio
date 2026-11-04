//! Tier 1 for `obs_source::audio_input_path`, the source audio input path of
//! `libobs/obs-source-audio-input-path.c.h`.
//!
//! The cases run the path on a fake source that records every write and call in
//! order, and panics on `resample_offset` touched without `audio_buf_mutex`, on
//! the resampler or the balance getter with that mutex held, on the filters
//! without `filter_mutex`, on the output path without `filter_mutex` and
//! `audio_mutex`, on `audio_mutex` taken without `filter_mutex`, and on a lock
//! taken twice. `test_source_audio_input.c` pins the same path on the real libobs.

use std::cell::{Cell, RefCell};

use obs_source::audio_input::{FORCE_MONO, Format, SPEAKERS_STEREO};
use obs_source::audio_input_path::{Audio, MAX_PLANES, Planes, Resampled, Source, output_audio};
use proptest::prelude::*;

const FLOAT_PLANAR: i32 = 8;
const RATE: u32 = 48_000;
const FRAMES: u32 = 1024;
/// `FRAMES` at 48 kHz.
const DUR: u64 = 21_333_333;
const NOW: u64 = 100_000_000_000;

fn output_format() -> Format {
    Format {
        format: FLOAT_PLANAR,
        samples_per_sec: RATE,
        speakers: SPEAKERS_STEREO,
    }
}

/// Every plane set, so a test sees which ones the path clears.
fn planes() -> Planes<u32> {
    let mut out = [None; MAX_PLANES];
    for (plane, value) in out.iter_mut().zip(100..) {
        *plane = Some(value);
    }
    out
}

fn packet() -> Audio<u32> {
    Audio {
        data: planes(),
        frames: FRAMES,
        speakers: SPEAKERS_STEREO,
        format: FLOAT_PLANAR,
        samples_per_sec: RATE,
        timestamp: NOW,
    }
}

/// What `filter_audio` answers.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    Keep,
    Drop,
}

struct Fake {
    log: RefCell<Vec<String>>,
    audio_buf: Cell<bool>,
    filter: Cell<bool>,
    audio: Cell<bool>,

    valid: Cell<bool>,
    destroying: Cell<bool>,
    input: Cell<Audio<u32>>,
    audio_planes: Cell<usize>,

    channels: Cell<usize>,
    block_size: Cell<usize>,
    output_planes: Cell<usize>,

    resampler: Cell<bool>,
    create_works: Cell<bool>,
    next_resample_in_ts: Cell<u64>,
    sample_info: Cell<Format>,
    last_compensation: Cell<bool>,
    compensation: Cell<bool>,
    failed: Cell<bool>,
    resample_offset: Cell<u64>,
    storage: Cell<usize>,
    data_frames: Cell<u32>,
    data_timestamp: Cell<u64>,
    flags: Cell<u32>,
    balance: Cell<f32>,
    resampled_frames: Cell<u32>,
    resample_seen: Cell<Option<Planes<u32>>>,
    copied: RefCell<Vec<(usize, Option<u32>, usize)>>,
    filter_answer: Cell<Filter>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            log: RefCell::default(),
            audio_buf: Cell::default(),
            filter: Cell::default(),
            audio: Cell::default(),
            valid: Cell::new(true),
            destroying: Cell::default(),
            input: Cell::new(packet()),
            audio_planes: Cell::new(2),
            channels: Cell::new(2),
            block_size: Cell::new(4),
            output_planes: Cell::new(2),
            resampler: Cell::default(),
            create_works: Cell::new(true),
            next_resample_in_ts: Cell::default(),
            sample_info: Cell::new(output_format()),
            last_compensation: Cell::default(),
            compensation: Cell::default(),
            failed: Cell::default(),
            resample_offset: Cell::default(),
            storage: Cell::default(),
            data_frames: Cell::default(),
            data_timestamp: Cell::default(),
            flags: Cell::default(),
            balance: Cell::new(0.5),
            resampled_frames: Cell::new(FRAMES),
            resample_seen: Cell::default(),
            copied: RefCell::default(),
            filter_answer: Cell::default(),
        }
    }
}

impl Fake {
    fn rec(&self, text: impl Into<String>) {
        self.log.borrow_mut().push(text.into());
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.log.borrow_mut())
    }

    fn run(&self) -> Vec<String> {
        output_audio(self, ());
        assert!(!self.audio_buf.get(), "audio_buf_mutex left locked");
        assert!(!self.filter.get(), "filter_mutex left locked");
        assert!(!self.audio.get(), "audio_mutex left locked");
        self.take()
    }

    fn no_lock(&self, what: &str) {
        assert!(!self.audio_buf.get(), "{what} with audio_buf_mutex held");
        assert!(!self.filter.get(), "{what} with filter_mutex held");
        assert!(!self.audio.get(), "{what} with audio_mutex held");
    }
}

fn position(log: &[String], text: &str) -> usize {
    log.iter()
        .position(|line| line == text)
        .unwrap_or_else(|| panic!("no {text} in {log:?}"))
}

impl Source for Fake {
    type Plane = u32;
    type Input = ();
    type Output = u8;

    fn valid(&self) -> bool {
        self.rec("valid");
        self.valid.get()
    }
    fn destroying(&self) -> bool {
        self.rec("destroying");
        self.destroying.get()
    }
    fn input_valid(&self, (): ()) -> bool {
        self.rec("input_valid");
        self.input.get().frames != u32::MAX
    }
    fn read_input(&self, (): ()) -> Audio<u32> {
        self.rec("read_input");
        self.input.get()
    }
    fn audio_planes(&self, format: i32, speakers: i32) -> usize {
        self.rec(format!("audio_planes {format} {speakers}"));
        self.audio_planes.get()
    }
    fn output_format(&self) -> Format {
        self.rec("output_format");
        output_format()
    }
    fn planes(&self) -> usize {
        self.rec("planes");
        self.output_planes.get()
    }
    fn block_size(&self) -> usize {
        self.rec("block_size");
        self.block_size.get()
    }
    fn channels(&self) -> usize {
        self.rec("channels");
        self.channels.get()
    }
    fn lock_audio_buf(&self) {
        assert!(!self.audio_buf.replace(true), "audio_buf_mutex twice");
        self.rec("lock buf");
    }
    fn unlock_audio_buf(&self) {
        assert!(self.audio_buf.replace(false), "audio_buf_mutex not locked");
        self.rec("unlock buf");
    }
    fn lock_filter(&self) {
        assert!(!self.filter.replace(true), "filter_mutex twice");
        self.rec("lock filter");
    }
    fn unlock_filter(&self) {
        assert!(self.filter.replace(false), "filter_mutex not locked");
        self.rec("unlock filter");
    }
    fn lock_audio(&self) {
        assert!(self.filter.get(), "audio_mutex without filter_mutex");
        assert!(!self.audio.replace(true), "audio_mutex twice");
        self.rec("lock audio");
    }
    fn unlock_audio(&self) {
        assert!(self.audio.replace(false), "audio_mutex not locked");
        self.rec("unlock audio");
    }
    fn has_resampler(&self) -> bool {
        self.resampler.get()
    }
    fn next_resample_in_ts(&self) -> u64 {
        self.next_resample_in_ts.get()
    }
    fn set_next_resample_in_ts(&self, value: u64) {
        self.rec(format!("next_ts={value}"));
        self.next_resample_in_ts.set(value);
    }
    fn sample_info(&self) -> Format {
        self.sample_info.get()
    }
    fn sample_speakers(&self) -> i32 {
        self.sample_info.get().speakers
    }
    fn set_sample_info(&self, value: Format) {
        self.no_lock("sample_info");
        self.rec(format!(
            "sample_info={} {} {}",
            value.format, value.samples_per_sec, value.speakers
        ));
        self.sample_info.set(value);
    }
    fn last_async_compensation(&self) -> bool {
        self.last_compensation.get()
    }
    fn set_last_async_compensation(&self, value: bool) {
        self.rec(format!("last_comp={value}"));
        self.last_compensation.set(value);
    }
    fn async_compensation(&self) -> bool {
        self.rec("read comp");
        self.compensation.get()
    }
    fn audio_failed(&self) -> bool {
        self.failed.get()
    }
    fn set_audio_failed(&self, value: bool) {
        self.rec(format!("failed={value}"));
        self.failed.set(value);
    }
    fn resample_offset(&self) -> u64 {
        assert!(self.audio_buf.get(), "resample_offset without the mutex");
        self.rec("read offset");
        self.resample_offset.get()
    }
    fn set_resample_offset(&self, value: u64) {
        assert!(self.audio_buf.get(), "resample_offset without the mutex");
        self.rec(format!("offset={value}"));
        self.resample_offset.set(value);
    }
    fn audio_storage_size(&self) -> usize {
        self.storage.get()
    }
    fn set_audio_storage_size(&self, value: usize) {
        self.rec(format!("storage={value}"));
        self.storage.set(value);
    }
    fn set_data_frames(&self, value: u32) {
        self.no_lock("audio_data");
        self.rec(format!("data_frames={value}"));
        self.data_frames.set(value);
    }
    fn set_data_timestamp(&self, value: u64) {
        self.no_lock("audio_data");
        self.rec(format!("data_ts={value}"));
        self.data_timestamp.set(value);
    }
    fn flags(&self) -> u32 {
        self.flags.get()
    }
    fn destroy_resampler(&self) {
        self.no_lock("destroy");
        self.rec("destroy");
        self.resampler.set(false);
    }
    fn create_resampler(&self, output: Format) -> bool {
        self.no_lock("create");
        assert_eq!(output, output_format());
        self.rec("create");
        self.resampler.set(self.create_works.get());
        self.create_works.get()
    }
    fn log_resampler_failed(&self) {
        self.rec("log failed");
    }
    fn resample(&self, data: &Planes<u32>, frames: u32, offset: u64) -> Resampled<u32> {
        self.no_lock("resample");
        self.rec(format!("resample {frames} {offset}"));
        self.resample_seen.set(Some(*data));
        let mut out = [None; MAX_PLANES];
        out[0] = Some(7);
        out[1] = Some(8);
        Resampled {
            planes: out,
            frames: self.resampled_frames.get(),
            offset: offset + 3,
        }
    }
    fn free_storage(&self, plane: usize) {
        self.rec(format!("free {plane}"));
    }
    fn alloc_storage(&self, plane: usize, size: usize) {
        self.rec(format!("alloc {plane} {size}"));
    }
    fn copy_to_storage(&self, plane: usize, data: Option<u32>, size: usize) {
        self.rec(format!("copy {plane}"));
        self.copied.borrow_mut().push((plane, data, size));
    }
    fn balance_value(&self) -> f32 {
        self.no_lock("balance_value");
        self.rec("balance_value");
        self.balance.get()
    }
    fn apply_balance(&self, frames: u32, balance: f32) {
        self.rec(format!("balance {frames} {balance}"));
    }
    fn downmix(&self, channels: usize, frames: u32) {
        self.rec(format!("downmix {channels} {frames}"));
    }
    fn filter_audio(&self) -> Option<u8> {
        assert!(self.filter.get(), "filters without filter_mutex");
        assert!(!self.audio.get(), "filters with audio_mutex held");
        assert!(!self.audio_buf.get(), "filters with audio_buf_mutex held");
        self.rec("filter");
        (self.filter_answer.get() == Filter::Keep).then_some(9)
    }
    fn output_audio_data(&self, output: u8) {
        assert!(self.filter.get(), "output without filter_mutex");
        assert!(self.audio.get(), "output without audio_mutex");
        assert!(!self.audio_buf.get(), "output with audio_buf_mutex held");
        assert_eq!(output, 9);
        self.rec("output");
    }
}

#[test]
fn an_invalid_source_stops_at_the_first_check() {
    let fake = Fake::default();
    fake.valid.set(false);
    assert_eq!(fake.run(), ["valid"]);
}

#[test]
fn a_destroying_source_stops_before_the_packet_is_checked() {
    let fake = Fake::default();
    fake.destroying.set(true);
    assert_eq!(fake.run(), ["valid", "destroying"]);
}

#[test]
fn a_null_packet_stops_at_the_third_check() {
    let fake = Fake::default();
    let mut audio = packet();
    audio.frames = u32::MAX;
    fake.input.set(audio);
    assert_eq!(fake.run(), ["valid", "destroying", "input_valid"]);
}

#[test]
fn a_packet_in_the_output_format_is_copied_and_sent_on() {
    let fake = Fake::default();
    let log = fake.run();
    let size = 4096;
    assert_eq!(
        log,
        [
            "valid".to_string(),
            "destroying".into(),
            "input_valid".into(),
            "read_input".into(),
            format!("audio_planes {FLOAT_PLANAR} {SPEAKERS_STEREO}"),
            "read comp".into(),
            format!("next_ts={}", NOW + DUR),
            "planes".into(),
            "block_size".into(),
            format!("data_frames={FRAMES}"),
            format!("data_ts={NOW}"),
            "free 0".into(),
            format!("alloc 0 {size}"),
            "copy 0".into(),
            "free 1".into(),
            format!("alloc 1 {size}"),
            "copy 1".into(),
            format!("storage={size}"),
            "channels".into(),
            "balance_value".into(),
            "lock filter".into(),
            "filter".into(),
            "lock audio".into(),
            "output".into(),
            "unlock audio".into(),
            "unlock filter".into(),
        ]
    );
    assert_eq!(
        *fake.copied.borrow(),
        [(0, Some(100), size), (1, Some(101), size)]
    );
}

#[test]
fn the_storage_only_grows() {
    let fake = Fake::default();
    fake.run();
    let mut small = packet();
    small.frames = FRAMES / 2;
    fake.input.set(small);
    let log = fake.run();
    assert!(!log.iter().any(|line| line.starts_with("alloc")), "{log:?}");
    assert!(!log.iter().any(|line| line.starts_with("free")), "{log:?}");
    assert!(
        !log.iter().any(|line| line.starts_with("storage")),
        "{log:?}"
    );
    assert_eq!(fake.storage.get(), 4096);
    assert_eq!(fake.copied.borrow().last(), Some(&(1, Some(101), 2048)));
}

#[test]
fn unused_planes_are_cleared_for_the_resampler() {
    let fake = Fake::default();
    fake.resampler.set(true);
    fake.run();
    let seen = fake.resample_seen.get().unwrap();
    assert_eq!(seen[0], Some(100));
    assert_eq!(seen[1], Some(101));
    assert!(seen[2..].iter().all(Option::is_none), "{seen:?}");
}

#[test]
fn more_planes_than_the_array_clears_nothing() {
    let fake = Fake::default();
    fake.resampler.set(true);
    fake.audio_planes.set(MAX_PLANES + 1);
    fake.run();
    let seen = fake.resample_seen.get().unwrap();
    assert!(seen.iter().all(Option::is_some), "{seen:?}");
}

#[test]
fn the_resampler_runs_on_a_copy_of_the_offset_with_the_mutex_released() {
    let fake = Fake::default();
    fake.resampler.set(true);
    fake.resample_offset.set(5);
    fake.resampled_frames.set(100);
    let log = fake.run();
    let read = position(&log, "read offset");
    let run = position(&log, "resample 1024 5");
    let store = position(&log, "offset=8");
    assert!(read < run && run < store, "{log:?}");
    assert_eq!(log[read - 1], "lock buf");
    assert_eq!(log[read + 1], "unlock buf");
    assert_eq!(log[store - 1], "lock buf");
    assert_eq!(log[store + 1], "unlock buf");
    // the storage holds what the resampler wrote, and its count
    assert_eq!(fake.data_frames.get(), 100);
    assert_eq!(
        *fake.copied.borrow(),
        [(0, Some(7), 400), (1, Some(8), 400)]
    );
}

#[test]
fn a_format_change_resets_in_the_order_of_the_c() {
    let fake = Fake::default();
    let mut audio = packet();
    audio.samples_per_sec = 44_100;
    fake.input.set(audio);
    fake.resample_offset.set(5);
    let log = fake.run();
    let start = position(&log, "output_format");
    assert_eq!(
        log[start..start + 10],
        [
            "output_format".to_string(),
            format!("sample_info={FLOAT_PLANAR} 44100 {SPEAKERS_STEREO}"),
            "read comp".into(),
            "last_comp=false".into(),
            "destroy".into(),
            "lock buf".into(),
            "offset=0".into(),
            "unlock buf".into(),
            "create".into(),
            "failed=false".into(),
        ]
    );
    assert!(fake.resampler.get());
}

#[test]
fn a_reset_to_the_output_format_removes_the_resampler() {
    let fake = Fake::default();
    fake.resampler.set(true);
    fake.last_compensation.set(true);
    fake.failed.set(true);
    let log = fake.run();
    assert!(log.contains(&"destroy".to_string()), "{log:?}");
    assert!(!log.contains(&"create".to_string()), "{log:?}");
    assert!(log.contains(&"failed=false".to_string()), "{log:?}");
    assert!(!fake.resampler.get());
    assert!(!fake.failed.get());
}

#[test]
fn the_compensation_flag_creates_a_resampler() {
    let fake = Fake::default();
    fake.compensation.set(true);
    let log = fake.run();
    assert!(log.contains(&"last_comp=true".to_string()), "{log:?}");
    assert!(log.contains(&"create".to_string()), "{log:?}");
    assert!(
        log.iter().any(|line| line.starts_with("resample")),
        "{log:?}"
    );
}

#[test]
fn a_discontinuity_resets_only_with_a_resampler() {
    for (has_resampler, resets) in [(true, true), (false, false)] {
        let fake = Fake::default();
        fake.resampler.set(has_resampler);
        fake.next_resample_in_ts.set(NOW + 100_000_000);
        let log = fake.run();
        assert_eq!(log.contains(&"destroy".to_string()), resets, "{log:?}");
    }
}

#[test]
fn a_packet_that_continues_does_not_reset() {
    let fake = Fake::default();
    fake.resampler.set(true);
    fake.next_resample_in_ts.set(NOW);
    fake.resample_offset.set(5);
    let log = fake.run();
    assert!(!log.contains(&"destroy".to_string()), "{log:?}");
    assert!(log.contains(&"resample 1024 5".to_string()), "{log:?}");
}

#[test]
fn a_failed_resampler_sends_the_previous_packet_on() {
    let fake = Fake::default();
    fake.create_works.set(false);
    let mut audio = packet();
    audio.samples_per_sec = 44_100;
    fake.input.set(audio);
    fake.data_frames.set(11);
    fake.data_timestamp.set(22);
    let log = fake.run();
    let failed = position(&log, "failed=true");
    assert_eq!(log[failed + 1], "log failed");
    assert!(!log.iter().any(|line| line.starts_with("copy")), "{log:?}");
    assert!(!log.contains(&"balance_value".to_string()), "{log:?}");
    assert!(log.contains(&"output".to_string()), "{log:?}");
    assert_eq!(fake.data_frames.get(), 11);
    assert_eq!(fake.data_timestamp.get(), 22);
}

#[test]
fn a_packet_with_no_sample_rate_keeps_the_next_stamp() {
    let fake = Fake::default();
    let mut audio = packet();
    audio.samples_per_sec = 0;
    fake.input.set(audio);
    fake.next_resample_in_ts.set(77);
    let log = fake.run();
    assert!(
        !log.iter().any(|line| line.starts_with("next_ts")),
        "{log:?}"
    );
    assert_eq!(fake.next_resample_in_ts.get(), 77);
}

#[test]
fn the_next_stamp_wraps_as_in_the_c() {
    let fake = Fake::default();
    let mut audio = packet();
    audio.timestamp = u64::MAX;
    fake.input.set(audio);
    fake.run();
    assert_eq!(fake.next_resample_in_ts.get(), DUR - 1);
}

#[test]
fn the_balance_gate_uses_the_resampled_frames() {
    let fake = Fake::default();
    fake.resampler.set(true);
    fake.resampled_frames.set(100);
    fake.balance.set(0.25);
    let log = fake.run();
    assert!(log.contains(&"balance 100 0.25".to_string()), "{log:?}");
}

#[test]
fn the_balance_gate_skips_a_mono_output_and_a_centered_balance() {
    let fake = Fake::default();
    fake.balance.set(0.25);
    fake.channels.set(1);
    let log = fake.run();
    assert!(log.contains(&"balance_value".to_string()), "{log:?}");
    assert!(
        !log.iter().any(|line| line.starts_with("balance ")),
        "{log:?}"
    );

    let fake = Fake::default();
    fake.balance.set(0.5);
    let log = fake.run();
    assert!(
        !log.iter().any(|line| line.starts_with("balance ")),
        "{log:?}"
    );
}

#[test]
fn force_mono_asks_the_channels_again() {
    let fake = Fake::default();
    fake.flags.set(FORCE_MONO);
    let log = fake.run();
    assert_eq!(
        log.iter().filter(|line| *line == "channels").count(),
        2,
        "{log:?}"
    );
    assert!(log.contains(&format!("downmix 2 {FRAMES}")), "{log:?}");
}

#[test]
fn force_mono_on_a_mono_output_does_nothing() {
    let fake = Fake::default();
    fake.flags.set(FORCE_MONO);
    fake.channels.set(1);
    let log = fake.run();
    assert_eq!(
        log.iter().filter(|line| *line == "channels").count(),
        1,
        "{log:?}"
    );
    assert!(
        !log.iter().any(|line| line.starts_with("downmix")),
        "{log:?}"
    );
}

#[test]
fn a_dropped_packet_never_takes_audio_mutex() {
    let fake = Fake::default();
    fake.filter_answer.set(Filter::Drop);
    let log = fake.run();
    assert!(!log.contains(&"lock audio".to_string()), "{log:?}");
    assert!(!log.contains(&"output".to_string()), "{log:?}");
    assert_eq!(log.last().unwrap(), "unlock filter");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Any packets keep the locks balanced (the fake panics otherwise) and the
    /// storage at the largest size seen.
    #[test]
    fn any_packets_keep_the_rules(
        steps in prop::collection::vec(
            (0u32..4096, prop_oneof![Just(RATE), Just(44_100u32), Just(0u32)], any::<bool>(), any::<bool>(), any::<u64>()),
            1..16,
        ),
    ) {
        let fake = Fake::default();
        let mut largest = 0usize;
        for (frames, rate, compensation, works, timestamp) in steps {
            fake.compensation.set(compensation);
            fake.create_works.set(works);
            let mut audio = packet();
            audio.frames = frames;
            audio.samples_per_sec = rate;
            audio.timestamp = timestamp;
            fake.input.set(audio);
            fake.resampled_frames.set(frames);
            fake.run();
            if !fake.failed.get() {
                largest = largest.max(usize::try_from(frames).unwrap() * 4);
            }
            prop_assert_eq!(fake.storage.get(), largest);
        }
    }
}
