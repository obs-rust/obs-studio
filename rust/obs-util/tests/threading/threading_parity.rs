//! Tier 3: the event/semaphore/thread-name C ABI against the original C,
//! compiled as an oracle. Unix only: on Windows these symbols come from
//! `util/threading-windows.c`, which this port does not replace.
//!
//! `os_event_t` and `os_sem_t` are opaque in `util/threading.h`, so this
//! port adds no `#[repr(C)]` struct and no layout test.
//!
//! What is compared: the return code of every call in a random op
//! sequence run single-threaded, so nothing can block. `wait` and
//! `sem_wait` are only issued when a shadow model says they return
//! immediately; `timedwait` uses 0/1 ms so an unsignalled wait
//! deterministically reports ETIMEDOUT.
//!
//! Excluded from comparison, not generated here:
//! - NULL event handles and a NULL out-pointer: the C dereferences them
//!   (UB); the shim returns EINVAL/EAGAIN as documented in ffi/threading.rs.
//! - `os_sem_destroy`/`os_event_destroy(NULL)` and `os_sem_post`/`os_sem_wait`
//!   NULL: defined in the C (-1 for the sem pair); pinned in
//!   `null_args_match` instead of generated.
//! - The `volatile bool signalled` read inside `pthread_cond_wait`: not
//!   observable through the API.
//! - `timedwait` with timeouts long enough to race scheduling.
//! - The wait clock itself: C deadlines are `CLOCK_REALTIME`/`gettimeofday`
//!   absolutes recomputed per iteration; Rust's `Condvar::wait_timeout` is
//!   a relative monotonic wait. They diverge only under a wall-clock jump
//!   mid-wait — a documented difference in `src/threading.rs`.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

#![cfg(unix)]

use core::ffi::{c_int, c_ulong};
use std::ffi::CString;
use std::ptr;
#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use obs_c_oracle::threading::{
    OracleEvent, OracleSem, oracle_os_event_destroy, oracle_os_event_init, oracle_os_event_reset,
    oracle_os_event_signal, oracle_os_event_timedwait, oracle_os_event_try, oracle_os_event_wait,
    oracle_os_sem_destroy, oracle_os_sem_init, oracle_os_sem_post, oracle_os_sem_trywait,
    oracle_os_sem_wait, oracle_os_set_thread_name,
};
use obs_util::ffi::threading::{
    os_event_destroy, os_event_init, os_event_reset, os_event_signal, os_event_timedwait,
    os_event_try, os_event_wait, os_sem_destroy, os_sem_init, os_sem_post, os_sem_wait,
    os_set_thread_name,
};
use obs_util::threading::Sem;
use proptest::prelude::*;

const MANUAL: c_int = 1;

/// One deterministic operation on an event or a semaphore.
#[derive(Debug, Clone)]
enum Op {
    /// `os_event_signal`
    Signal,
    /// `os_event_try` — consumes an auto signal.
    Try,
    /// `os_event_reset`
    Reset,
    /// `os_event_timedwait` with a 0 or 1 ms timeout.
    TimedWait(u64),
    /// `os_event_wait`, only issued while the model says it is signalled.
    WaitIfSignalled,
    /// `os_sem_post`
    Post,
    /// `os_sem_wait`, only issued while the model count is positive.
    WaitIfPositive,
    /// `os_sem_trywait` probe (oracle helper + [`Sem::try_wait`]): issued
    /// unconditionally so a phantom credit shows up as a return-code
    /// mismatch instead of an invisible extra.
    SemTry,
}

/// The C-callable event surface, once per implementation.
struct EventApi {
    init: unsafe fn(*mut *mut u8, c_int) -> c_int,
    destroy: unsafe fn(*mut u8),
    wait: unsafe fn(*mut u8) -> c_int,
    timedwait: unsafe fn(*mut u8, c_ulong) -> c_int,
    try_: unsafe fn(*mut u8) -> c_int,
    signal: unsafe fn(*mut u8) -> c_int,
    reset: unsafe fn(*mut u8),
}

