//! Push-to enable transitions, with the original lock and synchronous signal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Mute,
    Talk,
}
impl Kind {
    pub fn function(self) -> &'static str {
        match self {
            Self::Mute => "obs_source_enable_push_to_mute",
            Self::Talk => "obs_source_enable_push_to_talk",
        }
    }
}
/// Operations on a borrowed source. State and callbacks are accessed under the audio lock.
pub trait Source {
    fn lock(&mut self);
    fn unlock(&mut self);
    fn enabled(&self, kind: Kind) -> bool;
    fn output_flags(&self) -> u32;
    fn log_change(&mut self, kind: Kind, enabled: bool);
    fn store(&mut self, kind: Kind, enabled: bool);
    fn signal(&mut self, kind: Kind, enabled: bool);
}
pub fn enable<S: Source>(
    source: Option<&mut S>,
    kind: Kind,
    enabled: bool,
    null_log: impl FnOnce(&'static str),
) {
    let Some(source) = source else {
        null_log(kind.function());
        return;
    };
    source.lock();
    let changed = source.enabled(kind) != enabled;
    // Preserve the output-flag read even when the value is unchanged.
    let flags = source.output_flags();
    if crate::monitoring::push_to_logs(flags, changed) {
        source.log_change(kind, enabled);
    }
    source.store(kind, enabled);
    if changed {
        source.signal(kind, enabled);
    }
    source.unlock();
}
