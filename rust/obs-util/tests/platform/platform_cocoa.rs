//! Tier 1 for `libobs/util/platform-cocoa.m`.
//!
//! The test names follow `test/cmocka/test_platform_cocoa.c`. `cfstr_copy_*`
//! needs a `CFStringRef`, so that round trip lives in the parity test.

#![cfg(target_os = "macos")]

use obs_c_oracle as _;
use obs_util::platform::unix::cocoa;

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn written(buf: &[u8], n: i32) -> &[u8] {
    assert!(n > 0, "path length");
    let n = n as usize;
    assert!(n < buf.len());
    assert_eq!(buf[n], 0);
    &buf[..n]
}

#[test]
fn format_path_omits_the_slash_for_an_empty_name() {
    assert_eq!(cocoa::format_path(b"/base", None), b"/base");
    assert_eq!(cocoa::format_path(b"/base", Some(b"")), b"/base");
    assert_eq!(cocoa::format_path(b"/base", Some(b"obs")), b"/base/obs");
}

#[test]
fn format_path_ptr_always_appends_a_slash() {
    assert_eq!(cocoa::format_path_ptr(b"/base", None), b"/base/");
    assert_eq!(cocoa::format_path_ptr(b"/base", Some(b"")), b"/base/");
    assert_eq!(cocoa::format_path_ptr(b"/base", Some(b"obs")), b"/base/obs");
}

#[test]
fn cpu_percent_matches_the_query_formula() {
    assert_eq!(cocoa::cpu_percent(0, 0, 4), 0.0);
    assert_eq!(cocoa::cpu_percent(50, 100, 2), 25.0);
    // no cores read: the quotient is infinite, capped at 100 (#3367)
    assert_eq!(cocoa::cpu_percent(1, 1, 0), 100.0);
}

#[test]
fn inhibit_create_uses_a_null_cstring_only_for_a_null_reason() {
    assert!(cocoa::inhibit_uses_null_cstring(true));
    assert!(!cocoa::inhibit_uses_null_cstring(false));
}

#[test]
fn free_disk_cast_wraps_a_negative_free_space() {
    assert_eq!(cocoa::free_disk_cast(-1), u64::MAX);
    assert_eq!(cocoa::free_disk_cast(12), 12);
}

#[test]
fn test_config_path_application_support() {
    let mut buf = [0u8; 1024];
    let n = cocoa::config_path_into(Some(&mut buf), None);
    let path = written(&buf, n);
    assert!(contains(path, b"Application Support"));
    assert_ne!(path.last(), Some(&b'/'));

    let n = cocoa::config_path_into(Some(&mut buf), Some(b""));
    let path = written(&buf, n);
    assert_ne!(path.last(), Some(&b'/'));

    let n = cocoa::config_path_into(Some(&mut buf), Some(b"obs-cocoa-test"));
    let path = written(&buf, n);
    assert!(contains(path, b"Application Support/obs-cocoa-test"));

    let owned = cocoa::config_path_ptr(None);
    assert!(contains(&owned, b"Application Support"));
    assert_eq!(owned.last(), Some(&b'/'));

    let owned = cocoa::config_path_ptr(Some(b""));
    assert_eq!(owned.last(), Some(&b'/'));

    let owned = cocoa::config_path_ptr(Some(b"obs-cocoa-test"));
    assert!(contains(&owned, b"Application Support/obs-cocoa-test"));
    assert_ne!(owned.last(), Some(&b'/'));
}

#[test]
fn test_program_data_path() {
    let mut buf = [0u8; 1024];
    let n = cocoa::program_data_path_into(Some(&mut buf), None);
    let path = written(&buf, n);
    assert!(contains(path, b"Application Support"));
    assert_ne!(path.last(), Some(&b'/'));

    let owned = cocoa::program_data_path_ptr(Some(b"obs-cocoa-test"));
    assert!(contains(&owned, b"Application Support/obs-cocoa-test"));

    let owned = cocoa::program_data_path_ptr(None);
    assert_eq!(owned.last(), Some(&b'/'));
}

#[test]
fn test_executable_path_ptr() {
    let owned = cocoa::executable_path(None).expect("executable path");
    assert_eq!(owned.last(), Some(&b'/'));

    let owned = cocoa::executable_path(Some(b"")).expect("executable path");
    assert_eq!(owned.last(), Some(&b'/'));

    let owned = cocoa::executable_path(Some(b"obs-cocoa-name")).expect("executable path");
    assert!(contains(&owned, b"/obs-cocoa-name"));
}

#[test]
fn test_gettime_ns_monotonic() {
    let first = cocoa::gettime_ns();
    let second = cocoa::gettime_ns();
    assert!(first > 0);
    assert!(second >= first);
}

#[test]
fn test_core_counts_stable() {
    let physical = cocoa::physical_cores();
    let logical = cocoa::logical_cores();
    assert_eq!(physical, cocoa::physical_cores());
    assert_eq!(logical, cocoa::logical_cores());
    assert!(physical > 0);
    assert!(logical >= physical);
}

#[test]
fn test_sys_memory() {
    assert!(cocoa::sys_total_size() > 0);
    assert!(cocoa::sys_free_size() > 0);
    assert!(cocoa::proc_resident_size() > 0);
    assert!(cocoa::proc_virtual_size() > 0);
    let usage = cocoa::proc_memory().expect("proc memory");
    assert!(usage.resident > 0);
    assert!(usage.virtual_size > 0);
}

#[test]
fn test_emulation_status() {
    #[cfg(target_arch = "aarch64")]
    assert!(!cocoa::emulation_status());
    #[cfg(not(target_arch = "aarch64"))]
    let _ = cocoa::emulation_status();
}

#[test]
fn test_cpu_usage_null_query() {
    let mut info = cocoa::cpu_usage_start().expect("cpu usage");
    let sample = cocoa::cpu_usage_query(Some(&mut info));
    assert!(!sample.is_nan());
    assert_eq!(cocoa::cpu_usage_query(None), 0.0);
}

#[test]
fn test_high_performance_null_end() {
    let token = cocoa::request_high_performance(Some(b"obs-cocoa-test")).expect("activity");
    cocoa::end_high_performance(Some(token));
    cocoa::end_high_performance(None);
}

#[test]
fn test_inhibit_null() {
    let mut info = cocoa::inhibit_create(Some(b"ignored-on-purpose"));
    assert!(!cocoa::inhibit_set_active(None, true));
    assert!(!cocoa::inhibit_set_active(Some(&mut info), false));
    cocoa::inhibit_destroy(None);
    cocoa::inhibit_destroy(Some(info));
}

#[test]
fn test_free_space_null() {
    assert_eq!(cocoa::free_space(None), 0);
    assert_eq!(cocoa::free_disk_space(None), 0);
    assert!(cocoa::free_space(Some(b"/")) > 0);
}
