//! macOS core selection uses the same source sets as the reference build.
use crate::{
    SharedLibrary, SourceCompileOptions,
    libobs::{Options, append_sources},
};
use std::{collections::BTreeMap, path::Path};

/// Development framework lookup layout; release bundle metadata/signing is separate.
pub fn stage_identity(artifact: &Path) -> Result<(), String> {
    let framework = artifact
        .parent()
        .ok_or("core has no parent")?
        .join("libobs.framework");
    std::fs::create_dir_all(framework.join("Versions/A")).map_err(|e| e.to_string())?;
    for destination in [
        framework.join("libobs"),
        framework.join("Versions/A/libobs"),
    ] {
        std::fs::copy(artifact, destination).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn configure(
    root: &Path,
    sets: &BTreeMap<String, Vec<String>>,
    library: &mut SharedLibrary,
) -> Result<(), String> {
    append_sources(sets, "LIBOBS_MACOS_SOURCES", root, library)?;
    library.file_name = Some("libobs.dylib".into());
    library.c_compile_options.push("-std=gnu17".into());
    library
        .definitions
        .push(("SIMDE_ENABLE_OPENMP".into(), None));
    library.compile_options.extend(
        [
            "-fno-strict-aliasing",
            "-fvisibility=hidden",
            "-fopenmp-simd",
            "-mmacosx-version-min=13.0",
            "-Wno-strict-prototypes",
            "-Wno-shorten-64-to-32",
        ]
        .map(String::from),
    );
    library.source_compile_options.push(SourceCompileOptions {
        sources: vec![root.join("libobs/build/libobs.macos-cocoa+.h.m")],
        options: vec!["-fobjc-arc".into()],
    });
    for framework in [
        "AppKit",
        "AudioToolbox",
        "AudioUnit",
        "Carbon",
        "Cocoa",
        "CoreAudio",
        "IOKit",
    ] {
        library
            .link_options
            .extend(["-framework".into(), framework.into()]);
    }
    library.link_options.extend([
        "-Wl,-install_name,@rpath/libobs.framework/Versions/A/libobs".into(),
        "-Wl,-compatibility_version,1.0".into(),
        "-Wl,-current_version,33".into(),
    ]);
    Ok(())
}

pub fn dependencies(options: &Options, library: &mut SharedLibrary) -> Result<(), String> {
    let root = options
        .native_dependencies
        .as_ref()
        .ok_or("macOS requires an explicit OBS native dependency bundle")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let include = root.join("include");
    for header in [
        "libavcodec/avcodec.h",
        "jansson.h",
        "simde/x86/sse2.h",
        "uthash.h",
    ] {
        if !include.join(header).is_file() {
            return Err(format!(
                "native dependency header missing: {}",
                include.join(header).display()
            ));
        }
    }
    library.include_dirs.push(include);
    for name in [
        "avformat",
        "avcodec",
        "avutil",
        "swscale",
        "swresample",
        "jansson",
    ] {
        let shared = root.join(format!("lib/lib{name}.dylib"));
        let archive = root.join(format!("lib/lib{name}.a"));
        let path = if shared.is_file() {
            shared
        } else if archive.is_file() {
            archive
        } else {
            return Err(format!(
                "native dependency {name} missing from {}",
                root.display()
            ));
        };
        library.link_options.push(path.display().to_string());
    }
    library.link_options.extend([
        "-lz".into(),
        format!("-Wl,-rpath,{}", root.join("lib").display()),
    ]);
    Ok(())
}

pub fn configure_variant(
    root: &Path,
    options: &Options,
    sets: &BTreeMap<String, Vec<String>>,
    library: &mut SharedLibrary,
    rust_archive: Option<&Path>,
    config: &Path,
) -> Result<(), String> {
    if let Some(archive) = rust_archive {
        let archive = archive.canonicalize().map_err(|e| e.to_string())?;
        let exports = config.join("rust-unexports-macos.txt");
        crate::libobs::write_hidden_list(
            root,
            crate::libobs::HiddenLinker::Macos,
            options.hevc,
            &exports,
        )?;
        let mut flags = vec![
            "-Xlinker".into(),
            "-force_load".into(),
            "-Xlinker".into(),
            archive.display().to_string(),
            "-Xlinker".into(),
            "-unexported_symbols_list".into(),
            "-Xlinker".into(),
            exports.display().to_string(),
            "-lSystem".into(),
            "-lobjc".into(),
            "-liconv".into(),
        ];
        flags.append(&mut library.link_options);
        library.link_options = flags;
    } else {
        append_sources(sets, "LIBOBS_MACOS_PORTED_SOURCES", root, library)?;
        library.source_compile_options.push(SourceCompileOptions {
            sources: vec![root.join("libobs/build/libobs.ported-macos-cocoa+.h.m")],
            options: vec!["-fobjc-arc".into()],
        });
    }
    Ok(())
}
