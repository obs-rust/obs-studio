//! C ABI for `libobs/obs-task-queue.c.h`: `obs_in_task_thread()`,
//! `obs_queue_task()` and `obs_wait_for_destroy_queue()` (`EXPORT` in
//! `obs.h`, `rust-exports.d`).
//!
//! Each runs the safe core ([`crate::task_queue`]) on `Seam`, which reads
//! the thread flags, the UI task handler, the destruction task queue, the
//! video thread's flag and the audio output, and locks and pushes the
//! graphics and audio task queues, through the private adapters of
//! `obs-task-queue-fields.c.h`, so `struct obs_core` and the two thread-locals
//! (`is_graphics_thread`, `is_audio_thread`, still defined in `obs.c.h`) are
//! never mirrored. The other calls (the UI thread mark, the `os_task_queue_*`,
//! `os_event_*` and `os_sleep_ms` calls, `blog`) go through [`Calls`] too, so
//! the parity test hands both sides the same recorder; [`Libobs`] is
//! libobs's own.
//!
//! `task_wait_callback` is [`task_wait_callback`]`::<C>`: it reads the
//! waiter through its raw pointer and holds no reference to it past the
//! signal or the flag, after which the waiting thread may return.
//!
//! One intentional difference: a task routed to run in place with a NULL
//! function does nothing, where the C calls NULL.

use core::ffi::{c_char, c_int, c_void};
use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use core::sync::atomic::Ordering;

use crate::task_queue::{self as tq, Host, Locked, Log, Waiter};

/// `obs_task_t`, NULL as `None`.
pub type ObsTask = Option<unsafe extern "C" fn(param: *mut c_void)>;

