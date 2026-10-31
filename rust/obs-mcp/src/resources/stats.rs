//! The `obs://stats` resource: frame rate, CPU usage and dropped frames (#3288).
//!
//! It talks to OBS through [`StatsBackend`], so it is tested with a fake.
//! Binding it to libobs is a follow-up (#3283).

use std::rc::Rc;

use super::{Resource, ResourceRegistry};

/// A snapshot of OBS performance counters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Frames per second the video thread is producing.
    pub fps: f64,
    /// CPU usage of the OBS process, in percent.
    pub cpu_usage: f64,
    /// Frames dropped (skipped or lagged) since OBS started.
    pub dropped_frames: u64,
}

/// The counters the resource needs from OBS.
pub trait StatsBackend {
    /// The current counters.
    fn stats(&self) -> Stats;
}

/// The `obs://stats` resource bound to a backend.
struct StatsResource {
    backend: Rc<dyn StatsBackend>,
}

impl Resource for StatsResource {
    fn uri(&self) -> &str {
        "obs://stats"
    }

    fn name(&self) -> &str {
        "stats"
    }

    fn description(&self) -> &str {
        "OBS performance: fps, cpu_usage (percent) and dropped_frames."
    }

    fn mime_type(&self) -> &str {
        "application/json"
    }

    fn read(&self) -> Result<String, String> {
        let _ = &self.backend;
        todo!("RED: obs://stats (#3288)")
    }
}

/// Registers `obs://stats` on `registry`, backed by `backend`.
pub fn register(registry: &mut ResourceRegistry, backend: &Rc<dyn StatsBackend>) {
    registry.register(Box::new(StatsResource {
        backend: Rc::clone(backend),
    }));
}
