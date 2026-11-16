//! Private C field addresses; queue selection and packet iteration live in the safe core.
use crate::output_caption_queue::{Packets, Track, Tracks, enqueue};
use core::ffi::{c_int, c_void};
use obs_util::ffi::deque::{deque, deque_push_back};

/// The public obs_source_cea_708 layout, checked against the real C header.
#[repr(C)]
pub struct Captions {
    pub data: *const u8,
    pub packets: u32,
    pub timestamp: u64,
}

unsafe extern "C" {
    fn obs_output_caption_queue_slots() -> usize;
    fn obs_output_caption_queue_track(
        output: *mut c_void,
        index: usize,
        mutex: *mut *mut c_void,
    ) -> *mut deque;
    fn pthread_mutex_lock(mutex: *mut c_void) -> c_int;
    fn pthread_mutex_unlock(mutex: *mut c_void) -> c_int;
}

struct Output(*mut c_void);
struct Locked {
    mutex: *mut c_void,
    queue: *mut deque,
}
impl Drop for Locked {
    fn drop(&mut self) {
        // SAFETY: this guard owns the live track's locked mutex.
        unsafe { pthread_mutex_unlock(self.mutex) };
    }
}
impl Track for Locked {
    fn append(&mut self, packet: [u8; 3]) {
        // SAFETY: the track remains locked; deque copies these three bytes using bmem.
        unsafe { deque_push_back(self.queue, packet.as_ptr().cast(), packet.len()) };
    }
}
impl Tracks for Output {
    type Locked<'a> = Locked;
    fn slots(&self) -> usize {
        // SAFETY: returns only the C compile-time slot count.
        unsafe { obs_output_caption_queue_slots() }
    }
    fn lock(&mut self, index: usize) -> Option<Locked> {
        let mut mutex = core::ptr::null_mut();
        // SAFETY: output is live and index is bounded by the real C slot count.
        let queue = unsafe { obs_output_caption_queue_track(self.0, index, &mut mutex) };
        if queue.is_null() {
            return None;
        }
        // SAFETY: adapter returned the configured track's live mutex.
        unsafe { pthread_mutex_lock(mutex) };
        Some(Locked { mutex, queue })
    }
}
struct Input(*const Captions);
impl Packets for Input {
    fn count(&self) -> usize {
        // SAFETY: caller supplies live captions whenever any track is configured.
        unsafe { (*self.0).packets as usize }
    }
    fn packet(&self, index: usize) -> [u8; 3] {
        // SAFETY: caller supplies packets*3 readable bytes; core bounds index by count.
        unsafe { (*self.0).data.add(index * 3).cast::<[u8; 3]>().read() }
    }
}

/// Append copied CEA-708 packets to every configured track.
///
/// # Safety
/// Output and its configured tracks must remain live throughout this call.
/// When any track exists, captions must be readable and its data must contain
/// packets*3 readable bytes. Track configuration and the input must not change
/// concurrently. A zero packet count permits a null data pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_output_caption(output: *mut c_void, captions: *const Captions) {
    if output.is_null() {
        return;
    }
    enqueue(&mut Output(output), &Input(captions));
}
