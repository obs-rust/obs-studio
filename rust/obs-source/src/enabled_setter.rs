//! Store the enabled flag before dispatching the synchronous enable signal.

pub trait Source {
    fn store_enabled(&self, enabled: bool);
    fn signal_enable(&self, enabled: bool);
}

pub fn set_enabled(source: Option<&impl Source>, enabled: bool, log_null: impl FnOnce()) {
    let _ = (source, enabled, log_null);
    todo!("port obs_source_set_enabled")
}
