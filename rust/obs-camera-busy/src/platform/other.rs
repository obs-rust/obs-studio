//! Hosts without V4L2: no process is signalled and no device is watched.

use std::path::Path;

/// The user OBS runs as: unknown here, so no holder can be closed.
pub fn current_uid() -> Option<u32> {
    None
}

/// The owner of a `/proc` entry: unknown here.
pub fn owner_uid(_path: &Path) -> Option<u32> {
    None
}

/// Signals that are never sent.
#[derive(Debug, Default)]
pub struct ProcessSignals;

impl crate::Signals for ProcessSignals {
    fn terminate(&mut self, _pid: u32) -> bool {
        false
    }

    fn kill(&mut self, _pid: u32) -> bool {
        false
    }

    fn alive(&mut self, _pid: u32) -> bool {
        false
    }

    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// A watcher that cannot start.
#[derive(Debug)]
pub struct Watcher;

impl Watcher {
    pub fn start(
        _device: &Path,
        _settle_ms: u64,
        _on_open: Box<dyn FnMut() + Send>,
    ) -> std::io::Result<Self> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
}
