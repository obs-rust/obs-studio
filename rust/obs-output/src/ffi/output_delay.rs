//! C ABI for `libobs/obs-output-delay.c.h`.
//!
//! `obs_output_set_delay`, `obs_output_get_delay` and
//! `obs_output_get_active_delay` are `EXPORT` (`obs.h`). `process_delay`,
//! `obs_output_delay_start`, `obs_output_delay_stop` and
//! `obs_output_cleanup_delay` are declared in `obs-internal.h` without
//! `EXPORT`, so they stay hidden (`rust-hidden.txt`): `obs-output.c` calls
//! them inside libobs, and installs `process_delay` as the encoded packet
//! callback, which the encoder later finds again by comparing the function
//! pointer. That works because `obs-output.c` names the symbol, which is
//! this function in `libobs-with-rust`.
//!
//! The output is `struct obs_output`, private and platform-sized, so this
//! never reads it: libobs's hidden `obs_output_delay_fields()` gives
//! pointers to the delay fields (#1550), and `obs_output_delay_signal()`
//! emits the output's signals. Every field is read and written through
//! those pointers at the point the C reads and writes it, and `delay_mutex`
//! is taken where the C takes it. The `os_atomic_*` fields are atomics of
//! their C size (`bool`, `long`), sequentially consistent like the C on
//! every host.
//!
//! `delay_data` is a C `struct deque` of `struct delay_data`, freed by
//! `obs-output.c`. It is pushed, peeked and popped in place with the Rust
//! `util/deque.h` (`obs_util::ffi::deque`), whose buffer is `bmalloc`
//! memory either side can grow or free. A held packet is libobs's
//! reference-counted copy (`obs_encoder_packet_create_instance`, hidden in
//! libobs) and goes back through `obs_encoder_packet_release`.
//!
//! Each entry point reads `os_gettime_ns()` once, as the C does, and runs
//! the `*_at` function with that time; the `*_at` functions are what the
//! Tier 3 test drives with a test clock, as `test_output_delay` renames the
//! C clock.
//!
//! No intentional differences from the C. Two places only differ in what
//! cannot be observed: a packet entry without timing stores zeros where
//! the C leaves its stack copy uninitialized, and an entry is read back
//! into a zeroed copy before it is used. A null `delay_callback` aborts
//! where the C calls through a null pointer; libobs sets it before it
//! installs the delay.

use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};

use obs_util::ffi::deque::{deque, deque_peek_front, deque_pop_front, deque_push_back};

use crate::delay::{self, DelayOutput, Entry, NullLog, Signal};

/// `LOG_WARNING` (`util/base.h`).
pub const LOG_WARNING: c_int = 200;
/// `LOG_DEBUG` (`util/base.h`).
pub const LOG_DEBUG: c_int = 400;

/// `DELAY_MSG_PACKET` (`enum delay_msg`, `obs-internal.h`).
pub const DELAY_MSG_PACKET: c_int = 0;
/// `DELAY_MSG_START`.
pub const DELAY_MSG_START: c_int = 1;
/// `DELAY_MSG_STOP`.
pub const DELAY_MSG_STOP: c_int = 2;

/// `struct encoder_packet` (`obs-encoder.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct encoder_packet {
    pub data: *mut u8,
    pub size: usize,
    pub pts: i64,
    pub dts: i64,
    pub timebase_num: i32,
    pub timebase_den: i32,
    /// `enum obs_encoder_type`.
    pub r#type: c_int,
    pub keyframe: bool,
    pub dts_usec: i64,
    pub sys_dts_usec: i64,
    pub priority: c_int,
    pub drop_priority: c_int,
    pub track_idx: usize,
    /// `obs_encoder_t *`.
    pub encoder: *mut c_void,
}

impl Default for encoder_packet {
    /// A zeroed packet, as C's `= {0}`.
    fn default() -> Self {
        Self {
            data: ptr::null_mut(),
            size: 0,
            pts: 0,
            dts: 0,
            timebase_num: 0,
            timebase_den: 0,
            r#type: 0,
            keyframe: false,
            dts_usec: 0,
            sys_dts_usec: 0,
            priority: 0,
            drop_priority: 0,
            track_idx: 0,
            encoder: ptr::null_mut(),
        }
    }
}

