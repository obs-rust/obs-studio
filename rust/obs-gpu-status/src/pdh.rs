//! Windows GPU utilization from the PDH `GPU Engine` counter set
//! (obs-rust/obs-studio#1022). Platform-neutral parsing and aggregation; the
//! sampler that reads the counters lives in `platform/windows.rs`.
//!
//! Each instance of `\GPU Engine(*)\Utilization Percentage` is named
//! `pid_<pid>_luid_0x<high>_0x<low>_phys_<n>_eng_<n>_engtype_<type>` and holds
//! one process's share of one engine. Task Manager's whole-adapter value is
//! the busiest engine, with each engine summed over processes.

use crate::Utilization;

/// One parsed `GPU Engine` counter instance name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdhInstance {
    pub pid: u32,
    /// The adapter LUID, `(high << 32) | low`, as DXGI reports it.
    pub luid: u64,
    pub phys: u32,
    pub eng: u32,
    pub engtype: String,
}

/// Parses an instance name; `None` when it does not have the expected shape.
pub fn parse_pdh_instance(name: &str) -> Option<PdhInstance> {
    fn dec(s: &str) -> Option<u32> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    }
    fn hex(s: &str) -> Option<u32> {
        let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(s, 16).ok()
    }
    let rest = name.strip_prefix("pid_")?;
    let (pid, rest) = rest.split_once("_luid_")?;
    let (high, rest) = rest.split_once('_')?;
    let (low, rest) = rest.split_once("_phys_")?;
    let (phys, rest) = rest.split_once("_eng_")?;
    let (eng, engtype) = rest.split_once("_engtype_")?;
    if engtype.is_empty() {
        return None;
    }
    Some(PdhInstance {
        pid: dec(pid)?,
        luid: (u64::from(hex(high)?) << 32) | u64::from(hex(low)?),
        phys: dec(phys)?,
        eng: dec(eng)?,
        engtype: engtype.to_owned(),
    })
}

/// Utilization of adapter `luid` (only process `pid`'s share when `Some`)
/// from `(instance name, percent)` samples: each engine `(phys, eng)` summed
/// over processes, the busiest engine, clamped to 0..=100 and rounded.
/// `Unavailable` when no sample matches.
pub fn pdh_utilization(samples: &[(String, f64)], luid: u64, pid: Option<u32>) -> Utilization {
    let mut engines: std::collections::BTreeMap<(u32, u32), f64> = Default::default();
    for (name, value) in samples {
        let Some(inst) = parse_pdh_instance(name) else {
            continue;
        };
        if inst.luid != luid || pid.is_some_and(|p| p != inst.pid) {
            continue;
        }
        let v = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        *engines.entry((inst.phys, inst.eng)).or_insert(0.0) += v;
    }
    match engines.values().copied().reduce(f64::max) {
        // Clamped to 0..=100 first, so the cast cannot truncate.
        Some(max) => Utilization::Percent(max.clamp(0.0, 100.0).round() as u8),
        None => Utilization::Unavailable,
    }
}

/// An open `GPU Engine` query that collects once per call (#1022). The real
/// one is in `platform/windows.rs`; tests give fakes.
pub trait EngineCounters: Send {
    /// The counter values since the previous call. `Some` with no values on
    /// the first call, which only sets the baseline; `None` when the query
    /// failed and has to be opened again.
    fn collect(&mut self) -> Option<Vec<(String, f64)>>;
}

/// Opens a query; `None` when PDH cannot (no GPU, counters disabled).
pub type OpenCounters = Box<dyn FnMut() -> Option<Box<dyn EngineCounters>> + Send>;

/// Keeps one query open between the label's ticks (#1022), so a tick costs
/// one collection and no wait: the previous tick is the baseline.
pub struct PdhTicker {
    open: OpenCounters,
    query: Option<Box<dyn EngineCounters>>,
}

impl PdhTicker {
    pub fn new(open: OpenCounters) -> Self {
        Self { open, query: None }
    }

    /// Utilization of adapter `luid` since the previous call. `Unavailable`
    /// on the first call after the query opens, and when PDH fails; a failed
    /// query is opened again on the next call.
    pub fn utilization(&mut self, luid: u64) -> Utilization {
        if self.query.is_none() {
            self.query = (self.open)();
        }
        let Some(query) = self.query.as_mut() else {
            return Utilization::Unavailable;
        };
        match query.collect() {
            Some(samples) => pdh_utilization(&samples, luid, None),
            None => {
                self.query = None;
                Utilization::Unavailable
            }
        }
    }
}
