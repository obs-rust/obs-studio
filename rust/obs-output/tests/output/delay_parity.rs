//! Tier 2 layout and Tier 3 parity: the shims against the original
//! `libobs/obs-output-delay.c.h` (oracle/output_delay.c).
//!
//! `test_output_delay` compiles the C into itself, so it never sees the
//! Rust swap; this file is what compares the two.
//!
//! Both sides run on a test output ([`Fixture`]) through the same stand-ins
//! defined here: the accessors `obs_output_delay_fields` and
//! `obs_output_delay_signal`, the `obs-output.c` calls, the packet copy and
//! release, and the delay callback. Each records its call, with whether
//! `delay_mutex` was held, into the output it was given. The C reads its
//! clock from `oracle_output_delay_clock_ns`, which the test sets; the shim
//! gets the same time through its `*_at` functions.
//!
//! Each run feeds one sequence of packets (with and without timing),
//! starts, stops, cleanups, setter and getter calls (also on a null output),
//! changes of the delay flags, stub results and clock to a C output and a
//! Rust output, and after every step compares the return value, the
//! recorded calls, the log line, the packet copies made and released, every
//! delay field, the deque's size, positions and capacity, and every queued
//! entry field by field (the packet timing only where it is valid, as the C
//! leaves it uninitialized otherwise). The runs that compare log lines swap
//! the process-wide log handler, so they run in a child process.
//!
//! Intentional differences: none. Nothing is excluded. One oracle artifact
//! is undone before comparing: `obs_output_set_delay` logs its own name
//! through `__FUNCTION__`, which in the renamed oracle reads
//! `oracle_obs_output_set_delay`.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ffi::{CStr, c_char, c_int, c_long, c_void};
use std::mem::{align_of, offset_of, size_of};
use std::ptr;

use obs_c_oracle::output_delay::{
    LAYOUT_NAMES, VALUE_NAMES, oracle_obs_output_cleanup_delay, oracle_obs_output_delay_start,
    oracle_obs_output_delay_stop, oracle_obs_output_get_active_delay, oracle_obs_output_get_delay,
    oracle_obs_output_set_delay, oracle_output_delay_layout, oracle_output_delay_mutex_create,
    oracle_output_delay_mutex_destroy, oracle_output_delay_mutex_held, oracle_output_delay_value,
    oracle_process_delay,
};
use obs_output::ffi::output_delay::{
    DELAY_MSG_PACKET, DELAY_MSG_START, DELAY_MSG_STOP, LOG_DEBUG, LOG_WARNING, delay_data,
    encoded_callback_t, encoder_packet, encoder_packet_time, obs_output_cleanup_delay,
    obs_output_delay_fields, obs_output_delay_start, obs_output_delay_start_at,
    obs_output_delay_stop, obs_output_delay_stop_at, obs_output_get_active_delay,
    obs_output_get_delay, obs_output_set_delay, process_delay, process_delay_at,
};
use obs_output::{OBS_OUTPUT_DELAY_PRESERVE, OBS_OUTPUT_ENCODED};
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use obs_util::ffi::darray::{bfree, bmalloc};
use obs_util::ffi::deque::{deque, deque_data, deque_free, deque_init};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

use super::isolated;

const SEC: u64 = 1_000_000_000;
const OBS_OUTPUT_VIDEO: u32 = 1 << 0;

// ---------------------------------------------------------------------------
// Tier 2: layouts and values

#[test]
fn values_match_c() {
    let rust: [i64; 11] = [
        DELAY_MSG_PACKET.into(),
        DELAY_MSG_START.into(),
        DELAY_MSG_STOP.into(),
        OBS_OUTPUT_ENCODED.into(),
        OBS_OUTPUT_VIDEO.into(),
        OBS_OUTPUT_DELAY_PRESERVE.into(),
        LOG_WARNING.into(),
        LOG_DEBUG.into(),
        0,
        1,
        i64::try_from(size_of::<c_long>()).unwrap(),
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_output_delay_value(i) };
        assert_eq!(value, c, "{name}");
    }
}

