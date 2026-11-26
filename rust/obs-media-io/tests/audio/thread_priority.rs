//! Tier 1 for the audio thread's real-time request (#5770): the order of the
//! methods, the clamps, the CPU-time limit and its guard, the fallbacks and
//! the log line, against a fake host that records every call. No test here
//! changes the scheduling or the limits of the test process.

use obs_media_io::thread_priority::{
    AUDIO_RT_PRIORITY, HIGH_PRIORITY_NICE, Limits, Method, Outcome, Previous, RealtimeHost,
    Service, TimeConstraint, ns_to_mach_abs, raise, time_constraint,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    RtprioLimit,
    Direct(u32),
    Limits(Service),
    RttimeLimit(u64, u64),
    ArmDemotion,
    Realtime(Service, u32),
    HighPriority(Service, i32),
}

/// A host whose answers the test sets, and that records each call.
struct Fake {
    rtprio_limit: u64,
    direct: Result<(), String>,
    portal: Result<Limits, String>,
    rtkit: Result<Limits, String>,
    rttime: Result<(), String>,
    arm: Result<(), String>,
    realtime_portal: Result<(), String>,
    realtime_rtkit: Result<(), String>,
    high_portal: Result<(), String>,
    high_rtkit: Result<(), String>,
    calls: Vec<Call>,
}

const RTKIT_DEFAULTS: Limits = Limits {
    max_realtime_priority: 20,
    min_nice_level: -15,
    rttime_usec_max: 200_000,
};

fn refused(what: &str) -> Result<(), String> {
    Err(format!("{what} refused"))
}

impl Fake {
    /// A desktop without RLIMIT_RTPRIO, without the portal, with rtkit.
    fn desktop() -> Self {
        Fake {
            rtprio_limit: 0,
            direct: refused("direct"),
            portal: Err("org.freedesktop.portal.Desktop not running".into()),
            rtkit: Ok(RTKIT_DEFAULTS),
            rttime: Ok(()),
            arm: Ok(()),
            realtime_portal: refused("portal"),
            realtime_rtkit: Ok(()),
            high_portal: refused("portal"),
            high_rtkit: Ok(()),
            calls: Vec::new(),
        }
    }
}

impl RealtimeHost for Fake {
    fn rtprio_limit(&mut self) -> u64 {
        self.calls.push(Call::RtprioLimit);
        self.rtprio_limit
    }
    fn set_direct_realtime(&mut self, priority: u32) -> Result<(), String> {
        self.calls.push(Call::Direct(priority));
        self.direct.clone()
    }
    fn limits(&mut self, service: Service) -> Result<Limits, String> {
        self.calls.push(Call::Limits(service));
        match service {
            Service::Portal => self.portal.clone(),
            Service::Rtkit => self.rtkit.clone(),
        }
    }
    fn set_rttime_limit(&mut self, soft_usec: u64, hard_usec: u64) -> Result<(), String> {
        self.calls.push(Call::RttimeLimit(soft_usec, hard_usec));
        self.rttime.clone()
    }
    fn arm_demotion(&mut self) -> Result<(), String> {
        self.calls.push(Call::ArmDemotion);
        self.arm.clone()
    }
    fn make_realtime(&mut self, service: Service, priority: u32) -> Result<(), String> {
        self.calls.push(Call::Realtime(service, priority));
        match service {
            Service::Portal => self.realtime_portal.clone(),
            Service::Rtkit => self.realtime_rtkit.clone(),
        }
    }
    fn make_high_priority(&mut self, service: Service, nice: i32) -> Result<(), String> {
        self.calls.push(Call::HighPriority(service, nice));
        match service {
            Service::Portal => self.high_portal.clone(),
            Service::Rtkit => self.high_rtkit.clone(),
        }
    }
}

#[test]
fn direct_realtime_when_rlimit_rtprio_allows_it_needs_no_service() {
    let mut host = Fake::desktop();
    host.rtprio_limit = 95;
    host.direct = Ok(());
    let outcome = raise(&mut host, Previous::Unknown);
    assert_eq!(outcome.method, Some(Method::Direct));
    assert_eq!(outcome.priority, AUDIO_RT_PRIORITY as i32);
    assert_eq!(
        host.calls,
        [
            Call::RtprioLimit,
            Call::ArmDemotion,
            Call::Direct(AUDIO_RT_PRIORITY)
        ]
    );
}

#[test]
fn direct_realtime_is_clamped_to_rlimit_rtprio() {
    let mut host = Fake::desktop();
    host.rtprio_limit = 15;
    host.direct = Ok(());
    let outcome = raise(&mut host, Previous::Unknown);
    assert_eq!(outcome.method, Some(Method::Direct));
    assert_eq!(outcome.priority, 15);
    assert_eq!(
        host.calls,
        [Call::RtprioLimit, Call::ArmDemotion, Call::Direct(15)]
    );
}

