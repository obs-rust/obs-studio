//! Safe core of `libobs/obs-output-delay.c.h`: the stream delay.
//!
//! An output with a delay set (`obs_output_set_delay`) does not send the
//! packets its encoders hand it. `process_delay`, installed as the encoded
//! packet callback, copies each packet into a queue with the time it
//! arrived and releases the entries at the front once they are older than
//! the running delay. The start and the stop of the output are queued the
//! same way, so the real start and stop happen when the stream reaches
//! them. With `OBS_OUTPUT_DELAY_PRESERVE`, a reconnect holds the queue and
//! grows the delay to the age of the oldest entry, so the stream picks up
//! where it was cut.
//!
//! The functions are generic over the output ([`DelayOutput`]): its fields,
//! the queue, the delay lock and the libobs calls the C makes. The C ABI
//! shim runs them on the real `struct obs_output` through libobs's hidden
//! accessors, taking `delay_mutex` exactly where the C takes it; tests run
//! them on a model. The clock is a parameter: the C reads
//! `os_gettime_ns()` once per call, and so does the shim.
//!
//! Six behaviours are characterized, not endorsed (`test_output_delay`),
//! and kept: the getters log a null output under the setter's name; the
//! whole-seconds getter wraps past `u32::MAX` seconds; an entry is held
//! while its age equals the delay, so a zero delay still waits for a later
//! packet; a clock that goes backwards wraps the age and releases at once;
//! a start whose data capture fails leaves `starting` set; and a delay
//! grown while reconnecting never shrinks back.

use core::ffi::CStr;

/// `OBS_OUTPUT_ENCODED` (`obs-output.h`): the output takes encoded packets.
pub const OBS_OUTPUT_ENCODED: u32 = 1 << 2;

/// `OBS_OUTPUT_DELAY_PRESERVE` (`obs.h`): hold the queue while reconnecting.
pub const OBS_OUTPUT_DELAY_PRESERVE: u32 = 1 << 0;

/// Nanoseconds per second.
pub const NS_PER_SEC: u64 = 1_000_000_000;

/// The function name the setter's checks log with, which both getters use
/// too (characterized, not endorsed).
pub const SET_DELAY: &CStr = c"obs_output_set_delay";

/// What the front of the queue does on one pass (`pop_packet`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Not due yet: stays queued.
    Hold,
    /// Due: leaves the queue and runs.
    Pop,
    /// Preserving through a reconnect: stays queued, and the running delay
    /// becomes this age of the entry, in nanoseconds.
    Extend(u64),
}

/// The decision for the front entry, queued at `front_ts`, at time `now`.
///
/// The age is `now - front_ts` with wrapping (an unsigned difference in C),
/// and an entry is due only once its age is strictly greater than
/// `active_delay_ns`. While `preserve && reconnecting` nothing is due and
/// the delay follows the age.
#[must_use]
pub fn pop_decision(
    front_ts: u64,
    now: u64,
    active_delay_ns: u64,
    preserve: bool,
    reconnecting: bool,
) -> Decision {
    let elapsed = now.wrapping_sub(front_ts);
    if preserve && reconnecting {
        Decision::Extend(elapsed)
    } else if elapsed > active_delay_ns {
        Decision::Pop
    } else {
        Decision::Hold
    }
}

/// `obs_output_get_active_delay`: the running delay in whole seconds,
/// truncated, then cast to `u32` (which wraps past `u32::MAX` seconds).
#[must_use]
pub fn active_delay_secs(active_delay_ns: u64) -> u32 {
    // The truncating cast is the C's (characterized, not endorsed).
    (active_delay_ns / NS_PER_SEC) as u32
}

/// One queued entry (`struct delay_data`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry<P, T> {
    /// `DELAY_MSG_PACKET`: a held copy of a packet and its timing, if the
    /// encoder gave one.
    Packet { ts: u64, packet: P, time: Option<T> },
    /// `DELAY_MSG_START`: the output's real start.
    Start { ts: u64 },
    /// `DELAY_MSG_STOP`: the output's real stop, at the time it was asked
    /// for.
    Stop { ts: u64 },
}

impl<P, T> Entry<P, T> {
    /// When the entry was queued.
    #[must_use]
    pub fn ts(&self) -> u64 {
        match *self {
            Self::Packet { ts, .. } | Self::Start { ts } | Self::Stop { ts } => ts,
        }
    }
}

/// The output signals the delay emits itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// `"starting"`, on a restart while the delay still runs.
    Starting,
    /// `"stopping"`, when a stop is queued.
    Stopping,
}