#[test]
fn layout_matches_c() {
    type P = encoder_packet;
    type T = encoder_packet_time;
    type D = delay_data;
    type F = obs_output_delay_fields;
    let rust = [
        size_of::<P>(),
        align_of::<P>(),
        offset_of!(P, data),
        offset_of!(P, size),
        offset_of!(P, pts),
        offset_of!(P, dts),
        offset_of!(P, timebase_num),
        offset_of!(P, timebase_den),
        offset_of!(P, r#type),
        offset_of!(P, keyframe),
        offset_of!(P, dts_usec),
        offset_of!(P, sys_dts_usec),
        offset_of!(P, priority),
        offset_of!(P, drop_priority),
        offset_of!(P, track_idx),
        offset_of!(P, encoder),
        size_of::<T>(),
        align_of::<T>(),
        offset_of!(T, pts),
        offset_of!(T, cts),
        offset_of!(T, fer),
        offset_of!(T, ferc),
        offset_of!(T, pir),
        size_of::<D>(),
        align_of::<D>(),
        offset_of!(D, msg),
        offset_of!(D, ts),
        offset_of!(D, packet),
        offset_of!(D, packet_time_valid),
        offset_of!(D, packet_time),
        size_of::<F>(),
        align_of::<F>(),
        offset_of!(F, delay_mutex),
        offset_of!(F, delay_data),
        offset_of!(F, active_delay_ns),
        offset_of!(F, delay_sec),
        offset_of!(F, delay_flags),
        offset_of!(F, delay_cur_flags),
        offset_of!(F, delay_restart_refs),
        offset_of!(F, delay_active),
        offset_of!(F, delay_capturing),
        offset_of!(F, delay_callback),
        offset_of!(F, starting),
        offset_of!(F, reconnecting),
        offset_of!(F, flags),
        offset_of!(F, name),
        size_of::<c_int>(),
        size_of::<c_int>(),
        size_of::<bool>(),
        size_of::<c_long>(),
        align_of::<c_long>(),
        size_of::<encoded_callback_t>(),
        size_of::<deque>(),
        align_of::<deque>(),
        offset_of!(deque, data),
        offset_of!(deque, size),
        offset_of!(deque, start_pos),
        offset_of!(deque, end_pos),
        offset_of!(deque, capacity),
        size_of::<*mut c_void>(),
    ];
    for (i, (name, value)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter; `i` is in range.
        let c = unsafe { oracle_output_delay_layout(i) };
        assert_eq!(value, c, "{name}");
    }
}

// ---------------------------------------------------------------------------
// the test output and the stand-ins both sides call

/// What a stand-in recorded, with whether `delay_mutex` was held.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    CanBegin {
        locked: bool,
    },
    InitializeEncoders {
        locked: bool,
        starting: bool,
    },
    BeginCapture {
        locked: bool,
    },
    ActualStart {
        locked: bool,
    },
    ActualStop {
        locked: bool,
        force: bool,
        ts: u64,
    },
    Packet {
        locked: bool,
        packet: PacketView,
        time: Option<encoder_packet_time>,
    },
    Signal {
        locked: bool,
        name: String,
    },
}

/// A packet's fields and data, and whether the data is a held copy.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PacketView {
    bytes: Vec<u8>,
    held_copy: bool,
    size: usize,
    pts: i64,
    dts: i64,
    timebase: (i32, i32),
    kind: c_int,
    keyframe: bool,
    dts_usec: i64,
    sys_dts_usec: i64,
    priority: (c_int, c_int),
    track_idx: usize,
    encoder: usize,
}

impl PacketView {
    /// # Safety
    ///
    /// `p.data` is null or valid for `p.size` bytes.
    unsafe fn of(p: &encoder_packet) -> Self {
        let bytes = if p.data.is_null() || p.size == 0 {
            Vec::new()
        } else {
            // SAFETY: caller contract.
            unsafe { std::slice::from_raw_parts(p.data, p.size) }.to_vec()
        };
        Self {
            bytes,
            held_copy: LIVE.with(|l| l.borrow().contains(&(p.data as usize))),
            size: p.size,
            pts: p.pts,
            dts: p.dts,
            timebase: (p.timebase_num, p.timebase_den),
            kind: p.r#type,
            keyframe: p.keyframe,
            dts_usec: p.dts_usec,
            sys_dts_usec: p.sys_dts_usec,
            priority: (p.priority, p.drop_priority),
            track_idx: p.track_idx,
            encoder: p.encoder as usize,
        }
    }
}

/// The fields of `struct obs_output` the delay reads, the stub results,
/// and the recorded calls.
#[derive(Debug)]
struct Fixture {
    mutex: *mut c_void,
    queue: deque,
    active_delay_ns: u64,
    delay_sec: u32,
    delay_flags: u32,
    delay_cur_flags: u32,
    restart_refs: c_long,
    delay_active: bool,
    delay_capturing: bool,
    callback: encoded_callback_t,
    starting: bool,
    reconnecting: bool,
    flags: u32,
    name: *mut c_char,
    can_begin: bool,
    initialize: bool,
    begin: bool,
    actual_start: bool,
    events: Vec<Event>,
}

static NAME: &CStr = c"delayed output";

impl Fixture {
    /// The C test's `setup()`.
    fn new() -> Box<Self> {
        // SAFETY: creates a mutex for this output alone.
        let mutex = unsafe { oracle_output_delay_mutex_create() };
        assert!(!mutex.is_null());
        let mut f = Box::new(Self {
            mutex,
            queue: deque {
                data: ptr::null_mut(),
                size: 0,
                start_pos: 0,
                end_pos: 0,
                capacity: 0,
            },
            active_delay_ns: 0,
            delay_sec: 0,
            delay_flags: 0,
            delay_cur_flags: 0,
            restart_refs: 0,
            delay_active: false,
            delay_capturing: false,
            callback: Some(delayed_packet),
            starting: false,
            reconnecting: false,
            flags: OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO,
            name: NAME.as_ptr().cast_mut(),
            can_begin: true,
            initialize: true,
            begin: true,
            actual_start: true,
            events: Vec::new(),
        });
        // SAFETY: a writable deque.
        unsafe { deque_init(&raw mut f.queue) };
        f
    }

