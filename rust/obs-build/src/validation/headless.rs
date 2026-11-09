//! Generated includes for the unchanged hidden-C characterization fixtures.
use super::graph::Dependency;
use crate::NativeBuild;
use std::{fs, path::Path};

/// Match the original CMake extraction boundaries without copying production code.
pub fn prepare_headers(build: &NativeBuild, root: &Path) -> Result<Dependency, String> {
    let output = build.output.join("headless-characterization-headers");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    for (source, start, end, destination) in [
        (
            "libobs/obs-service-state.c.h",
            "void obs_service_set_destroy(",
            "#undef get_weak",
            "service-state-characterization.inc",
        ),
        (
            "shared/obs-scripting/obs-scripting-lua-source.c.h",
            "static bool lock_in_graphics(pthread_mutex_t *mutex)",
            "#define lock_script_in_graphics(skip)",
            "lua-graphics-lock-bounded-under-test.h",
        ),
    ] {
        let text = fs::read_to_string(root.join(source)).map_err(|e| e.to_string())?;
        let first = text
            .find(start)
            .ok_or_else(|| format!("missing {start} in {source}"))?;
        let last = text
            .find(end)
            .filter(|last| *last > first)
            .ok_or_else(|| format!("missing trailing {end} in {source}"))?;
        fs::write(output.join(destination), &text[first..last]).map_err(|e| e.to_string())?;
    }
    let source_context = fs::read_to_string(root.join("libobs/obs-source-init-context.c.h"))
        .map_err(|e| e.to_string())?;
    let body = source_context
        .strip_prefix("#include \"libobs/build/libobs.pch.h\"\n")
        .ok_or("source context PCH changed")?;
    fs::write(
        output.join("source-init-context-characterization.inc"),
        body,
    )
    .map_err(|e| e.to_string())?;
    fs::copy(
        root.join("test/cmocka/fixtures/lua-graphics-lock-original.h"),
        output.join("lua-graphics-lock-under-test.h"),
    )
    .map_err(|e| e.to_string())?;
    Ok(Dependency {
        include_dirs: vec![output],
        ..Default::default()
    })
}

/// Production adapter under the unchanged C test's dependency macros.
pub fn prepare_source_context_rust_headers(
    build: &NativeBuild,
    root: &Path,
) -> Result<Dependency, String> {
    let output = build.output.join("source-init-context-rust-headers");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let text = fs::read_to_string(root.join("libobs/obs-source-init-context-fields.c.h"))
        .map_err(|e| e.to_string())?;
    let body = text
        .strip_prefix("#include \"libobs/build/libobs.pch.h\"\n")
        .ok_or("source context adapter PCH changed")?;
    fs::write(
        output.join("source-init-context-characterization.inc"),
        body,
    )
    .map_err(|e| e.to_string())?;
    Ok(Dependency {
        include_dirs: vec![output],
        ..Default::default()
    })
}
