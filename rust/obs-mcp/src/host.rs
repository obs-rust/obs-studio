//! Host entry point: libobs lifecycle plus the stdio server (#3289, #3317).

use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::backend::ObsBackend;
use crate::ffi::{self, Libobs, Pipeline};

/// Output names the frontend gives its simple-mode record and stream outputs.
pub const RECORD_OUTPUT: &str = "simple_file_output";
/// See [`RECORD_OUTPUT`].
pub const STREAM_OUTPUT: &str = "simple_stream";

/// How the host brings libobs up, like the application does (#3317).
#[derive(Clone, Debug)]
pub struct StartOptions {
    /// Graphics module to load (`libobs-opengl`, `libobs-d3d11`), by name or path.
    pub graphics_module: String,
    /// Canvas and output size in pixels.
    pub size: (u32, u32),
    /// Frames per second.
    pub fps: u32,
    /// Where libobs finds its effect files, when not in a default place.
    pub data_path: Option<PathBuf>,
    /// Plugin directory (`bin`) and its per-module data directory pattern
    /// (`data`, with `%module%`), as `obs_add_module_path` takes them.
    pub module_path: Option<(PathBuf, PathBuf)>,
    /// Directory the record output writes its files to.
    pub record_dir: PathBuf,
}

impl StartOptions {
    /// Defaults, then the `OBS_MCP_GRAPHICS_MODULE`, `OBS_MCP_DATA_PATH`,
    /// `OBS_MCP_MODULE_BIN` and `OBS_MCP_MODULE_DATA` environment variables,
    /// then an install laid out next to the executable
    /// (`../lib/obs-plugins`, `../share/obs/obs-plugins/%module%`,
    /// `../share/obs/libobs`). `record_dir` defaults to a directory under the
    /// system temp directory.
    #[must_use]
    pub fn from_env(record_dir: Option<PathBuf>) -> Self {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
        let install = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent()?.parent().map(Path::to_path_buf));
        let exists = |p: PathBuf| p.is_dir().then_some(p);
        let module_path = match (var("OBS_MCP_MODULE_BIN"), var("OBS_MCP_MODULE_DATA")) {
            (Some(bin), Some(data)) => Some((bin.into(), data.into())),
            (Some(bin), None) => Some((bin.clone().into(), PathBuf::from(bin).join("%module%"))),
            _ => install.as_ref().and_then(|root| {
                let bin = exists(root.join("lib/obs-plugins"))?;
                Some((bin, root.join("share/obs/obs-plugins/%module%")))
            }),
        };
        Self {
            graphics_module: var("OBS_MCP_GRAPHICS_MODULE")
                .map_or_else(default_graphics_module, |v| {
                    v.to_string_lossy().into_owned()
                }),
            size: (1280, 720),
            fps: 30,
            data_path: var("OBS_MCP_DATA_PATH").map(PathBuf::from).or_else(|| {
                install
                    .as_ref()
                    .and_then(|root| exists(root.join("share/obs/libobs")))
            }),
            module_path,
            record_dir: record_dir.unwrap_or_else(|| std::env::temp_dir().join("obs-mcp")),
        }
    }
}

fn default_graphics_module() -> String {
    if cfg!(windows) {
        "libobs-d3d11"
    } else {
        "libobs-opengl"
    }
    .to_owned()
}

/// A started libobs with video, audio, modules and the record and stream
/// outputs set up. Every step after `obs_startup` that fails is recorded, not
/// fatal: the MCP tools that need it report the reason instead.
///
/// Dropping it tears the outputs down and shuts libobs down, so drop every
/// [`ObsBackend`] made by [`Runtime::backend`] first.
pub struct Runtime {
    // Field order is drop order: outputs, then libobs.
    pipeline: Pipeline,
    _libobs: Libobs,
    record_dir: PathBuf,
}

impl Runtime {
    /// Starts libobs, video, audio and modules, and creates the outputs.
    ///
    /// # Errors
    ///
    /// Returns an error only when `obs_startup` itself fails.
    pub fn start(options: &StartOptions) -> io::Result<Self> {
        let libobs = Libobs::start().map_err(io::Error::other)?;
        let pipeline = ffi::configure(options);
        for note in pipeline.notes() {
            eprintln!("obs-mcp: {note}");
        }
        Ok(Self {
            pipeline,
            _libobs: libobs,
            record_dir: options.record_dir.clone(),
        })
    }

    /// A backend over this libobs, controlling the record and stream outputs.
    #[must_use]
    pub fn backend(&self) -> ObsBackend {
        ObsBackend::new()
            .with_output_names(Some(RECORD_OUTPUT), Some(STREAM_OUTPUT))
            .with_record_dir(&self.record_dir)
            .with_startup_errors(
                self.pipeline.video_error().map(str::to_owned),
                self.pipeline.output_error().map(str::to_owned),
            )
    }

    /// Why video is unavailable, if it is.
    #[must_use]
    pub fn video_error(&self) -> Option<&str> {
        self.pipeline.video_error()
    }
}

/// Starts libobs like the application does and serves obs-mcp on
/// stdin/stdout until stdin reaches end of input, then shuts libobs down.
///
/// Descriptor 1 is pointed at stderr first, so libobs logs and anything else
/// that prints cannot corrupt the protocol stream.
///
/// # Errors
///
/// Returns an error when libobs fails to start or stdio fails.
pub fn serve_stdio(options: &StartOptions) -> io::Result<()> {
    let mut protocol = ffi::claim_stdout()?;
    // Declared first, so it is dropped last: after the server and the backend.
    let runtime = Runtime::start(options)?;
    let backend = Rc::new(runtime.backend());
    let mut server = ObsBackend::server(&backend);
    let result = server.serve(io::stdin().lock(), &mut protocol);
    drop(server);
    drop(backend);
    result
}
