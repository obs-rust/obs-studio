//! Reused CMake test executables classify names using their generated manifest.
//! Give a replacement Cargo-native core that same input, including an empty list.
use std::path::Path;

pub(super) fn for_reference(reference: Option<&Path>) -> Result<Vec<String>, String> {
    let Some(reference) = reference else {
        return Ok(Vec::new()); // Standalone builds retain their default empty list.
    };
    let build = reference
        .ancestors()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == "libobs-c-reference")
        })
        .and_then(Path::parent)
        .ok_or_else(|| format!("not a CMake reference library: {}", reference.display()))?;
    let manifest = build.join("libobs/obs-core-modules.c");
    let source = std::fs::read_to_string(&manifest)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    parse(&source).map_err(|error| format!("{}: {error}", manifest.display()))
}

fn parse(source: &str) -> Result<Vec<String>, String> {
    // The shared .c.in emits JSON-compatible quoted target names without a
    // trailing comma, or a lone NULL when count is zero. This is not a C parser.
    let entries = source
        .split_once("const char *obs_core_modules[] = {")
        .and_then(|(_, rest)| rest.split_once("};"))
        .map(|(entries, _)| entries.trim())
        .ok_or("missing generated core-module array")?;
    let names: Vec<String> = if entries == "NULL" {
        Vec::new()
    } else {
        serde_json::from_str(&format!("[{entries}]")).map_err(|error| error.to_string())?
    };
    let count: usize = source
        .split_once("const unsigned int obs_core_modules_count = ")
        .and_then(|(_, rest)| rest.split_once(';'))
        .ok_or("missing generated core-module count")?
        .0
        .trim()
        .parse()
        .map_err(|error| format!("invalid generated core-module count: {error}"))?;
    if names.len() != count {
        return Err(format!(
            "core-module count {count} differs from {} names",
            names.len()
        ));
    }
    Ok(names)
}
