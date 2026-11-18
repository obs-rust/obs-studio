//! SDK-independent source selection, shared with the application contract tests.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub(super) fn common_sources(
    sets: &BTreeMap<String, Vec<String>>,
    root: &Path,
    test_support: bool,
) -> Result<Vec<PathBuf>, String> {
    let mut common = sources(sets, "LIBOBS_SOURCES", root)?;
    if test_support {
        common.extend(sources(sets, "LIBOBS_TEST_SOURCES", root)?);
    }
    Ok(common)
}

pub(super) fn sources(
    sets: &BTreeMap<String, Vec<String>>,
    name: &str,
    root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let members = sets
        .get(name)
        .ok_or_else(|| format!("missing source set {name}"))?;
    Ok(members
        .iter()
        .filter(|s| {
            s.ends_with(".c") || s.ends_with(".cpp") || s.ends_with(".m") || s.ends_with(".mm")
        })
        .map(|s| root.join("libobs").join(s))
        .collect())
}
