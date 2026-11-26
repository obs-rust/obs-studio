//! Linux side of the [platform boundary](super): real-time priority for the
//! audio thread (obs-rust/obs-studio#5770), the counterpart of the MMCSS
//! "Audio" class on Windows.
//!
//! [`crate::thread_priority::raise`] decides; this host does the side
//! effects: `RLIMIT_RTPRIO` and `RLIMIT_RTTIME`, `SCHED_RR`, the D-Bus calls
//! to the Realtime portal and rtkit (with the `gio` feature), and the
//! `SIGXCPU` guard.
//!
//! The guard: the portal and rtkit grant real time only with a process-wide
//! `RLIMIT_RTTIME`. A real-time thread that runs longer than the soft limit
//! without blocking gets `SIGXCPU`, and is killed at the hard limit. The
//! audio thread can do that after a long stall, when it mixes the missed
//! ticks without sleeping. The handler puts every registered audio thread
//! back to `SCHED_OTHER` (one `sched_setscheduler` call each, which is
//! async-signal-safe, keeping `SCHED_RESET_ON_FORK`, which an unprivileged
//! thread may not clear), so OBS loses real-time priority but keeps running.
//! It then calls the handler that was installed before it, if any.

use core::ffi::{c_int, c_void};
use std::sync::Mutex;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

use crate::thread_priority::{Limits, Outcome, Previous, RealtimeHost, Service, raise};

const LOG_WARNING: i32 = 200;
const LOG_INFO: i32 = 300;

/// What the first audio thread of the process found; later threads reuse it
/// (no D-Bus round trip per thread when nothing applies).
static PREVIOUS: Mutex<Previous> = Mutex::new(Previous::Unknown);

/// Audio threads the `SIGXCPU` guard demotes (0 = free slot).
const GUARD_SLOTS: usize = 16;
static GUARDED: [AtomicI32; GUARD_SLOTS] = [const { AtomicI32::new(0) }; GUARD_SLOTS];
/// Set by the handler; the audio thread logs it at its next tick.
static DEMOTED: AtomicBool = AtomicBool::new(false);
/// The handler installed before ours, and whether it takes `siginfo`.
static PREVIOUS_HANDLER: AtomicUsize = AtomicUsize::new(0);
static PREVIOUS_SIGINFO: AtomicBool = AtomicBool::new(false);
static INSTALL: Once = Once::new();
static INSTALLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigxcpu(signal: c_int, info: *mut libc::siginfo_t, context: *mut c_void) {
    let param = libc::sched_param { sched_priority: 0 };
    for slot in &GUARDED {
        let tid = slot.load(Ordering::Acquire);
        if tid != 0 {
            // Keep SCHED_RESET_ON_FORK: clearing it needs CAP_SYS_NICE, so
            // a plain SCHED_OTHER fails with EPERM and the thread stays
            // real-time until the kernel kills the process at the hard
            // limit (seen in the stall check on a real desktop).
            // SAFETY: a plain syscall wrapper, async-signal-safe; a thread
            // that has already exited only makes it fail with ESRCH.
            unsafe {
                libc::sched_setscheduler(tid, libc::SCHED_OTHER | libc::SCHED_RESET_ON_FORK, &param)
            };
        }
    }
    DEMOTED.store(true, Ordering::Release);
    let previous = PREVIOUS_HANDLER.load(Ordering::Acquire);
    if previous == libc::SIG_DFL || previous == libc::SIG_IGN {
        return;
    }
    if PREVIOUS_SIGINFO.load(Ordering::Acquire) {
        // SAFETY: the value came from `sa_sigaction` of a SA_SIGINFO handler.
        let handler: extern "C" fn(c_int, *mut libc::siginfo_t, *mut c_void) =
            unsafe { core::mem::transmute(previous) };
        handler(signal, info, context);
    } else {
        // SAFETY: the value came from `sa_sigaction` of a plain handler.
        let handler: extern "C" fn(c_int) = unsafe { core::mem::transmute(previous) };
        handler(signal);
    }
}