/// An output the delay runs on: the delay fields of `struct obs_output`,
/// its queue and lock, and the libobs calls the C file makes.
///
/// The queue methods are called with the lock held, except by [`cleanup`],
/// which takes no lock (as the C does not).
pub trait DelayOutput {
    /// What an encoder hands the output (`struct encoder_packet`, owned by
    /// the encoder).
    type Source: ?Sized;
    /// A held copy of a packet, owned by the queue until it is delivered or
    /// released.
    type Packet;
    /// The packet's timing (`struct encoder_packet_time`).
    type Time;

    /// Takes `delay_mutex`.
    fn lock(&mut self);
    /// Releases `delay_mutex`.
    fn unlock(&mut self);

    /// `delay_data` holds no entry.
    fn is_empty(&self) -> bool;
    /// `deque_push_back` of one entry.
    fn push_back(&mut self, entry: Entry<Self::Packet, Self::Time>);
    /// The front entry's time, if there is one (`deque_peek_front`).
    fn front_ts(&self) -> Option<u64>;
    /// `deque_pop_front` of one entry. `None` when the entry holds a
    /// message this file never queues; it is dropped as the C drops it,
    /// with nothing run and nothing released.
    fn pop_front(&mut self) -> Option<Entry<Self::Packet, Self::Time>>;

    fn active_delay_ns(&self) -> u64;
    fn set_active_delay_ns(&mut self, ns: u64);
    fn delay_sec(&self) -> u32;
    fn set_delay_sec(&mut self, sec: u32);
    fn set_delay_flags(&mut self, flags: u32);
    /// `delay_cur_flags`: the flags of the running delay.
    fn delay_cur_flags(&self) -> u32;
    /// `os_atomic_set_long(delay_restart_refs, 0)`.
    fn clear_restart_refs(&mut self);
    /// `os_atomic_inc_long(delay_restart_refs)`.
    fn inc_restart_refs(&mut self);
    /// `os_atomic_load_bool(delay_active)`.
    fn delay_active(&self) -> bool;
    /// `os_atomic_load_bool(delay_capturing)`.
    fn delay_capturing(&self) -> bool;
    /// `os_atomic_set_bool(starting, ..)`.
    fn set_starting(&mut self, starting: bool);
    fn reconnecting(&self) -> bool;
    /// `info.flags`.
    fn flags(&self) -> u32;