/// The reads and calls of the entry points, one C function each.
///
/// # Safety
/// Each method is the C function its doc names, called as the C calls it,
/// with the core initialized.
pub unsafe trait Calls {
    /// `obs_task_queue_graphics_thread()`: `is_graphics_thread`.
    fn graphics_thread() -> bool;
    /// `obs_task_queue_audio_thread()`: `is_audio_thread`.
    fn audio_thread() -> bool;
    /// `obs_core_is_ui_thread()`.
    fn ui_thread() -> bool;
    /// `obs_task_queue_destruction()`: `obs->destruction_task_thread`.
    fn destruction() -> *mut c_void;
    /// `os_task_queue_inside(queue)`.
    ///
    /// # Safety
    /// `queue` is what [`Calls::destruction`] gave.
    unsafe fn inside(queue: *mut c_void) -> bool;
    /// `obs_task_queue_has_ui_handler()`: `obs->ui_task_handler != NULL`.
    fn has_ui_handler() -> bool;
    /// `obs_task_queue_call_ui_handler(task, param, wait)`:
    /// `obs->ui_task_handler(task, param, wait)`.
    ///
    /// # Safety
    /// [`Calls::has_ui_handler`] was true; `task` and `param` are the
    /// caller's.
    unsafe fn call_ui_handler(task: ObsTask, param: *mut c_void, wait: bool);
    /// `blog(level, format)`, or `blog(level, format, value)`.
    ///
    /// # Safety
    /// `format` is a static NUL-terminated format that takes one `int` when
    /// `value` is given and none otherwise.
    unsafe fn log(level: c_int, format: *const c_char, value: Option<c_int>);
    /// `os_event_init(event, kind)`.
    ///
    /// # Safety
    /// `event` is writable.
    unsafe fn event_init(event: *mut *mut c_void, kind: c_int) -> c_int;
    /// `os_event_signal(event)`.
    ///
    /// # Safety
    /// `event` is live.
    unsafe fn event_signal(event: *mut c_void);
    /// `os_event_wait(event)`.
    ///
    /// # Safety
    /// As [`Calls::event_signal`].
    unsafe fn event_wait(event: *mut c_void);
    /// `os_event_destroy(event)`.
    ///
    /// # Safety
    /// `event` is NULL or live, and not used after.
    unsafe fn event_destroy(event: *mut c_void);
    /// `os_sleep_ms(ms)`.
    fn sleep_ms(ms: u32);
    /// `obs_task_queue_lock_graphics()`.
    fn lock_graphics();
    /// `obs_task_queue_push_graphics(task, param)`.
    ///
    /// # Safety
    /// The graphics task lock is held; `task` and `param` are the caller's.
    unsafe fn push_graphics(task: ObsTask, param: *mut c_void);
    /// `obs_task_queue_unlock_graphics()`.
    fn unlock_graphics();
    /// `obs_task_queue_lock_audio()`.
    fn lock_audio();
    /// `obs_task_queue_push_audio(task, param)`.
    ///
    /// # Safety
    /// As [`Calls::push_graphics`], for the audio task lock.
    unsafe fn push_audio(task: ObsTask, param: *mut c_void);
    /// `obs_task_queue_unlock_audio()`.
    fn unlock_audio();
    /// `os_task_queue_queue_task(queue, task, param)`.
    ///
    /// # Safety
    /// `queue` is what [`Calls::destruction`] gave; `task` and `param` are
    /// the caller's.
    unsafe fn queue_destroy(queue: *mut c_void, task: ObsTask, param: *mut c_void) -> bool;
    /// `os_task_queue_wait(queue)`.
    ///
    /// # Safety
    /// As [`Calls::inside`].
    unsafe fn wait_destroy(queue: *mut c_void) -> bool;
    /// `obs_task_queue_video_thread_initialized()`.
    fn video_thread_initialized() -> bool;
    /// `obs_task_queue_has_audio()`: `obs->audio.audio != NULL`.
    fn has_audio() -> bool;
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_task_queue_graphics_thread() -> bool;
    fn obs_task_queue_audio_thread() -> bool;
    fn obs_task_queue_destruction() -> *mut c_void;
    fn obs_task_queue_has_ui_handler() -> bool;
    fn obs_task_queue_call_ui_handler(task: ObsTask, param: *mut c_void, wait: bool);
    fn obs_task_queue_lock_graphics();
    fn obs_task_queue_push_graphics(task: ObsTask, param: *mut c_void);
    fn obs_task_queue_unlock_graphics();
    fn obs_task_queue_lock_audio();
    fn obs_task_queue_push_audio(task: ObsTask, param: *mut c_void);
    fn obs_task_queue_unlock_audio();
    fn obs_task_queue_video_thread_initialized() -> bool;
    fn obs_task_queue_has_audio() -> bool;
    fn os_task_queue_inside(queue: *mut c_void) -> bool;
    fn os_task_queue_queue_task(queue: *mut c_void, task: ObsTask, param: *mut c_void) -> bool;
    fn os_task_queue_wait(queue: *mut c_void) -> bool;
    fn os_event_init(event: *mut *mut c_void, kind: c_int) -> c_int;
    fn os_event_signal(event: *mut c_void) -> c_int;
    fn os_event_wait(event: *mut c_void) -> c_int;
    fn os_event_destroy(event: *mut c_void);
    fn os_sleep_ms(ms: u32);
    fn blog(level: c_int, format: *const c_char, ...);
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's values.
unsafe impl Calls for Libobs {
    fn graphics_thread() -> bool {
        // SAFETY: reads this thread's flag.
        unsafe { obs_task_queue_graphics_thread() }
    }
    fn audio_thread() -> bool {
        // SAFETY: reads this thread's flag.
        unsafe { obs_task_queue_audio_thread() }
    }
    fn ui_thread() -> bool {
        crate::ui_task_handler::is_ui_thread()
    }
    fn destruction() -> *mut c_void {
        // SAFETY: reads the initialized core's field, as the C did.
        unsafe { obs_task_queue_destruction() }
    }
    unsafe fn inside(queue: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { os_task_queue_inside(queue) }
    }
    fn has_ui_handler() -> bool {
        // SAFETY: reads the initialized core's field, as the C did.
        unsafe { obs_task_queue_has_ui_handler() }
    }
    unsafe fn call_ui_handler(task: ObsTask, param: *mut c_void, wait: bool) {
        // SAFETY: see the trait.
        unsafe { obs_task_queue_call_ui_handler(task, param, wait) }
    }
    unsafe fn log(level: c_int, format: *const c_char, value: Option<c_int>) {
        // SAFETY: see the trait; the argument count matches the format.
        unsafe {
            match value {
                Some(value) => blog(level, format, value),
                None => blog(level, format),
            }
        }
    }
    unsafe fn event_init(event: *mut *mut c_void, kind: c_int) -> c_int {
        // SAFETY: see the trait.
        unsafe { os_event_init(event, kind) }
    }
    unsafe fn event_signal(event: *mut c_void) {
        // SAFETY: see the trait; the C ignores the result.
        unsafe { os_event_signal(event) };
    }
    unsafe fn event_wait(event: *mut c_void) {
        // SAFETY: see the trait; the C ignores the result.
        unsafe { os_event_wait(event) };
    }
    unsafe fn event_destroy(event: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { os_event_destroy(event) }
    }
    fn sleep_ms(ms: u32) {
        // SAFETY: a value only.
        unsafe { os_sleep_ms(ms) }
    }
    fn lock_graphics() {
        // SAFETY: the initialized core's mutex.
        unsafe { obs_task_queue_lock_graphics() }
    }
    unsafe fn push_graphics(task: ObsTask, param: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_task_queue_push_graphics(task, param) }
    }
    fn unlock_graphics() {
        // SAFETY: the lock this thread took.
        unsafe { obs_task_queue_unlock_graphics() }
    }
    fn lock_audio() {
        // SAFETY: the initialized core's mutex.
        unsafe { obs_task_queue_lock_audio() }
    }
    unsafe fn push_audio(task: ObsTask, param: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_task_queue_push_audio(task, param) }
    }
    fn unlock_audio() {
        // SAFETY: the lock this thread took.
        unsafe { obs_task_queue_unlock_audio() }
    }
    unsafe fn queue_destroy(queue: *mut c_void, task: ObsTask, param: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { os_task_queue_queue_task(queue, task, param) }
    }
    unsafe fn wait_destroy(queue: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { os_task_queue_wait(queue) }
    }
    fn video_thread_initialized() -> bool {
        // SAFETY: reads the initialized core's field, as the C did.
        unsafe { obs_task_queue_video_thread_initialized() }
    }
    fn has_audio() -> bool {
        // SAFETY: reads the initialized core's field, as the C did.
        unsafe { obs_task_queue_has_audio() }
    }
}

