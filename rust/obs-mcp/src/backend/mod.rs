//! `ObsBackend`: every backend trait over in-process libobs (#3283).
//!
//! One type implements [`SceneBackend`], [`SourceBackend`], [`OutputBackend`],
//! [`CaptureBackend`] and [`StatsBackend`] by calling libobs directly through
//! the safe wrappers in `crate::ffi`; nothing goes through obs-websocket.
//!
//! - **Scenes.** The program scene is the source on output channel 0. Removing
//!   it clears the channel.
//! - **Sources.** libobs destroys a source when its last reference is
//!   released, so the backend keeps a reference to every scene and source it
//!   creates until that object is removed or the backend is dropped. Other
//!   objects (made by the frontend or a script) are found by name.
//! - **Outputs.** libobs has no "the recording output": the frontend owns the
//!   encoders and the service. [`ObsBackend::with_output_names`] names the
//!   libobs outputs to control; unnamed ones report "not configured".
//! - **Screenshots.** A texrender draws the source and a stage surface reads
//!   it back, which needs the video subsystem. Without it the tool returns an
//!   error instead of crashing.
//!
//! The host starts libobs before creating the backend and drops the backend
//! before `obs_shutdown`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::ffi::{self, CpuUsage, Output, Source};
use crate::resources::stats::{Stats, StatsBackend};
use crate::resources::{self, ResourceRegistry};
use crate::server::{Server, ToolRegistry};
use crate::tools::outputs::{self, OutputBackend, OutputKind, OutputStatus};
use crate::tools::scenes::{self, SceneBackend};
use crate::tools::screenshot::{self, CaptureBackend, Frame};
use crate::tools::sources::{self, ItemInfo, ItemRef, SourceBackend, SourceInfo, Transform};

/// Largest screenshot side, to keep a request from allocating without bound.
const MAX_SIDE: u32 = 16_384;

/// The libobs-backed implementation of every backend trait.
pub struct ObsBackend {
    /// References to the scenes and sources created here, by name.
    owned: RefCell<HashMap<String, Source>>,
    record: Option<String>,
    stream: Option<String>,
    /// Directory a fresh recording file is put in on each `record_start`.
    record_dir: Option<PathBuf>,
    /// Why the host could not bring video up, quoted in the errors that need it.
    video_error: Option<String>,
    /// Why the host could not create the outputs, quoted in the errors that need them.
    output_error: Option<String>,
    cpu: Option<CpuUsage>,
}

impl Default for ObsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ObsBackend {
    /// A backend with no outputs configured.
    ///
    /// libobs must already be started.
    #[must_use]
    pub fn new() -> Self {
        crate::ffi::use_absolute_coordinates();
        Self {
            owned: RefCell::new(HashMap::new()),
            record: None,
            stream: None,
            record_dir: None,
            video_error: None,
            output_error: None,
            cpu: CpuUsage::start(),
        }
    }

    /// Records into a new `obs-mcp-<unix seconds>.mkv` in `dir` on each
    /// `record_start` (the directory is created when missing).
    #[must_use]
    pub fn with_record_dir(mut self, dir: &Path) -> Self {
        self.record_dir = Some(dir.to_owned());
        self
    }

    /// Reasons the host could not start video or create the outputs; the
    /// errors of the tools that depend on them say why.
    #[must_use]
    pub fn with_startup_errors(mut self, video: Option<String>, outputs: Option<String>) -> Self {
        self.video_error = video;
        self.output_error = outputs;
        self
    }

    /// `message`, followed by `reason` when there is one.
    fn because(message: &str, reason: Option<&str>) -> String {
        match reason {
            Some(reason) => format!("{message}: {reason}"),
            None => message.to_owned(),
        }
    }

    /// Names the libobs outputs the record and stream tools control.
    ///
    /// The outputs are looked up by name on every call, so they may be created
    /// after the backend.
    #[must_use]
    pub fn with_output_names(mut self, record: Option<&str>, stream: Option<&str>) -> Self {
        self.record = record.map(str::to_owned);
        self.stream = stream.map(str::to_owned);
        self
    }

    /// A server exposing every tool and resource over `backend`.
    #[must_use]
    pub fn server(backend: &Rc<Self>) -> Server {
        let scenes: Rc<dyn SceneBackend> = Rc::clone(backend) as _;
        let sources: Rc<dyn SourceBackend> = Rc::clone(backend) as _;
        let outputs: Rc<dyn OutputBackend> = Rc::clone(backend) as _;
        let capture: Rc<dyn CaptureBackend> = Rc::clone(backend) as _;
        let stats: Rc<dyn StatsBackend> = Rc::clone(backend) as _;

        let mut tools = ToolRegistry::new();
        scenes::register(&mut tools, &scenes);
        sources::register(&mut tools, &sources);
        outputs::register(&mut tools, &outputs);
        screenshot::register(&mut tools, &capture);

        let mut resources = ResourceRegistry::new();
        resources::scenes::register(&mut resources, &scenes, &sources);
        resources::stats::register(&mut resources, &stats);

        Server::with_tools(tools).with_resources(resources)
    }

