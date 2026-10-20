//! Tier 1 mirror of `test/cmocka/test_output_delay.c`.
//!
//! The twenty C cases keep their names and their assertions, in order,
//! against the safe core run on a model output ([`Out`]) that records the
//! libobs calls as the C test's stubs do: each with whether the delay lock
//! was held. The edge cases after them are new.
//!
//! The C test's fixture is [`Out::setup`] and its teardown, which cleans up
//! and checks that every held packet was released, is [`Out::teardown`].
//! The clock is the `now` the core is given, as `os_gettime_ns()` is renamed
//! to a test clock there. Where the C test reads the struct (`queued()`,
//! `out->active_delay_ns`), this reads the model.
//!
//! The model formats the two log lines the way the shim does, so the text
//! assertions say which line the core asked for and with which function
//! name; the levels and the `blog` formats themselves are the shim's, and
//! Tier 3 compares them with the C.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::CStr;

use obs_output::delay::{
    self, Decision, DelayOutput, Entry, NullLog, OBS_OUTPUT_DELAY_PRESERVE, OBS_OUTPUT_ENCODED,
    SET_DELAY, Signal, active_delay_secs, pop_decision,
};

const SEC: u64 = 1_000_000_000;
/// `OBS_OUTPUT_VIDEO`.
const OBS_OUTPUT_VIDEO: u32 = 1 << 0;
const LOG_WARNING: i32 = 200;
const LOG_DEBUG: i32 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    CanBegin,
    InitializeEncoders,
    BeginCapture,
    ActualStart,
    ActualStop,
    Packet,
    SignalStarting,
    SignalStopping,
}

/// `struct encoder_packet_time`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PacketTime {
    pts: i64,
    cts: u64,
    fer: u64,
    ferc: u64,
    pir: u64,
}

/// What an encoder hands the output: data the encoder owns.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EncPacket {
    pts: i64,
    data: [u8; 4],
}

/// A held copy (`obs_encoder_packet_create_instance`): its own buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    pts: i64,
    data: Vec<u8>,
    /// Which copy this is, counting from 1.
    copy: usize,
}

/// One recorded call, as `struct event` in the C test.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Event {
    kind: Kind,
    /// The delay lock was held when the call was made.
    locked: bool,
    /// `InitializeEncoders`: `starting` at the call.
    starting: bool,
    /// `ActualStop`.
    force: bool,
    ts: u64,
    /// `Packet`.
    pts: i64,
    first_byte: u8,
    copy: usize,
    time: Option<PacketTime>,
}

impl Event {
    fn new(kind: Kind, locked: bool) -> Self {
        Self {
            kind,
            locked,
            starting: false,
            force: false,
            ts: 0,
            pts: 0,
            first_byte: 0,
            copy: 0,
            time: None,
        }
    }
}

#[derive(Debug, Default)]
struct Log {
    last: String,
    level: i32,
    count: usize,
}

type Queued = Option<Entry<Held, PacketTime>>;

/// The output of the C test: the delay fields of `struct obs_output`, the
/// stub results and the recorded calls.
#[derive(Debug)]
struct Out {
    name: &'static str,
    flags: u32,
    queue: VecDeque<Queued>,
    active_delay_ns: u64,
    delay_sec: u32,
    delay_flags: u32,
    delay_cur_flags: u32,
    restart_refs: i64,
    delay_active: bool,
    delay_capturing: bool,
    starting: bool,
    reconnecting: bool,

    locked: bool,
    lock_calls: usize,
    events: Vec<Event>,
    clock_ns: u64,
    can_begin_result: bool,
    initialize_result: bool,
    begin_result: bool,
    actual_start_result: bool,
    /// Held copies not yet released (`bnum_allocs()` in the C test).
    live: i64,
    copies: usize,
    log: RefCell<Log>,
}

impl Out {
    /// `setup()`.
    fn setup() -> Self {
        Self {
            name: "delayed output",
            flags: OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO,
            queue: VecDeque::new(),
            active_delay_ns: 0,
            delay_sec: 0,
            delay_flags: 0,
            delay_cur_flags: 0,
            restart_refs: 0,
            delay_active: false,
            delay_capturing: false,
            starting: false,
            reconnecting: false,
            locked: false,
            lock_calls: 0,
            events: Vec::new(),
            clock_ns: 1000 * SEC,
            can_begin_result: true,
            initialize_result: true,
            begin_result: true,
            actual_start_result: true,
            live: 0,
            copies: 0,
            log: RefCell::new(Log {
                last: String::new(),
                level: -1,
                count: 0,
            }),
        }
    }

    /// `teardown()`: every packet the delay held has been released.
    fn teardown(mut self) {
        delay::cleanup(&mut self);
        assert!(self.queue.is_empty());
        assert!(!self.locked);
        assert_eq!(self.live, 0, "every packet the delay held is released");
    }

    fn record(&mut self, kind: Kind) -> &mut Event {
        assert!(self.events.len() < 64);
        let ev = Event::new(kind, self.locked);
        self.events.push(ev);
        self.events.last_mut().unwrap()
    }

    /// `start_capturing()`: delay running and capturing, as
    /// `hook_data_capture()` and `begin_delayed_capture()` leave it.
    fn start_capturing(&mut self, delay_ns: u64) {
        self.active_delay_ns = delay_ns;
        self.delay_active = true;
        self.delay_capturing = true;
    }

