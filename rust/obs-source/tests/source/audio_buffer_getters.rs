//! Tier 1 mirrors of test_source_audio_buffer_getters.c.
use obs_source::audio_buffer_getters::{
    AUDIO, MAX_AUDIO_CHANNELS, MAX_AUDIO_MIXES, MIX_FUNCTION, Mix, OutputBuffers, SOURCE,
    TIMESTAMP_FUNCTION, Timestamp, mix, timestamp,
};
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

type Logs = RefCell<Vec<(&'static str, &'static str)>>;

fn logger(logs: &Logs) -> impl FnOnce(&'static core::ffi::CStr, &'static core::ffi::CStr) + '_ {
    |function, parameter| {
        logs.borrow_mut()
            .push((function.to_str().unwrap(), parameter.to_str().unwrap()));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Lock,
    Read,
    Unlock,
}

/// A model source recording each step on its timestamp and mutex.
struct Model {
    audio_ts: u64,
    locked: bool,
    steps: RefCell<Vec<Step>>,
}

impl Timestamp for Model {
    fn lock(&mut self) {
        assert!(!self.locked, "locked twice");
        self.locked = true;
        self.steps.get_mut().push(Step::Lock);
    }
    fn audio_ts(&self) -> u64 {
        assert!(self.locked, "audio_ts read without the mutex");
        self.steps.borrow_mut().push(Step::Read);
        self.audio_ts
    }
    fn unlock(&mut self) {
        assert!(self.locked, "unlocked without the lock");
        self.locked = false;
        self.steps.get_mut().push(Step::Unlock);
    }
}

/// null_source_timestamp_is_zero_and_names_the_caller: NULL is 0 and logs
/// once under obs_source_get_audio_timestamp.
#[test]
fn null_source_timestamp_is_zero_and_names_the_caller() {
    let logs = Logs::default();
    assert_eq!(timestamp(None::<&mut Model>, logger(&logs)), 0);
    assert_eq!(
        *logs.borrow(),
        [("obs_source_get_audio_timestamp", "source")]
    );
    assert_eq!(TIMESTAMP_FUNCTION, c"obs_source_get_audio_timestamp");
    assert_eq!(SOURCE, c"source");
}

/// timestamp_reads_only_audio_ts: the answer is audio_ts, read once under
/// the mutex, twice in a row, leaving it unlocked, with no log.
#[test]
fn timestamp_reads_only_audio_ts() {
    let logs = Logs::default();
    for value in [0, 1, 48_000, 0x0123_4567_89ab_cdef, u64::MAX] {
        let mut source = Model {
            audio_ts: value,
            locked: false,
            steps: RefCell::default(),
        };
        for _ in 0..2 {
            source.steps.get_mut().clear();
            assert_eq!(timestamp(Some(&mut source), logger(&logs)), value);
            assert_eq!(
                *source.steps.borrow(),
                [Step::Lock, Step::Read, Step::Unlock]
            );
            assert!(!source.locked);
            assert_eq!(source.audio_ts, value);
        }
    }
    assert!(logs.borrow().is_empty());
}

/// A source shared with another thread: a spin lock stands for
/// audio_buf_mutex.
struct Shared {
    mutex: Arc<AtomicBool>,
    audio_ts: Arc<AtomicU64>,
}

impl Timestamp for Shared {
    fn lock(&mut self) {
        while self
            .mutex
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::thread::yield_now();
        }
    }
    fn audio_ts(&self) -> u64 {
        self.audio_ts.load(Ordering::Relaxed)
    }
    fn unlock(&mut self) {
        self.mutex.store(false, Ordering::Release);
    }
}