/// The C-callable semaphore surface.
struct SemApi {
    init: unsafe fn(*mut *mut u8, c_int) -> c_int,
    destroy: unsafe fn(*mut u8),
    post: unsafe fn(*mut u8) -> c_int,
    wait: unsafe fn(*mut u8) -> c_int,
    try_: unsafe fn(*mut u8) -> c_int,
}

fn rust_event_api() -> EventApi {
    unsafe fn init(e: *mut *mut u8, t: c_int) -> c_int {
        unsafe { os_event_init(e.cast(), t) }
    }
    EventApi {
        init,
        destroy: |e| unsafe { os_event_destroy(e.cast()) },
        wait: |e| unsafe { os_event_wait(e.cast()) },
        timedwait: |e, ms| unsafe { os_event_timedwait(e.cast(), ms) },
        try_: |e| unsafe { os_event_try(e.cast()) },
        signal: |e| unsafe { os_event_signal(e.cast()) },
        reset: |e| unsafe { os_event_reset(e.cast()) },
    }
}

fn oracle_event_api() -> EventApi {
    unsafe fn init(e: *mut *mut u8, t: c_int) -> c_int {
        unsafe { oracle_os_event_init(e.cast::<*mut OracleEvent>(), t) }
    }
    EventApi {
        init,
        destroy: |e| unsafe { oracle_os_event_destroy(e.cast()) },
        wait: |e| unsafe { oracle_os_event_wait(e.cast()) },
        timedwait: |e, ms| unsafe { oracle_os_event_timedwait(e.cast(), ms) },
        try_: |e| unsafe { oracle_os_event_try(e.cast()) },
        signal: |e| unsafe { oracle_os_event_signal(e.cast()) },
        reset: |e| unsafe { oracle_os_event_reset(e.cast()) },
    }
}

fn rust_sem_api() -> SemApi {
    unsafe fn init(s: *mut *mut u8, v: c_int) -> c_int {
        unsafe { os_sem_init(s.cast(), v) }
    }
    SemApi {
        init,
        destroy: |s| unsafe { os_sem_destroy(s.cast()) },
        post: |s| unsafe { os_sem_post(s.cast()) },
        wait: |s| unsafe { os_sem_wait(s.cast()) },
        try_: |s| {
            if unsafe { &*s.cast::<Sem>() }.try_wait() {
                0
            } else {
                -1
            }
        },
    }
}

fn oracle_sem_api() -> SemApi {
    unsafe fn init(s: *mut *mut u8, v: c_int) -> c_int {
        unsafe { oracle_os_sem_init(s.cast::<*mut OracleSem>(), v) }
    }
    SemApi {
        init,
        destroy: |s| unsafe { oracle_os_sem_destroy(s.cast()) },
        post: |s| unsafe { oracle_os_sem_post(s.cast()) },
        wait: |s| unsafe { oracle_os_sem_wait(s.cast()) },
        try_: |s| unsafe { oracle_os_sem_trywait(s.cast()) },
    }
}

/// Run the op sequence against one implementation; return the return codes.
fn run(manual: bool, initial: c_int, ops: &[Op], api: &EventApi, sem_api: &SemApi) -> Vec<c_int> {
    let mut results = Vec::new();
    let mut event: *mut u8 = ptr::null_mut();
    let mut sem: *mut u8 = ptr::null_mut();
    let mut signalled = false;
    let mut count: i64 = initial as i64;

    results.push(unsafe { (api.init)(&mut event, if manual { MANUAL } else { 0 }) });
    results.push(unsafe { (sem_api.init)(&mut sem, initial) });
    assert!(!event.is_null() && !sem.is_null());

    for op in ops {
        let code = match *op {
            Op::Signal => {
                signalled = true;
                unsafe { (api.signal)(event) }
            }
            Op::Try => {
                let c = unsafe { (api.try_)(event) };
                if c == 0 {
                    signalled = manual;
                }
                c
            }
            Op::Reset => {
                signalled = false;
                unsafe {
                    (api.reset)(event);
                }
                continue;
            }
            Op::TimedWait(ms) => {
                let c = unsafe { (api.timedwait)(event, ms as c_ulong) };
                if c == 0 {
                    signalled = manual;
                }
                c
            }
            Op::WaitIfSignalled => {
                if !signalled {
                    continue;
                }
                let c = unsafe { (api.wait)(event) };
                signalled = manual;
                c
            }
            Op::Post => {
                count += 1;
                unsafe { (sem_api.post)(sem) }
            }
            Op::WaitIfPositive => {
                if count <= 0 {
                    continue;
                }
                let c = unsafe { (sem_api.wait)(sem) };
                count -= 1;
                c
            }
            Op::SemTry => {
                let c = unsafe { (sem_api.try_)(sem) };
                if c == 0 {
                    count -= 1;
                }
                c
            }
        };
        results.push(code);
    }

    unsafe {
        (api.destroy)(event);
        (sem_api.destroy)(sem);
    }
    results
}