    /// `make_packet()`.
    fn make_packet(pts: i64) -> EncPacket {
        EncPacket {
            pts,
            data: [0xA0_u8.wrapping_add(pts as u8), 1, 2, 3],
        }
    }

    /// `send_packet_at()`.
    fn send_packet_at(&mut self, t: u64, pts: i64) {
        let pkt = Self::make_packet(pts);
        self.clock_ns = t;
        let copies = self.copies;
        delay::process_delay(self, &pkt, None, t);
        // the caller's packet is copied, not taken
        assert_eq!(pkt, Self::make_packet(pts));
        assert_eq!(self.copies, copies + 1);
    }

    fn delay_start(&mut self) -> bool {
        let now = self.clock_ns;
        delay::start(self, now)
    }

    fn delay_stop(&mut self) {
        let now = self.clock_ns;
        delay::stop(self, now);
    }

    fn set_delay(&mut self, sec: u32, flags: u32) {
        let log = NoNullLog;
        delay::set_delay(Some(self), &log, sec, flags);
    }

    fn get_delay(&self) -> u32 {
        delay::get_delay(Some(self), &NoNullLog)
    }

    fn get_active_delay(&self) -> u32 {
        delay::get_active_delay(Some(self), &NoNullLog)
    }

    fn queued(&self) -> usize {
        self.queue.len()
    }

    fn queued_at(&self, idx: usize) -> &Entry<Held, PacketTime> {
        self.queue[idx].as_ref().expect("an entry the delay queued")
    }

    fn count_events(&self, kind: Kind) -> usize {
        self.events.iter().filter(|e| e.kind == kind).count()
    }

    fn num_logs(&self) -> usize {
        self.log.borrow().count
    }

    fn last_log(&self) -> (i32, String) {
        let log = self.log.borrow();
        (log.level, log.last.clone())
    }

    fn write_log(&self, level: i32, line: String) {
        let mut log = self.log.borrow_mut();
        log.level = level;
        log.last = line;
        log.count += 1;
    }
}

/// The null-output log of the C test's capture handler.
impl NullLog for Out {
    fn null_output(&self, func: &'static CStr) {
        self.write_log(
            LOG_DEBUG,
            format!("{}: Null 'output' parameter", func.to_str().unwrap()),
        );
    }
}

/// For calls on a live output, which never log a null output.
struct NoNullLog;

impl NullLog for NoNullLog {
    fn null_output(&self, func: &'static CStr) {
        panic!("{func:?} logged a null output for a live one");
    }
}

impl DelayOutput for Out {
    type Source = EncPacket;
    type Packet = Held;
    type Time = PacketTime;

    fn lock(&mut self) {
        assert!(!self.locked, "delay_mutex is not recursive");
        self.locked = true;
        self.lock_calls += 1;
    }

