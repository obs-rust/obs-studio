//! Build PCH through the same cc tool and flags used by its consumers.
use crate::NativeBuild;
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

#[expect(clippy::too_many_lines, reason = "one block per header")]
pub(crate) fn apply(
    build: &NativeBuild,
    compiler: &mut cc::Build,
    language: crate::Language,
    header: &Path,
) -> Result<Option<PathBuf>, String> {
    let tool = compiler.get_compiler();
    let kind = crate::platform::pch_kind(&build.target, &tool)?;
    let mut signature = std::collections::hash_map::DefaultHasher::new();
    (
        &build.host,
        &build.target,
        header,
        language,
        tool.path(),
        tool.args(),
        tool.env(),
    )
        .hash(&mut signature);
    let directory = build
        .output
        .join("pch")
        .join(format!("{:016x}", signature.finish()));
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let directory = dunce::canonicalize(directory).map_err(|e| e.to_string())?;
    let source = directory.join("prefix-input.h");
    let path = dunce::simplified(header)
        .to_str()
        .ok_or("non-UTF8 PCH path")?
        .replace('\\', "/");
    if path.contains(['\n', '\r', '"']) {
        return Err(format!("unrepresentable PCH include: {path:?}"));
    }
    let guard = if matches!(kind, crate::platform::PchKind::Msvc) {
        "#pragma once\n"
    } else {
        ""
    };
    crate::frontend::write(&source, &format!("{guard}#include \"{path}\"\n"))?;
    println!("cargo:rerun-if-changed={}", header.display());
    let artifact = directory.join(match kind {
        crate::platform::PchKind::Gnu => "prefix.h.gch",
        crate::platform::PchKind::Clang | crate::platform::PchKind::Msvc => "prefix.pch",
    });
    let mut command = tool.to_command();
    let object = if matches!(kind, crate::platform::PchKind::Msvc) {
        if !matches!(language, crate::Language::C | crate::Language::Cpp) {
            return Err("MSVC PCH requires C or C++".into());
        }
        let wrapper = dunce::simplified(&source)
            .to_str()
            .ok_or("non-UTF8 PCH wrapper path")?
            .replace('\\', "/");
        if wrapper.contains(['\n', '\r', '"']) {
            return Err(format!("unrepresentable PCH wrapper include: {wrapper:?}"));
        }
        let input = directory.join(format!("prefix-input.{}", language.extension()));
        crate::frontend::write(&input, &format!("#include \"{wrapper}\"\n"))?;
        let object = directory.join("prefix.obj");
        command
            .arg(format!("/Yc{wrapper}"))
            .arg(format!("/Fp{}", dunce::simplified(&artifact).display()))
            .arg(format!("/Fo{}", dunce::simplified(&object).display()))
            .arg("/c")
            .arg(&input);
        Some(object)
    } else {
        command
            .args(["-x", language.header_language(), "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&artifact);
        None
    };
    let result = command
        .output()
        .map_err(|e| format!("execute PCH compiler: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "PCH {} failed ({})\n{}\n{}",
            header.display(),
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    match kind {
        crate::platform::PchKind::Gnu => {
            let fallback = directory.join("prefix.h");
            // GCC otherwise ignores an unusable PCH silently. The fallback
            // header makes requested PCH consumption a checked invariant.
            crate::frontend::write(
                &fallback,
                "#error obs-build requested PCH was not consumed\n",
            )?;
            compiler
                .flag("-Werror=invalid-pch")
                .flag("-include")
                .flag(fallback.to_str().ok_or("non-UTF8 PCH fallback path")?);
        }
        crate::platform::PchKind::Clang => {
            compiler
                .flag("-include-pch")
                .flag(artifact.to_str().ok_or("non-UTF8 PCH artifact path")?);
        }
        crate::platform::PchKind::Msvc => {
            let wrapper = dunce::simplified(&source)
                .to_str()
                .ok_or("non-UTF8 PCH wrapper path")?
                .replace('\\', "/");
            compiler
                .flag(format!("/Yu{wrapper}"))
                .flag(format!("/FI{wrapper}"))
                .flag(format!("/Fp{}", dunce::simplified(&artifact).display()));
        }
    }
    // /Yc with /Z7 can put required code/debug symbols in this object. Link it
    // once alongside all consumers, including when archiving native objects.
    Ok(object)
}
