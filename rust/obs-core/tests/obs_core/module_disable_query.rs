//! Safe mirrors of test_module_disable_query.c (#5166, #5183).
//!
//! `membership_dispatch` mirrors the C macro-spy case: membership is asked
//! first and once with the borrowed name itself, then the policy once with
//! that answer, and the policy's answer is returned unchanged.
//! `generated_core_names` mirrors the generated-list case over a model of
//! the generated matcher (case-sensitive exact match) and the real Rust
//! policy, for a non-empty and the valid empty list.
use core::ptr;

use obs_core::module_disable_query::{Dependencies, get_module_allow_disable};
use obs_core::module_load_rules::allow_disable;

/// Records the dependency calls, as the C test's `recorded_*` spies do.
struct Recorded {
    expected_name: *const str,
    membership: bool,
    permission: bool,
    calls: u32,
}

impl<'a> Dependencies<&'a str> for Recorded {
    fn in_core_module_list(&mut self, name: &'a str) -> bool {
        assert_eq!(self.calls, 0, "membership is asked first");
        self.calls += 1;
        assert!(ptr::eq(name, self.expected_name), "borrowed name identity");
        self.membership
    }

    fn allow_disable(&mut self, core_name: bool) -> bool {
        assert_eq!(self.calls, 1, "policy is asked after membership");
        self.calls += 1;
        assert_eq!(core_name, self.membership);
        self.permission
    }
}

#[test]
fn membership_dispatch() {
    let name = String::from("dispatch-token");
    // Vary the policy result independently: the wrapper must forward it,
    // rather than repeat the existing Rust policy or normalize the name.
    for i in 0..4u32 {
        let mut deps = Recorded {
            expected_name: name.as_str(),
            membership: i & 1 != 0,
            permission: i & 2 != 0,
            calls: 0,
        };
        assert_eq!(
            get_module_allow_disable(&mut deps, name.as_str()),
            deps.permission
        );
        assert_eq!(deps.calls, 2);
    }
}

/// The generated matcher's contract (`strcmp()` over the build list) and
/// the existing Rust policy.
struct Generated<'l> {
    list: &'l [&'l str],
}

impl Dependencies<&str> for Generated<'_> {
    fn in_core_module_list(&mut self, name: &str) -> bool {
        self.list.contains(&name)
    }

    fn allow_disable(&mut self, core_name: bool) -> bool {
        allow_disable(core_name)
    }
}

fn toggle_ascii_case(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

fn check_generated(list: &[&str]) {
    let mut deps = Generated { list };
    for name in list {
        assert!(!get_module_allow_disable(&mut deps, *name), "{name}");
        let changed = toggle_ascii_case(name);
        // A differently cased entry could itself be in a future build list.
        let listed = list.contains(&changed.as_str());
        assert_eq!(
            get_module_allow_disable(&mut deps, changed.as_str()),
            !listed,
            "{changed}"
        );
    }
}

#[test]
fn generated_core_names() {
    check_generated(&[
        "obs-ffmpeg",
        "image-source",
        "obs-x264",
        "Mixed-Case",
        "mIXED-cASE",
        "rtmp-services",
    ]);
    // A zero-length core list is valid. Do not invent a platform core name.
    check_generated(&[]);
}

#[test]
fn names_outside_the_list_may_be_disabled() {
    let mut deps = Generated {
        list: &["obs-ffmpeg", "image-source"],
    };
    for name in [
        "",
        "obs",
        "obs-ffmpeg ",
        " obs-ffmpeg",
        "obs-ffmpe",
        "OBS-FFMPEG",
    ] {
        assert!(get_module_allow_disable(&mut deps, name), "{name:?}");
    }
}
