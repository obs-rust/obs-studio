//! Deterministic native registration module discovery, shared with its contract tests.
use std::{fs, path::Path};

pub fn declarations(directory: &Path) -> Result<String, String> {
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut paths = fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    paths.retain(|path| {
        path.is_file() && path.extension().is_some_and(|extension| extension == "rs")
    });
    paths.sort();
    let mut output = String::new();
    for path in paths {
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("invalid module name")?;
        if name.is_empty()
            || !name.bytes().enumerate().all(|(index, byte)| {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            })
        {
            return Err(format!("invalid registration module name {name:?}"));
        }
        output.push_str(&format!("#[path = {:?}]\nmod {name};\n", path));
    }
    Ok(output)
}
