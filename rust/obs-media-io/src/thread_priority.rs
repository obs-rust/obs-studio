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
        let reasons = if self.reasons.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.reasons.join("; "))
        };
        match self.method {
            Some(Method::Direct) => format!(
                "audio thread: real-time priority {} (SCHED_RR){reasons}",
                self.priority
            ),
            Some(Method::Realtime(service)) => format!(
                "audio thread: real-time priority {} through {}{reasons}",
                self.priority,
                service.name()
            ),
            Some(Method::HighPriority(service)) => format!(
                "audio thread: high priority, nice {}, through {}{reasons}",
                self.priority,
                service.name()
            ),
            None => format!("audio thread: default priority{reasons}"),
        }
    }

    /// What the next audio thread of this process should do.
    pub fn previous(&self) -> Previous {
        match self.method {
            Some(method) => Previous::Worked(method),
            None => Previous::Nothing,
        }
    }
}

/// The steps one call runs: everything, or only the method that worked.
struct Steps {
    direct: bool,
    realtime: Option<Service>,
    high: Option<Service>,
    services: &'static [Service],
}

fn steps(previous: Previous) -> Option<Steps> {
    const ALL: &[Service] = &Service::ORDER;
    const PORTAL: &[Service] = &[Service::Portal];
    const RTKIT: &[Service] = &[Service::Rtkit];
    let only = |service| match service {
        Service::Portal => PORTAL,
        Service::Rtkit => RTKIT,
    };
    match previous {
        Previous::Nothing => None,
        Previous::Unknown => Some(Steps {
            direct: true,
            realtime: None,
            high: None,
            services: ALL,
        }),
        Previous::Worked(Method::Direct) => Some(Steps {
            direct: true,
            realtime: None,
            high: None,
            services: &[],
        }),
        Previous::Worked(Method::Realtime(service)) => Some(Steps {
            direct: false,
            realtime: Some(service),
            high: None,
            services: only(service),
        }),
        Previous::Worked(Method::HighPriority(service)) => Some(Steps {
            direct: false,
            realtime: None,
            high: Some(service),
            services: only(service),
        }),
    }
}

/// `RLIMIT_RTTIME` for a service maximum: hard at the maximum, soft at half,
/// so the `SIGXCPU` guard runs before the kernel kills at the hard limit.
fn rttime_limits(limits: &Limits) -> Option<(u64, u64)> {
    let hard = u64::try_from(limits.rttime_usec_max)
        .ok()
        .filter(|&hard| hard > 1)?;
    Some((hard / 2, hard))
}

/// Raises the calling thread's priority with the first method that applies.
pub fn raise(host: &mut impl RealtimeHost, previous: Previous) -> Outcome {
    let mut reasons = Vec::new();
    let Some(steps) = steps(previous) else {
        return Outcome {
            method: None,
            priority: 0,
            reasons: vec!["no method applied for an earlier audio thread".into()],
        };
    };
    let done = |method, priority, reasons| Outcome {
        method: Some(method),
        priority,
        reasons,
    };

    if steps.direct {
        let limit = host.rtprio_limit();
        if limit > 0 {
            let priority = AUDIO_RT_PRIORITY.min(u32::try_from(limit).unwrap_or(u32::MAX));
            // An in-process PipeWire client can set RLIMIT_RTTIME for the
            // process, so the direct path needs the guard as well.
            match host.arm_demotion() {
                Err(error) => reasons.push(format!("SIGXCPU guard: {error}")),
                Ok(()) => match host.set_direct_realtime(priority) {
                    Ok(()) => return done(Method::Direct, priority as i32, reasons),
                    Err(error) => reasons.push(format!("SCHED_RR: {error}")),
                },
            }
        }
    }

    // Real time through the first service that grants it.
    let mut reachable = Vec::new();
    for &service in steps.services {
        let limits = match host.limits(service) {
            Ok(limits) => limits,
            Err(error) => {
                reasons.push(format!("{}: {error}", service.name()));
                continue;
            }
        };
        reachable.push((service, limits));
        if steps.high.is_some() || steps.realtime.is_some_and(|only| only != service) {
            continue;
        }
        let Ok(max) = u32::try_from(limits.max_realtime_priority) else {
            reasons.push(format!("{}: no real-time priority allowed", service.name()));
            continue;
        };
        if max == 0 {
            reasons.push(format!("{}: no real-time priority allowed", service.name()));
            continue;
        }
        let Some((soft, hard)) = rttime_limits(&limits) else {
            reasons.push(format!("{}: no RLIMIT_RTTIME maximum", service.name()));
            continue;
        };
        if let Err(error) = host.set_rttime_limit(soft, hard) {
            reasons.push(format!("RLIMIT_RTTIME: {error}"));
            continue;
        }
        if let Err(error) = host.arm_demotion() {
            reasons.push(format!("SIGXCPU guard: {error}"));
            continue;
        }
        let priority = AUDIO_RT_PRIORITY.min(max);
        match host.make_realtime(service, priority) {
            Ok(()) => return done(Method::Realtime(service), priority as i32, reasons),
            Err(error) => reasons.push(format!("{}: {error}", service.name())),
        }
    }

    // High priority through the first reachable service that grants it.
    for (service, limits) in reachable {
        if steps.high.is_some_and(|only| only != service) {
            continue;
        }
        let nice = HIGH_PRIORITY_NICE.max(limits.min_nice_level);
        if nice >= 0 {
            // A service without rtkit behind it reports MinNiceLevel 0: the
            // request could not raise anything.
            reasons.push(format!("{}: no higher priority allowed", service.name()));
            continue;
        }
        match host.make_high_priority(service, nice) {
            Ok(()) => return done(Method::HighPriority(service), nice, reasons),
            Err(error) => reasons.push(format!("{} (high priority): {error}", service.name())),
        }
    }

    Outcome {
        method: None,
        priority: 0,
        reasons,
    }
}
