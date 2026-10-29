//! Intentional C difference: replacements do not inherit old scaler buffers.
//!
//! Intentional C differences: scaler creation rechecks duplicate inputs,
//! and self-disconnection does not skip the following video subscriber.
//!
//! Safe core of `libobs/media-io/video-io.c`.
//!
//! The video thread, the cache, and the input list live here. The C ABI
//! shims and the FFmpeg scaler calls live in [`crate::ffi::video_io`].
//!
//! Intentional differences from the C file, also listed in `rust/README.md`:
//! cache frames are zeroed (`VideoFrame` allocates with `vec`), where C's
//! `bmalloc` leaves them uninitialized. `close` on a frame-rate-divisor
//! child stops the root and frees only the child; C frees the parent's
//! cache and mutexes too. The stop flag is an atomic; C uses a plain
//! `bool`. `connect` on a null output returns false; C crashes in
//! `get_root` before the null check. A null `get_skipped_frames` or
//! `get_total_frames` returns 0; C dereferences the handle. A null `info`
//! or out-pointer to `open` returns `VIDEO_OUTPUT_INVALIDPARAM`; C crashes.
//! A null frame pointer to `lock_frame` returns true and writes nothing;
//! C crashes in `memcpy`.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use obs_util::threading::Sem;

use crate::platform::ReMutex;
use crate::video_frame::VideoFrame;
use crate::video_io::{MAX_AV_PLANES, VideoFormat, video_format_from_c};

const MAX_CACHE: usize = 16;
const MAX_CONVERT_BUFFERS: usize = 3;
const VIDEO_RANGE_FULL: i32 = 2;
const VIDEO_CS_DEFAULT: i32 = 0;
const VIDEO_CS_709: i32 = 2;
const VIDEO_CS_SRGB: i32 = 3;
const VIDEO_CS_2100_PQ: i32 = 4;
const VIDEO_CS_2100_HLG: i32 = 5;

/// One frame the callback is allowed to read. The pointers stay valid until
/// the callback returns: the source buffer lives in the cache, and a convert
/// buffer lives on the input, which the input lock still owns.
#[derive(Clone, Copy)]
pub(crate) struct FrameView {
    pub data: [*mut u8; MAX_AV_PLANES],
    pub linesize: [u32; MAX_AV_PLANES],
    pub timestamp: u64,
}

/// `struct video_scale_info` as plain integers. The shim copies the C struct.
#[derive(Clone, Copy)]
pub(crate) struct RawScale {
    pub format: i32,
    pub width: u32,
    pub height: u32,
    pub range: i32,
    pub colorspace: i32,
}

pub(crate) enum ScalerFail {
    BadConversion,
    Failed,
}

/// What [`VideoHost::create_scaler`] builds. The FFmpeg object stays in the
/// shim; this trait is the only thing the core calls.
pub(crate) trait Scaler: Send {
    fn scale(&mut self, src: &FrameView, dst: &mut VideoFrame) -> bool;
}

/// Host services the core cannot name without the C ABI: the profiler, the
/// log, the callback, and the scaler. Implemented in `ffi`.
pub(crate) trait VideoHost: Send + Sync {
    fn thread_start(&self);
    fn profile_begin(&self);
    fn profile_end(&self);
    fn reenable(&self);
    fn deliver(&self, callback: usize, param: usize, frame: &FrameView);
    fn warn_scale_failed(&self);
    fn scaler_error(&self, bad_conversion: bool);
    fn log_skipped(&self, skipped: i64, total: i64);
    fn create_scaler(&self, dst: &RawScale, src: &RawScale) -> Result<Box<dyn Scaler>, ScalerFail>;
}

/// Parameters `video_output_open` accepted. `cache_size` is already clamped.
pub(crate) struct OpenInfo {
    pub format: i32,
    pub width: u32,
    pub height: u32,
    pub cache_size: usize,
    pub range: i32,
    pub colorspace: i32,
}

struct Slot {
    frame: Option<VideoFrame>,
    timestamp: u64,
    skipped: i32,
    count: i32,
}