/// NULL-argument behavior the C defines or the shim chooses: identical on
/// both sides where the C defines it.
#[test]
fn null_args_match() {
    unsafe {
        // Defined in the C: sem post/wait return -1 on NULL.
        assert_eq!(os_sem_post(ptr::null_mut()), -1);
        assert_eq!(oracle_os_sem_post(ptr::null_mut()), -1);
        assert_eq!(os_sem_wait(ptr::null_mut()), -1);
        assert_eq!(oracle_os_sem_wait(ptr::null_mut()), -1);
        // Defined in the C: destroying NULL is a no-op.
        os_sem_destroy(ptr::null_mut());
        oracle_os_sem_destroy(ptr::null_mut());
        os_event_destroy(ptr::null_mut());
        oracle_os_event_destroy(ptr::null_mut());
    }
}

/// errno slot access for the two OSes CI covers. On non-Apple POSIX the
/// C's `sem_init` sets errno itself; on macOS `semaphore_create` never
/// touches it.
#[cfg(target_os = "linux")]
fn errno_slot() -> *mut c_int {
    unsafe { libc::__errno_location() }
}
/// See `errno_slot` above.
#[cfg(target_os = "macos")]
fn errno_slot() -> *mut c_int {
    unsafe { libc::__error() }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn errno() -> c_int {
    unsafe { *errno_slot() }
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn set_errno(value: c_int) {
    unsafe { *errno_slot() = value };
}

/// `os_sem_init` with a negative value fails the same per-OS way.
#[test]
fn sem_init_negative_matches() {
    for value in [-1, i32::MIN] {
        let mut rust_sem: *mut Sem = ptr::null_mut();
        let mut oracle_sem: *mut OracleSem = ptr::null_mut();
        let rust = unsafe { os_sem_init(&mut rust_sem, value) };
        let oracle = unsafe { oracle_os_sem_init(&mut oracle_sem, value) };
        assert_eq!(rust, oracle, "sem_init({value})");
        // Both sides must return -1 itself on every Unix: C returns
        // `sem_init`'s return value (errno EINVAL is separate), and
        // macOS `semaphore_create` failure is mapped to -1.
        assert_eq!(rust, -1, "sem_init({value}) return code");
        assert_eq!(rust_sem.is_null(), oracle_sem.is_null());
        unsafe {
            os_sem_destroy(rust_sem);
            oracle_os_sem_destroy(oracle_sem);
        }
    }
}

/// A failed init must leave the same errno the C leaves: `sem_init`
/// sets EINVAL on non-Apple POSIX; the macOS path never touches errno.
/// Sentinel 424242 is chosen to differ from EINVAL on every platform.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn sem_init_errno_matches() {
    let mut rust_sem: *mut Sem = ptr::null_mut();
    let mut oracle_sem: *mut OracleSem = ptr::null_mut();

    set_errno(424242);
    let rust = unsafe { os_sem_init(&mut rust_sem, -1) };
    let rust_errno = errno();
    set_errno(424242);
    let oracle = unsafe { oracle_os_sem_init(&mut oracle_sem, -1) };
    let oracle_errno = errno();

    assert_eq!(rust, oracle);
    assert_eq!(rust_errno, oracle_errno);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(rust_errno, libc::EINVAL);
}

/// `sem_post` at saturation: C returns `sem_post`'s result, so a post past
/// SEM_VALUE_MAX is -1/EOVERFLOW on non-Apple POSIX, not a silent credit.
/// The implementations must agree on code and errno; on Linux the
/// expected values are pinned outright.
#[test]
fn sem_post_saturation_matches() {
    let mut rust_sem: *mut Sem = ptr::null_mut();
    let mut oracle_sem: *mut OracleSem = ptr::null_mut();
    let rust_init = unsafe { os_sem_init(&mut rust_sem, c_int::MAX) };
    let oracle_init = unsafe { oracle_os_sem_init(&mut oracle_sem, c_int::MAX) };
    assert_eq!(rust_init, oracle_init);
    if rust_sem.is_null() {
        // Init failed identically on both sides; nothing else to check.
        return;
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        set_errno(0);
        let rust = unsafe { os_sem_post(rust_sem) };
        let rust_errno = errno();
        set_errno(0);
        let oracle = unsafe { oracle_os_sem_post(oracle_sem) };
        let oracle_errno = errno();
        assert_eq!(rust, oracle);
        assert_eq!(rust_errno, oracle_errno);
        #[cfg(target_os = "linux")]
        {
            assert_eq!(rust, -1);
            assert_eq!(rust_errno, libc::EOVERFLOW);
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let rust = unsafe { os_sem_post(rust_sem) };
        let oracle = unsafe { oracle_os_sem_post(oracle_sem) };
        assert_eq!(rust, oracle);
    }

    unsafe {
        os_sem_destroy(rust_sem);
        oracle_os_sem_destroy(oracle_sem);
    }
}

/// A signal interrupting `os_sem_wait` must surface as the C's -1/EINTR,
/// not be absorbed into an indefinite block. Linux only: `sem_wait` is
/// documented to return EINTR there.
#[cfg(target_os = "linux")]
#[test]
fn sem_wait_eintr_matches() {
    unsafe extern "C" fn noop_handler(_: c_int) {}

    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        libc::sigemptyset(&mut sa.sa_mask);
        sa.sa_sigaction = noop_handler as *const () as usize;
        // No SA_RESTART: the wait must be interruptible.
        sa.sa_flags = 0;
        libc::sigaction(libc::SIGUSR1, &sa, ptr::null_mut());
    }

    unsafe fn rust_wait(s: *mut u8) -> c_int {
        unsafe { os_sem_wait(s.cast()) }
    }
    unsafe fn rust_post(s: *mut u8) -> c_int {
        unsafe { os_sem_post(s.cast()) }
    }
    unsafe fn oracle_wait(s: *mut u8) -> c_int {
        unsafe { oracle_os_sem_wait(s.cast()) }
    }
    unsafe fn oracle_post(s: *mut u8) -> c_int {
        unsafe { oracle_os_sem_post(s.cast()) }
    }

    /// Block in `wait` on an empty sem while SIGUSR1 is delivered
    /// repeatedly; return the code `wait` reported. If the implementation
    /// swallows the signal, release the waiter with one post so the test
    /// still terminates and reports the wrong 0.
    fn interrupted_wait(
        sem: usize,
        wait: unsafe fn(*mut u8) -> c_int,
        post: unsafe fn(*mut u8) -> c_int,
    ) -> c_int {
        let pthread = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let result = Arc::new(std::sync::Mutex::new(None));
        let handle = {
            let (pthread, done, result) =
                (Arc::clone(&pthread), Arc::clone(&done), Arc::clone(&result));
            std::thread::spawn(move || {
                pthread.store(unsafe { libc::pthread_self() } as usize, Ordering::SeqCst);
                let r = unsafe { wait(sem as *mut u8) };
                *result.lock().unwrap() = Some(r);
                done.store(true, Ordering::SeqCst);
            })
        };
        while pthread.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
        }
        let tid = pthread.load(Ordering::SeqCst) as libc::pthread_t;
        for _ in 0..10_000 {
            if done.load(Ordering::SeqCst) {
                break;
            }
            unsafe { libc::pthread_kill(tid, libc::SIGUSR1) };
            std::thread::yield_now();
        }
        if !done.load(Ordering::SeqCst) {
            unsafe { post(sem as *mut u8) };
        }
        handle.join().unwrap();
        result.lock().unwrap().expect("waiter recorded a return")
    }

    let mut rust_sem: *mut Sem = ptr::null_mut();
    let mut oracle_sem: *mut OracleSem = ptr::null_mut();
    assert_eq!(unsafe { os_sem_init(&mut rust_sem, 0) }, 0);
    assert_eq!(unsafe { oracle_os_sem_init(&mut oracle_sem, 0) }, 0);

    let rust = interrupted_wait(rust_sem as usize, rust_wait, rust_post);
    let oracle = interrupted_wait(oracle_sem as usize, oracle_wait, oracle_post);
    assert_eq!(rust, -1, "interrupted sem_wait must fail, not block");
    assert_eq!(rust, oracle);

    unsafe {
        os_sem_destroy(rust_sem);
        oracle_os_sem_destroy(oracle_sem);
    }
}