/// `struct encoder_packet_time` (`obs-encoder.h`).
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct encoder_packet_time {
    pub pts: i64,
    pub cts: u64,
    pub fer: u64,
    pub ferc: u64,
    pub pir: u64,
}

/// `struct delay_data` (`obs-internal.h`): one entry of the queue.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Default, Clone, Copy)]
pub struct delay_data {
    /// `enum delay_msg`.
    pub msg: c_int,
    pub ts: u64,
    pub packet: encoder_packet,
    pub packet_time_valid: bool,
    pub packet_time: encoder_packet_time,
}

/// `encoded_callback_t` (`obs-internal.h`).
#[allow(non_camel_case_types)]
pub type encoded_callback_t =
    Option<unsafe extern "C" fn(*mut c_void, *mut encoder_packet, *mut encoder_packet_time)>;

/// `struct obs_output_delay_fields` (`obs-internal.h`): pointers into one
/// `struct obs_output`. `flags` and `name` point at `info.flags` and
/// `context.name`.
#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub struct obs_output_delay_fields {
    /// `pthread_mutex_t *`.
    pub delay_mutex: *mut c_void,
    /// `struct deque *` of `struct delay_data`.
    pub delay_data: *mut deque,
    pub active_delay_ns: *mut u64,
    pub delay_sec: *mut u32,
    pub delay_flags: *mut u32,
    pub delay_cur_flags: *mut u32,
    /// `volatile long *`.
    pub delay_restart_refs: *mut c_long,
    pub delay_active: *mut bool,
    pub delay_capturing: *mut bool,
    pub delay_callback: *mut encoded_callback_t,
    pub starting: *mut bool,
    pub reconnecting: *mut bool,
    pub flags: *const u32,
    pub name: *const *mut c_char,
}

unsafe extern "C" {
    // obs-output.c, hidden in libobs (obs-internal.h).
    fn obs_output_delay_fields(output: *const c_void) -> obs_output_delay_fields;
    fn obs_output_delay_signal(output: *mut c_void, signal: *const c_char);
    fn obs_output_actual_start(output: *mut c_void) -> bool;
    fn obs_output_actual_stop(output: *mut c_void, force: bool, ts: u64);
    // obs-output.c, EXPORT.
    fn obs_output_can_begin_data_capture(output: *const c_void, flags: u32) -> bool;
    fn obs_output_initialize_encoders(output: *mut c_void, flags: u32) -> bool;
    fn obs_output_begin_data_capture(output: *mut c_void, flags: u32) -> bool;
    // obs-encoder.c: create_instance is hidden in libobs, release EXPORT.
    fn obs_encoder_packet_create_instance(dst: *mut encoder_packet, src: *const encoder_packet);
    fn obs_encoder_packet_release(packet: *mut encoder_packet);
    // libobs's pthread (w32-pthreads on Windows); the mutex is opaque here.
    fn pthread_mutex_lock(mutex: *mut c_void) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> c_int;
    fn os_gettime_ns() -> u64;
    fn blog(level: c_int, format: *const c_char, ...);
}

/// `os_atomic_*_long` on a C `long`, whose size is the host's.
fn atomic_long_clear(p: *mut c_long) {
    if size_of::<c_long>() == size_of::<i64>() {
        // SAFETY: `p` is a live, aligned `long` of 8 bytes that C accesses
        // only atomically.
        unsafe { AtomicI64::from_ptr(p.cast()) }.store(0, Ordering::SeqCst);
    } else {
        // SAFETY: as above, 4 bytes.
        unsafe { AtomicI32::from_ptr(p.cast()) }.store(0, Ordering::SeqCst);
    }
}

fn atomic_long_inc(p: *mut c_long) {
    if size_of::<c_long>() == size_of::<i64>() {
        // SAFETY: as in `atomic_long_clear`.
        unsafe { AtomicI64::from_ptr(p.cast()) }.fetch_add(1, Ordering::SeqCst);
    } else {
        // SAFETY: as in `atomic_long_clear`.
        unsafe { AtomicI32::from_ptr(p.cast()) }.fetch_add(1, Ordering::SeqCst);
    }
}

