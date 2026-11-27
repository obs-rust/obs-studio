//! `obs_source_audio_output_capture_device_changed()` (`obs.h`, `EXPORT`) and
//! its `activate` signal handler,
//! `obs_source_audio_output_capture_device_activated()` (no `EXPORT`): the
//! check an Audio Output Capture source runs when its device changes, which
//! starts or stops the monitoring deduplication.
//!
//! Everything the check reads and calls stays C and is reached through
//! [`Device`], one C step per method: the core's monitoring fields
//! (`obs->audio`), the source's output flags and name, the host's default
//! device (`obs_host_default_monitoring_device()`), the platform's
//! `devices_match()`, the `deduplication_changed` signal, the `activate`
//! connection, the log lines and the settings read.
#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `OBS_SOURCE_DO_NOT_SELF_MONITOR` (`obs-source.h`): the source captures
/// an output device, so it may hear the monitoring device.
pub const DO_NOT_SELF_MONITOR: u32 = 1 << 9;

/// What the check reads and does, each method one C step.
pub trait Device {
    /// A source as the C passes it (never checked for NULL, as in the C).
    type Source: Copy + PartialEq;
    /// A device id (`const char *`); `None` is NULL.
    type Id: Copy;
    /// The handler's `calldata_t *`.
    type Calldata: Copy;
    /// A source's settings (`obs_data_t *`).
    type Settings: Copy;

    /// `obs->audio.monitoring_device_name != NULL`.
    fn monitoring_device_set(&mut self) -> bool;
    /// `source->info.output_flags`.
    fn output_flags(&mut self, source: Self::Source) -> u32;
    /// `obs->audio.monitoring_device_id`.
    fn monitoring_id(&mut self) -> Option<Self::Id>;
    /// `obs_host_default_monitoring_device(device_id, &def_id)`: `None` when
    /// it returns false, else `Some(def_id)` (which may be NULL).
    fn host_default(&mut self, device_id: Option<Self::Id>) -> Option<Option<Self::Id>>;
    /// `devices_match(id1, id2)`.
    fn devices_match(&mut self, id1: Option<Self::Id>, id2: Option<Self::Id>) -> bool;
    /// `bfree(def_id)`, for a non-NULL id [`Device::host_default`] gave.
    fn free_default(&mut self, id: Self::Id);
    /// `deduplication_changed` on the core's signal handler, with `"source"`
    /// set to `source` (NULL for `None`).
    fn signal_deduplication(&mut self, source: Option<Self::Source>);
    /// Connects the source's `activate` signal to the activate handler.
    fn connect_activate(&mut self, source: Self::Source);
    /// Disconnects the source's `activate` signal from the activate handler.
    fn disconnect_activate(&mut self, source: Self::Source);
    /// The `LOG_INFO` line naming the source that now deduplicates.
    fn log_started(&mut self, source: Self::Source);
    /// `LOG_INFO "Deduplication logic stopped."`.
    fn log_stopped(&mut self);
    /// `obs->audio.monitoring_duplicating_source` (`None` is NULL).
    fn duplicating_source(&mut self) -> Option<Self::Source>;
    /// `calldata_ptr(cd, "source")` (`None` is NULL).
    fn calldata_source(&mut self, cd: Self::Calldata) -> Option<Self::Source>;
    /// `obs_source_get_settings(source)`.
    fn settings(&mut self, source: Self::Source) -> Self::Settings;
    /// `obs_data_get_string(settings, "device_id")`.
    fn settings_device_id(&mut self, settings: Self::Settings) -> Option<Self::Id>;
    /// `obs_data_release(settings)`.
    fn release_settings(&mut self, settings: Self::Settings);
}

/// `obs_source_audio_output_capture_device_changed(source, device_id)`:
/// nothing unless a monitoring device is set and the source has
/// [`DO_NOT_SELF_MONITOR`]. Then the monitoring id is read and matched
/// against the host's default device when the host answers (freeing a
/// non-NULL one after the match), else against `device_id`. A match sends
/// `deduplication_changed` with the source, connects its `activate`
/// handler and logs the started line, whoever deduplicated before. No
/// match stops only the deduplicating source: its handler disconnected,
/// `deduplication_changed` with NULL, the stopped line.
pub fn device_changed<D: Device>(device: &mut D, source: D::Source, device_id: Option<D::Id>) {
    if !device.monitoring_device_set() {
        return;
    }
    if device.output_flags(source) & DO_NOT_SELF_MONITOR == 0 {
        return;
    }
    let monitoring_id = device.monitoring_id();
    let id_match = match device.host_default(device_id) {
        Some(def_id) => {
            let id_match = device.devices_match(def_id, monitoring_id);
            if let Some(def_id) = def_id {
                device.free_default(def_id);
            }
            id_match
        }
        None => device.devices_match(device_id, monitoring_id),
    };
    if id_match {
        device.signal_deduplication(Some(source));
        device.connect_activate(source);
        device.log_started(source);
    } else if device.duplicating_source() == Some(source) {
        device.disconnect_activate(source);
        device.signal_deduplication(None);
        device.log_stopped();
    }
}

/// `obs_source_audio_output_capture_device_activated(NULL, cd)`: nothing
/// for a calldata with no source; otherwise [`device_changed`] with the
/// `"device_id"` of the source's settings, which are released after.
pub fn device_activated<D: Device>(device: &mut D, cd: D::Calldata) {
    let Some(source) = device.calldata_source(cd) else {
        return;
    };
    let settings = device.settings(source);
    let device_id = device.settings_device_id(settings);
    device_changed(device, source, device_id);
    device.release_settings(settings);
}