#[test]
fn portal_comes_before_rtkit_and_sets_the_cpu_time_limit_and_guard_first() {
    let mut host = Fake::desktop();
    host.portal = Ok(Limits {
        max_realtime_priority: 20,
        min_nice_level: -15,
        rttime_usec_max: 200_000,
    });
    host.realtime_portal = Ok(());
    let outcome = raise(&mut host, Previous::Unknown);
    assert_eq!(outcome.method, Some(Method::Realtime(Service::Portal)));
    assert_eq!(outcome.priority, 20);
    assert_eq!(
        host.calls,
        [
            Call::RtprioLimit,
            Call::Limits(Service::Portal),
            Call::RttimeLimit(100_000, 200_000),
            Call::ArmDemotion,
            Call::Realtime(Service::Portal, 20),
        ]
    );
}

#[test]
fn rtkit_when_the_portal_is_missing_with_priority_clamped_to_its_maximum() {
    let mut host = Fake::desktop();
    host.rtkit = Ok(Limits {
        max_realtime_priority: 10,
        ..RTKIT_DEFAULTS
    });
    let outcome = raise(&mut host, Previous::Unknown);
    assert_eq!(outcome.method, Some(Method::Realtime(Service::Rtkit)));
    assert_eq!(outcome.priority, 10);
    assert_eq!(
        host.calls,
        [
            Call::RtprioLimit,
            Call::Limits(Service::Portal),
            Call::Limits(Service::Rtkit),
            Call::RttimeLimit(100_000, 200_000),
            Call::ArmDemotion,
            Call::Realtime(Service::Rtkit, 10),
        ]
    );
    assert_eq!(outcome.reasons.len(), 1, "{:?}", outcome.reasons);
    assert!(
        outcome.reasons[0].contains("not running"),
        "{:?}",
        outcome.reasons
    );
}

/// The guard also covers the direct path: an in-process PipeWire client
/// (the screen capture plugin) can set `RLIMIT_RTTIME` for the process.
#[test]
fn no_direct_realtime_without_the_demotion_guard() {
    let mut host = Fake::desktop();
    host.rtprio_limit = 95;
    host.direct = Ok(());
    host.arm = Err("sigaction failed".into());
    let outcome = raise(&mut host, Previous::Unknown);
    assert!(
        !host.calls.iter().any(|c| matches!(c, Call::Direct(..))),
        "{:?}",
        host.calls
    );
    assert_eq!(outcome.method, Some(Method::HighPriority(Service::Rtkit)));
}

#[test]
fn no_realtime_request_without_the_demotion_guard() {
    let mut host = Fake::desktop();
    host.arm = Err("sigaction failed".into());
    let outcome = raise(&mut host, Previous::Unknown);
    assert!(
        !host.calls.iter().any(|c| matches!(c, Call::Realtime(..))),
        "{:?}",
        host.calls
    );
    // Without real time, the thread still gets high priority.
    assert_eq!(outcome.method, Some(Method::HighPriority(Service::Rtkit)));
}

#[test]
fn no_realtime_request_without_a_cpu_time_limit() {
    for (rttime_usec_max, setrlimit) in [(0, Ok(())), (200_000, refused("setrlimit"))] {
        let mut host = Fake::desktop();
        host.rtkit = Ok(Limits {
            rttime_usec_max,
            ..RTKIT_DEFAULTS
        });
        host.rttime = setrlimit.clone();
        let outcome = raise(&mut host, Previous::Unknown);
        assert!(
            !host.calls.iter().any(|c| matches!(c, Call::Realtime(..))),
            "{rttime_usec_max} {setrlimit:?}: {:?}",
            host.calls
        );
        assert_eq!(outcome.method, Some(Method::HighPriority(Service::Rtkit)));
    }
}

#[test]
fn high_priority_uses_the_service_minimum_nice_when_it_is_higher() {
    for (min_nice_level, nice) in [(-15, HIGH_PRIORITY_NICE), (-5, -5)] {
        let mut host = Fake::desktop();
        host.rtkit = Ok(Limits {
            min_nice_level,
            ..RTKIT_DEFAULTS
        });
        host.realtime_rtkit = refused("rtkit");
        let outcome = raise(&mut host, Previous::Unknown);
        assert_eq!(outcome.method, Some(Method::HighPriority(Service::Rtkit)));
        assert_eq!(outcome.priority, nice);
        assert_eq!(
            host.calls.last(),
            Some(&Call::HighPriority(Service::Rtkit, nice))
        );
    }
}

