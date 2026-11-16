//! Native runtime for the in-process MCP host, without Qt/frontend builds.
use crate::{NativeBuild, NativeProfile, graphics, libobs, plugins};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub root: PathBuf,
    pub core: PathBuf,
    pub output: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub graphics_module: PathBuf,
    pub module_bin: PathBuf,
    pub module_data: PathBuf,
    pub data_path: PathBuf,
}

fn stage(source: &Path, directory: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let name = source.file_name().ok_or("artifact has no filename")?;
    let destination = directory.join(name);
    let temporary = directory.join(format!(
        ".{}.{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    std::fs::copy(source, &temporary).map_err(|error| error.to_string())?;
    // Keep the inode of a library already mapped by an active listener intact.
    std::fs::rename(temporary, &destination).map_err(|error| error.to_string())?;
    Ok(destination)
}

pub fn build(request: &Request) -> Result<Manifest, String> {
    let root = request
        .root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let core = request
        .core
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let native = core
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid core artifact path")?;
    let build = NativeBuild {
        host: crate::BUILD_HOST.into(),
        target: crate::BUILD_TARGET.into(),
        output: native.to_path_buf(),
        profile: NativeProfile::Release,
    };
    crate::platform::validate_application_build(&build)?;
    let options = libobs::Options {
        pch: true,
        ..Default::default()
    };
    let renderer = graphics::build_opengl(&build, &root, &core, &options)?;
    let modules = [
        plugins::build_source(
            &build,
            &root,
            &core,
            plugins::SourceModule::V4l2 { udev: false },
        )?,
        plugins::build_ffmpeg(
            &build,
            &root,
            &core,
            &options,
            &plugins::FfmpegOptions {
                mpegts: false,
                ..Default::default()
            },
        )?
        .module,
        plugins::build_x264(&build, &root, &core)?,
    ];
    let module_bin = request.output.join("plugins");
    for module in modules {
        stage(&module.library, &module_bin)?;
    }
    let manifest = Manifest {
        graphics_module: stage(&renderer, &request.output.join("graphics"))?,
        module_bin,
        module_data: root.join("plugins/%module%/data"),
        data_path: root.join("libobs/data"),
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    let temporary = request
        .output
        .join(format!("manifest.{}.json", std::process::id()));
    std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, request.output.join("manifest.json"))
        .map_err(|error| error.to_string())?;
    Ok(manifest)
}