    fn unlock(&mut self) {
        assert!(self.locked, "unlock without lock");
        self.locked = false;
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    fn push_back(&mut self, entry: Entry<Held, PacketTime>) {
        assert!(self.locked, "the queue is written under the lock");
        self.queue.push_back(Some(entry));
    }

    fn front_ts(&self) -> Option<u64> {
        assert!(self.locked, "the queue is read under the lock");
        self.queue.front().map(|e| e.as_ref().map_or(0, Entry::ts))
    }

    fn pop_front(&mut self) -> Queued {
        self.queue.pop_front().expect("pop of an empty queue")
    }

    fn active_delay_ns(&self) -> u64 {
        self.active_delay_ns
    }

    fn set_active_delay_ns(&mut self, ns: u64) {
        self.active_delay_ns = ns;
    }

    fn delay_sec(&self) -> u32 {
        self.delay_sec
    }

    fn set_delay_sec(&mut self, sec: u32) {
        self.delay_sec = sec;
    }

    fn set_delay_flags(&mut self, flags: u32) {
        self.delay_flags = flags;
    }

    fn delay_cur_flags(&self) -> u32 {
        self.delay_cur_flags
    }

    fn clear_restart_refs(&mut self) {
        self.restart_refs = 0;
    }

    fn inc_restart_refs(&mut self) {
        self.restart_refs += 1;
    }

    fn delay_active(&self) -> bool {
        self.delay_active
    }

    fn delay_capturing(&self) -> bool {
        self.delay_capturing
    }

    fn set_starting(&mut self, starting: bool) {
        self.starting = starting;
    }

    fn reconnecting(&self) -> bool {
        self.reconnecting
    }

    fn flags(&self) -> u32 {
        self.flags
    }

    fn warn_raw_output(&self, func: &'static CStr) {
        let line = format!(
            "Output '{}': Tried to use {} on a raw output",
            self.name,
            func.to_str().unwrap()
        );
        self.write_log(LOG_WARNING, line);
    }

    fn can_begin_data_capture(&mut self) -> bool {
        self.record(Kind::CanBegin);
        self.can_begin_result
    }

    fn initialize_encoders(&mut self) -> bool {
        let starting = self.starting;
        self.record(Kind::InitializeEncoders).starting = starting;
        self.initialize_result
    }

    fn begin_data_capture(&mut self) -> bool {
        self.record(Kind::BeginCapture);
        self.begin_result
    }

    fn actual_start(&mut self) {
        self.record(Kind::ActualStart);
        let _ = self.actual_start_result;
    }

    fn actual_stop(&mut self, force: bool, ts: u64) {
        let ev = self.record(Kind::ActualStop);
        ev.force = force;
        ev.ts = ts;
    }

    fn signal(&mut self, signal: Signal) {
        self.record(match signal {
            Signal::Starting => Kind::SignalStarting,
            Signal::Stopping => Kind::SignalStopping,
        });
    }

    fn copy_packet(&mut self, src: &EncPacket) -> Held {
        self.copies += 1;
        self.live += 1;
        Held {
            pts: src.pts,
            data: src.data.to_vec(),
            copy: self.copies,
        }
    }

    /// `delayed_packet()`: the callback owns the packet and releases it.
    fn deliver(&mut self, packet: Held, time: Option<PacketTime>) {
        let ev = self.record(Kind::Packet);
        ev.pts = packet.pts;
        ev.first_byte = packet.data.first().copied().unwrap_or(0);
        ev.copy = packet.copy;
        ev.time = time;
        self.release(packet);
    }

    fn release(&mut self, packet: Held) {
        drop(packet);
        self.live -= 1;
    }
}

fn msg_ts(entry: &Entry<Held, PacketTime>) -> u64 {
    entry.ts()
}

fn is_start(entry: &Entry<Held, PacketTime>) -> bool {
    matches!(entry, Entry::Start { .. })
}

fn is_stop(entry: &Entry<Held, PacketTime>) -> bool {
    matches!(entry, Entry::Stop { .. })
}

fn packet_of(entry: &Entry<Held, PacketTime>) -> (&Held, Option<PacketTime>) {
    match entry {
        Entry::Packet { packet, time, .. } => (packet, *time),
        other => panic!("not a packet: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// settings

#[test]
fn set_delay_stores_seconds_and_flags() {
    let mut out = Out::setup();

    out.set_delay(20, OBS_OUTPUT_DELAY_PRESERVE);
    assert_eq!(out.delay_sec, 20);
    assert_eq!(out.delay_flags, OBS_OUTPUT_DELAY_PRESERVE);
    assert_eq!(out.get_delay(), 20);

    // The setting is only stored: the running delay and its flags are
    // copied from it when data capture is hooked (obs-output.c).
    assert_eq!(out.active_delay_ns, 0);
    assert_eq!(out.delay_cur_flags, 0);
    assert_eq!(out.get_active_delay(), 0);

    // flags are stored as given, unknown bits too
    out.set_delay(0, 0xF0);
    assert_eq!(out.get_delay(), 0);
    assert_eq!(out.delay_flags, 0xF0);

    out.set_delay(u32::MAX, 0);
    assert_eq!(out.get_delay(), u32::MAX);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.num_logs(), 0);
    out.teardown();
}

#[test]
fn set_delay_ignored_on_raw_output() {
    let mut out = Out::setup();

    out.set_delay(5, OBS_OUTPUT_DELAY_PRESERVE);
    out.flags = OBS_OUTPUT_VIDEO;

    out.set_delay(30, 0);
    assert_eq!(out.delay_sec, 5);
    assert_eq!(out.delay_flags, OBS_OUTPUT_DELAY_PRESERVE);
    assert_eq!(out.num_logs(), 1);
    assert_eq!(
        out.last_log(),
        (
            LOG_WARNING,
            "Output 'delayed output': Tried to use obs_output_set_delay on a raw output".into()
        )
    );

    // the getters do not check the flag
    assert_eq!(out.get_delay(), 5);
    out.teardown();
}

#[test]
fn getters_on_null_output() {
    let out = Out::setup();

    delay::set_delay(None::<&mut Out>, &out, 10, 0);
    assert_eq!(
        out.last_log(),
        (
            LOG_DEBUG,
            "obs_output_set_delay: Null 'output' parameter".into()
        )
    );

    // Characterized, not endorsed: both getters log under the setter's
    // name.
    out.log.borrow_mut().last.clear();
    assert_eq!(delay::get_delay(None::<&Out>, &out), 0);
    assert_eq!(
        out.last_log().1,
        "obs_output_set_delay: Null 'output' parameter"
    );
    out.log.borrow_mut().last.clear();
    assert_eq!(delay::get_active_delay(None::<&Out>, &out), 0);
    assert_eq!(
        out.last_log().1,
        "obs_output_set_delay: Null 'output' parameter"
    );
    assert_eq!(out.num_logs(), 3);
    out.teardown();
}

#[test]
fn active_delay_is_whole_seconds() {
    let mut out = Out::setup();

    out.active_delay_ns = 2 * SEC + SEC / 2;
    assert_eq!(out.get_active_delay(), 2);
    out.active_delay_ns = SEC - 1;
    assert_eq!(out.get_active_delay(), 0);
    out.active_delay_ns = 30 * SEC;
    assert_eq!(out.get_active_delay(), 30);

    // Characterized, not endorsed: the cast to uint32_t wraps past
    // UINT32_MAX seconds. 18446744073 s mod 2^32 = 1266874889.
    out.active_delay_ns = u64::MAX;
    assert_eq!(out.get_active_delay(), 1_266_874_889);

    // independent of the stored setting
    out.set_delay(7, 0);
    out.active_delay_ns = 3 * SEC;
    assert_eq!(out.get_active_delay(), 3);
    assert_eq!(out.get_delay(), 7);
    out.teardown();
}

// ---------------------------------------------------------------------------
// packets

#[test]
fn packets_held_then_released_in_order() {
    let mut out = Out::setup();
    let t0 = 1000 * SEC;
    out.start_capturing(SEC);

    out.send_packet_at(t0, 0);
    out.send_packet_at(t0 + SEC / 2, 1);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.queued(), 2);

    // each entry keeps the time it arrived
    assert!(matches!(out.queued_at(0), Entry::Packet { .. }));
    assert_eq!(msg_ts(out.queued_at(0)), t0);
    assert_eq!(msg_ts(out.queued_at(1)), t0 + SEC / 2);
    assert_eq!(packet_of(out.queued_at(0)).1, None);

    // held while elapsed == delay: the comparison is strict
    out.send_packet_at(t0 + SEC, 2);
    assert_eq!(out.events.len(), 0);

    // released once more than the delay has passed
    out.send_packet_at(t0 + SEC + 1, 3);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].kind, Kind::Packet);
    assert_eq!(out.events[0].pts, 0);
    assert_eq!(out.events[0].time, None);
    assert_eq!(out.queued(), 3);

