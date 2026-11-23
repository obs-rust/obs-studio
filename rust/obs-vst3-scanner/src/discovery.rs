use crate::cache::{Cache, Class};
use crate::{metadata, platform};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const AUDIO_MODULE_CLASS: &str = "Audio Module Class";

pub(super) fn find_candidates(roots: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                pending.push(path.clone());
            }
            if path.extension().is_some_and(|ext| ext == "vst3") && platform::is_candidate(&path) {
                paths.insert(path);
            }
        }
    }
    paths
}

pub(super) fn scan(candidates: &BTreeSet<PathBuf>, cached: Option<Cache>) -> Cache {
    let mut reusable: HashMap<_, Vec<_>> = HashMap::new();
    if let Some(cached) = cached {
        for class in cached.plugins {
            if candidates.contains(&class.path) {
                reusable.entry(class.path.clone()).or_default().push(class);
            }
        }
    }

    let mut classes = Vec::new();
    for path in candidates {
        if let Some(previous) = reusable.remove(path) {
            if previous.iter().any(|class| class.discardable) {
                classes.extend(enumerate(path));
            } else {
                classes.extend(previous);
            }
        } else {
            classes.extend(read_classes(path));
        }
    }
    classes.sort_by_cached_key(|class| {
        (
            class.name.to_ascii_lowercase(),
            class.plugin_name.to_ascii_lowercase(),
            class.path.as_os_str().to_os_string(),
            class.id.clone(),
        )
    });
    Cache::new(classes)
}

fn read_classes(path: &Path) -> Vec<Class> {
    let mut classes = metadata::classes(path);
    if classes.is_empty() {
        classes = enumerate(path);
    }
    classes
}

fn enumerate(path: &Path) -> Vec<Class> {
    obs_vst3::enumerate(path)
        .unwrap_or_default()
        .into_iter()
        .map(|native| {
            class(
                path,
                native.name,
                native.id,
                native.discardable,
                native.sub_categories.split('|'),
            )
        })
        .collect()
}

pub(super) fn class(
    path: &Path,
    name: String,
    id: String,
    discardable: bool,
    sub_categories: impl Iterator<Item = impl AsRef<str>>,
) -> Class {
    Class {
        name,
        id,
        path: path.to_owned(),
        plugin_name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        discardable,
        is_fx: !sub_categories
            .into_iter()
            .any(|category| category.as_ref().contains("Instrument")),
    }
}