    /// Fails when a source called `name` already exists.
    fn ensure_free(name: &str) -> Result<(), String> {
        if Source::by_name(name).is_some() {
            Err(format!("a source named '{name}' already exists"))
        } else {
            Ok(())
        }
    }

    /// The scene called `name`.
    fn scene(name: &str) -> Result<Source, String> {
        let source = Source::by_name(name).ok_or_else(|| format!("no scene named '{name}'"))?;
        if source.is_scene() {
            Ok(source)
        } else {
            Err(format!("'{name}' is not a scene"))
        }
    }

    /// The output of `kind`, configured and present in libobs.
    fn output(&self, kind: OutputKind) -> Result<Output, String> {
        let (label, name) = match kind {
            OutputKind::Record => ("record", &self.record),
            OutputKind::Stream => ("stream", &self.stream),
        };
        let name = name
            .as_deref()
            .ok_or_else(|| format!("the {label} output is not configured"))?;
        Output::by_name(name).ok_or_else(|| {
            Self::because(
                &format!("no output named '{name}'"),
                self.output_error.as_deref(),
            )
        })
    }

    /// Points the record output at a fresh file in the record directory.
    fn prepare_recording(&self, output: &Output) -> Result<(), String> {
        let Some(dir) = &self.record_dir else {
            return Ok(());
        };
        std::fs::create_dir_all(dir).map_err(|e| {
            format!(
                "cannot create the recording directory {}: {e}",
                dir.display()
            )
        })?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let path = dir.join(format!("obs-mcp-{stamp}.mkv"));
        output.update(&json!({"path": path.to_string_lossy()}))
    }
}

/// Sorts `(name, kind)` pairs by name so listings are stable.
fn sorted(mut list: Vec<(String, String)>) -> Vec<(String, String)> {
    list.sort();
    list
}

impl SceneBackend for ObsBackend {
    fn scenes(&self) -> Vec<String> {
        sorted(ffi::scenes())
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    fn current(&self) -> Option<String> {
        ffi::program().filter(Source::is_scene).map(|s| s.name())
    }

    fn set_current(&self, name: &str) -> Result<(), String> {
        let scene = Self::scene(name)?;
        ffi::set_program(Some(&scene));
        Ok(())
    }

    fn create(&self, name: &str) -> Result<(), String> {
        Self::ensure_free(name)?;
        let scene = Source::create_scene(name)?;
        // A client that makes the first scene wants to see it: with no
        // program scene the new one becomes the program scene (#3339).
        if self.current().is_none() {
            ffi::set_program(Some(&scene));
        }
        self.owned.borrow_mut().insert(name.to_owned(), scene);
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        let scene = Self::scene(name)?;
        if self.current().as_deref() == Some(name) {
            ffi::set_program(None);
        }
        scene.remove();
        self.owned.borrow_mut().remove(name);
        Ok(())
    }
}

impl SourceBackend for ObsBackend {
    fn sources(&self) -> Vec<SourceInfo> {
        sorted(ffi::inputs())
            .into_iter()
            .map(|(name, kind)| SourceInfo { name, kind })
            .collect()
    }

    fn create(&self, kind: &str, name: &str, settings: &Value) -> Result<(), String> {
        Self::ensure_free(name)?;
        let source = Source::create(kind, name, settings)?;
        self.owned.borrow_mut().insert(name.to_owned(), source);
        Ok(())
    }

    fn set_settings(&self, name: &str, settings: &Value) -> Result<(), String> {
        Source::by_name(name)
            .ok_or_else(|| format!("no source named '{name}'"))?
            .update(settings)
    }

    fn add_item(
        &self,
        scene: &str,
        source: &str,
        position: Option<(f64, f64)>,
    ) -> Result<ItemInfo, String> {
        let position = position.map(|(x, y)| (narrow(x), narrow(y)));
        Self::scene(scene)?
            .add_item(source, position)
            .map(item_info)
    }

    fn items(&self, scene: &str) -> Result<Vec<ItemInfo>, String> {
        Ok(Self::scene(scene)?
            .items()?
            .into_iter()
            .map(item_info)
            .collect())
    }

    fn set_item_visible(&self, scene: &str, item: &ItemRef, visible: bool) -> Result<(), String> {
        Self::scene(scene)?.with_item(&item_key(item), |item| item.set_visible(visible))
    }