    // the callback gets the held copy, not the encoder's buffer
    assert_eq!(out.events[0].copy, 1);
    assert_eq!(out.events[0].first_byte, 0xA0);

    // one call releases every packet that is due, oldest first, but never
    // the one it just queued
    out.send_packet_at(t0 + 10 * SEC, 4);
    assert_eq!(out.events.len(), 4);
    for i in 1..4 {
        assert_eq!(out.events[i].kind, Kind::Packet);
        assert_eq!(out.events[i].pts, i as i64);
        assert_eq!(out.events[i].first_byte, 0xA0 + i as u8);
        assert!(!out.events[i].locked);
    }
    assert_eq!(out.queued(), 1);
    assert_eq!(packet_of(out.queued_at(0)).0.pts, 4);
    out.teardown();
}

#[test]
fn packet_time_is_copied() {
    let mut out = Out::setup();
    out.start_capturing(SEC);

    let pkt = Out::make_packet(7);
    let mut pt = PacketTime {
        pts: 7,
        cts: 11,
        fer: 13,
        ferc: 17,
        pir: 0,
    };
    out.clock_ns = 2000 * SEC;
    delay::process_delay(&mut out, &pkt, Some(pt), 2000 * SEC);
    assert!(packet_of(out.queued_at(0)).1.is_some());

    // changing the caller's copy afterwards does not matter
    pt.cts = 99;
    assert_eq!(pt.cts, 99);

    out.send_packet_at(2000 * SEC + 2 * SEC, 8);
    assert_eq!(out.events.len(), 1);
    let time = out.events[0].time.expect("the packet time was delivered");
    assert_eq!(time.pts, 7);
    assert_eq!(time.cts, 11);
    assert_eq!(time.fer, 13);
    assert_eq!(time.ferc, 17);
    out.teardown();
}

#[test]
fn zero_delay_still_waits_for_the_next_packet() {
    let mut out = Out::setup();
    out.start_capturing(0);

    // Characterized, not endorsed: with no delay left (cleanup sets 0) a
    // packet is still held, because its own elapsed time is 0, and 0 > 0
    // is false. A later packet at the same time does not release it
    // either.
    out.send_packet_at(5 * SEC, 0);
    out.send_packet_at(5 * SEC, 1);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.queued(), 2);

    out.send_packet_at(5 * SEC + 1, 2);
    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[0].pts, 0);
    assert_eq!(out.events[1].pts, 1);
    assert_eq!(out.queued(), 1);
    out.teardown();
}

#[test]
fn packets_dropped_unless_delay_is_capturing() {
    let mut out = Out::setup();
    out.start_capturing(SEC);

    out.send_packet_at(10 * SEC, 0);
    let held_allocs = out.live;

    // delay active, capture ended (a stop, or a reconnect without
    // preserve): due packets are released, not delivered
    out.delay_capturing = false;
    out.send_packet_at(12 * SEC, 1);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.queued(), 1);
    assert_eq!(out.live, held_allocs);

    // delay no longer active: the same
    out.delay_active = false;
    out.delay_capturing = true;
    out.send_packet_at(14 * SEC, 2);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.queued(), 1);
    assert_eq!(out.live, held_allocs);

    // the check is made when the packet leaves, not when it arrives
    out.delay_active = true;
    out.send_packet_at(16 * SEC, 3);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].pts, 2);
    out.teardown();
}

#[test]
fn clock_going_backwards_releases_at_once() {
    let mut out = Out::setup();
    out.start_capturing(30 * SEC);

    out.send_packet_at(100 * SEC, 0);

    // Characterized, not endorsed: the elapsed time is an unsigned
    // difference, so an earlier time wraps to a huge one and the packet is
    // released at once. os_gettime_ns() is monotonic, so it is not reached
    // in practice.
    out.send_packet_at(100 * SEC - 1, 1);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].pts, 0);
    assert_eq!(out.queued(), 1);
    out.teardown();
}

// ---------------------------------------------------------------------------
// start and stop

#[test]
fn delay_start_first_start() {
    let mut out = Out::setup();
    out.set_delay(1, 0);
    out.clock_ns = 50 * SEC;

    assert!(out.delay_start());

    // checks, then `starting`, then the encoders, then data capture; no
    // "starting" signal of its own (obs-output.c's begin emits it)
    assert_eq!(out.events.len(), 3);
    assert_eq!(out.events[0].kind, Kind::CanBegin);
    assert_eq!(out.events[1].kind, Kind::InitializeEncoders);
    assert!(out.events[1].starting);
    assert_eq!(out.events[2].kind, Kind::BeginCapture);
    for ev in &out.events {
        assert!(!ev.locked);
    }

    assert!(out.starting);
    assert_eq!(out.restart_refs, 1);

    // the real start is queued behind the delay
    assert_eq!(out.queued(), 1);
    assert!(is_start(out.queued_at(0)));
    assert_eq!(msg_ts(out.queued_at(0)), 50 * SEC);
    out.teardown();
}

