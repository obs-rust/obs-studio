//! MSVC DATA-only import libraries for private-parser compatibility fixtures.
use crate::{NativeBuild, SharedLibrary};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn build(
    build: &NativeBuild,
    definition: &Path,
    output: &Path,
) -> Result<PathBuf, String> {
    let machine = if build.target.starts_with("x86_64-") {
        "X64"
    } else if build.target.starts_with("aarch64-") {
        "ARM64"
    } else if build.target.starts_with("i686-") {
        "X86"
    } else {
        return Err(format!(
            "unsupported import-library architecture {}",
            build.target
        ));
    };
    if !build.target.contains("windows-msvc") {
        return Err("DATA-only import library requires MSVC".into());
    }
    fs::create_dir_all(output.parent().ok_or("import library has no parent")?)
        .map_err(|e| e.to_string())?;
    let definition = dunce::canonicalize(definition).map_err(|e| e.to_string())?;
    println!("cargo:rerun-if-changed={}", definition.display());
    let mut command = build
        .compiler(&SharedLibrary::default(), &build.output, false)
        .get_archiver();
    command
        .arg("/nologo")
        .arg(format!("/def:{}", dunce::simplified(&definition).display()))
        .arg(format!("/machine:{machine}"))
        .arg(format!("/out:{}", dunce::simplified(output).display()));
    let result = command
        .output()
        .map_err(|e| format!("execute import librarian: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "{command:?}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    if !output.is_file() {
        return Err(format!("librarian did not produce {}", output.display()));
    }
    Ok(output.into())
}
