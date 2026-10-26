//! Windows core sources, import libraries, native resources and Rust exports.
use crate::{
    NativeBuild, SharedLibrary, SourceCompileOptions,
    libobs::{Options, append_sources},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub fn defaults(library: &mut SharedLibrary) {
    library.c_compile_options.push("/std:c17".into());
    library
        .compile_options
        .extend(["/utf-8", "/Zc:preprocessor", "/EHc-"].map(String::from));
    library.definitions.extend(
        [
            "NOMINMAX",
            "UNICODE",
            "_UNICODE",
            "_CRT_SECURE_NO_WARNINGS",
            "_CRT_NONSTDC_NO_WARNINGS",
        ]
        .map(|name| (name.into(), None)),
    );
}

pub fn configure(
    build: &NativeBuild,
    root: &Path,
    sets: &BTreeMap<String, Vec<String>>,
    library: &mut SharedLibrary,
) -> Result<(), String> {
    defaults(library);
    append_sources(sets, "LIBOBS_WINDOWS_SOURCES", root, library)?;
    append_sources(sets, "LIBOBS_WINDOWS_OBFUSCATE_SOURCES", root, library)?;
    library.file_name = Some("obs.dll".into());
    library.include_dirs.push(root.join("deps/w32-pthreads"));
    let version = api_version(root)?;
    library
        .source_compile_options
        .extend([SourceCompileOptions {
            sources: vec![root.join("libobs/build/libobs.windows-crash-handler+.h.c")],
            options: vec![format!("/DOBS_VERSION=\"{}\"", version.join("."))],
        }]);
    for name in [
        "kernel32", "user32", "gdi32", "winspool", "shell32", "ole32", "oleaut32", "uuid",
        "comdlg32", "advapi32", "avrt", "dwmapi", "dxgi", "winmm", "rpcrt4",
    ] {
        library.link_options.push(format!("{name}.lib"));
    }
    library
        .link_options
        .extend(["/IGNORE:4098".into(), "/SAFESEH:NO".into()]);
    add_resource(
        root,
        "libobs/cmake/windows/obs-module.rc.in",
        &build.output.join("config/libobs.rc"),
        library,
    )?;
    library.dependencies.push(pthreads(build, root)?);
    Ok(())
}

pub fn dependencies(options: &Options, library: &mut SharedLibrary) -> Result<(), String> {
    let root = options
        .native_dependencies
        .as_ref()
        .ok_or("Windows requires an explicit OBS native dependency bundle")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    for header in [
        "libavcodec/avcodec.h",
        "jansson.h",
        "simde/x86/sse2.h",
        "uthash.h",
    ] {
        if !root.join("include").join(header).is_file() {
            return Err(format!("native dependency header missing: {header}"));
        }
    }
    library.include_dirs.push(root.join("include"));
    for name in [
        "avformat",
        "avcodec",
        "avutil",
        "swscale",
        "swresample",
        "jansson",
        "zlib",
    ] {
        let path = root.join(format!("lib/{name}.lib"));
        if !path.is_file() {
            return Err(format!("native dependency missing: {}", path.display()));
        }
        library.dependencies.push(path);
    }
    Ok(())
}

pub fn configure_variant(
    root: &Path,
    options: &Options,
    sets: &BTreeMap<String, Vec<String>>,
    library: &mut SharedLibrary,
    rust_archive: Option<&Path>,
) -> Result<(), String> {
    if let Some(archive) = rust_archive {
        let archive = dunce::canonicalize(archive).map_err(|e| e.to_string())?;
        library
            .link_options
            .push(format!("/WHOLEARCHIVE:{}", archive.display()));
        library.dependencies.push(archive);
        let mut fragments = std::fs::read_dir(root.join("libobs/cmake/rust-exports.d"))
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        fragments.sort();
        for fragment in fragments {
            if fragment.extension().is_none_or(|ext| ext != "txt")
                || (!options.hevc
                    && fragment
                        .file_name()
                        .is_some_and(|name| name == "obs-codec-hevc.txt"))
            {
                continue;
            }
            println!("cargo:rerun-if-changed={}", fragment.display());
            for line in std::fs::read_to_string(&fragment)
                .map_err(|e| e.to_string())?
                .lines()
            {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.ends_with(" unix") {
                    continue;
                }
                if !line
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                {
                    return Err(format!(
                        "invalid Windows export in {}: {line}",
                        fragment.display()
                    ));
                }
                library.link_options.push(format!("/EXPORT:{line}"));
            }
        }
        for name in [
            "legacy_stdio_definitions",
            "ntdll",
            "userenv",
            "ws2_32",
            "dbghelp",
        ] {
            library.link_options.push(format!("{name}.lib"));
        }
    } else {
        append_sources(sets, "LIBOBS_WINDOWS_PORTED_SOURCES", root, library)?;
        library.source_compile_options.push(SourceCompileOptions {
            sources: vec![root.join("libobs/build/libobs.ported-data+.h.c")],
            options: vec!["/wd4996".into()],
        });
    }
    Ok(())
}

fn pthreads(build: &NativeBuild, root: &Path) -> Result<PathBuf, String> {
    println!(
        "cargo:rerun-if-changed={}",
        root.join("deps/w32-pthreads").display()
    );
    let mut library = SharedLibrary {
        name: "w32-pthreads".into(),
        sources: vec![root.join("deps/w32-pthreads/pthread.c")],
        include_dirs: vec![root.join("deps/w32-pthreads")],
        definitions: vec![("__CLEANUP_C".into(), None), ("PTW32_BUILD".into(), None)],
        ..SharedLibrary::default()
    };
    defaults(&mut library);
    add_resource(
        root,
        "deps/w32-pthreads/cmake/windows/obs-module.rc.in",
        &build.output.join("config/w32-pthreads.rc"),
        &mut library,
    )?;
    build.shared_library(&library)
}

fn api_version(root: &Path) -> Result<[String; 3], String> {
    let header =
        std::fs::read_to_string(root.join("libobs/obs-config.h")).map_err(|e| e.to_string())?;
    let mut version = Vec::new();
    for part in ["MAJOR", "MINOR", "PATCH"] {
        let prefix = format!("#define LIBOBS_API_{part}_VER ");
        let value = header
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .ok_or_else(|| format!("missing LIBOBS_API_{part}_VER"))?;
        value.trim().parse::<u16>().map_err(|e| e.to_string())?;
        version.push(value.trim().to_owned());
    }
    version.try_into().map_err(|_| "invalid API version".into())
}

pub(super) fn add_resource(
    root: &Path,
    template: &str,
    output: &Path,
    library: &mut SharedLibrary,
) -> Result<(), String> {
    let template = root.join(template);
    let bootstrap = root.join("cmake/common/bootstrap.cmake");
    let metadata = std::fs::read_to_string(&bootstrap).map_err(|e| e.to_string())?;
    let mut text = std::fs::read_to_string(&template).map_err(|e| e.to_string())?;
    let version = api_version(root)?;
    for (name, value) in ["MAJOR", "MINOR", "PATCH"].into_iter().zip(&version) {
        text = text.replace(&format!("${{OBS_VERSION_{name}}}"), value);
    }
    text = text.replace("${OBS_VERSION_CANONICAL}", &version.join("."));
    for name in [
        "OBS_COMPANY_NAME",
        "OBS_PRODUCT_NAME",
        "OBS_COMMENTS",
        "OBS_LEGAL_COPYRIGHT",
    ] {
        let prefix = format!("set({name} \"");
        let value = metadata
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix(&prefix)
                    .and_then(|line| line.strip_suffix("\")"))
            })
            .ok_or_else(|| format!("missing literal resource metadata {name}"))?;
        text = text.replace(&format!("${{{name}}}"), value);
    }
    if text.contains("${") {
        return Err(format!(
            "unresolved resource template {}",
            template.display()
        ));
    }
    crate::frontend::write(output, &text)?;
    library.resource_files.push(output.into());
    library
        .resource_input_files
        .extend([template, bootstrap, root.join("libobs/obs-config.h")]);
    Ok(())
}