    fn ptr(&mut self) -> *mut c_void {
        ptr::from_mut(self).cast()
    }

    fn locked(&self) -> bool {
        // SAFETY: this output's mutex.
        unsafe { oracle_output_delay_mutex_held(self.mutex) }
    }

    /// The queued entries, field by field.
    fn entries(&mut self) -> Vec<QueuedEntry> {
        let sz = size_of::<delay_data>();
        assert_eq!(self.queue.size % sz, 0, "whole entries");
        (0..self.queue.size / sz)
            .map(|i| {
                // An entry may wrap around the end of the buffer, so it is
                // read a byte at a time from the front.
                let bytes: Vec<u8> = (0..sz)
                    .map(|k| {
                        // SAFETY: `i * sz + k` is below the size.
                        unsafe { *deque_data(&raw mut self.queue, i * sz + k).cast::<u8>() }
                    })
                    .collect();
                // SAFETY: the bytes of one `struct delay_data`, written by
                // C or Rust (its bools are 0 or 1).
                let dd: delay_data = unsafe { ptr::read_unaligned(bytes.as_ptr().cast()) };
                QueuedEntry {
                    msg: dd.msg,
                    ts: dd.ts,
                    // SAFETY: a held copy of `size` bytes, or zeroed.
                    packet: unsafe { PacketView::of(&dd.packet) },
                    time: dd.packet_time_valid.then_some(dd.packet_time),
                }
            })
            .collect()
    }