/// `os_set_thread_name` sets the same kernel-visible name in both
/// implementations, including past the glibc 15-byte limit.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn thread_name_matches() {
    use std::ffi::CStr;

    fn current_thread_name() -> String {
        let mut buf = [0u8; 64];
        // SAFETY: `buf` is a valid (buffer, size) pair; pthread_getname_np
        // writes the calling thread's name on Linux and macOS.
        assert_eq!(
            unsafe {
                libc::pthread_getname_np(libc::pthread_self(), buf.as_mut_ptr().cast(), buf.len())
            },
            0
        );
        // SAFETY: the buffer is NUL-terminated on success.
        unsafe { CStr::from_ptr(buf.as_ptr().cast()) }
            .to_string_lossy()
            .into_owned()
    }

    for name in [
        "t",
        "test-thread",
        "exactly-15-bytes",
        "a-very-long-thread-name-over-limit",
        "name with spaces",
        "",
    ] {
        let c_name = CString::new(name).unwrap();
        unsafe { os_set_thread_name(c_name.as_ptr()) };
        let rust = current_thread_name();
        unsafe { oracle_os_set_thread_name(c_name.as_ptr()) };
        let oracle = current_thread_name();
        assert_eq!(rust, oracle, "thread name for {name:?}");
    }
}

