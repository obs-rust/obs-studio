//! Real-time priority for the audio thread (obs-rust/obs-studio#5770).
//!
//! The decision is host-neutral: which method to try, in which order, with
//! which priority and CPU-time limit. Every side effect (the scheduler call,
//! `RLIMIT_RTTIME`, the D-Bus calls) goes through [`RealtimeHost`], so the
//! order and the fallbacks are tested against a fake on every target.
//!
//! The order follows PipeWire's `module-rt`:
//! 1. the direct scheduler call, when `RLIMIT_RTPRIO` allows it;
//! 2. the `org.freedesktop.portal.Realtime` portal (covers Flatpak);
//! 3. rtkit (`org.freedesktop.RealtimeKit1`).
//!
//! If no real-time request applies, the thread asks for high priority
//! (a negative nice level). If nothing applies, it runs at default priority.
//! A refused request never stops the thread.
//!
//! The portal and rtkit grant real time only with `RLIMIT_RTTIME` set. The
//! limit is process-wide, and a thread that uses more CPU time than the
//! soft limit without blocking gets `SIGXCPU`. After a long stall, the
//! audio thread catches up by mixing many ticks without sleeping, so the
//! host must arm a guard first that demotes the thread on `SIGXCPU`
//! ([`RealtimeHost::arm_demotion`]). The soft limit is half of the hard
//! limit, so the guard runs before the kernel kills the process. The direct
//! path arms the guard too: an in-process PipeWire client (the screen
//! capture plugin) can set `RLIMIT_RTTIME` for the whole process.

/// The real-time priority the audio thread asks for, clamped to what the
/// host or service allows. The Linux measurement in #5458 used this value.
pub const AUDIO_RT_PRIORITY: u32 = 20;

/// The nice level of the high-priority fallback (rtkit's default minimum).
pub const HIGH_PRIORITY_NICE: i32 = -11;

/// A D-Bus service that grants real-time or high priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// `org.freedesktop.portal.Realtime` on the session bus.
    Portal,
    /// `org.freedesktop.RealtimeKit1` on the system bus.
    Rtkit,
}

impl Service {
    /// The services in the order they are tried.
    pub const ORDER: [Service; 2] = [Service::Portal, Service::Rtkit];

    pub fn name(self) -> &'static str {
        match self {
            Service::Portal => "the Realtime portal",
            Service::Rtkit => "rtkit",
        }
    }
}

/// The method that applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `SCHED_RR` set by the thread itself.
    Direct,
    /// Real-time scheduling granted by a service.
    Realtime(Service),
    /// A negative nice level granted by a service.
    HighPriority(Service),
}

/// The limits a service reports (its D-Bus properties).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// `MaxRealtimePriority`.
    pub max_realtime_priority: i32,
    /// `MinNiceLevel`.
    pub min_nice_level: i32,
    /// `RTTimeUSecMax`: the largest `RLIMIT_RTTIME` the service accepts.
    pub rttime_usec_max: i64,
}

/// What an earlier audio thread of this process found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Previous {
    /// No audio thread has asked yet.
    Unknown,
    /// Nothing applied: do not ask again (no D-Bus round trip per thread).
    Nothing,
    /// This method applied: ask for it again.
    Worked(Method),
}

/// Every side effect of [`raise`]. The Linux host implements it with the
/// scheduler, `setrlimit`, a `SIGXCPU` handler and GIO; tests use a fake.
pub trait RealtimeHost {
    /// The soft `RLIMIT_RTPRIO` (0 when real time is not allowed directly).
    fn rtprio_limit(&mut self) -> u64;
    /// `SCHED_RR` at `priority` for the calling thread.
    fn set_direct_realtime(&mut self, priority: u32) -> Result<(), String>;
    /// The limits `service` reports; an error when it is not reachable.
    fn limits(&mut self, service: Service) -> Result<Limits, String>;
    /// `RLIMIT_RTTIME` for the process, in microseconds.
    fn set_rttime_limit(&mut self, soft_usec: u64, hard_usec: u64) -> Result<(), String>;
    /// Installs the `SIGXCPU` guard that demotes the calling thread.
    fn arm_demotion(&mut self) -> Result<(), String>;
    /// Asks `service` for real-time `priority` for the calling thread.
    fn make_realtime(&mut self, service: Service, priority: u32) -> Result<(), String>;
    /// Asks `service` for the nice level `nice` for the calling thread.
    fn make_high_priority(&mut self, service: Service, nice: i32) -> Result<(), String>;
}

/// The result for one audio thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The method that applied, if any.
    pub method: Option<Method>,
    /// The real-time priority, or the nice level for high priority.
    pub priority: i32,
    /// Why each earlier method did not apply, in the order tried.
    pub reasons: Vec<String>,
}

impl Outcome {
    /// The startup log line.
    pub fn log_line(&self) -> String {
        todo!("#5770")
    }

    /// What the next audio thread of this process should do.
    pub fn previous(&self) -> Previous {
        todo!("#5770")
    }
}

/// Raises the calling thread's priority with the first method that applies.
pub fn raise(host: &mut impl RealtimeHost, previous: Previous) -> Outcome {
    let _ = (host, previous);
    todo!("#5770")
}
