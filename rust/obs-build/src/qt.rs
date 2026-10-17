//! Explicit Qt code generation for native Cargo targets.
use crate::SharedLibrary;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Generate one moc translation unit using the target's includes and definitions.
/// Cross builds can provide a host executable through OBS_QT_MOC.
pub fn moc(library: &mut SharedLibrary, header: &Path, output: &Path) -> Result<PathBuf, String> {
    let mut command = command("moc")?;
    for directory in &library.include_dirs {
        command.arg("-I").arg(directory);
    }
    for (name, value) in &library.definitions {
        command.arg(match value {
            Some(value) => format!("-D{name}={value}"),
            None => format!("-D{name}"),
        });
    }
    generate(command, header, output)?;
    library.sources.push(output.into());
    Ok(output.into())
}

/// Generate a UI header and put its output directory first in the include search.
pub fn uic(library: &mut SharedLibrary, form: &Path, output: &Path) -> Result<PathBuf, String> {
    generate(command("uic")?, form, output)?;
    let directory = output
        .parent()
        .ok_or("UI output has no parent directory")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    library.include_dirs.retain(|path| path != &directory);
    library.include_dirs.insert(0, directory);
    Ok(output.into())
}

/// Embed a resource collection and track each resource file as a Cargo input.
pub fn rcc(
    library: &mut SharedLibrary,
    resources: &Path,
    output: &Path,
) -> Result<PathBuf, String> {
    let resources = resources.canonicalize().map_err(|e| e.to_string())?;
    let mut list = command("rcc")?;
    list.arg("--list");
    let files = run(list, &resources)?;
    for file in std::str::from_utf8(&files)
        .map_err(|e| e.to_string())?
        .lines()
    {
        let path = resources
            .parent()
            .ok_or("resource input has no parent")?
            .join(file);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let name = resources
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("invalid resource collection name")?;
    let mut compiler = command("rcc")?;
    compiler.arg("--name").arg(name);
    generate(compiler, &resources, output)?;
    library.sources.push(output.into());
    Ok(output.into())
}

fn command(name: &str) -> Result<Command, String> {
    let variable = format!("OBS_QT_{}", name.to_ascii_uppercase());
    println!("cargo:rerun-if-env-changed={variable}");
    let executable = if let Some(path) = std::env::var_os(variable) {
        PathBuf::from(path)
    } else {
        PathBuf::from(pkg_config::get_variable("Qt6Core", "libexecdir").map_err(|e| e.to_string())?)
            .join(name)
    };
    Ok(Command::new(executable))
}

fn run(mut command: Command, input: &Path) -> Result<Vec<u8>, String> {
    let input = input.canonicalize().map_err(|e| e.to_string())?;
    println!("cargo:rerun-if-changed={}", input.display());
    let result = command
        .arg(&input)
        .output()
        .map_err(|e| format!("execute Qt tool: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "Qt tool {command:?} failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(result.stdout)
}

fn generate(command: Command, input: &Path, output: &Path) -> Result<(), String> {
    let code = run(command, input)?;
    if code.is_empty() {
        return Err(format!("Qt tool produced no code for {}", input.display()));
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if std::fs::read(output).ok().as_deref() != Some(code.as_slice()) {
        std::fs::write(output, code).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Combine generated metaobjects with the target's textual header closure.
/// This mirrors Qt's mocs_compilation.cpp without requiring precompiled headers.
pub fn combine_moc(
    library: &mut SharedLibrary,
    output: &Path,
    prelude: &Path,
) -> Result<(), String> {
    let directory = output.parent().ok_or("moc group output has no parent")?;
    let is_moc = |path: &PathBuf| {
        path.parent() == Some(directory)
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("moc_"))
    };
    let sources: Vec<_> = library
        .sources
        .iter()
        .filter(|path| is_moc(path))
        .cloned()
        .collect();
    if sources.is_empty() {
        return Ok(());
    }
    let mut content = String::from("/* Generated Qt metaobject group. */\n");
    for path in std::iter::once(prelude.to_owned()).chain(sources) {
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let name = path
            .to_str()
            .ok_or("non-UTF8 moc include")?
            .replace('\\', "/");
        if name.contains(['\n', '\r', '"']) {
            return Err("moc include path contains a quote or newline".into());
        }
        content.push_str(&format!("#include \"{name}\"\n"));
    }
    if std::fs::read_to_string(output).ok().as_deref() != Some(&content) {
        std::fs::write(output, content).map_err(|e| e.to_string())?;
    }
    library.sources.retain(|path| !is_moc(path));
    library.sources.push(output.into());
    Ok(())
}