    fn set_item_transform(
        &self,
        scene: &str,
        item: &ItemRef,
        transform: &Transform,
    ) -> Result<(), String> {
        Self::scene(scene)?.with_item(&item_key(item), |item| {
            let (mut pos, mut scale, mut rot) = item.transform();
            if let Some((x, y)) = transform.position {
                pos = (narrow(x), narrow(y));
            }
            if let Some((x, y)) = transform.scale {
                scale = (narrow(x), narrow(y));
            }
            if let Some(degrees) = transform.rotation {
                rot = narrow(degrees);
            }
            item.set_transform(pos, scale, rot);
        })
    }
}

/// The libobs lookup key for `item`.
fn item_key(item: &ItemRef) -> ffi::ItemKey<'_> {
    match item {
        ItemRef::Id(id) => ffi::ItemKey::Id(*id),
        ItemRef::Source(name) => ffi::ItemKey::Source(name),
    }
}

/// The tool-facing form of an item snapshot.
fn item_info(item: ffi::ItemState) -> ItemInfo {
    let pair = |(x, y): (f32, f32)| (f64::from(x), f64::from(y));
    ItemInfo {
        id: item.id,
        source: item.source,
        visible: item.visible,
        position: pair(item.position),
        scale: pair(item.scale),
        rotation: f64::from(item.rotation),
    }
}

/// libobs stores transforms as `f32`.
#[allow(clippy::cast_possible_truncation, reason = "libobs transforms are f32")]
fn narrow(value: f64) -> f32 {
    value as f32
}

impl OutputBackend for ObsBackend {
    fn start(&self, kind: OutputKind) -> Result<(), String> {
        let output = self.output(kind)?;
        if output.active() {
            return Err("the output is already running".to_owned());
        }
        if kind == OutputKind::Record {
            self.prepare_recording(&output)?;
        }
        output.start()
    }

    fn stop(&self, kind: OutputKind) -> Result<(), String> {
        let output = self.output(kind)?;
        if !output.active() {
            return Err("the output is not running".to_owned());
        }
        output.stop();
        Ok(())
    }

    fn status(&self, kind: OutputKind) -> OutputStatus {
        let Ok(output) = self.output(kind) else {
            return OutputStatus::default();
        };
        if !output.active() {
            return OutputStatus::default();
        }
        // libobs keeps no start time; frames at the video rate give the duration.
        let duration_ms = ffi::video_frame_rate()
            .filter(|rate| *rate > 0.0)
            .map(|rate| duration_ms(output.total_frames(), rate));
        OutputStatus {
            active: true,
            duration_ms,
            bytes: Some(output.total_bytes()),
        }
    }
}

/// Milliseconds that `frames` last at `rate` frames per second.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "frame counts are far below 2^52 and the result is a rounded, non-negative duration"
)]
fn duration_ms(frames: u64, rate: f64) -> u64 {
    (frames as f64 * 1000.0 / rate).round() as u64
}

impl CaptureBackend for ObsBackend {
    fn capture(
        &self,
        source: Option<&str>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<Frame, String> {
        let source = match source {
            Some(name) => {
                Source::by_name(name).ok_or_else(|| format!("no source named '{name}'"))?
            }
            None => ffi::program().ok_or("no program scene is set")?,
        };
        if !ffi::has_video() {
            return Err(Self::because(
                "the video subsystem is not initialized, so there is no graphics context to render with",
                self.video_error.as_deref(),
            ));
        }
        let native = source.size();
        if native.0 == 0 || native.1 == 0 {
            return Err(format!("source '{}' has no size to capture", source.name()));
        }
        let target = target_size(native, width, height);
        if target.0 > MAX_SIDE || target.1 > MAX_SIDE {
            return Err(format!(
                "the screenshot is larger than {MAX_SIDE} pixels a side"
            ));
        }
        let rgba = ffi::render_rgba(&source, native, target)?;
        Ok(Frame {
            width: target.0,
            height: target.1,
            rgba,
        })
    }
}

/// The output size for a `native`-sized source: a side left `None` keeps the
/// aspect ratio, both `None` keeps the native size.
fn target_size(native: (u32, u32), width: Option<u32>, height: Option<u32>) -> (u32, u32) {
    let scaled = |side: u32, to: u32, from: u32| -> u32 {
        let value = (u64::from(side) * u64::from(to) + u64::from(from) / 2) / u64::from(from);
        u32::try_from(value).unwrap_or(u32::MAX).max(1)
    };
    match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, scaled(native.1, w, native.0)),
        (None, Some(h)) => (scaled(native.0, h, native.1), h),
        (None, None) => native,
    }
}

impl StatsBackend for ObsBackend {
    fn stats(&self) -> Stats {
        Stats {
            fps: ffi::active_fps(),
            cpu_usage: self.cpu.as_ref().map_or(0.0, CpuUsage::query),
            dropped_frames: ffi::dropped_frames(),
        }
    }
}