    /// The warning `od_log_flag_encoded` logs when `func` is used on an
    /// output that is not encoded.
    fn warn_raw_output(&self, func: &'static CStr);
    /// `obs_output_can_begin_data_capture(output, 0)`.
    fn can_begin_data_capture(&mut self) -> bool;
    /// `obs_output_initialize_encoders(output, 0)`.
    fn initialize_encoders(&mut self) -> bool;
    /// `obs_output_begin_data_capture(output, 0)`.
    fn begin_data_capture(&mut self) -> bool;
    /// `obs_output_actual_start`; the delay does not use its result.
    fn actual_start(&mut self);
    /// `obs_output_actual_stop`.
    fn actual_stop(&mut self, force: bool, ts: u64);
    /// `obs_output_delay_signal`.
    fn signal(&mut self, signal: Signal);
    /// `obs_encoder_packet_create_instance`: a held copy of `src`.
    fn copy_packet(&mut self, src: &Self::Source) -> Self::Packet;
    /// `delay_callback`, which takes ownership of the packet.
    fn deliver(&mut self, packet: Self::Packet, time: Option<Self::Time>);
    /// `obs_encoder_packet_release`.
    fn release(&mut self, packet: Self::Packet);
}

/// Where a call on a null output is logged (`obs_output_valid`).
pub trait NullLog {
    fn null_output(&self, func: &'static CStr);
}

/// `obs_output_set_delay`: stores the setting for an encoded output. The
/// running delay is copied from it when data capture is hooked
/// (`obs-output.c`), not here.
pub fn set_delay<O: DelayOutput>(
    output: Option<&mut O>,
    log: &impl NullLog,
    delay_sec: u32,
    flags: u32,
) {
    let Some(output) = output else {
        log.null_output(SET_DELAY);
        return;
    };
    if output.flags() & OBS_OUTPUT_ENCODED == 0 {
        output.warn_raw_output(SET_DELAY);
        return;
    }
    output.set_delay_sec(delay_sec);
    output.set_delay_flags(flags);
}

/// `obs_output_get_delay`: the stored setting, 0 for a null output.
pub fn get_delay<O: DelayOutput>(output: Option<&O>, log: &impl NullLog) -> u32 {
    match output {
        Some(output) => output.delay_sec(),
        None => {
            log.null_output(SET_DELAY);
            0
        }
    }
}

/// `obs_output_get_active_delay`: the running delay in whole seconds
/// ([`active_delay_secs`]), 0 for a null output.
pub fn get_active_delay<O: DelayOutput>(output: Option<&O>, log: &impl NullLog) -> u32 {
    match output {
        Some(output) => active_delay_secs(output.active_delay_ns()),
        None => {
            log.null_output(SET_DELAY);
            0
        }
    }
}

/// `process_delay`: queues a copy of `src` at `now`, then runs every entry
/// that is due, oldest first.
pub fn process_delay<O: DelayOutput>(
    output: &mut O,
    src: &O::Source,
    time: Option<O::Time>,
    now: u64,
) {
    // push_packet: the copy is made before the lock is taken.
    let packet = output.copy_packet(src);
    output.lock();
    output.push_back(Entry::Packet {
        ts: now,
        packet,
        time,
    });
    output.unlock();
    while pop_packet(output, now) {}
}

/// `pop_packet`: one pass over the front of the queue. True if an entry
/// left the queue.
pub fn pop_packet<O: DelayOutput>(output: &mut O, now: u64) -> bool {
    // Read before the lock, as the C reads it.
    let preserve = output.delay_cur_flags() & OBS_OUTPUT_DELAY_PRESERVE != 0;
    let mut popped = None;

    output.lock();
    if let Some(front_ts) = output.front_ts() {
        match pop_decision(
            front_ts,
            now,
            output.active_delay_ns(),
            preserve,
            output.reconnecting(),
        ) {
            Decision::Extend(age) => output.set_active_delay_ns(age),
            Decision::Pop => popped = Some(output.pop_front()),
            Decision::Hold => {}
        }
    }
    output.unlock();

    // The entry runs after the lock is released.
    match popped {
        Some(entry) => {
            if let Some(entry) = entry {
                dispatch(output, entry);
            }
            true
        }
        None => false,
    }
}

/// `process_delay_data`: runs an entry that left the queue. A packet is
/// delivered while the delay is active and capturing, and released
/// otherwise; a start or stop runs whatever the delay state.
pub fn dispatch<O: DelayOutput>(output: &mut O, entry: Entry<O::Packet, O::Time>) {
    match entry {
        Entry::Packet { packet, time, .. } => {
            if !output.delay_active() || !output.delay_capturing() {
                output.release(packet);
            } else {
                output.deliver(packet, time);
            }
        }
        Entry::Start { .. } => output.actual_start(),
        Entry::Stop { ts } => output.actual_stop(false, ts),
    }
}

/// `obs_output_cleanup_delay`: empties the queue without running anything,
/// releasing the held packets, and resets the running delay and the
/// restart references. Takes no lock.
pub fn cleanup<O: DelayOutput>(output: &mut O) {
    while !output.is_empty() {
        if let Some(Entry::Packet { packet, .. }) = output.pop_front() {
            output.release(packet);
        }
    }
    output.set_active_delay_ns(0);
    output.clear_restart_refs();
}

/// `obs_output_delay_start`: queues the real start at `now`. A first start
/// checks the output, sets `starting`, initializes the encoders and begins
/// data capture; a restart while the delay is active only queues and
/// signals `"starting"`.
pub fn start<O: DelayOutput>(output: &mut O, now: u64) -> bool {
    if !output.delay_active() {
        if !output.can_begin_data_capture() {
            return false;
        }
        output.set_starting(true);
        if !output.initialize_encoders() {
            output.set_starting(false);
            return false;
        }
    }

    output.lock();
    output.push_back(Entry::Start { ts: now });
    output.unlock();

    output.inc_restart_refs();

    if output.delay_active() {
        output.signal(Signal::Starting);
        return true;
    }

    if !output.begin_data_capture() {
        // Characterized, not endorsed: `starting` stays set.
        cleanup(output);
        return false;
    }

    true
}

/// `obs_output_delay_stop`: queues the real stop at `now` and signals
/// `"stopping"`.
pub fn stop<O: DelayOutput>(output: &mut O, now: u64) {
    output.lock();
    output.push_back(Entry::Stop { ts: now });
    output.unlock();

    output.signal(Signal::Stopping);
}
