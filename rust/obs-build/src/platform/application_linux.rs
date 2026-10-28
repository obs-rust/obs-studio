//! Development runtime layout for Linux applications.
use crate::application::{RuntimeInputs, StagedRuntime};
use std::path::Path;

/// Cargo's managed Rust sysroot must not hide this native application's SDK.
/// Target-qualified pkg-config settings take precedence over a managed sysroot's
/// generic defaults, and preserve an explicit caller-selected SDK directory.
pub fn configure_archive_command(
    build: &crate::NativeBuild,
    command: &mut std::process::Command,
) -> Result<(), String> {
    let target = &build.target;
    let suffix = target.replace('-', "_");
    let supplied = [
        format!("PKG_CONFIG_LIBDIR_{target}"),
        format!("PKG_CONFIG_LIBDIR_{suffix}"),
        "HOST_PKG_CONFIG_LIBDIR".into(),
        "PKG_CONFIG_LIBDIR".into(),
    ]
    .iter()
    .find_map(std::env::var_os);
    let directory = if let Some(directory) = supplied {
        directory
    } else {
        let executable = [
            format!("PKG_CONFIG_{target}"),
            format!("PKG_CONFIG_{suffix}"),
            "HOST_PKG_CONFIG".into(),
            "PKG_CONFIG".into(),
        ]
        .iter()
        .find_map(std::env::var_os)
        .unwrap_or_else(|| "pkg-config".into());
        let result = std::process::Command::new(executable)
            .args(["--variable=pc_path", "pkg-config"])
            .output()
            .map_err(|e| format!("query native pkg-config directories: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "query native pkg-config directories: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        let value = String::from_utf8(result.stdout).map_err(|e| e.to_string())?;
        if value.trim().is_empty() {
            return Err(
                "native pkg-config has no default directories; set PKG_CONFIG_LIBDIR explicitly"
                    .into(),
            );
        }
        value.trim().into()
    };
    command.env(format!("PKG_CONFIG_LIBDIR_{target}"), directory);
    Ok(())
}

#[expect(
    clippy::too_many_lines,
    reason = "ordered staging copies, kept together"
)]
pub fn stage(inputs: &RuntimeInputs<'_>, destination: &Path) -> Result<StagedRuntime, String> {
    if destination.exists()
        && std::fs::read_dir(destination)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err(format!(
            "runtime destination must be empty: {}",
            destination.display()
        ));
    }
    let mut selected = std::collections::BTreeSet::new();
    for module in inputs.modules {
        let name = module.library.file_name().ok_or("module has no filename")?;
        if !selected.insert(name) {
            return Err(format!(
                "duplicate runtime module: {}",
                name.to_string_lossy()
            ));
        }
    }
    if !inputs
        .modules
        .iter()
        .any(|module| module.library == inputs.ffmpeg.module.library)
    {
        return Err("runtime modules omit the selected FFmpeg module".into());
    }
    if let Some(browser) = inputs.browser
        && !inputs
            .modules
            .iter()
            .any(|module| module.library == browser.module.library)
    {
        return Err("runtime modules omit the selected browser module".into());
    }
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let runtime = destination.canonicalize().map_err(|e| e.to_string())?;
    for data in [
        inputs.root.join("libobs/data"),
        inputs.root.join("frontend/data"),
    ]
    .iter()
    .chain(inputs.modules.iter().map(|module| &module.data))
    {
        let source = data.canonicalize().map_err(|e| e.to_string())?;
        if runtime.starts_with(&source) {
            return Err("runtime destination is inside its source resources".into());
        }
    }
    let mut script_metadata = None;
    let bin = runtime.join("bin");
    let libraries = runtime.join("lib64");
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&libraries).map_err(|e| e.to_string())?;
    copy(inputs.executable, &bin.join("obs-studio-rust"))?;
    for library in [inputs.core, inputs.api, inputs.renderer] {
        copy(
            library,
            &libraries.join(
                library
                    .file_name()
                    .ok_or("runtime library has no filename")?,
            ),
        )?;
    }
    inputs.ffmpeg.deploy_muxer(&bin)?;
    if let Some(scripts) = inputs.scripting {
        let library_name = scripts
            .library
            .file_name()
            .ok_or("scripting library has no filename")?;
        copy(&scripts.library, &libraries.join(library_name))?;
        let mut modules = Vec::new();
        let mut languages = Vec::new();
        for (language, module) in [
            ("lua", scripts.lua_module.as_deref()),
            ("python", scripts.python_module.as_deref()),
        ] {
            if let Some(module) = module {
                let relative = module
                    .strip_prefix(&scripts.runtime)
                    .map_err(|e| e.to_string())?;
                copy(module, &runtime.join(relative))?;
                modules.push(relative.to_string_lossy().into_owned());
                languages.push(language);
            }
        }
        if let Some(wrapper) = &scripts.python_wrapper {
            copy(
                wrapper,
                &runtime.join(
                    wrapper
                        .strip_prefix(&scripts.runtime)
                        .map_err(|e| e.to_string())?,
                ),
            )?;
        }
        let metadata = serde_json::json!({"library": format!("lib64/{}", library_name.to_string_lossy()), "modules": modules, "languages": languages});
        script_metadata = Some(metadata);
    }
    if let Some(helper) = inputs.nvenc_helper {
        copy(
            helper,
            &bin.join(helper.file_name().ok_or("NVENC helper has no filename")?),
        )?;
    }
    copy_tree(
        &inputs.root.join("libobs/data"),
        &runtime.join("share/obs/libobs"),
    )?;
    copy_tree(
        &inputs.root.join("frontend/data"),
        &runtime.join("share/obs/obs-studio"),
    )?;
    copy(
        &inputs.root.join("AUTHORS"),
        &runtime.join("share/obs/obs-studio/authors"),
    )?;
    let mut names = Vec::new();
    for module in inputs.modules {
        let name = module
            .library
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("invalid module filename")?;
        let target = runtime.join("lib64/obs-modules/core");
        std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        copy(
            &module.library,
            &target.join(module.library.file_name().ok_or("module has no filename")?),
        )?;
        copy_tree(
            &module.data,
            &runtime.join("share/obs/obs-modules/core").join(name),
        )?;
        names.push(name.to_owned());
    }
    if let Some(browser) = inputs.browser {
        browser.deploy_runtime(&runtime.join("lib64/obs-modules/core"))?;
    }
    Ok(StagedRuntime {
        executable: runtime.join("bin/obs-studio-rust"),
        directory: runtime,
        modules: names,
        scripting: script_metadata,
    })
}

fn copy(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(source, destination).map(|_| ()).map_err(|e| {
        format!(
            "copy {} to {}: {e}",
            source.display(),
            destination.display()
        )
    })
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            copy(&entry.path(), &target)?;
        }
    }
    Ok(())
}