    fn snapshot(&mut self, ret: Ret, live_delta: i64, log: LogLine) -> Snapshot {
        Snapshot {
            ret,
            events: std::mem::take(&mut self.events),
            entries: self.entries(),
            deque: (
                self.queue.size,
                self.queue.start_pos,
                self.queue.end_pos,
                self.queue.capacity,
            ),
            fields: Fields {
                active_delay_ns: self.active_delay_ns,
                delay_sec: self.delay_sec,
                delay_flags: self.delay_flags,
                delay_cur_flags: self.delay_cur_flags,
                restart_refs: self.restart_refs,
                delay_active: self.delay_active,
                delay_capturing: self.delay_capturing,
                starting: self.starting,
                reconnecting: self.reconnecting,
                flags: self.flags,
            },
            live_delta,
            log,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The C test's teardown, without its leak check (`live_delta` per
        // step is the check): release what is held, free the queue.
        let sz = size_of::<delay_data>();
        while self.queue.size >= sz {
            let mut dd = delay_data::default();
            // SAFETY: a valid deque of whole entries.
            unsafe {
                obs_util::ffi::deque::deque_pop_front(
                    &raw mut self.queue,
                    (&raw mut dd).cast(),
                    sz,
                );
            }
            if dd.msg == DELAY_MSG_PACKET {
                // SAFETY: a held copy.
                unsafe { obs_encoder_packet_release(&raw mut dd.packet) };
            }
        }
        // SAFETY: the deque and the mutex are this output's.
        unsafe {
            deque_free(&raw mut self.queue);
            oracle_output_delay_mutex_destroy(self.mutex);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueuedEntry {
    msg: c_int,
    ts: u64,
    packet: PacketView,
    time: Option<encoder_packet_time>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fields {
    active_delay_ns: u64,
    delay_sec: u32,
    delay_flags: u32,
    delay_cur_flags: u32,
    restart_refs: c_long,
    delay_active: bool,
    delay_capturing: bool,
    starting: bool,
    reconnecting: bool,
    flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ret {
    None,
    Bool(bool),
    U32(u32),
}

/// Lines logged by the step, then the level and text of the last one.
type LogLine = (c_int, c_int, String);

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    ret: Ret,
    events: Vec<Event>,
    entries: Vec<QueuedEntry>,
    deque: (usize, usize, usize, usize),
    fields: Fields,
    live_delta: i64,
    log: LogLine,
}

thread_local! {
    /// `oracle_output_delay_clock_ns`.
    static CLOCK: Cell<u64> = const { Cell::new(0) };
    /// Data pointers of the held copies not yet released.
    static LIVE: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

fn live() -> i64 {
    LIVE.with(|l| i64::try_from(l.borrow().len()).unwrap())
}

/// The C file's `os_gettime_ns`.
#[unsafe(no_mangle)]
pub extern "C" fn oracle_output_delay_clock_ns() -> u64 {
    CLOCK.with(Cell::get)
}

/// # Safety
///
/// `output` is a live [`Fixture`].
unsafe fn fixture<'a>(output: *const c_void) -> &'a mut Fixture {
    // SAFETY: caller contract; the call is the only one using it.
    unsafe { &mut *output.cast::<Fixture>().cast_mut() }
}

/// libobs's `obs_output_delay_fields`, over a [`Fixture`].
///
/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_delay_fields(output: *const c_void) -> obs_output_delay_fields {
    let o = output.cast::<Fixture>().cast_mut();
    // SAFETY: field addresses of a live fixture.
    unsafe {
        obs_output_delay_fields {
            delay_mutex: (*o).mutex,
            delay_data: &raw mut (*o).queue,
            active_delay_ns: &raw mut (*o).active_delay_ns,
            delay_sec: &raw mut (*o).delay_sec,
            delay_flags: &raw mut (*o).delay_flags,
            delay_cur_flags: &raw mut (*o).delay_cur_flags,
            delay_restart_refs: &raw mut (*o).restart_refs,
            delay_active: &raw mut (*o).delay_active,
            delay_capturing: &raw mut (*o).delay_capturing,
            delay_callback: &raw mut (*o).callback,
            starting: &raw mut (*o).starting,
            reconnecting: &raw mut (*o).reconnecting,
            flags: &raw const (*o).flags,
            name: &raw const (*o).name,
        }
    }
}

/// libobs's `obs_output_delay_signal`, recorded.
///
/// # Safety
///
/// `output` is a live [`Fixture`], `signal` a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_delay_signal(output: *mut c_void, signal: *const c_char) {
    // SAFETY: caller contract.
    let (o, name) = unsafe { (fixture(output), CStr::from_ptr(signal)) };
    let locked = o.locked();
    o.events.push(Event::Signal {
        locked,
        name: name.to_string_lossy().into_owned(),
    });
}

/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_can_begin_data_capture(
    output: *const c_void,
    flags: u32,
) -> bool {
    assert_eq!(flags, 0);
    // SAFETY: caller contract.
    let o = unsafe { fixture(output) };
    let locked = o.locked();
    o.events.push(Event::CanBegin { locked });
    o.can_begin
}

/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_initialize_encoders(output: *mut c_void, flags: u32) -> bool {
    assert_eq!(flags, 0);
    // SAFETY: caller contract.
    let o = unsafe { fixture(output) };
    let locked = o.locked();
    let starting = o.starting;
    o.events
        .push(Event::InitializeEncoders { locked, starting });
    o.initialize
}

/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_begin_data_capture(output: *mut c_void, flags: u32) -> bool {
    assert_eq!(flags, 0);
    // SAFETY: caller contract.
    let o = unsafe { fixture(output) };
    let locked = o.locked();
    o.events.push(Event::BeginCapture { locked });
    o.begin
}

/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_actual_start(output: *mut c_void) -> bool {
    // SAFETY: caller contract.
    let o = unsafe { fixture(output) };
    let locked = o.locked();
    o.events.push(Event::ActualStart { locked });
    o.actual_start
}

/// # Safety
///
/// `output` is a live [`Fixture`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_actual_stop(output: *mut c_void, force: bool, ts: u64) {
    // SAFETY: caller contract.
    let o = unsafe { fixture(output) };
    let locked = o.locked();
    o.events.push(Event::ActualStop { locked, force, ts });
}

/// The reference-counted copy of `obs-encoder.c` (hidden in libobs), whose
/// data is tracked so a delivered packet can be told from the encoder's.
///
/// # Safety
///
/// `dst` is writable, `src` a valid packet of `size` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_packet_create_instance(
    dst: *mut encoder_packet,
    src: *const encoder_packet,
) {
    // SAFETY: caller contract; the block holds the count and the data.
    unsafe {
        *dst = *src;
        let size = (*src).size;
        let refs = bmalloc(size + size_of::<c_long>()).cast::<c_long>();
        refs.write(1);
        let data = refs.add(1).cast::<u8>();
        if size > 0 {
            ptr::copy_nonoverlapping((*src).data, data, size);
        }
        (*dst).data = data;
        LIVE.with(|l| l.borrow_mut().insert(data as usize));
    }
}

/// `obs_encoder_packet_release` of `obs-encoder.c`.
///
/// # Safety
///
/// `pkt` is null or a packet whose data is null or a held copy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_encoder_packet_release(pkt: *mut encoder_packet) {
    if pkt.is_null() {
        return;
    }
    // SAFETY: caller contract.
    unsafe {
        let data = (*pkt).data;
        if !data.is_null() {
            let refs = data.cast::<c_long>().sub(1);
            *refs -= 1;
            if *refs == 0 {
                LIVE.with(|l| l.borrow_mut().remove(&(data as usize)));
                bfree(refs.cast());
            }
        }
        *pkt = encoder_packet::default();
    }
}

/// The delay callback: `delayed_packet()` of the C test. It owns the
/// packet.
unsafe extern "C" fn delayed_packet(
    data: *mut c_void,
    packet: *mut encoder_packet,
    packet_time: *mut encoder_packet_time,
) {
    // SAFETY: the output the delay runs on, and the packet it hands over.
    unsafe {
        let o = fixture(data);
        let locked = o.locked();
        let view = PacketView::of(&*packet);
        o.events.push(Event::Packet {
            locked,
            packet: view,
            time: packet_time.as_ref().copied(),
        });
        obs_encoder_packet_release(packet);
    }
}

// ---------------------------------------------------------------------------
// log capture (only in a child process)

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
    fn base_test_record_any_thread();
    fn os_gettime_ns() -> u64;
}

/// The process-wide log handler, pointed at a record while it lives.
struct Capture {
    rec: Box<Rec>,
    saved: (*mut c_void, *mut c_void),
}

impl Capture {
    fn arm() -> Self {
        let mut saved = (ptr::null_mut(), ptr::null_mut());
        let mut rec = Box::new(Rec {
            calls: 0,
            level: -1,
            message: [0; 256],
        });
        // SAFETY: the record outlives the handler (restored in drop).
        unsafe {
            base_get_log_handler(&raw mut saved.0, &raw mut saved.1);
            base_test_record_only_this_thread();
            base_set_log_handler(base_test_record_ptr(), (&raw mut *rec).cast());
        }
        Self { rec, saved }
    }

