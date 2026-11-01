//! The value `os_cpu_usage_info_query` reports on every platform
//! (`libobs/util/platform-{nix,cocoa,windows}`): the process's share of the
//! machine in percent over one interval.

/// The share of the machine, in percent, from the process time `busy` the
/// process used over `wall` of real time (the same unit) on `cores` cores,
/// at most 100 (see [`cap`]).
pub fn percent(busy: f64, wall: f64, cores: f64) -> f64 {
    cap(busy / wall / cores * 100.0)
}

/// At most 100 %. The time counters tick coarsely (`times()`: 10 ms), so over
/// a short interval the quotient can come out above what the cores can run;
/// no process uses more than all of the machine (#3367). As in C
/// (`percent > 100.0 ? 100.0 : percent`), a NaN stays NaN.
pub fn cap(percent: f64) -> f64 {
    if percent > 100.0 { 100.0 } else { percent }
}