fn install_guard() -> Result<(), String> {
    INSTALL.call_once(|| {
        // SAFETY: a zeroed sigaction with our SA_SIGINFO handler and an
        // empty mask; `old` receives the handler it replaces.
        unsafe {
            let mut action: libc::sigaction = core::mem::zeroed();
            let handler: extern "C" fn(c_int, *mut libc::siginfo_t, *mut c_void) = on_sigxcpu;
            action.sa_sigaction = handler as usize;
            action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
            libc::sigemptyset(&mut action.sa_mask);
            let mut old: libc::sigaction = core::mem::zeroed();
            if libc::sigaction(libc::SIGXCPU, &action, &mut old) == 0 {
                PREVIOUS_HANDLER.store(old.sa_sigaction, Ordering::Release);
                PREVIOUS_SIGINFO.store(old.sa_flags & libc::SA_SIGINFO != 0, Ordering::Release);
                INSTALLED.store(true, Ordering::Release);
            }
        }
    });
    if INSTALLED.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

fn register(tid: i32) -> Result<(), String> {
    for slot in &GUARDED {
        if slot
            .compare_exchange(0, tid, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return Ok(());
        }
    }
    Err("too many audio threads".into())
}

fn unregister(tid: i32) {
    for slot in &GUARDED {
        let _ = slot.compare_exchange(tid, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

fn errno_text(code: c_int) -> String {
    std::io::Error::from_raw_os_error(code).to_string()
}

struct LinuxHost {
    #[cfg_attr(not(feature = "gio"), allow(dead_code))] // only the D-Bus calls name the process
    pid: i32,
    tid: i32,
    armed: bool,
}

#[cfg(feature = "gio")]
fn target(service: Service) -> &'static obs_util::ffi::dbus_realtime::Target {
    use obs_util::ffi::dbus_realtime::{PORTAL, RTKIT};
    match service {
        Service::Portal => &PORTAL,
        Service::Rtkit => &RTKIT,
    }
}

impl RealtimeHost for LinuxHost {
    fn rtprio_limit(&mut self) -> u64 {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `limit` is a live rlimit.
        if unsafe { libc::getrlimit(libc::RLIMIT_RTPRIO, &mut limit) } != 0 {
            return 0;
        }
        limit.rlim_cur
    }

    fn set_direct_realtime(&mut self, priority: u32) -> Result<(), String> {
        let param = libc::sched_param {
            sched_priority: priority as c_int,
        };
        // SCHED_RESET_ON_FORK: a process the thread starts does not inherit
        // real time.
        // SAFETY: the calling thread and a live sched_param.
        let error = unsafe {
            libc::pthread_setschedparam(
                libc::pthread_self(),
                libc::SCHED_RR | libc::SCHED_RESET_ON_FORK,
                &param,
            )
        };
        if error == 0 {
            Ok(())
        } else {
            Err(errno_text(error))
        }
    }

    #[cfg(feature = "gio")]
    fn limits(&mut self, service: Service) -> Result<Limits, String> {
        use obs_util::ffi::dbus_realtime::property;
        let target = target(service);
        let int = |name| {
            property(target, name)
                .and_then(|value| i32::try_from(value).map_err(|_| format!("{value} out of range")))
        };
        Ok(Limits {
            max_realtime_priority: int(c"MaxRealtimePriority")?,
            min_nice_level: int(c"MinNiceLevel")?,
            rttime_usec_max: property(target, c"RTTimeUSecMax")?,
        })
    }

    #[cfg(not(feature = "gio"))]
    fn limits(&mut self, _service: Service) -> Result<Limits, String> {
        Err("built without D-Bus".into())
    }

    fn set_rttime_limit(&mut self, soft_usec: u64, hard_usec: u64) -> Result<(), String> {
        let limit = libc::rlimit {
            rlim_cur: soft_usec,
            rlim_max: hard_usec,
        };
        // SAFETY: a live rlimit; the soft limit is below the hard limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_RTTIME, &limit) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }

    fn arm_demotion(&mut self) -> Result<(), String> {
        if self.armed {
            return Ok(());
        }
        install_guard()?;
        register(self.tid)?;
        self.armed = true;
        Ok(())
    }

    #[cfg(feature = "gio")]
    fn make_realtime(&mut self, service: Service, priority: u32) -> Result<(), String> {
        obs_util::ffi::dbus_realtime::make_thread_realtime(
            target(service),
            self.pid as u64,
            self.tid as u64,
            priority,
        )
    }

    #[cfg(not(feature = "gio"))]
    fn make_realtime(&mut self, _service: Service, _priority: u32) -> Result<(), String> {
        Err("built without D-Bus".into())
    }

    #[cfg(feature = "gio")]
    fn make_high_priority(&mut self, service: Service, nice: i32) -> Result<(), String> {
        obs_util::ffi::dbus_realtime::make_thread_high_priority(
            target(service),
            self.pid as u64,
            self.tid as u64,
            nice,
        )
    }

    #[cfg(not(feature = "gio"))]
    fn make_high_priority(&mut self, _service: Service, _nice: i32) -> Result<(), String> {
        Err("built without D-Bus".into())
    }
}

/// Real-time (or high) priority for the audio thread's lifetime, with the
/// `SIGXCPU` guard registered while it runs.
pub(crate) struct AudioThreadClass {
    tid: i32,
    armed: bool,
    line: Option<String>,
    demotion_logged: bool,
}

impl AudioThreadClass {
    pub(crate) fn enter(_samples_per_sec: u32) -> Self {
        // SAFETY: getpid and gettid have no preconditions.
        let (pid, tid) = unsafe { (libc::getpid(), libc::gettid()) };
        let mut host = LinuxHost {
            pid,
            tid,
            armed: false,
        };
        let outcome: Outcome = {
            let mut previous = PREVIOUS.lock().unwrap_or_else(|e| e.into_inner());
            let outcome = raise(&mut host, *previous);
            *previous = outcome.previous();
            outcome
        };
        let armed = host.armed && outcome.method.is_some();
        if host.armed && !armed {
            unregister(tid);
        }
        Self {
            tid,
            armed,
            line: Some(outcome.log_line()),
            demotion_logged: false,
        }
    }

    /// A line for the log, once at the first tick and once after a demotion.
    pub(crate) fn notice(&mut self) -> Option<(i32, String)> {
        if let Some(line) = self.line.take() {
            return Some((LOG_INFO, line));
        }
        if self.armed && !self.demotion_logged && DEMOTED.load(Ordering::Acquire) {
            self.demotion_logged = true;
            return Some((
                LOG_WARNING,
                "audio thread: real-time priority removed after SIGXCPU (RLIMIT_RTTIME), \
                 the thread continues at default priority"
                    .into(),
            ));
        }
        None
    }
}

impl Drop for AudioThreadClass {
    fn drop(&mut self) {
        if self.armed {
            unregister(self.tid);
        }
    }
}
