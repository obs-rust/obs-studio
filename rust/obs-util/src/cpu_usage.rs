//! The value `os_cpu_usage_info_query` reports on every platform
//! (`libobs/util/platform-{nix,cocoa,windows}`): the process's share of the
//! machine in percent over one interval.

/// The share of the machine, in percent, from the process time `busy` the
/// process used over `wall` of real time (the same unit) on `cores` cores.
pub fn percent(busy: f64, wall: f64, cores: f64) -> f64 {
    busy / wall / cores * 100.0
}