/// timestamp_waits_for_the_audio_buffer_mutex: while another thread holds
/// the mutex the call waits, and returns what was written before the unlock.
#[test]
fn timestamp_waits_for_the_audio_buffer_mutex() {
    let mutex = Arc::new(AtomicBool::new(true));
    let audio_ts = Arc::new(AtomicU64::new(1000));
    let (started, done) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    let reader = {
        let mut source = Shared {
            mutex: Arc::clone(&mutex),
            audio_ts: Arc::clone(&audio_ts),
        };
        let (started, done) = (Arc::clone(&started), Arc::clone(&done));
        std::thread::spawn(move || {
            started.store(true, Ordering::SeqCst);
            let ts = timestamp(Some(&mut source), |_, _| panic!("no log"));
            done.store(true, Ordering::SeqCst);
            ts
        })
    };
    while !started.load(Ordering::SeqCst) {
        std::hint::spin_loop();
    }
    // A reader that skipped the mutex would be done long before this.
    std::thread::sleep(Duration::from_millis(100));
    assert!(!done.load(Ordering::SeqCst));
    audio_ts.store(2000, Ordering::Relaxed);
    mutex.store(false, Ordering::Release);
    assert_eq!(reader.join().unwrap(), 2000);
    assert!(done.load(Ordering::SeqCst));
}

/// Output buffers standing for pointers, counting reads per slot.
struct Buffers {
    buf: Mix<usize>,
    reads: RefCell<Mix<u32>>,
}

impl OutputBuffers for Buffers {
    type Buffer = usize;
    fn output_buf(&self, mix: usize, channel: usize) -> usize {
        self.reads.borrow_mut()[mix][channel] += 1;
        self.buf[mix][channel]
    }
}

fn buffers() -> Buffers {
    let mut buf = [[0; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES];
    for (m, row) in buf.iter_mut().enumerate() {
        for (c, slot) in row.iter_mut().enumerate() {
            // NULL every third, else a distinct address.
            *slot = if (m + c) % 3 == 0 {
                0
            } else {
                0x1000 + 4 * (m * MAX_AUDIO_CHANNELS + c)
            };
        }
    }
    Buffers {
        buf,
        reads: RefCell::default(),
    }
}

/// null_source_mix_names_the_caller_and_writes_nothing: NULL logs once under
/// obs_source_get_audio_mix and leaves audio untouched; both NULL logs the
/// source line only.
#[test]
fn null_source_mix_names_the_caller_and_writes_nothing() {
    let logs = Logs::default();
    let mut audio: Mix<usize> = [[0x5a5a_5a5a; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES];
    let snapshot = audio;
    mix(None::<&Buffers>, Some(&mut audio), logger(&logs));
    assert_eq!(*logs.borrow(), [("obs_source_get_audio_mix", "source")]);
    assert_eq!(audio, snapshot);

    logs.borrow_mut().clear();
    mix(None::<&Buffers>, None, logger(&logs));
    assert_eq!(*logs.borrow(), [("obs_source_get_audio_mix", "source")]);
    assert_eq!(MIX_FUNCTION, c"obs_source_get_audio_mix");
}

/// null_audio_names_the_parameter_and_writes_nothing: a NULL audio logs
/// "audio: Null 'audio' parameter" and reads nothing from the source.
#[test]
fn null_audio_names_the_parameter_and_writes_nothing() {
    let logs = Logs::default();
    let source = buffers();
    let snapshot = source.buf;
    mix(Some(&source), None, logger(&logs));
    assert_eq!(*logs.borrow(), [("audio", "audio")]);
    assert_eq!(AUDIO, c"audio");
    assert_eq!(source.buf, snapshot);
    assert_eq!(
        *source.reads.borrow(),
        [[0; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES]
    );
}

/// mix_copies_every_buffer_pointer: every output[mix].data[ch] is the
/// matching audio_output_buf[mix][ch], each read once, the source unchanged.
#[test]
fn mix_copies_every_buffer_pointer() {
    let logs = Logs::default();
    for fill in [0x00, usize::MAX, 0x5a5a_5a5a] {
        let source = buffers();
        let snapshot = source.buf;
        let mut audio: Mix<usize> = [[!fill; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES];
        mix(Some(&source), Some(&mut audio), logger(&logs));
        assert_eq!(audio, source.buf);
        assert_eq!(source.buf, snapshot);
        assert_eq!(
            *source.reads.borrow(),
            [[1; MAX_AUDIO_CHANNELS]; MAX_AUDIO_MIXES]
        );
    }
    assert!(logs.borrow().is_empty());
}