    fn take(&mut self) -> LogLine {
        let bytes: Vec<u8> = self.rec.message.iter().map(|&c| c as u8).collect();
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let line = (
            self.rec.calls,
            self.rec.level,
            String::from_utf8_lossy(&bytes[..end]).into_owned(),
        );
        self.rec.calls = 0;
        self.rec.level = -1;
        self.rec.message[0] = 0;
        line
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: the handler read in `arm`.
        unsafe {
            base_set_log_handler(self.saved.0, self.saved.1);
            base_test_record_any_thread();
        }
    }
}

// ---------------------------------------------------------------------------
// the steps

/// What an encoder hands the output.
#[derive(Debug, Clone)]
struct Src {
    bytes: Vec<u8>,
    pts: i64,
    dts: i64,
    timebase: (i32, i32),
    kind: c_int,
    keyframe: bool,
    dts_usec: i64,
    sys_dts_usec: i64,
    priority: (c_int, c_int),
    track_idx: usize,
    encoder: usize,
}

impl Src {
    fn simple(pts: i64) -> Self {
        Self {
            bytes: vec![0xA0_u8.wrapping_add(pts as u8), 1, 2, 3],
            pts,
            dts: pts,
            timebase: (1, 30),
            kind: 1,
            keyframe: pts == 0,
            dts_usec: 0,
            sys_dts_usec: 0,
            priority: (0, 0),
            track_idx: 0,
            encoder: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum Op {
    /// Advance the clock by `dt` (wrapping), then `process_delay`.
    Packet {
        dt: u64,
        src: Src,
        time: Option<encoder_packet_time>,
    },
    Start {
        dt: u64,
    },
    Stop {
        dt: u64,
    },
    Cleanup,
    /// Move the clock back.
    Rewind(u64),
    SetDelay {
        null: bool,
        sec: u32,
        flags: u32,
    },
    GetDelay {
        null: bool,
    },
    GetActiveDelay {
        null: bool,
    },
    SetActive(bool),
    SetCapturing(bool),
    SetReconnecting(bool),
    SetCurFlags(u32),
    SetFlags(u32),
    SetActiveDelayNs(u64),
    SetRestartRefs(i32),
    Results {
        can_begin: bool,
        initialize: bool,
        begin: bool,
        actual_start: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    C,
    Rust,
}

/// Runs one step on one side's output at time `now`.
fn run_op(side: Side, f: &mut Fixture, op: &Op, now: u64) -> Ret {
    CLOCK.with(|c| c.set(now));
    let o = f.ptr();
    let target = |null: bool| if null { ptr::null_mut() } else { o };
    // SAFETY: `o` is a live fixture (or null where the C allows it); the
    // packets and times are valid for the calls.
    unsafe {
        match op {
            Op::Packet { src, time, .. } => {
                let mut bytes = src.bytes.clone();
                let mut pkt = encoder_packet {
                    data: bytes.as_mut_ptr(),
                    size: bytes.len(),
                    pts: src.pts,
                    dts: src.dts,
                    timebase_num: src.timebase.0,
                    timebase_den: src.timebase.1,
                    r#type: src.kind,
                    keyframe: src.keyframe,
                    dts_usec: src.dts_usec,
                    sys_dts_usec: src.sys_dts_usec,
                    priority: src.priority.0,
                    drop_priority: src.priority.1,
                    track_idx: src.track_idx,
                    encoder: ptr::without_provenance_mut(src.encoder),
                };
                let mut t = time.unwrap_or_default();
                let t_ptr = if time.is_some() {
                    &raw mut t
                } else {
                    ptr::null_mut()
                };
                match side {
                    Side::C => oracle_process_delay(o, (&raw mut pkt).cast(), t_ptr.cast()),
                    Side::Rust => process_delay_at(o, &raw mut pkt, t_ptr, now),
                }
                // the caller's packet is copied, not taken
                assert_eq!(pkt.data, bytes.as_mut_ptr());
                assert_eq!(bytes, src.bytes);
                Ret::None
            }
            Op::Start { .. } => Ret::Bool(match side {
                Side::C => oracle_obs_output_delay_start(o),
                Side::Rust => obs_output_delay_start_at(o, now),
            }),
            Op::Stop { .. } => {
                match side {
                    Side::C => oracle_obs_output_delay_stop(o),
                    Side::Rust => obs_output_delay_stop_at(o, now),
                }
                Ret::None
            }
            Op::Cleanup => {
                match side {
                    Side::C => oracle_obs_output_cleanup_delay(o),
                    Side::Rust => obs_output_cleanup_delay(o),
                }
                Ret::None
            }
            Op::SetDelay { null, sec, flags } => {
                match side {
                    Side::C => oracle_obs_output_set_delay(target(*null), *sec, *flags),
                    Side::Rust => obs_output_set_delay(target(*null), *sec, *flags),
                }
                Ret::None
            }
            Op::GetDelay { null } => Ret::U32(match side {
                Side::C => oracle_obs_output_get_delay(target(*null)),
                Side::Rust => obs_output_get_delay(target(*null)),
            }),
            Op::GetActiveDelay { null } => Ret::U32(match side {
                Side::C => oracle_obs_output_get_active_delay(target(*null)),
                Side::Rust => obs_output_get_active_delay(target(*null)),
            }),
            Op::Rewind(_) => Ret::None,
            Op::SetActive(v) => {
                f.delay_active = *v;
                Ret::None
            }
            Op::SetCapturing(v) => {
                f.delay_capturing = *v;
                Ret::None
            }
            Op::SetReconnecting(v) => {
                f.reconnecting = *v;
                Ret::None
            }
            Op::SetCurFlags(v) => {
                f.delay_cur_flags = *v;
                Ret::None
            }
            Op::SetFlags(v) => {
                f.flags = *v;
                Ret::None
            }
            Op::SetActiveDelayNs(v) => {
                f.active_delay_ns = *v;
                Ret::None
            }
            Op::SetRestartRefs(v) => {
                f.restart_refs = c_long::from(*v);
                Ret::None
            }
            Op::Results {
                can_begin,
                initialize,
                begin,
                actual_start,
            } => {
                f.can_begin = *can_begin;
                f.initialize = *initialize;
                f.begin = *begin;
                f.actual_start = *actual_start;
                Ret::None
            }
        }
    }
}

/// Runs `ops` on a C output and a Rust output, comparing after every step.
fn run_sequence(ops: &[Op], mut log: Option<&mut Capture>) {
    let mut c = Fixture::new();
    let mut rust = Fixture::new();
    let mut now: u64 = 1000 * SEC;
    for (i, op) in ops.iter().enumerate() {
        match op {
            Op::Packet { dt, .. } | Op::Start { dt } | Op::Stop { dt } => {
                now = now.wrapping_add(*dt);
            }
            Op::Rewind(dt) => now = now.wrapping_sub(*dt),
            _ => {}
        }
        let mut side = |side: Side, f: &mut Fixture| {
            let before = live();
            let ret = run_op(side, f, op, now);
            let mut line = log.as_mut().map_or((0, -1, String::new()), |l| l.take());
            if side == Side::C {
                // The oracle's renamed setter names itself through
                // __FUNCTION__; libobs's says obs_output_set_delay.
                line.2 = line
                    .2
                    .replace("oracle_obs_output_set_delay", "obs_output_set_delay");
            }
            f.snapshot(ret, live() - before, line)
        };
        let c_snap = side(Side::C, &mut c);
        let rust_snap = side(Side::Rust, &mut rust);
        assert_eq!(
            c_snap, rust_snap,
            "step {i} ({op:?}) of {ops:#?}\nC (left) and Rust (right) differ"
        );
    }
}

fn packet(dt: u64, pts: i64) -> Op {
    Op::Packet {
        dt,
        src: Src::simple(pts),
        time: None,
    }
}

/// The characterization scenarios of `test_output_delay`, the six
/// "characterized, not endorsed" behaviours among them, compared step by
/// step with the C.
fn fixed_sequences() -> Vec<Vec<Op>> {
    let timed = Op::Packet {
        dt: SEC,
        src: Src::simple(7),
        time: Some(encoder_packet_time {
            pts: 7,
            cts: 11,
            fer: 13,
            ferc: 17,
            pir: 19,
        }),
    };
    vec![
        // set and get; flags stored as given
        vec![
            Op::SetDelay {
                null: false,
                sec: 20,
                flags: OBS_OUTPUT_DELAY_PRESERVE,
            },
            Op::GetDelay { null: false },
            Op::GetActiveDelay { null: false },
            Op::SetDelay {
                null: false,
                sec: u32::MAX,
                flags: 0xF0,
            },
            Op::GetDelay { null: false },
        ],
        // raw output warns and stores nothing
        vec![
            Op::SetFlags(OBS_OUTPUT_VIDEO),
            Op::SetDelay {
                null: false,
                sec: 30,
                flags: 0,
            },
            Op::GetDelay { null: false },
        ],
        // null output: the getters log under the setter's name
        vec![
            Op::SetDelay {
                null: true,
                sec: 10,
                flags: 0,
            },
            Op::GetDelay { null: true },
            Op::GetActiveDelay { null: true },
        ],
        // whole seconds, and the cast wrapping past u32::MAX seconds
        vec![
            Op::SetActiveDelayNs(2 * SEC + SEC / 2),
            Op::GetActiveDelay { null: false },
            Op::SetActiveDelayNs(u64::MAX),
            Op::GetActiveDelay { null: false },
        ],
        // held while age == delay, released past it, in order
        vec![
            Op::SetActiveDelayNs(SEC),
            Op::SetActive(true),
            Op::SetCapturing(true),
            packet(0, 0),
            packet(SEC / 2, 1),
            packet(SEC / 2, 2),
            packet(1, 3),
            timed,
            packet(10 * SEC, 4),
        ],
        // a zero delay still waits for a later packet
        vec![
            Op::SetActive(true),
            Op::SetCapturing(true),
            packet(0, 0),
            packet(0, 1),
            packet(1, 2),
        ],
        // a clock going backwards releases at once
        vec![
            Op::SetActiveDelayNs(30 * SEC),
            Op::SetActive(true),
            Op::SetCapturing(true),
            packet(0, 0),
            Op::Rewind(1),
            packet(0, 1),
        ],
        // first start, refused start, failed encoders, failed capture
        // (which leaves `starting` set)
        vec![
            Op::SetDelay {
                null: false,
                sec: 1,
                flags: 0,
            },
            Op::Start { dt: 0 },
            Op::Cleanup,
            Op::Results {
                can_begin: false,
                initialize: true,
                begin: true,
                actual_start: true,
            },
            Op::Start { dt: 1 },
            Op::Results {
                can_begin: true,
                initialize: false,
                begin: true,
                actual_start: true,
            },
            Op::Start { dt: 1 },
            Op::SetActiveDelayNs(SEC),
            Op::Results {
                can_begin: true,
                initialize: true,
                begin: false,
                actual_start: true,
            },
            Op::Start { dt: 1 },
        ],
        // restart while active, the queued start and stop running later
        vec![
            Op::SetActiveDelayNs(2 * SEC),
            Op::SetActive(true),
            Op::SetCapturing(true),
            Op::Start { dt: 0 },
            packet(1, 0),
            Op::Stop { dt: SEC },
            packet(SEC, 1),
            Op::Start { dt: 0 },
            packet(SEC + 2, 2),
            Op::SetActive(false),
            packet(10 * SEC, 3),
            Op::Cleanup,
        ],
        // preserve while reconnecting grows the delay and keeps it
        vec![
            Op::SetCurFlags(OBS_OUTPUT_DELAY_PRESERVE),
            Op::SetActiveDelayNs(10 * SEC),
            Op::SetActive(true),
            Op::SetCapturing(true),
            packet(0, 0),
            packet(5 * SEC, 1),
            Op::SetReconnecting(true),
            packet(10 * SEC, 2),
            packet(25 * SEC, 3),
            Op::GetActiveDelay { null: false },
            Op::SetReconnecting(false),
            packet(0, 4),
            packet(1, 5),
            packet(5 * SEC, 6),
            Op::SetCurFlags(!OBS_OUTPUT_DELAY_PRESERVE),
            Op::SetReconnecting(true),
            packet(60 * SEC, 7),
        ],
    ]
}

#[test]
fn parity_fixed_sequences() {
    if !isolated(concat!(module_path!(), "::parity_fixed_sequences")) {
        return;
    }
    let mut log = Capture::arm();
    for ops in fixed_sequences() {
        run_sequence(&ops, Some(&mut log));
    }
}

fn dt() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => Just(0),
        2 => Just(1),
        4 => 0..3 * SEC,
        2 => Just(SEC),
        2 => Just(SEC + 1),
        1 => Just(10 * SEC),
        1 => any::<u64>(),
    ]
}

fn delay_ns() -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(0),
        Just(1),
        Just(SEC),
        0..5 * SEC,
        Just(u64::MAX),
        any::<u64>(),
    ]
}

fn flag_bits() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => Just(OBS_OUTPUT_DELAY_PRESERVE),
        2 => Just(0),
        1 => any::<u32>(),
    ]
}