/// An `obs_task_t` and its param, as the caller gave them.
#[derive(Clone, Copy, Debug)]
pub struct Task {
    /// The function, NULL as `None`.
    pub func: ObsTask,
    /// Its param, passed back unchanged.
    pub param: *mut c_void,
}

/// `struct task_wait_info` of the bridge.
type Info = Waiter<Task, NonNull<c_void>>;

/// `task_wait_callback`: [`tq::task_wait_callback`] on the waiter `param`
/// points to.
///
/// # Safety
/// `param` is a live [`Info`] whose waiter waits for this call; nothing of it
/// is read after the signal or the flag.
unsafe extern "C" fn task_wait_callback<C: Calls>(param: *mut c_void) {
    let info = param.cast::<Info>().cast_const();
    // SAFETY: live per the contract; the fields are read by value, so no
    // reference to the waiter outlives this statement.
    let (task, event) = unsafe { ((*info).task, (*info).event) };
    tq::task_wait_callback(&mut Seam::<C>::new(), task, event, || {
        // SAFETY: still live: without an event, the waiter polls this flag
        // and returns only after it is set.
        unsafe { (*info).done.store(true, Ordering::SeqCst) }
    });
}

/// The core's [`Host`] over the adapters and `C`.
///
/// Its methods are safe because the core passes back only what it was
/// given (the caller's task and param, the destruction queue as the core
/// holds it, the events it created); a seam is made only by the `*_with`
/// functions below and the wait callback.
struct Seam<C: Calls> {
    calls: PhantomData<C>,
}

impl<C: Calls> Seam<C> {
    const fn new() -> Self {
        Self { calls: PhantomData }
    }
}

impl<C: Calls> Host for Seam<C> {
    type Task = Task;
    type Event = NonNull<c_void>;
    type Queue = *mut c_void;