#[test]
fn delay_start_refused_before_queueing() {
    let mut out = Out::setup();

    out.can_begin_result = false;
    assert!(!out.delay_start());
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].kind, Kind::CanBegin);
    assert!(!out.starting);
    assert_eq!(out.queued(), 0);
    assert_eq!(out.restart_refs, 0);

    // encoders that fail to initialize clear `starting` again
    out.events.clear();
    out.can_begin_result = true;
    out.initialize_result = false;
    assert!(!out.delay_start());
    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[1].kind, Kind::InitializeEncoders);
    assert!(out.events[1].starting);
    assert!(!out.starting);
    assert_eq!(out.queued(), 0);
    assert_eq!(out.restart_refs, 0);
    out.teardown();
}

#[test]
fn delay_start_failed_capture_cleans_up() {
    let mut out = Out::setup();
    out.active_delay_ns = SEC;
    out.begin_result = false;

    assert!(!out.delay_start());
    assert_eq!(out.events.len(), 3);
    assert_eq!(out.events[2].kind, Kind::BeginCapture);

    // the queued start is dropped without running, and the delay reset
    assert_eq!(out.queued(), 0);
    assert_eq!(out.count_events(Kind::ActualStart), 0);
    assert_eq!(out.active_delay_ns, 0);
    assert_eq!(out.restart_refs, 0);

    // Characterized, not endorsed: unlike a failed encoder initialization,
    // this path leaves `starting` set.
    assert!(out.starting);
    out.teardown();
}

#[test]
fn delay_start_while_delay_active() {
    let mut out = Out::setup();
    out.start_capturing(SEC);
    out.can_begin_result = false;
    out.initialize_result = false;
    out.begin_result = false;

    // A restart while the delay still runs (stopped, then started again
    // before the stop left the queue): no checks, no new capture, just a
    // queued start, a reference and the "starting" signal.
    out.clock_ns = 60 * SEC;
    assert!(out.delay_start());
    out.clock_ns = 61 * SEC;
    assert!(out.delay_start());

    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[0].kind, Kind::SignalStarting);
    assert_eq!(out.events[1].kind, Kind::SignalStarting);
    assert!(!out.events[0].locked);
    assert!(!out.starting);
    assert_eq!(out.restart_refs, 2);
    assert_eq!(out.queued(), 2);
    assert!(is_start(out.queued_at(0)));
    assert_eq!(msg_ts(out.queued_at(0)), 60 * SEC);
    assert_eq!(msg_ts(out.queued_at(1)), 61 * SEC);
    out.teardown();
}

#[test]
fn queued_start_runs_after_the_delay() {
    let mut out = Out::setup();
    out.set_delay(2, 0);
    out.clock_ns = 100 * SEC;
    assert!(out.delay_start());
    // what obs-output.c's hook_data_capture() does on begin
    out.start_capturing(2 * SEC);
    out.events.clear();

    out.send_packet_at(100 * SEC + 1, 0);
    out.send_packet_at(101 * SEC, 1);
    assert_eq!(out.events.len(), 0);

    // The start leaves the queue first, then the packets behind it; the
    // start's return value is not used.
    out.actual_start_result = false;
    out.send_packet_at(102 * SEC + 2, 2);
    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[0].kind, Kind::ActualStart);
    assert!(!out.events[0].locked);
    assert_eq!(out.events[1].kind, Kind::Packet);
    assert_eq!(out.events[1].pts, 0);

    // start and stop entries run whatever the delay state
    out.clock_ns = 103 * SEC;
    out.delay_active = false;
    out.delay_stop();
    out.events.clear();
    out.send_packet_at(200 * SEC, 3);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].kind, Kind::ActualStop);
    assert_eq!(out.queued(), 1);
    out.teardown();
}

#[test]
fn delay_stop_queues_a_stop_with_its_time() {
    let mut out = Out::setup();
    out.start_capturing(5 * SEC);

    out.send_packet_at(10 * SEC, 0);
    out.clock_ns = 11 * SEC;
    out.delay_stop();

    // "stopping" is signalled at once, the stop itself is queued
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].kind, Kind::SignalStopping);
    assert!(!out.events[0].locked);
    assert_eq!(out.queued(), 2);
    assert!(is_stop(out.queued_at(1)));
    assert_eq!(msg_ts(out.queued_at(1)), 11 * SEC);

    // packets that arrive after the stop are queued behind it
    out.send_packet_at(12 * SEC, 1);
    assert_eq!(out.queued(), 3);

    // the packet before the stop is delivered first; the stop is not
    // forced and carries the time it was asked for
    out.send_packet_at(16 * SEC + 1, 2);
    assert_eq!(out.events.len(), 3);
    assert_eq!(out.events[1].kind, Kind::Packet);
    assert_eq!(out.events[1].pts, 0);
    assert_eq!(out.events[2].kind, Kind::ActualStop);
    assert!(!out.events[2].force);
    assert_eq!(out.events[2].ts, 11 * SEC);
    assert!(!out.events[2].locked);

    // The stub stop leaves the delay capturing, so the next packet is
    // still delivered; the real stop ends the capture.
    out.send_packet_at(17 * SEC + 1, 3);
    assert_eq!(out.events.len(), 4);
    assert_eq!(out.events[3].kind, Kind::Packet);
    assert_eq!(out.events[3].pts, 1);

    // stopping does not take a restart reference
    assert_eq!(out.restart_refs, 0);
    out.teardown();
}