fn src() -> impl Strategy<Value = Src> {
    (
        proptest::collection::vec(any::<u8>(), 0..8),
        any::<i64>(),
        any::<i64>(),
        (any::<i32>(), any::<i32>()),
        0..2_i32,
        any::<bool>(),
        (any::<i64>(), any::<i64>()),
        (any::<i32>(), any::<i32>()),
        0..6_usize,
        prop_oneof![Just(0_usize), (1..16_usize).prop_map(|n| n * 0x1000)],
    )
        .prop_map(
            |(bytes, pts, dts, timebase, kind, keyframe, usec, priority, track_idx, encoder)| Src {
                bytes,
                pts,
                dts,
                timebase,
                kind,
                keyframe,
                dts_usec: usec.0,
                sys_dts_usec: usec.1,
                priority,
                track_idx,
                encoder,
            },
        )
}

fn packet_time() -> impl Strategy<Value = Option<encoder_packet_time>> {
    proptest::option::of(
        (
            any::<i64>(),
            any::<u64>(),
            any::<u64>(),
            any::<u64>(),
            any::<u64>(),
        )
            .prop_map(|(pts, cts, fer, ferc, pir)| encoder_packet_time {
                pts,
                cts,
                fer,
                ferc,
                pir,
            }),
    )
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        12 => (dt(), src(), packet_time()).prop_map(|(dt, src, time)| Op::Packet { dt, src, time }),
        2 => dt().prop_map(|dt| Op::Start { dt }),
        2 => dt().prop_map(|dt| Op::Stop { dt }),
        1 => Just(Op::Cleanup),
        1 => prop_oneof![Just(1_u64), 0..10 * SEC].prop_map(Op::Rewind),
        1 => (prop::bool::weighted(0.2), any::<u32>(), flag_bits())
            .prop_map(|(null, sec, flags)| Op::SetDelay { null, sec, flags }),
        1 => prop::bool::weighted(0.2).prop_map(|null| Op::GetDelay { null }),
        1 => prop::bool::weighted(0.2).prop_map(|null| Op::GetActiveDelay { null }),
        2 => any::<bool>().prop_map(Op::SetActive),
        2 => any::<bool>().prop_map(Op::SetCapturing),
        2 => any::<bool>().prop_map(Op::SetReconnecting),
        2 => flag_bits().prop_map(Op::SetCurFlags),
        1 => prop_oneof![
            3 => Just(OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO),
            1 => Just(OBS_OUTPUT_VIDEO),
            1 => any::<u32>(),
        ]
        .prop_map(Op::SetFlags),
        2 => delay_ns().prop_map(Op::SetActiveDelayNs),
        1 => (-2..4_i32).prop_map(Op::SetRestartRefs),
        1 => (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()).prop_map(
            |(can_begin, initialize, begin, actual_start)| Op::Results {
                can_begin,
                initialize,
                begin,
                actual_start,
            }
        ),
    ]
}

