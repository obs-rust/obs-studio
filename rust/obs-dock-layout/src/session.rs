//! One main window's dock layout between the Qt adapter and the [`Model`]
//! (obs-rust/obs-studio#5475): the layout blob the window started with, and
//! when to take a snapshot. The adapter maps Qt events to these calls, builds
//! the snapshot and applies the output; it decides nothing.

use crate::{Cause, Model, Output, Snapshot};

/// The decisions of one main window, around its [`Model`].
#[derive(Debug, Default)]
pub struct Session {
    model: Model,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// The window restored `blob` (SHA-256 `blob_hash`) with the saved
    /// `DockSizes` `record` (none: Reset Docks, a layout with no record):
    /// [`Model::start`]. The session keeps the blob: a save may write it,
    /// and [`Output::restore_again`] restores it.
    pub fn start(&mut self, record: Option<&str>, _blob: &[u8], blob_hash: &str) -> bool {
        self.model.start(record, blob_hash)
    }

    /// [`Model::observe`].
    pub fn observe(&mut self, snap: &Snapshot, cause: Cause) -> Output {
        self.model.observe(snap, cause)
    }

    /// The blob a save writes instead of the live layout: the started blob
    /// while [`Model::saves_restored_blob`]; none for an empty blob.
    pub fn blob_to_save(&self) -> Option<&[u8]> {
        None
    }

    /// The blob of the last [`Session::start`], to restore again when
    /// [`Output::restore_again`] says so.
    pub fn restored_blob(&self) -> &[u8] {
        &[]
    }

    /// Something changed for `cause`. The strongest cause since the last
    /// snapshot is kept (`Restored` over `SeparatorDrag` over `Change`).
    /// True when the adapter must post a turn of the event loop that calls
    /// [`Session::turn`]: none is posted yet.
    pub fn queue(&mut self, _cause: Cause) -> bool {
        false
    }

    /// The left button went down on the window: a separator drag may start.
    /// No snapshot is taken until the release.
    pub fn press(&mut self) {}

    /// The left button came up. After a press it queues
    /// [`Cause::SeparatorDrag`] and returns what [`Session::queue`] returns;
    /// without a press it does nothing.
    pub fn release(&mut self) -> bool {
        false
    }

    /// The posted turn of the event loop: the cause to take a snapshot for
    /// now, or none. Nothing before the first start (the queued cause is
    /// dropped); nothing during a drag (kept for the release); while the
    /// window is hidden, a queued `Restored` is kept for when it shows and
    /// anything else is dropped.
    pub fn turn(&mut self, _visible: bool) -> Option<Cause> {
        None
    }

    /// [`Model::record`].
    pub fn record(&self, blob_hash: &str) -> String {
        self.model.record(blob_hash)
    }

    pub fn model(&self) -> &Model {
        &self.model
    }
}