struct Data {
    slots: Vec<Slot>,
    cache_size: usize,
    available: usize,
    first_added: usize,
    last_added: usize,
}

struct Input {
    identity: Arc<()>,
    callback: usize,
    param: usize,
    divisor: u32,
    counter: u32,
    /// A grouped encoder's phase starts at the group's start frame: frames
    /// rendered before this timestamp are not delivered and do not move
    /// `counter`. 0 when the input has no pending phase.
    start_timestamp: u64,
    scaler: Option<Box<dyn Scaler>>,
    buffers: Vec<VideoFrame>,
    /// C's `cur_frame`, which starts at 0 and is incremented before use, so
    /// the first scaled frame is buffer 1.
    cur: usize,
}

/// What one input needs for this frame. The scaler and the convert buffer
/// come out of the list so `scale` can log without holding `&mut` on it.
enum Prep {
    Next,
    Direct {
        callback: usize,
        param: usize,
    },
    Scale {
        identity: Arc<()>,
        callback: usize,
        param: usize,
        cur: usize,
        scaler: Box<dyn Scaler>,
        buffer: Box<VideoFrame>,
    },
}

enum Scaled {
    Gone,
    Failed,
    Ready(FrameView),
}

struct Shared {
    data: ReMutex<Data>,
    inputs: ReMutex<Vec<Input>>,
    sem: std::pin::Pin<Box<Sem>>,
    stop: AtomicBool,
    skipped: AtomicI64,
    total: AtomicI64,
    raw_active: AtomicBool,
    gpu_refs: AtomicI64,
    width: u32,
    height: u32,
    format: i32,
    range: i32,
    colorspace: i32,
    host: Box<dyn VideoHost>,
}