/// One output, seen through its delay fields.
struct COutput {
    output: *mut c_void,
    f: obs_output_delay_fields,
}

impl COutput {
    /// # Safety
    ///
    /// `output` is a live `obs_output_t`.
    unsafe fn new(output: *const c_void) -> Self {
        Self {
            output: output.cast_mut(),
            // SAFETY: a live output (caller contract).
            f: unsafe { obs_output_delay_fields(output) },
        }
    }

    /// The front entry, copied out.
    fn peek(&self) -> delay_data {
        let mut dd = delay_data::default();
        // SAFETY: the queue is a valid deque of whole entries, borrowed
        // under the lock; `dd` is one entry.
        unsafe {
            deque_peek_front(
                self.f.delay_data,
                (&raw mut dd).cast(),
                size_of::<delay_data>(),
            );
        }
        dd
    }
}

impl DelayOutput for COutput {
    type Source = encoder_packet;
    type Packet = encoder_packet;
    type Time = encoder_packet_time;

    fn lock(&mut self) {
        // SAFETY: the output's initialized `delay_mutex`.
        unsafe { pthread_mutex_lock(self.f.delay_mutex) };
    }

    fn unlock(&mut self) {
        // SAFETY: held by this thread (`lock`).
        unsafe { pthread_mutex_unlock(self.f.delay_mutex) };
    }

    fn is_empty(&self) -> bool {
        // SAFETY: a live deque.
        unsafe { (*self.f.delay_data).size == 0 }
    }

    fn push_back(&mut self, entry: Entry<encoder_packet, encoder_packet_time>) {
        let dd = match entry {
            Entry::Packet { ts, packet, time } => delay_data {
                msg: DELAY_MSG_PACKET,
                ts,
                packet,
                packet_time_valid: time.is_some(),
                packet_time: time.unwrap_or_default(),
            },
            Entry::Start { ts } => delay_data {
                msg: DELAY_MSG_START,
                ts,
                ..delay_data::default()
            },
            Entry::Stop { ts } => delay_data {
                msg: DELAY_MSG_STOP,
                ts,
                ..delay_data::default()
            },
        };
        // SAFETY: a valid deque, under the lock; `dd` is one entry.
        unsafe {
            deque_push_back(
                self.f.delay_data,
                (&raw const dd).cast(),
                size_of::<delay_data>(),
            );
        }
    }

    fn front_ts(&self) -> Option<u64> {
        (!self.is_empty()).then(|| self.peek().ts)
    }

    fn pop_front(&mut self) -> Option<Entry<encoder_packet, encoder_packet_time>> {
        let mut dd = delay_data::default();
        // SAFETY: as in `peek`.
        unsafe {
            deque_pop_front(
                self.f.delay_data,
                (&raw mut dd).cast(),
                size_of::<delay_data>(),
            );
        }
        match dd.msg {
            DELAY_MSG_PACKET => Some(Entry::Packet {
                ts: dd.ts,
                packet: dd.packet,
                time: dd.packet_time_valid.then_some(dd.packet_time),
            }),
            DELAY_MSG_START => Some(Entry::Start { ts: dd.ts }),
            DELAY_MSG_STOP => Some(Entry::Stop { ts: dd.ts }),
            _ => None,
        }
    }

    fn active_delay_ns(&self) -> u64 {
        // SAFETY: a live field.
        unsafe { *self.f.active_delay_ns }
    }

    fn set_active_delay_ns(&mut self, ns: u64) {
        // SAFETY: a live field.
        unsafe { *self.f.active_delay_ns = ns };
    }

    fn delay_sec(&self) -> u32 {
        // SAFETY: a live field.
        unsafe { *self.f.delay_sec }
    }

    fn set_delay_sec(&mut self, sec: u32) {
        // SAFETY: a live field.
        unsafe { *self.f.delay_sec = sec };
    }

    fn set_delay_flags(&mut self, flags: u32) {
        // SAFETY: a live field.
        unsafe { *self.f.delay_flags = flags };
    }