#[test]
fn cleanup_releases_everything() {
    let mut out = Out::setup();
    out.set_delay(4, OBS_OUTPUT_DELAY_PRESERVE);
    out.delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
    out.start_capturing(4 * SEC);
    out.restart_refs = 3;

    out.send_packet_at(10 * SEC, 0);
    assert!(out.delay_start());
    out.send_packet_at(10 * SEC, 1);
    out.delay_stop();
    out.send_packet_at(10 * SEC, 2);
    assert_eq!(out.queued(), 5);
    out.events.clear();

    delay::cleanup(&mut out);

    // nothing runs: no start, no stop, no packet
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.queued(), 0);
    assert_eq!(out.active_delay_ns, 0);
    assert_eq!(out.restart_refs, 0);

    // the rest of the delay state is left alone
    assert!(out.delay_active);
    assert!(out.delay_capturing);
    assert_eq!(out.delay_sec, 4);
    assert_eq!(out.delay_flags, OBS_OUTPUT_DELAY_PRESERVE);
    assert_eq!(out.delay_cur_flags, OBS_OUTPUT_DELAY_PRESERVE);
    assert_eq!(out.get_active_delay(), 0);

    // the queue still works afterwards (teardown checks no packet leaks)
    delay::cleanup(&mut out);
    out.send_packet_at(11 * SEC, 3);
    assert_eq!(out.queued(), 1);
    out.teardown();
}

// ---------------------------------------------------------------------------
// preserve on disconnect

#[test]
fn preserve_while_reconnecting_grows_the_delay() {
    let mut out = Out::setup();
    out.delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
    out.start_capturing(10 * SEC);

    out.send_packet_at(100 * SEC, 0);
    out.send_packet_at(105 * SEC, 1);

    // While reconnecting, nothing leaves the queue, and the delay becomes
    // the age of the oldest entry: the stream picks up where it was cut,
    // later by the time the outage took.
    out.reconnecting = true;
    out.send_packet_at(115 * SEC, 2);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.active_delay_ns, 15 * SEC);
    out.send_packet_at(140 * SEC, 3);
    assert_eq!(out.active_delay_ns, 40 * SEC);
    assert_eq!(out.get_active_delay(), 40);
    assert_eq!(out.queued(), 4);

    // back online: the grown delay stays
    out.reconnecting = false;
    out.send_packet_at(140 * SEC, 4);
    assert_eq!(out.events.len(), 0);
    out.send_packet_at(140 * SEC + 1, 5);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].pts, 0);
    assert_eq!(out.active_delay_ns, 40 * SEC);

    // Characterized, not endorsed: it never shrinks back to the 10 s that
    // was set; the next packets keep the 40 s spacing.
    out.send_packet_at(145 * SEC + 1, 6);
    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[1].pts, 1);
    assert_eq!(out.active_delay_ns, 40 * SEC);
    out.teardown();
}

#[test]
fn preserve_holds_even_when_not_capturing() {
    let mut out = Out::setup();
    out.delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
    out.start_capturing(SEC);

    // obs_output_signal_stop() ends the capture before it reconnects;
    // with preserve the packets are kept, not dropped
    out.send_packet_at(10 * SEC, 0);
    out.delay_capturing = false;
    out.reconnecting = true;
    let held_allocs = out.live;
    out.send_packet_at(20 * SEC, 1);
    assert_eq!(out.queued(), 2);
    assert!(out.live > held_allocs);
    assert_eq!(out.active_delay_ns, 10 * SEC);

    // reconnected and capturing again: delivered in order
    out.reconnecting = false;
    out.delay_capturing = true;
    out.send_packet_at(25 * SEC, 2);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].pts, 0);
    assert_eq!(out.queued(), 2);
    out.teardown();
}

#[test]
fn reconnecting_without_preserve_keeps_releasing() {
    let mut out = Out::setup();
    out.start_capturing(SEC);

    out.send_packet_at(10 * SEC, 0);
    out.send_packet_at(10 * SEC, 1);
    out.reconnecting = true;
    out.delay_capturing = false;

    // the delay is unchanged and due packets are dropped
    out.send_packet_at(20 * SEC, 2);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.queued(), 1);
    assert_eq!(out.active_delay_ns, SEC);
    out.teardown();
}

#[test]
fn preserve_follows_the_running_flags() {
    let mut out = Out::setup();

    // Preserve is read from delay_cur_flags, which obs-output.c copies
    // from the setting when capture is hooked; changing the setting while
    // the delay runs does not change it.
    out.set_delay(1, OBS_OUTPUT_DELAY_PRESERVE);
    out.delay_cur_flags = 0;
    out.start_capturing(SEC);
    out.reconnecting = true;
    out.send_packet_at(10 * SEC, 0);
    out.send_packet_at(12 * SEC, 1);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.active_delay_ns, SEC);

    // and the other way round
    out.set_delay(1, 0);
    out.delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
    out.send_packet_at(20 * SEC, 2);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.active_delay_ns, 8 * SEC);

    // only the preserve bit counts
    out.delay_cur_flags = !OBS_OUTPUT_DELAY_PRESERVE;
    out.send_packet_at(28 * SEC, 3);
    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[1].pts, 1);
    out.teardown();
}

