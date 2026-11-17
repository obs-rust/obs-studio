//! Build-time oracle extraction registrations, one fragment per output.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct Extract {
    pub output: String,
    pub source: PathBuf,
    pub pch: String,
}

pub fn paths(directory: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .expect("read oracle.d")
        .map(|entry| entry.expect("oracle fragment entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "list"))
        .collect();
    paths.sort();
    paths
}

pub fn load(directory: &Path) -> Vec<Extract> {
    let mut outputs = BTreeSet::new();
    paths(directory)
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("read oracle fragment");
            let lines: Vec<_> = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .collect();
            assert_eq!(
                lines.len(),
                1,
                "{}: one extraction per fragment",
                path.display()
            );
            let fields: Vec<_> = lines[0].split('|').collect();
            assert_eq!(fields.len(), 3, "{}: output|source|pch", path.display());
            let [output, source, pch] = [fields[0], fields[1], fields[2]];
            assert!(
                output.ends_with(".inc")
                    && output
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                    && !output.contains(".."),
                "{}: invalid output filename",
                path.display()
            );
            assert!(
                !source.is_empty()
                    && !source.contains('\\')
                    && !source.contains(':')
                    && Path::new(source)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_))),
                "{}: source must be relative to the repository",
                path.display()
            );
            assert!(!pch.is_empty(), "{}: empty PCH marker", path.display());
            assert!(
                outputs.insert(output.to_owned()),
                "duplicate oracle output: {output}"
            );
            Extract {
                output: output.to_owned(),
                source: source.into(),
                pch: pch.to_owned(),
            }
        })
        .collect()
}