    fn delay_cur_flags(&self) -> u32 {
        // SAFETY: a live field.
        unsafe { *self.f.delay_cur_flags }
    }

    fn clear_restart_refs(&mut self) {
        atomic_long_clear(self.f.delay_restart_refs);
    }

    fn inc_restart_refs(&mut self) {
        atomic_long_inc(self.f.delay_restart_refs);
    }

    fn delay_active(&self) -> bool {
        // SAFETY: a live `bool` C accesses atomically.
        unsafe { AtomicBool::from_ptr(self.f.delay_active) }.load(Ordering::SeqCst)
    }

    fn delay_capturing(&self) -> bool {
        // SAFETY: as above.
        unsafe { AtomicBool::from_ptr(self.f.delay_capturing) }.load(Ordering::SeqCst)
    }

    fn set_starting(&mut self, starting: bool) {
        // SAFETY: as above.
        unsafe { AtomicBool::from_ptr(self.f.starting) }.store(starting, Ordering::SeqCst);
    }

    fn reconnecting(&self) -> bool {
        // SAFETY: a live `volatile bool`, read as the C reads it.
        unsafe { ptr::read_volatile(self.f.reconnecting) }
    }

    fn flags(&self) -> u32 {
        // SAFETY: a live field.
        unsafe { *self.f.flags }
    }

    fn warn_raw_output(&self, func: &'static CStr) {
        // SAFETY: the format and its three string arguments; `name` is the
        // output's name (or null, which C's printf prints as C does).
        unsafe {
            blog(
                LOG_WARNING,
                c"Output '%s': Tried to use %s on a%s output".as_ptr(),
                *self.f.name,
                func.as_ptr(),
                c" raw".as_ptr(),
            );
        }
    }

    fn can_begin_data_capture(&mut self) -> bool {
        // SAFETY: a live output.
        unsafe { obs_output_can_begin_data_capture(self.output, 0) }
    }

    fn initialize_encoders(&mut self) -> bool {
        // SAFETY: a live output.
        unsafe { obs_output_initialize_encoders(self.output, 0) }
    }

    fn begin_data_capture(&mut self) -> bool {
        // SAFETY: a live output.
        unsafe { obs_output_begin_data_capture(self.output, 0) }
    }

    fn actual_start(&mut self) {
        // SAFETY: a live output. The result is not used, as in C.
        let _ = unsafe { obs_output_actual_start(self.output) };
    }

    fn actual_stop(&mut self, force: bool, ts: u64) {
        // SAFETY: a live output.
        unsafe { obs_output_actual_stop(self.output, force, ts) };
    }

    fn signal(&mut self, signal: Signal) {
        let name = match signal {
            Signal::Starting => c"starting",
            Signal::Stopping => c"stopping",
        };
        // SAFETY: a live output and a signal it declares.
        unsafe { obs_output_delay_signal(self.output, name.as_ptr()) };
    }

    fn copy_packet(&mut self, src: &encoder_packet) -> encoder_packet {
        let mut dst = encoder_packet::default();
        // SAFETY: `src` is the encoder's packet, valid for `size` bytes of
        // data; `dst` is written whole.
        unsafe { obs_encoder_packet_create_instance(&raw mut dst, src) };
        dst
    }

    fn deliver(&mut self, packet: encoder_packet, time: Option<encoder_packet_time>) {
        let mut packet = packet;
        let mut time = time;
        let time_ptr = time.as_mut().map_or(ptr::null_mut(), ptr::from_mut);
        // SAFETY: a live field, set before the delay runs (see the module
        // doc for null); the callback takes the packet.
        unsafe {
            let callback = (*self.f.delay_callback).expect("delay_callback set before the delay");
            callback(self.output, &raw mut packet, time_ptr);
        }
    }

    fn release(&mut self, packet: encoder_packet) {
        let mut packet = packet;
        // SAFETY: a held copy from `copy_packet`.
        unsafe { obs_encoder_packet_release(&raw mut packet) };
    }
}

/// `obs_output_valid`'s debug line.
struct CLog;