/// Deterministic regression for the double-post mutation: after
/// `Post` + `WaitIfPositive` the count is back to zero, so `SemTry`
/// must fail. A `post` that adds a second credit leaves a phantom
/// `SemTry` success — and unlike the proptest this does not depend on
/// a generated seed, so it catches the mutation on every platform
/// (the original codex report only saw it on Linux; on macOS a second
/// `semaphore_signal` diverges the same way once this sequence runs).
#[test]
fn parity_post_credit_probe() {
    let ops = [Op::Post, Op::WaitIfPositive, Op::SemTry];
    let rust = run(false, 0, &ops, &rust_event_api(), &rust_sem_api());
    let oracle = run(false, 0, &ops, &oracle_event_api(), &oracle_sem_api());
    assert_eq!(rust, oracle);
    // event_init, sem_init, Post, WaitIfPositive, SemTry: the SemTry
    // must fail — a double-post mutation leaves a phantom credit here.
    assert_eq!(rust, vec![0, 0, 0, 0, -1]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(64)))]

    /// Random op sequences produce identical return codes, starting the
    /// semaphore at arbitrary initial values so leftover initial credit
    /// is observable.
    #[test]
    fn parity(
        manual in any::<bool>(),
        initial in 0..4i32,
        ops in prop::collection::vec(
            prop_oneof![
                3 => Just(Op::Signal),
                3 => Just(Op::Try),
                1 => Just(Op::Reset),
                2 => prop::sample::select(vec![0u64, 1]).prop_map(Op::TimedWait),
                2 => Just(Op::WaitIfSignalled),
                3 => Just(Op::Post),
                2 => Just(Op::WaitIfPositive),
                2 => Just(Op::SemTry),
            ],
            0..64,
        )
    ) {
        let rust = run(manual, initial, &ops, &rust_event_api(), &rust_sem_api());
        let oracle = run(manual, initial, &ops, &oracle_event_api(), &oracle_sem_api());
        assert_eq!(rust, oracle);
    }
}