    fn graphics_thread(&mut self) -> bool {
        C::graphics_thread()
    }
    fn audio_thread(&mut self) -> bool {
        C::audio_thread()
    }
    fn ui_thread(&mut self) -> bool {
        C::ui_thread()
    }
    fn destruction(&mut self) -> *mut c_void {
        C::destruction()
    }
    fn inside(&mut self, queue: *mut c_void) -> bool {
        // SAFETY: the core's destruction queue as it came.
        unsafe { C::inside(queue) }
    }
    fn has_ui_handler(&mut self) -> bool {
        C::has_ui_handler()
    }
    fn call_ui_handler(&mut self, task: Task, wait: bool) {
        // SAFETY: the route asked has_ui_handler just before; the caller's
        // task and param as they came.
        unsafe { C::call_ui_handler(task.func, task.param, wait) }
    }
    fn run(&mut self, task: Task) {
        if let Some(func) = task.func {
            // SAFETY: the caller's task with its own param, run on the
            // thread the route chose, as the C runs it.
            unsafe { func(task.param) }
        }
    }
    fn log(&mut self, log: Log) {
        let value = match log {
            Log::Unknown(task_type) => Some(task_type),
            Log::UiNone | Log::QueueEventFailed | Log::DestroyEventFailed => None,
        };
        // SAFETY: the static format takes one int exactly for Unknown.
        unsafe { C::log(log.level(), log.format().as_ptr(), value) }
    }
    fn event_init(&mut self, kind: i32) -> (bool, Option<NonNull<c_void>>) {
        let mut event = ptr::null_mut();
        // SAFETY: a local out-pointer.
        let failed = unsafe { C::event_init(&raw mut event, kind) } != 0;
        (failed, NonNull::new(event))
    }
    fn event_signal(&mut self, event: NonNull<c_void>) {
        // SAFETY: an event this wait created and has not destroyed.
        unsafe { C::event_signal(event.as_ptr()) }
    }
    fn event_wait(&mut self, event: NonNull<c_void>) {
        // SAFETY: as in event_signal.
        unsafe { C::event_wait(event.as_ptr()) }
    }
    fn event_destroy(&mut self, event: Option<NonNull<c_void>>) {
        let event = event.map_or(ptr::null_mut(), NonNull::as_ptr);
        // SAFETY: this wait's event, destroyed once at its end, or NULL.
        unsafe { C::event_destroy(event) }
    }
    fn sleep_ms(&mut self, ms: u32) {
        C::sleep_ms(ms);
    }
    fn lock(&mut self, queue: Locked) {
        match queue {
            Locked::Graphics => C::lock_graphics(),
            Locked::Audio => C::lock_audio(),
        }
    }
    fn push(&mut self, queue: Locked, task: Task) {
        // SAFETY: the core took the lock of this queue just before; the
        // caller's task and param as they came.
        unsafe {
            match queue {
                Locked::Graphics => C::push_graphics(task.func, task.param),
                Locked::Audio => C::push_audio(task.func, task.param),
            }
        }
    }
    fn unlock(&mut self, queue: Locked) {
        match queue {
            Locked::Graphics => C::unlock_graphics(),
            Locked::Audio => C::unlock_audio(),
        }
    }
    fn queue_destroy(&mut self, queue: *mut c_void, task: Task) {
        // SAFETY: the core's destruction queue; the caller's task and param.
        unsafe { C::queue_destroy(queue, task.func, task.param) };
    }
    fn wait_destroy(&mut self, queue: *mut c_void) -> bool {
        // SAFETY: the core's destruction queue as it came.
        unsafe { C::wait_destroy(queue) }
    }
    fn video_thread_initialized(&mut self) -> bool {
        C::video_thread_initialized()
    }
    fn has_audio(&mut self) -> bool {
        C::has_audio()
    }
    fn wait_task(&mut self, waiter: &Info) -> Task {
        Task {
            func: Some(task_wait_callback::<C>),
            param: ptr::from_ref(waiter).cast_mut().cast(),
        }
    }
}

/// [`obs_in_task_thread`] over `C`.
///
/// # Safety
/// The core is initialized.
pub unsafe fn in_task_thread_with<C: Calls>(task_type: c_int) -> bool {
    tq::in_task_thread(&mut Seam::<C>::new(), task_type)
}

/// [`obs_queue_task`] over `C`.
///
/// # Safety
/// The core is initialized; `task` is NULL or takes `param`, which stays
/// valid until the task ran (until this returns, with `wait`).
pub unsafe fn queue_task_with<C: Calls>(
    task_type: c_int,
    task: ObsTask,
    param: *mut c_void,
    wait: bool,
) {
    tq::queue_task(
        &mut Seam::<C>::new(),
        task_type,
        Task { func: task, param },
        wait,
    );
}

/// [`obs_wait_for_destroy_queue`] over `C`.
///
/// # Safety
/// The core is initialized.
pub unsafe fn wait_for_destroy_queue_with<C: Calls>() -> bool {
    tq::wait_for_destroy_queue(&mut Seam::<C>::new())
}