/// Starts most runs with the delay running and capturing, as
/// `hook_data_capture()` leaves it, so packets flow.
fn sequence() -> impl Strategy<Value = Vec<Op>> {
    (
        any::<bool>(),
        delay_ns(),
        proptest::collection::vec(op(), 1..48),
    )
        .prop_map(|(capturing, delay, mut ops)| {
            if capturing {
                let mut head = vec![
                    Op::SetActiveDelayNs(delay % (4 * SEC)),
                    Op::SetActive(true),
                    Op::SetCapturing(true),
                ];
                head.append(&mut ops);
                head
            } else {
                ops
            }
        })
}

#[test]
fn parity_random_sequences() {
    if !isolated(concat!(module_path!(), "::parity_random_sequences")) {
        return;
    }
    let log = RefCell::new(Capture::arm());
    let mut runner = TestRunner::new(Config {
        cases: 1024,
        failure_persistence: None,
        ..Config::default()
    });
    runner
        .run(&sequence(), |ops| {
            run_sequence(&ops, Some(&mut log.borrow_mut()));
            Ok(())
        })
        .unwrap();
}

/// The exported entry points read `os_gettime_ns()` once and pass it on:
/// the queued entries carry a time from the real clock.
#[test]
fn exported_entry_points_read_the_clock() {
    let mut f = Fixture::new();
    let o = f.ptr();
    // SAFETY: a live fixture and a valid packet.
    let (before, after) = unsafe {
        let before = os_gettime_ns();
        assert!(obs_output_delay_start(o));
        let mut bytes = [1_u8, 2, 3];
        let mut pkt = encoder_packet {
            data: bytes.as_mut_ptr(),
            size: bytes.len(),
            ..encoder_packet::default()
        };
        f.active_delay_ns = u64::MAX;
        process_delay(o, &raw mut pkt, ptr::null_mut());
        obs_output_delay_stop(o);
        (before, os_gettime_ns())
    };
    let entries = f.entries();
    assert_eq!(
        entries.iter().map(|e| e.msg).collect::<Vec<_>>(),
        [DELAY_MSG_START, DELAY_MSG_PACKET, DELAY_MSG_STOP]
    );
    for e in &entries {
        assert!(
            (before..=after).contains(&e.ts),
            "{} in {before}..={after}",
            e.ts
        );
    }
    assert!(entries[0].ts <= entries[1].ts && entries[1].ts <= entries[2].ts);
    assert!(entries[1].packet.held_copy);
    assert_eq!(entries[1].packet.bytes, [1, 2, 3]);
    let events = std::mem::take(&mut f.events);
    assert_eq!(
        events,
        [
            Event::CanBegin { locked: false },
            Event::InitializeEncoders {
                locked: false,
                starting: true
            },
            Event::BeginCapture { locked: false },
            Event::Signal {
                locked: false,
                name: "stopping".into()
            },
        ]
    );
    // SAFETY: a live fixture.
    unsafe { obs_output_cleanup_delay(o) };
    assert!(f.entries().is_empty());
}