// ---------------------------------------------------------------------------
// edge cases (new)

#[test]
fn pop_decision_is_strict_and_wrapping() {
    // exactly the delay: held; one more nanosecond: due
    assert_eq!(
        pop_decision(10, 10 + SEC, SEC, false, false),
        Decision::Hold
    );
    assert_eq!(pop_decision(10, 11 + SEC, SEC, false, false), Decision::Pop);
    // a zero delay holds an entry of age zero
    assert_eq!(pop_decision(5, 5, 0, false, false), Decision::Hold);
    assert_eq!(pop_decision(5, 6, 0, false, false), Decision::Pop);
    // an earlier `now` wraps to a huge age
    assert_eq!(
        pop_decision(5, 4, u64::MAX - 1, false, false),
        Decision::Pop
    );
    assert_eq!(pop_decision(5, 4, u64::MAX, false, false), Decision::Hold);
    assert_eq!(
        pop_decision(0, u64::MAX, u64::MAX - 1, false, false),
        Decision::Pop
    );
}

#[test]
fn pop_decision_extends_only_when_preserving_a_reconnect() {
    // either flag alone: the normal rule
    assert_eq!(pop_decision(0, 3 * SEC, SEC, true, false), Decision::Pop);
    assert_eq!(pop_decision(0, 3 * SEC, SEC, false, true), Decision::Pop);
    assert_eq!(pop_decision(0, SEC, 3 * SEC, true, false), Decision::Hold);
    // both: the age, whether it is longer or shorter than the delay, and
    // never a pop
    assert_eq!(
        pop_decision(0, 3 * SEC, SEC, true, true),
        Decision::Extend(3 * SEC)
    );
    assert_eq!(
        pop_decision(0, SEC, 3 * SEC, true, true),
        Decision::Extend(SEC)
    );
    assert_eq!(pop_decision(7, 7, 0, true, true), Decision::Extend(0));
    // the wrapped age too
    assert_eq!(
        pop_decision(1, 0, 0, true, true),
        Decision::Extend(u64::MAX)
    );
}

#[test]
fn active_delay_secs_truncates_then_wraps() {
    assert_eq!(active_delay_secs(0), 0);
    assert_eq!(active_delay_secs(SEC - 1), 0);
    assert_eq!(active_delay_secs(SEC), 1);
    assert_eq!(active_delay_secs(u64::from(u32::MAX) * SEC), u32::MAX);
    assert_eq!(active_delay_secs((u64::from(u32::MAX) + 1) * SEC), 0);
    assert_eq!(active_delay_secs((u64::from(u32::MAX) + 2) * SEC - 1), 0);
    assert_eq!(active_delay_secs(u64::MAX), 1_266_874_889);
}

#[test]
fn entry_ts_is_the_queued_time() {
    let p: Entry<Held, PacketTime> = Entry::Packet {
        ts: 3,
        packet: Held {
            pts: 0,
            data: vec![],
            copy: 0,
        },
        time: None,
    };
    assert_eq!(p.ts(), 3);
    assert_eq!(Entry::<Held, PacketTime>::Start { ts: 4 }.ts(), 4);
    assert_eq!(Entry::<Held, PacketTime>::Stop { ts: 5 }.ts(), 5);
}

#[test]
fn lock_is_taken_once_per_push_and_per_pass() {
    let mut out = Out::setup();
    out.start_capturing(SEC);

    // a packet into an empty queue: one lock to push, one pass that holds
    out.send_packet_at(10 * SEC, 0);
    assert_eq!(out.lock_calls, 2);

    // three due entries: one push, three passes that pop, one that holds
    out.send_packet_at(10 * SEC, 1);
    out.send_packet_at(10 * SEC, 2);
    out.lock_calls = 0;
    out.send_packet_at(20 * SEC, 3);
    assert_eq!(out.events.len(), 3);
    assert_eq!(out.lock_calls, 1 + 3 + 1);

    // start and stop lock once each; the setting, the getters and cleanup
    // never lock
    out.lock_calls = 0;
    assert!(out.delay_start());
    assert_eq!(out.lock_calls, 1);
    out.delay_stop();
    assert_eq!(out.lock_calls, 2);
    out.set_delay(3, 0);
    let _ = (out.get_delay(), out.get_active_delay());
    delay::cleanup(&mut out);
    assert_eq!(out.lock_calls, 2);
    out.teardown();
}

#[test]
fn pop_packet_on_an_empty_queue() {
    let mut out = Out::setup();
    out.start_capturing(0);
    assert!(!delay::pop_packet(&mut out, 10 * SEC));
    assert_eq!(out.lock_calls, 1);
    assert_eq!(out.events.len(), 0);

    // preserve while reconnecting with nothing queued leaves the delay
    out.delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
    out.reconnecting = true;
    out.active_delay_ns = 7;
    assert!(!delay::pop_packet(&mut out, 10 * SEC));
    assert_eq!(out.active_delay_ns, 7);
    out.teardown();
}