#[test]
fn nothing_applies_without_services_and_the_reasons_are_kept() {
    let mut host = Fake::desktop();
    host.rtkit = Err("org.freedesktop.RealtimeKit1 not running".into());
    let outcome = raise(&mut host, Previous::Unknown);
    assert_eq!(outcome.method, None);
    assert!(
        !host.calls.iter().any(|c| matches!(
            c,
            Call::RttimeLimit(..) | Call::ArmDemotion | Call::Realtime(..) | Call::HighPriority(..)
        )),
        "{:?}",
        host.calls
    );
    assert_eq!(outcome.reasons.len(), 2, "{:?}", outcome.reasons);
    assert_eq!(outcome.previous(), Previous::Nothing);
}

#[test]
fn a_process_that_found_nothing_does_not_ask_again() {
    let mut host = Fake::desktop();
    let outcome = raise(&mut host, Previous::Nothing);
    assert_eq!(outcome.method, None);
    assert!(host.calls.is_empty(), "{:?}", host.calls);
}

#[test]
fn a_later_thread_goes_straight_to_the_method_that_worked() {
    let mut host = Fake::desktop();
    let first = raise(&mut host, Previous::Unknown);
    assert_eq!(
        first.previous(),
        Previous::Worked(Method::Realtime(Service::Rtkit))
    );
    host.calls.clear();
    let second = raise(&mut host, first.previous());
    assert_eq!(second.method, Some(Method::Realtime(Service::Rtkit)));
    assert_eq!(
        host.calls,
        [
            Call::Limits(Service::Rtkit),
            Call::RttimeLimit(100_000, 200_000),
            Call::ArmDemotion,
            Call::Realtime(Service::Rtkit, 20),
        ]
    );
}

#[test]
fn the_log_line_names_the_method_and_the_reasons() {
    let line = |method, priority, reasons: &[&str]| {
        Outcome {
            method,
            priority,
            reasons: reasons.iter().map(|r| r.to_string()).collect(),
        }
        .log_line()
    };
    assert_eq!(
        line(Some(Method::Direct), 20, &[]),
        "audio thread: real-time priority 20 (SCHED_RR)"
    );
    assert_eq!(
        line(
            Some(Method::Realtime(Service::Rtkit)),
            20,
            &["portal: not running"]
        ),
        "audio thread: real-time priority 20 through rtkit (portal: not running)"
    );
    assert_eq!(
        line(Some(Method::HighPriority(Service::Portal)), -11, &[]),
        "audio thread: high priority, nice -11, through the Realtime portal"
    );
    assert_eq!(
        line(None, 0, &["a", "b"]),
        "audio thread: default priority (a; b)"
    );
}

/// The portal without rtkit behind it (seen on a real desktop): it answers,
/// but reports 0 for every limit. Nothing is requested, and the reasons say
/// why.
#[test]
fn a_service_that_allows_nothing_is_not_asked() {
    let mut host = Fake::desktop();
    host.portal = Ok(Limits::default());
    host.rtkit = Err("org.freedesktop.RealtimeKit1 not activatable".into());
    let outcome = raise(&mut host, Previous::Unknown);
    assert_eq!(outcome.method, None);
    assert!(
        !host.calls.iter().any(|c| matches!(
            c,
            Call::RttimeLimit(..) | Call::ArmDemotion | Call::Realtime(..) | Call::HighPriority(..)
        )),
        "{:?}",
        host.calls
    );
    assert!(
        outcome
            .reasons
            .iter()
            .any(|r| r.contains("no higher priority allowed")),
        "{:?}",
        outcome.reasons
    );
}

/// #5771: one audio tick is the period; a quarter of it is the computation,
/// half of it the constraint (the values measured in #5458).
#[test]
fn macos_time_constraint_follows_the_audio_tick() {
    assert_eq!(
        time_constraint(48_000),
        TimeConstraint {
            period_ns: 21_333_333,
            computation_ns: 5_333_333,
            constraint_ns: 10_666_666,
        }
    );
    assert_eq!(
        time_constraint(44_100),
        TimeConstraint {
            period_ns: 23_219_954,
            computation_ns: 5_804_988,
            constraint_ns: 11_609_977,
        }
    );
}

/// Apple silicon counts 24 MHz ticks (timebase 125/3), Intel counts
/// nanoseconds (1/1); the policy fields are 32 bits.
#[test]
fn nanoseconds_convert_to_mach_absolute_time() {
    assert_eq!(ns_to_mach_abs(21_333_333, 125, 3), 511_999);
    assert_eq!(ns_to_mach_abs(21_333_333, 1, 1), 21_333_333);
    assert_eq!(ns_to_mach_abs(0, 125, 3), 0);
    assert_eq!(ns_to_mach_abs(u64::MAX, 1, 1), u32::MAX);
    assert_eq!(
        ns_to_mach_abs(1_000, 0, 3),
        u32::MAX,
        "a zero numerator must not divide by zero"
    );
}
