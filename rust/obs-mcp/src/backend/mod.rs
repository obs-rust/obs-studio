//! `ObsBackend`: every backend trait over in-process libobs (#3283).
//!
//! RED stub: every method is `todo!()` until the GREEN commit.

use std::rc::Rc;

use serde_json::Value;

use crate::resources::stats::{Stats, StatsBackend};
use crate::server::Server;
use crate::tools::outputs::{OutputBackend, OutputKind, OutputStatus};
use crate::tools::scenes::SceneBackend;
use crate::tools::screenshot::{CaptureBackend, Frame};
use crate::tools::sources::{SourceBackend, SourceInfo, Transform};

/// The libobs-backed implementation of every backend trait.
#[derive(Default)]
pub struct ObsBackend {}

impl ObsBackend {
    /// A backend with no outputs configured.
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    /// Names the libobs outputs the record and stream tools control.
    #[must_use]
    pub fn with_output_names(self, _record: Option<&str>, _stream: Option<&str>) -> Self {
        todo!()
    }

    /// A server exposing every tool and resource over `backend`.
    #[must_use]
    pub fn server(_backend: &Rc<Self>) -> Server {
        todo!()
    }
}

impl SceneBackend for ObsBackend {
    fn scenes(&self) -> Vec<String> {
        todo!()
    }
    fn current(&self) -> Option<String> {
        todo!()
    }
    fn set_current(&self, _name: &str) -> Result<(), String> {
        todo!()
    }
    fn create(&self, _name: &str) -> Result<(), String> {
        todo!()
    }
    fn remove(&self, _name: &str) -> Result<(), String> {
        todo!()
    }
}

impl SourceBackend for ObsBackend {
    fn sources(&self) -> Vec<SourceInfo> {
        todo!()
    }
    fn create(&self, _kind: &str, _name: &str, _settings: &Value) -> Result<(), String> {
        todo!()
    }
    fn set_settings(&self, _name: &str, _settings: &Value) -> Result<(), String> {
        todo!()
    }
    fn set_item_visible(&self, _scene: &str, _source: &str, _visible: bool) -> Result<(), String> {
        todo!()
    }
    fn set_item_transform(
        &self,
        _scene: &str,
        _source: &str,
        _transform: &Transform,
    ) -> Result<(), String> {
        todo!()
    }
}

impl OutputBackend for ObsBackend {
    fn start(&self, _kind: OutputKind) -> Result<(), String> {
        todo!()
    }
    fn stop(&self, _kind: OutputKind) -> Result<(), String> {
        todo!()
    }
    fn status(&self, _kind: OutputKind) -> OutputStatus {
        todo!()
    }
}

impl CaptureBackend for ObsBackend {
    fn capture(
        &self,
        _source: Option<&str>,
        _width: Option<u32>,
        _height: Option<u32>,
    ) -> Result<Frame, String> {
        todo!()
    }
}

impl StatsBackend for ObsBackend {
    fn stats(&self) -> Stats {
        todo!()
    }
}