impl NullLog for CLog {
    fn null_output(&self, func: &'static CStr) {
        // SAFETY: the format and its two string arguments.
        unsafe {
            blog(
                LOG_DEBUG,
                c"%s: Null '%s' parameter".as_ptr(),
                func.as_ptr(),
                c"output".as_ptr(),
            )
        };
    }
}

/// Same contract as C `obs_output_set_delay`.
///
/// # Safety
///
/// `output` is null or a live `obs_output_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_set_delay(output: *mut c_void, delay_sec: u32, flags: u32) {
    // SAFETY: a live output when not null (caller contract).
    let mut o = (!output.is_null()).then(|| unsafe { COutput::new(output) });
    delay::set_delay(o.as_mut(), &CLog, delay_sec, flags);
}

/// Same contract as C `obs_output_get_delay`.
///
/// # Safety
///
/// As [`obs_output_set_delay`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_delay(output: *const c_void) -> u32 {
    // SAFETY: as above.
    let o = (!output.is_null()).then(|| unsafe { COutput::new(output) });
    delay::get_delay(o.as_ref(), &CLog)
}

/// Same contract as C `obs_output_get_active_delay`.
///
/// # Safety
///
/// As [`obs_output_set_delay`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_get_active_delay(output: *const c_void) -> u32 {
    // SAFETY: as above.
    let o = (!output.is_null()).then(|| unsafe { COutput::new(output) });
    delay::get_active_delay(o.as_ref(), &CLog)
}

/// Same contract as C `process_delay`, the encoded packet callback of a
/// delayed output.
///
/// # Safety
///
/// `data` is a live `obs_output_t` whose delay is set up, `packet` a valid
/// packet and `packet_time` null or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn process_delay(
    data: *mut c_void,
    packet: *mut encoder_packet,
    packet_time: *mut encoder_packet_time,
) {
    // SAFETY: forwarded caller contract.
    unsafe { process_delay_at(data, packet, packet_time, os_gettime_ns()) };
}

/// [`process_delay`] at time `now`.
///
/// # Safety
///
/// As [`process_delay`].
pub unsafe fn process_delay_at(
    data: *mut c_void,
    packet: *mut encoder_packet,
    packet_time: *mut encoder_packet_time,
    now: u64,
) {
    // SAFETY: forwarded caller contract.
    unsafe {
        let mut o = COutput::new(data);
        let time = packet_time.as_ref().copied();
        delay::process_delay(&mut o, &*packet, time, now);
    }
}

/// Same contract as C `obs_output_delay_start`.
///
/// # Safety
///
/// `output` is a live `obs_output_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_delay_start(output: *mut c_void) -> bool {
    // SAFETY: forwarded caller contract.
    unsafe { obs_output_delay_start_at(output, os_gettime_ns()) }
}

/// [`obs_output_delay_start`] at time `now`.
///
/// # Safety
///
/// As [`obs_output_delay_start`].
pub unsafe fn obs_output_delay_start_at(output: *mut c_void, now: u64) -> bool {
    // SAFETY: forwarded caller contract.
    let mut o = unsafe { COutput::new(output) };
    delay::start(&mut o, now)
}

/// Same contract as C `obs_output_delay_stop`.
///
/// # Safety
///
/// As [`obs_output_delay_start`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_delay_stop(output: *mut c_void) {
    // SAFETY: forwarded caller contract.
    unsafe { obs_output_delay_stop_at(output, os_gettime_ns()) };
}

/// [`obs_output_delay_stop`] at time `now`.
///
/// # Safety
///
/// As [`obs_output_delay_start`].
pub unsafe fn obs_output_delay_stop_at(output: *mut c_void, now: u64) {
    // SAFETY: forwarded caller contract.
    let mut o = unsafe { COutput::new(output) };
    delay::stop(&mut o, now);
}

/// Same contract as C `obs_output_cleanup_delay`.
///
/// # Safety
///
/// As [`obs_output_delay_start`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_cleanup_delay(output: *mut c_void) {
    // SAFETY: forwarded caller contract.
    let mut o = unsafe { COutput::new(output) };
    delay::cleanup(&mut o);
}