/// One root video output. A frame-rate-divisor child is not this type; the
/// shim keeps the child and forwards calls here.
pub(crate) struct VideoOutput {
    shared: Arc<Shared>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl VideoOutput {
    pub(crate) fn open(
        info: OpenInfo,
        frame_time: u64,
        host: Box<dyn VideoHost>,
    ) -> Result<Self, ()> {
        let format = video_format_from_c(info.format).unwrap_or(VideoFormat::None);
        let mut slots = Vec::with_capacity(MAX_CACHE);
        for i in 0..MAX_CACHE {
            let frame = if i < info.cache_size {
                Some(VideoFrame::new(format, info.width, info.height))
            } else {
                None
            };
            slots.push(Slot {
                frame,
                timestamp: 0,
                skipped: 0,
                count: 0,
            });
        }
        let sem = Sem::try_new(0).ok_or(())?;
        let shared = Arc::new(Shared {
            data: ReMutex::new(Data {
                slots,
                cache_size: info.cache_size,
                available: info.cache_size,
                first_added: 0,
                last_added: 0,
            }),
            inputs: ReMutex::new(Vec::new()),
            sem,
            stop: AtomicBool::new(false),
            skipped: AtomicI64::new(0),
            total: AtomicI64::new(0),
            raw_active: AtomicBool::new(false),
            gpu_refs: AtomicI64::new(0),
            width: info.width,
            height: info.height,
            format: info.format,
            range: info.range,
            colorspace: info.colorspace,
            host,
        });
        let thread_shared = Arc::clone(&shared);
        let handle = thread::Builder::new()
            .spawn(move || thread_main(thread_shared, frame_time))
            .map_err(|_| ())?;
        Ok(Self {
            shared,
            join: Mutex::new(Some(handle)),
        })
    }

    pub(crate) fn stop(&self) {
        if self.shared.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.shared.sem.post();
        let handle = self
            .join
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }

    pub(crate) fn stopped(&self) -> bool {
        self.shared.stop.load(Ordering::SeqCst)
    }

    pub(crate) fn active(&self) -> bool {
        self.shared.raw_active.load(Ordering::SeqCst)
    }

    pub(crate) fn skipped_frames(&self) -> u32 {
        self.shared.skipped.load(Ordering::SeqCst) as u32
    }

    pub(crate) fn total_frames(&self) -> u32 {
        self.shared.total.load(Ordering::SeqCst) as u32
    }

    pub(crate) fn inc_texture_frames(&self) {
        self.shared.total.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn inc_texture_skipped_frames(&self) {
        self.shared.skipped.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn inc_texture_encoders(&self) {
        let new = self.shared.gpu_refs.fetch_add(1, Ordering::SeqCst) + 1;
        if new == 1 && !self.shared.raw_active.load(Ordering::SeqCst) {
            self.reset_frames();
        }
    }

    pub(crate) fn dec_texture_encoders(&self) {
        let new = self.shared.gpu_refs.fetch_sub(1, Ordering::SeqCst) - 1;
        if new == 0 && !self.shared.raw_active.load(Ordering::SeqCst) {
            self.shared.host.log_skipped(
                self.shared.skipped.load(Ordering::SeqCst),
                self.shared.total.load(Ordering::SeqCst),
            );
        }
    }

    fn reset_frames(&self) {
        self.shared.skipped.store(0, Ordering::SeqCst);
        self.shared.total.store(0, Ordering::SeqCst);
    }

    pub(crate) fn connect(
        &self,
        conversion: Option<RawScale>,
        divisor: u32,
        callback: usize,
        param: usize,
    ) -> bool {
        if callback == 0 || divisor == 0 {
            return false;
        }
        let from = self.output_scale();
        let mut guard = self.shared.inputs.lock();
        // Decide under `&mut`, then drop that borrow before the host runs.
        // `create_scaler` calls `blog`, and a log handler on this thread can
        // connect again. The guard stays held so other threads still wait,
        // matching the C input mutex around `video_input_init`.
        let prepared = guard.with_mut(|inputs| {
            if inputs
                .iter()
                .any(|input| input.callback == callback && input.param == param)
            {
                return None;
            }
            let mut conversion = conversion.unwrap_or(from);
            if conversion.width == 0 {
                conversion.width = from.width;
            }
            if conversion.height == 0 {
                conversion.height = from.height;
            }
            Some((conversion, needs_scaler(&conversion, &from)))
        });
        let Some((conversion, scale)) = prepared else {
            return false;
        };
        let (scaler, buffers) = if scale {
            let scaler = match self.shared.host.create_scaler(&conversion, &from) {
                Ok(scaler) => scaler,
                Err(ScalerFail::BadConversion) => {
                    self.shared.host.scaler_error(true);
                    return false;
                }
                Err(ScalerFail::Failed) => {
                    self.shared.host.scaler_error(false);
                    return false;
                }
            };
            let format = video_format_from_c(conversion.format).unwrap_or(VideoFormat::None);
            let mut buffers = Vec::with_capacity(MAX_CONVERT_BUFFERS);
            for _ in 0..MAX_CONVERT_BUFFERS {
                buffers.push(VideoFrame::new(format, conversion.width, conversion.height));
            }
            (Some(scaler), buffers)
        } else {
            (None, Vec::new())
        };
        guard.with_mut(|inputs| {
            // Scaler creation may reenter connect through a log callback.
            // Recheck after it returns so the same input is stored only once.
            if inputs
                .iter()
                .any(|input| input.callback == callback && input.param == param)
            {
                return false;
            }
            let first = inputs.is_empty();
            inputs.push(Input {
                identity: Arc::new(()),
                callback,
                param,
                divisor,
                counter: 0,
                start_timestamp: 0,
                scaler,
                buffers,
                cur: 0,
            });
            if first {
                if self.shared.gpu_refs.load(Ordering::SeqCst) == 0 {
                    self.reset_frames();
                }
                self.shared.raw_active.store(true, Ordering::SeqCst);
            }
            true
        })
    }

    pub(crate) fn disconnect(&self, callback: usize, param: usize) -> bool {
        if callback == 0 {
            return false;
        }
        let mut guard = self.shared.inputs.lock();
        let removed = guard.with_mut(|inputs| {
            let idx = inputs
                .iter()
                .position(|input| input.callback == callback && input.param == param)?;
            inputs.remove(idx);
            Some(inputs.is_empty())
        });
        let Some(empty) = removed else {
            return false;
        };
        if empty {
            self.shared.raw_active.store(false, Ordering::SeqCst);
            if self.shared.gpu_refs.load(Ordering::SeqCst) == 0 {
                self.shared.host.log_skipped(
                    self.shared.skipped.load(Ordering::SeqCst),
                    self.shared.total.load(Ordering::SeqCst),
                );
            }
        }
        true
    }

    /// C's `video_output_align_frame_rate_divisor`: the next frame at or after
    /// `start_timestamp` is the first of a new divisor phase.
    pub(crate) fn align_divisor(
        &self,
        start_timestamp: u64,
        callback: usize,
        param: usize,
    ) -> bool {
        if callback == 0 {
            return false;
        }
        let mut guard = self.shared.inputs.lock();
        guard.with_mut(|inputs| {
            let Some(input) = inputs
                .iter_mut()
                .find(|input| input.callback == callback && input.param == param)
            else {
                return false;
            };
            input.counter = 0;
            input.start_timestamp = start_timestamp;
            true
        })
    }

    pub(crate) fn lock_frame(&self, count: i32, timestamp: u64) -> Option<FrameView> {
        let mut guard = self.shared.data.lock();
        guard.with_mut(|data| {
            if data.available == 0 {
                let slot = &mut data.slots[data.last_added];
                slot.count = slot.count.wrapping_add(count);
                slot.skipped = slot.skipped.wrapping_add(count);
                return None;
            }
            if data.available != data.cache_size {
                data.last_added += 1;
                if data.last_added == data.cache_size {
                    data.last_added = 0;
                }
            }
            let slot = &mut data.slots[data.last_added];
            slot.timestamp = timestamp;
            slot.count = count;
            slot.skipped = 0;
            let frame = slot.frame.as_mut().expect("locked slot has a frame");
            Some(frame_view(frame, timestamp))
        })
    }

    pub(crate) fn unlock_frame(&self) {
        let mut guard = self.shared.data.lock();
        guard.with_mut(|data| {
            data.available = data.available.wrapping_sub(1);
            let _ = self.shared.sem.post();
        });
    }

    fn output_scale(&self) -> RawScale {
        RawScale {
            format: self.shared.format,
            width: self.shared.width,
            height: self.shared.height,
            range: self.shared.range,
            colorspace: self.shared.colorspace,
        }
    }
}

impl Drop for VideoOutput {
    fn drop(&mut self) {
        self.stop();
    }
}

fn thread_main(shared: Arc<Shared>, frame_time: u64) {
    shared.host.thread_start();
    loop {
        if shared.sem.wait() != 0 {
            break;
        }
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        shared.host.profile_begin();
        while !shared.stop.load(Ordering::SeqCst) && !cur_frame(&shared, frame_time) {
            shared.total.fetch_add(1, Ordering::SeqCst);
        }
        shared.total.fetch_add(1, Ordering::SeqCst);
        shared.host.profile_end();
        shared.host.reenable();
    }
}

fn cur_frame(shared: &Shared, frame_time: u64) -> bool {
    let source = {
        let mut guard = shared.data.lock();
        guard.with_mut(|data| {
            let slot = &mut data.slots[data.first_added];
            let timestamp = slot.timestamp;
            let frame = slot.frame.as_mut().expect("cache slot has a frame");
            frame_view(frame, timestamp)
        })
    };

    {
        let mut guard = shared.inputs.lock();
        // Callbacks may remove inputs and reconnect the same callback/param.
        // Visit each registration at most once, and defer a replacement of an
        // already visited callback/param until the next frame. New subscribers
        // with a different callback/param still receive this frame.
        let mut visited = Vec::new();
        loop {
            let prep = guard.with_mut(|inputs| {
                let input = inputs.iter_mut().find(|input| {
                    !visited.iter().any(|(id, callback, param)| {
                        Arc::ptr_eq(id, &input.identity)
                            || (*callback == input.callback && *param == input.param)
                    })
                })?;
                visited.push((Arc::clone(&input.identity), input.callback, input.param));
                Some(prepare_input(input, source.timestamp))
            });
            let Some(prep) = prep else {
                break;
            };
            match prep {
                Prep::Next => {}
                Prep::Direct { callback, param } => {
                    // The `&mut` has ended. The guard stays held, so other
                    // threads still wait. This thread's callback may connect.
                    shared.host.deliver(callback, param, &source);
                }
                Prep::Scale {
                    identity,
                    callback,
                    param,
                    cur,
                    mut scaler,
                    mut buffer,
                } => {
                    // `video_scaler_scale` blogs when `sws_scale` fails. The
                    // scaler and the buffer are not in the list for that call.
                    let ok = scaler.scale(&source, &mut buffer);
                    let scaled = guard.with_mut(|inputs| {
                        let Some(input) = inputs
                            .iter_mut()
                            .find(|input| Arc::ptr_eq(&input.identity, &identity))
                        else {
                            return Scaled::Gone;
                        };
                        input.scaler = Some(scaler);
                        input.buffers.insert(cur, *buffer);
                        if ok {
                            Scaled::Ready(frame_view(&mut input.buffers[cur], source.timestamp))
                        } else {
                            Scaled::Failed
                        }
                    });
                    match scaled {
                        Scaled::Ready(view) => shared.host.deliver(callback, param, &view),
                        Scaled::Failed => shared.host.warn_scale_failed(),
                        Scaled::Gone => {}
                    }
                }
            }
        }
    }

    let mut guard = shared.data.lock();
    guard.with_mut(|data| {
        let slot = &mut data.slots[data.first_added];
        slot.timestamp = slot.timestamp.wrapping_add(frame_time);
        slot.count = slot.count.wrapping_sub(1);
        let complete = slot.count == 0;
        let skipped_flag = slot.skipped > 0;
        if complete {
            data.first_added += 1;
            if data.first_added == data.cache_size {
                data.first_added = 0;
            }
            data.available += 1;
            if data.available == data.cache_size {
                data.last_added = data.first_added;
            }
        } else if skipped_flag {
            slot.skipped = slot.skipped.wrapping_sub(1);
            shared.skipped.fetch_add(1, Ordering::SeqCst);
        }
        complete
    })
}

fn prepare_input(input: &mut Input, timestamp: u64) -> Prep {
    // A grouped encoder's phase starts at the group's start frame: the frames
    // rendered before it are dropped and leave the counter alone.
    if input.start_timestamp != 0 {
        if timestamp < input.start_timestamp {
            return Prep::Next;
        }
        input.start_timestamp = 0;
    }
    let skip = input.counter;
    input.counter = input.counter.wrapping_add(1);
    if input.counter == input.divisor {
        input.counter = 0;
    }
    if skip != 0 {
        return Prep::Next;
    }
    let callback = input.callback;
    let param = input.param;
    let Some(scaler) = input.scaler.take() else {
        return Prep::Direct { callback, param };
    };
    input.cur += 1;
    if input.cur == MAX_CONVERT_BUFFERS {
        input.cur = 0;
    }
    let cur = input.cur;
    let buffer = Box::new(input.buffers.remove(cur));
    Prep::Scale {
        identity: Arc::clone(&input.identity),
        callback,
        param,
        cur,
        scaler,
        buffer,
    }
}

fn frame_view(frame: &mut VideoFrame, timestamp: u64) -> FrameView {
    let mut data = [std::ptr::null_mut(); MAX_AV_PLANES];
    let mut linesize = [0; MAX_AV_PLANES];
    for plane in 0..MAX_AV_PLANES {
        linesize[plane] = frame.linesize(plane);
        if let Some(bytes) = frame.plane_mut(plane) {
            data[plane] = bytes.as_mut_ptr();
        }
    }
    FrameView {
        data,
        linesize,
        timestamp,
    }
}

fn needs_scaler(conversion: &RawScale, from: &RawScale) -> bool {
    conversion.width != from.width
        || conversion.height != from.height
        || conversion.format != from.format
        || !match_range(conversion.range, from.range)
        || !match_space(conversion.colorspace, from.colorspace)
}

fn match_range(a: i32, b: i32) -> bool {
    (a == VIDEO_RANGE_FULL) == (b == VIDEO_RANGE_FULL)
}

fn collapse_space(cs: i32) -> i32 {
    match cs {
        VIDEO_CS_SRGB => VIDEO_CS_709,
        VIDEO_CS_2100_HLG => VIDEO_CS_2100_PQ,
        other => other,
    }
}

fn match_space(a: i32, b: i32) -> bool {
    a == VIDEO_CS_DEFAULT || b == VIDEO_CS_DEFAULT || collapse_space(a) == collapse_space(b)
}

#[cfg(test)]
#[allow(unsafe_code)] // the scaler reenters through the test's output pointer
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    use super::{FrameView, OpenInfo, RawScale, Scaler, ScalerFail, VideoHost, VideoOutput};
    use crate::video_frame::VideoFrame;
    use crate::video_io::VideoFormat;

    struct State {
        output: AtomicPtr<VideoOutput>,
        reenter_creation: AtomicBool,
        self_remove: AtomicBool,
        replace_on_scale: AtomicBool,
        reconnects_left: AtomicUsize,
        hit: Mutex<Vec<usize>>,
        lock: Mutex<usize>,
        cv: Condvar,
    }

    struct TestHost(Arc<State>);

    struct Reenter {
        state: Arc<State>,
        once: AtomicBool,
    }

    impl Scaler for Reenter {
        fn scale(&mut self, _src: &FrameView, dst: &mut VideoFrame) -> bool {
            if self.once.swap(false, Ordering::SeqCst) {
                let ptr = self.state.output.load(Ordering::Acquire);
                assert!(!ptr.is_null());
                // SAFETY: the test stores this pointer before `connect` and
                // joins the video thread, via `stop`, before the output drops.
                let output = unsafe { &*ptr };
                if self.state.replace_on_scale.swap(false, Ordering::SeqCst) {
                    assert!(output.disconnect(1, 0));
                    assert!(output.connect(None, 1, 1, 0));
                }
                assert!(output.connect(None, 1, 2, 0));
            }
            let _ = dst.linesize(0);
            true
        }
    }

    impl VideoHost for TestHost {
        fn thread_start(&self) {}
        fn profile_begin(&self) {}
        fn profile_end(&self) {}
        fn reenable(&self) {}
        fn deliver(&self, callback: usize, _param: usize, _frame: &FrameView) {
            if callback == 1 && self.0.self_remove.swap(false, Ordering::SeqCst) {
                let ptr = self.0.output.load(Ordering::Acquire);
                // SAFETY: the output stays alive until its worker is joined.
                let output = unsafe { &*ptr };
                assert!(output.disconnect(callback, 0));
                if self
                    .0
                    .reconnects_left
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_ok()
                {
                    assert!(output.connect(None, 1, callback, 0));
                    self.0.self_remove.store(true, Ordering::SeqCst);
                }
            }
            self.0.hit.lock().unwrap().push(callback);
            let mut count = self.0.lock.lock().unwrap();
            *count += 1;
            self.0.cv.notify_all();
        }
        fn warn_scale_failed(&self) {}
        fn scaler_error(&self, _bad_conversion: bool) {}
        fn log_skipped(&self, _skipped: i64, _total: i64) {}
        fn create_scaler(
            &self,
            _dst: &RawScale,
            _src: &RawScale,
        ) -> Result<Box<dyn Scaler>, ScalerFail> {
            if self.0.reenter_creation.swap(false, Ordering::SeqCst) {
                let ptr = self.0.output.load(Ordering::Acquire);
                // SAFETY: the test keeps its output alive throughout connect.
                assert!(unsafe { &*ptr }.connect(None, 1, 1, 0));
            }
            Ok(Box::new(Reenter {
                state: Arc::clone(&self.0),
                once: AtomicBool::new(true),
            }))
        }
    }

    #[test]
    fn scale_reenter_connects() {
        let state = Arc::new(State {
            output: AtomicPtr::new(std::ptr::null_mut()),
            reenter_creation: AtomicBool::new(false),
            self_remove: AtomicBool::new(false),
            replace_on_scale: AtomicBool::new(false),
            reconnects_left: AtomicUsize::new(0),
            hit: Mutex::new(Vec::new()),
            lock: Mutex::new(0),
            cv: Condvar::new(),
        });
        let output = VideoOutput::open(
            OpenInfo {
                format: VideoFormat::Rgba as i32,
                width: 4,
                height: 4,
                cache_size: 1,
                range: 0,
                colorspace: 0,
            },
            33_333_333,
            Box::new(TestHost(Arc::clone(&state))),
        )
        .expect("open");
        state.output.store(
            std::ptr::from_ref(&output) as *mut VideoOutput,
            Ordering::Release,
        );
        let conversion = RawScale {
            format: VideoFormat::Rgba as i32,
            width: 2,
            height: 4,
            range: 0,
            colorspace: 0,
        };
        assert!(output.connect(Some(conversion), 1, 1, 0));
        assert!(output.lock_frame(1, 1_000).is_some());
        output.unlock_frame();

        let count = state.lock.lock().unwrap();
        let (count, timeout) = state
            .cv
            .wait_timeout_while(count, Duration::from_secs(2), |count| *count < 2)
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "scale reentry did not deliver both inputs"
        );
        assert_eq!(*count, 2);
        drop(count);
        output.stop();
        assert_eq!(state.hit.lock().unwrap().as_slice(), &[1, 2]);
    }
    #[test]
    fn scaler_creation_reentry_does_not_duplicate_input() {
        let state = Arc::new(State {
            output: AtomicPtr::new(std::ptr::null_mut()),
            reenter_creation: AtomicBool::new(true),
            self_remove: AtomicBool::new(false),
            replace_on_scale: AtomicBool::new(false),
            reconnects_left: AtomicUsize::new(0),
            hit: Mutex::new(Vec::new()),
            lock: Mutex::new(0),
            cv: Condvar::new(),
        });
        let output = VideoOutput::open(
            OpenInfo {
                format: VideoFormat::Rgba as i32,
                width: 4,
                height: 4,
                cache_size: 1,
                range: 0,
                colorspace: 0,
            },
            33_333_333,
            Box::new(TestHost(Arc::clone(&state))),
        )
        .unwrap();
        state
            .output
            .store(std::ptr::from_ref(&output).cast_mut(), Ordering::Release);
        let conversion = RawScale {
            format: VideoFormat::Rgba as i32,
            width: 2,
            height: 4,
            range: 0,
            colorspace: 0,
        };
        assert!(!output.connect(Some(conversion), 1, 1, 0));
        assert!(output.disconnect(1, 0));
        assert!(!output.active(), "duplicate input survived disconnect");
        output.stop();
    }
    #[test]
    fn self_disconnect_still_delivers_following_input() {
        let state = Arc::new(State {
            output: AtomicPtr::new(std::ptr::null_mut()),
            reenter_creation: AtomicBool::new(false),
            self_remove: AtomicBool::new(true),
            replace_on_scale: AtomicBool::new(false),
            reconnects_left: AtomicUsize::new(0),
            hit: Mutex::new(Vec::new()),
            lock: Mutex::new(0),
            cv: Condvar::new(),
        });
        let output = VideoOutput::open(
            OpenInfo {
                format: VideoFormat::Rgba as i32,
                width: 4,
                height: 4,
                cache_size: 1,
                range: 0,
                colorspace: 0,
            },
            33_333_333,
            Box::new(TestHost(Arc::clone(&state))),
        )
        .unwrap();
        state
            .output
            .store(std::ptr::from_ref(&output).cast_mut(), Ordering::Release);
        assert!(output.connect(None, 1, 1, 0));
        assert!(output.connect(None, 1, 2, 0));
        assert!(output.lock_frame(1, 1_000).is_some());
        output.unlock_frame();
        let count = state.lock.lock().unwrap();
        let (count, _) = state
            .cv
            .wait_timeout_while(count, Duration::from_secs(2), |n| *n < 2)
            .unwrap();
        let delivered = *count;
        drop(count);
        output.stop();
        assert_eq!(delivered, 2, "self removal skipped the next input");
        assert_eq!(*state.hit.lock().unwrap(), vec![1, 2]);
    }
    #[test]
    fn self_reconnect_waits_for_the_next_frame() {
        let state = Arc::new(State {
            output: AtomicPtr::new(std::ptr::null_mut()),
            reenter_creation: AtomicBool::new(false),
            self_remove: AtomicBool::new(true),
            replace_on_scale: AtomicBool::new(false),
            reconnects_left: AtomicUsize::new(3),
            hit: Mutex::new(Vec::new()),
            lock: Mutex::new(0),
            cv: Condvar::new(),
        });
        let output = VideoOutput::open(
            OpenInfo {
                format: VideoFormat::Rgba as i32,
                width: 4,
                height: 4,
                cache_size: 1,
                range: 0,
                colorspace: 0,
            },
            33_333_333,
            Box::new(TestHost(Arc::clone(&state))),
        )
        .unwrap();
        state
            .output
            .store(std::ptr::from_ref(&output).cast_mut(), Ordering::Release);
        assert!(output.connect(None, 1, 1, 0));
        assert!(output.connect(None, 1, 2, 0));
        assert!(output.lock_frame(1, 1_000).is_some());
        output.unlock_frame();
        let count = state.lock.lock().unwrap();
        let (count, _) = state
            .cv
            .wait_timeout_while(count, Duration::from_secs(2), |n| *n < 2)
            .unwrap();
        let delivered = *count;
        drop(count);
        output.stop();
        assert_eq!(
            delivered, 2,
            "reconnection changed the frame delivery count"
        );
        assert_eq!(*state.hit.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn replaced_input_does_not_receive_old_scaler_buffers() {
        let state = Arc::new(State {
            output: AtomicPtr::new(std::ptr::null_mut()),
            reenter_creation: AtomicBool::new(false),
            self_remove: AtomicBool::new(false),
            reconnects_left: AtomicUsize::new(0),
            replace_on_scale: AtomicBool::new(true),
            hit: Mutex::new(Vec::new()),
            lock: Mutex::new(0),
            cv: Condvar::new(),
        });
        let output = VideoOutput::open(
            OpenInfo {
                format: VideoFormat::Rgba as i32,
                width: 4,
                height: 4,
                cache_size: 2,
                range: 0,
                colorspace: 0,
            },
            33_333_333,
            Box::new(TestHost(Arc::clone(&state))),
        )
        .unwrap();
        state
            .output
            .store(std::ptr::from_ref(&output).cast_mut(), Ordering::Release);
        let conversion = RawScale {
            format: VideoFormat::Rgba as i32,
            width: 2,
            height: 4,
            range: 0,
            colorspace: 0,
        };
        assert!(output.connect(Some(conversion), 1, 1, 0));
        assert!(output.lock_frame(1, 1_000).is_some());
        output.unlock_frame();
        let count = state.lock.lock().unwrap();
        let (count, _) = state
            .cv
            .wait_timeout_while(count, Duration::from_secs(2), |n| *n < 1)
            .unwrap();
        let delivered = *count;
        drop(count);
        if delivered != 1 {
            output.stop();
        }
        assert_eq!(delivered, 1, "other subscriber lost the first frame");
        assert!(output.lock_frame(1, 2_000).is_some());
        output.unlock_frame();
        let count = state.lock.lock().unwrap();
        let (count, _) = state
            .cv
            .wait_timeout_while(count, Duration::from_secs(2), |n| *n < 3)
            .unwrap();
        let delivered = *count;
        drop(count);
        output.stop();
        assert_eq!(delivered, 3, "replacement missed the next frame");
        assert_eq!(*state.hit.lock().unwrap(), vec![2, 1, 2]);
        let mut guard = output.shared.inputs.lock();
        guard.with_mut(|inputs| {
            let replacement = inputs.iter().find(|input| input.callback == 1).unwrap();
            assert!(replacement.scaler.is_none());
            assert!(replacement.buffers.is_empty());
        });
    }
}
