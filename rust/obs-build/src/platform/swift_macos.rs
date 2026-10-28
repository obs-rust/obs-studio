//! Explicit Apple Swift compiler/link orchestration, independent of cc::Build.
use crate::{NativeBuild, SwiftLibrary};
use std::{ffi::OsString, path::PathBuf, process::Command};

fn setting(build: &NativeBuild, name: &str) -> Option<OsString> {
    let names = [
        format!("{name}_{}", build.target),
        format!("{name}_{}", build.target.replace('-', "_")),
        name.into(),
    ];
    let mut value = None;
    for name in names {
        println!("cargo:rerun-if-env-changed={name}");
        if value.is_none() {
            value = std::env::var_os(name);
        }
    }
    value
}

fn xcrun(build: &NativeBuild, arguments: &[&str]) -> Result<OsString, String> {
    if !build.host.contains("apple-darwin") {
        return Err(
            "cross-building Swift requires explicit SWIFTC and SDKROOT executable/SDK paths".into(),
        );
    }
    let result = Command::new("xcrun")
        .args(arguments)
        .output()
        .map_err(|e| format!("execute xcrun: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "xcrun failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    let value = String::from_utf8(result.stdout).map_err(|e| e.to_string())?;
    let value = value.trim();
    if value.is_empty() {
        return Err("xcrun returned an empty tool/SDK path".into());
    }
    Ok(value.into())
}

#[expect(
    clippy::too_many_lines,
    reason = "one block per build invocation, in order"
)]
pub fn build(build: &NativeBuild, library: &SwiftLibrary) -> Result<PathBuf, String> {
    for variable in ["DEVELOPER_DIR", "TOOLCHAINS"] {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    if library.sources.is_empty() {
        return Err("Swift module has no sources".into());
    }
    let architecture = match build.target.split('-').next() {
        Some("aarch64") => "arm64",
        Some("x86_64") => "x86_64",
        _ => return Err(format!("unsupported Swift architecture: {}", build.target)),
    };
    let compiler = match setting(build, "SWIFTC") {
        Some(value) => value,
        None => xcrun(build, &["--find", "swiftc"])?,
    };
    let sdk = match setting(build, "SDKROOT") {
        Some(value) => value,
        None => xcrun(build, &["--sdk", "macosx", "--show-sdk-path"])?,
    };
    let output = build.output.join(&library.name);
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let output = output.canonicalize().map_err(|e| e.to_string())?;
    let artifact = output.join(&library.file_name);
    let module = output.join(format!("{}.swiftmodule", library.module_name));
    let objc_header = output.join(format!("{}-Swift.h", library.module_name));
    let bridge = library
        .bridging_header
        .canonicalize()
        .map_err(|e| e.to_string())?;
    println!("cargo:rerun-if-changed={}", bridge.display());
    for input in &library.input_files {
        let input = input.canonicalize().map_err(|e| e.to_string())?;
        println!("cargo:rerun-if-changed={}", input.display());
    }
    let mut command = Command::new(compiler);
    command
        .args([
            "-emit-library",
            "-emit-module",
            "-emit-objc-header",
            "-module-name",
            &library.module_name,
            "-target",
            &format!("{architecture}-apple-macosx{}", library.deployment_target),
        ])
        .arg("-emit-module-path")
        .arg(&module)
        .arg("-emit-objc-header-path")
        .arg(&objc_header)
        .arg("-sdk")
        .arg(sdk)
        .arg("-module-cache-path")
        .arg(output.join("module-cache"))
        .arg("-import-objc-header")
        .arg(bridge)
        .args([
            "-Xcc",
            "-std=gnu17",
            "-Xcc",
            "-fmodules",
            "-Xcc",
            "-fobjc-arc",
        ]);
    command.arg(match build.profile.optimization() {
        "0" => "-Onone",
        "s" | "z" => "-Osize",
        _ => "-O",
    });
    command.args([
        "-assert-config",
        if build.profile.assertions() {
            "Debug"
        } else {
            "Release"
        },
    ]);
    if build.profile.debug_info() {
        command.arg("-g");
    }
    if !build.profile.assertions() {
        command.args(["-whole-module-optimization", "-Xcc", "-DNDEBUG"]);
    }
    for include in &library.include_dirs {
        let include = include.canonicalize().map_err(|e| e.to_string())?;
        command.arg("-Xcc").arg(format!("-I{}", include.display()));
    }
    for (name, value) in &library.definitions {
        let value = value
            .as_ref()
            .map_or_else(|| format!("-D{name}"), |value| format!("-D{name}={value}"));
        command.arg("-Xcc").arg(value);
    }
    command.args(&library.compile_options);
    for source in &library.sources {
        if source
            .extension()
            .is_none_or(|extension| extension != "swift")
        {
            return Err(format!(
                "non-Swift source in Swift module: {}",
                source.display()
            ));
        }
        let source = source.canonicalize().map_err(|e| e.to_string())?;
        println!("cargo:rerun-if-changed={}", source.display());
        command.arg(source);
    }
    for dependency in &library.dependencies {
        let dependency = dependency.canonicalize().map_err(|e| e.to_string())?;
        println!("cargo:rerun-if-changed={}", dependency.display());
        let parent = dependency
            .parent()
            .ok_or("Swift dependency has no parent")?;
        command
            .arg("-Xlinker")
            .arg("-rpath")
            .arg("-Xlinker")
            .arg(parent)
            .arg(dependency);
    }
    command
        .args([
            "-Xlinker",
            "-install_name",
            "-Xlinker",
            &format!("@rpath/{}", library.file_name),
            "-Xlinker",
            "-rpath",
            "-Xlinker",
            "@loader_path",
        ])
        .args(&library.link_options)
        .arg("-o")
        .arg(&artifact);
    let result = command
        .output()
        .map_err(|e| format!("execute Swift compiler {command:?}: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "Swift module {} failed ({})\n{}\n{}",
            library.name,
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    for required in [&artifact, &module, &objc_header] {
        if !required.is_file() || required.metadata().map_err(|e| e.to_string())?.len() == 0 {
            return Err(format!(
                "Swift compiler did not produce {}",
                required.display()
            ));
        }
    }
    Ok(artifact)
}