#[test]
fn preserve_extends_from_the_front_only() {
    let mut out = Out::setup();
    out.delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
    out.start_capturing(30 * SEC);
    out.reconnecting = true;

    // Characterized, not endorsed (follows from the strict rule): while
    // reconnecting the delay is the front's age even when that is shorter
    // than the delay that ran, so a fresh queue shrinks it.
    out.send_packet_at(100 * SEC, 0);
    assert_eq!(out.active_delay_ns, 0);
    out.send_packet_at(102 * SEC, 1);
    assert_eq!(out.active_delay_ns, 2 * SEC);
    assert_eq!(out.queued(), 2);

    // A start or stop at the front counts the same as a packet.
    delay::cleanup(&mut out);
    out.clock_ns = 200 * SEC;
    out.delay_stop();
    out.send_packet_at(205 * SEC, 2);
    assert_eq!(out.active_delay_ns, 5 * SEC);
    assert_eq!(out.count_events(Kind::ActualStop), 0);
    out.teardown();
}

#[test]
fn packet_dropped_when_inactive_reads_no_capture_flag() {
    // With the delay inactive the packet is released whatever the capture
    // flag says (the C `||` stops at the first test).
    let mut out = Out::setup();
    out.start_capturing(0);
    out.delay_active = false;
    out.delay_capturing = true;
    out.send_packet_at(SEC, 0);
    out.send_packet_at(2 * SEC, 1);
    assert_eq!(out.events.len(), 0);
    assert_eq!(out.live, 1);
    out.teardown();
}

#[test]
fn start_reads_the_clock_once_and_queues_that_time() {
    let mut out = Out::setup();
    out.clock_ns = 0;
    // the time given to start is the entry's time, also on a restart
    assert!(delay::start(&mut out, 77));
    out.start_capturing(SEC);
    assert!(delay::start(&mut out, 78));
    assert_eq!(msg_ts(out.queued_at(0)), 77);
    assert_eq!(msg_ts(out.queued_at(1)), 78);
    assert_eq!(out.restart_refs, 2);
    out.teardown();
}

#[test]
fn failed_capture_releases_packets_queued_before_the_start() {
    let mut out = Out::setup();
    out.start_capturing(10 * SEC);
    out.send_packet_at(SEC, 0);
    out.send_packet_at(SEC, 1);
    out.delay_active = false;
    out.begin_result = false;
    out.events.clear();

    // the failing start cleans up the whole queue, the packets with it,
    // without delivering any
    assert!(!out.delay_start());
    assert_eq!(out.queued(), 0);
    assert_eq!(out.live, 0);
    assert_eq!(out.count_events(Kind::Packet), 0);
    assert_eq!(out.active_delay_ns, 0);
    out.teardown();
}

#[test]
fn restart_reference_is_taken_before_capture_begins() {
    let mut out = Out::setup();
    out.begin_result = false;
    // the reference is taken, then cleared again by the cleanup
    assert!(!out.delay_start());
    assert_eq!(out.restart_refs, 0);

    out.begin_result = true;
    assert!(out.delay_start());
    assert!(out.delay_start());
    // without an active delay every start runs the checks again
    assert_eq!(out.count_events(Kind::CanBegin), 3);
    assert_eq!(out.restart_refs, 2);
    out.teardown();
}

#[test]
fn stop_on_an_empty_queue_runs_on_the_next_pass() {
    let mut out = Out::setup();
    out.start_capturing(SEC);
    out.clock_ns = 5 * SEC;
    out.delay_stop();
    assert_eq!(out.queued(), 1);
    assert!(delay::pop_packet(&mut out, 6 * SEC + 1));
    assert_eq!(out.events.len(), 2);
    assert_eq!(out.events[1].kind, Kind::ActualStop);
    assert_eq!(out.events[1].ts, 5 * SEC);
    assert!(!out.events[1].force);
    assert!(!delay::pop_packet(&mut out, 6 * SEC + 1));
    out.teardown();
}

#[test]
fn foreign_entry_is_dropped_and_counts_as_popped() {
    // An entry this file never queues (only C memory could hold one) leaves
    // the queue without running or releasing anything, and the pass goes
    // on to the next entry.
    let mut out = Out::setup();
    out.start_capturing(0);
    out.lock();
    out.queue.push_back(None);
    out.unlock();
    out.send_packet_at(SEC, 0);
    out.send_packet_at(SEC + 1, 1);
    assert_eq!(out.events.len(), 1);
    assert_eq!(out.events[0].pts, 0);
    assert_eq!(out.queued(), 1);

    // cleanup skips it too
    out.lock();
    out.queue.push_front(None);
    out.unlock();
    delay::cleanup(&mut out);
    assert_eq!(out.queued(), 0);
    out.teardown();
}

#[test]
fn set_delay_on_a_null_output_stores_nothing_and_checks_no_flag() {
    let out = Out::setup();
    delay::set_delay(None::<&mut Out>, &out, 1, 1);
    assert_eq!(out.num_logs(), 1);
    assert_eq!(out.last_log().0, LOG_DEBUG);
    assert_eq!(SET_DELAY, c"obs_output_set_delay");
    out.teardown();
}

#[test]
fn raw_output_warns_on_every_call() {
    let mut out = Out::setup();
    out.flags = 0;
    out.set_delay(1, 0);
    out.set_delay(2, 0);
    assert_eq!(out.num_logs(), 2);
    assert_eq!(out.delay_sec, 0);
    // other flag bits do not matter, only OBS_OUTPUT_ENCODED
    out.flags = !OBS_OUTPUT_ENCODED;
    out.set_delay(3, 0);
    assert_eq!(out.num_logs(), 3);
    out.flags = OBS_OUTPUT_ENCODED;
    out.set_delay(4, 0);
    assert_eq!(out.num_logs(), 3);
    assert_eq!(out.delay_sec, 4);
    out.teardown();
}
