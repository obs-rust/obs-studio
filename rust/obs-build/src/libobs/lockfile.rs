//! Whether a newer `Cargo.lock` changes what a Rust archive was built from
//! (#5323). A lockfile also changes when a crate the archive does not use is
//! added or updated; Cargo then correctly keeps the archive, and comparing
//! modification times alone reports it stale.
//!
//! Cargo's depfile lists only the archive's path dependencies, not its
//! registry packages, so the lockfile itself must tell which packages the
//! archive uses: its closure, every package reachable from the archive's
//! crate through the lock's dependency lists. When the archive is newer than
//! the lockfile, the check saves that closure in a stamp beside the archive.
//! A newer lockfile is fresh only when its closure equals the stamp's, for an
//! archive with the same modification time.

use std::{collections::BTreeSet, path::Path, time::SystemTime};

const STAMP: &str = "obs-build lockfile closure v1";

struct Package {
    name: String,
    version: String,
    source: String,
    checksum: String,
    dependencies: Vec<String>,
}

/// The closure of the local crate `root` in `lock`, one line per package,
/// sorted. `None` when `lock` is not a lockfile Cargo wrote, has no local
/// `root` crate, or names a dependency it does not lock.
pub(super) fn closure(lock: &str, root: &str) -> Option<String> {
    let packages = parse(lock)?;
    let start = packages
        .iter()
        .position(|package| package.name == root && package.source.is_empty())?;
    let mut lines = Vec::new();
    let mut seen = BTreeSet::from([start]);
    let mut queue = vec![start];
    while let Some(index) = queue.pop() {
        let package = &packages[index];
        // Cargo writes `name` or `name version` depending on how many
        // versions of `name` are locked: compare what an entry resolves to.
        let mut resolved = BTreeSet::new();
        for dependency in &package.dependencies {
            let mut words = dependency.split_whitespace();
            let name = words.next()?;
            let version = words.next();
            let before = resolved.len();
            for (other, candidate) in packages.iter().enumerate() {
                if candidate.name == name
                    && version.is_none_or(|version| candidate.version == version)
                {
                    resolved.insert(format!("{name} {}", candidate.version));
                    if seen.insert(other) {
                        queue.push(other);
                    }
                }
            }
            if resolved.len() == before {
                return None;
            }
        }
        lines.push(format!(
            "{} {} {} {} [{}]",
            package.name,
            package.version,
            package.source,
            package.checksum,
            resolved.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    lines.sort();
    Some(lines.join("\n"))
}

/// The stamp text for an archive modified at `modified` built from `closure`.
pub(super) fn stamp(modified: SystemTime, closure: &str) -> String {
    format!("{STAMP}\n{}\n{closure}\n", time(modified))
}

/// Whether `stamp` records `closure` for an archive modified at `modified`,
/// or why not.
pub(super) fn stamp_matches(
    stamp: &str,
    modified: SystemTime,
    closure: &str,
) -> Result<(), &'static str> {
    let mut lines = stamp.splitn(3, '\n');
    if lines.next() != Some(STAMP) || lines.next() != Some(time(modified).as_str()) {
        return Err("the lockfile stamp is for another build of the archive");
    }
    match lines
        .next()
        .map(|rest| rest.strip_suffix('\n').unwrap_or(rest))
    {
        Some(saved) if saved == closure => Ok(()),
        _ => Err("it changes the packages of libobs-rust"),
    }
}

fn time(modified: SystemTime) -> String {
    match modified.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(since) => format!("{}.{:09}", since.as_secs(), since.subsec_nanos()),
        Err(before) => format!("-{:?}", before.duration()),
    }
}

/// The `name` in the `[package]` table of a crate manifest.
pub(super) fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && let Some(name) = string_value(line, "name") {
            return Some(name);
        }
    }
    None
}

/// The stamp path beside `archive`.
pub(super) fn stamp_path(archive: &Path) -> std::path::PathBuf {
    archive.with_extension("lock-closure")
}

/// The packages of a lockfile, or `None` when it is not one Cargo wrote: no
/// top-level `version`, a package without name or version, an unclosed list.
fn parse(lock: &str) -> Option<Vec<Package>> {
    let mut packages: Vec<Package> = Vec::new();
    let mut versioned = false;
    let mut in_package = false;
    let mut in_dependencies = false;
    for line in lock.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if in_dependencies {
            if line == "]" {
                in_dependencies = false;
            } else {
                let entry = line
                    .trim_end_matches(',')
                    .strip_prefix('"')?
                    .strip_suffix('"')?;
                packages.last_mut()?.dependencies.push(entry.to_owned());
            }
            continue;
        }
        if line.starts_with('[') {
            in_package = line == "[[package]]";
            if in_package {
                packages.push(Package {
                    name: String::new(),
                    version: String::new(),
                    source: String::new(),
                    checksum: String::new(),
                    dependencies: Vec::new(),
                });
            }
            continue;
        }
        if !in_package {
            versioned |= line.starts_with("version =");
            continue;
        }
        let package = packages.last_mut()?;
        if let Some(name) = string_value(line, "name") {
            package.name = name;
        } else if let Some(version) = string_value(line, "version") {
            package.version = version;
        } else if let Some(source) = string_value(line, "source") {
            package.source = source;
        } else if let Some(checksum) = string_value(line, "checksum") {
            package.checksum = checksum;
        } else if let Some(list) = line.strip_prefix("dependencies =") {
            match list.trim() {
                "[" => in_dependencies = true,
                "[]" => {}
                _ => return None,
            }
        }
    }
    let complete = packages
        .iter()
        .all(|package| !package.name.is_empty() && !package.version.is_empty());
    (versioned && !in_dependencies && complete).then_some(packages)
}

fn string_value(line: &str, key: &str) -> Option<String> {
    let value = line
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim();
    // A trailing `# comment` may follow the closing quote.
    let (text, _) = value.strip_prefix('"')?.split_once('"')?;
    Some(text.to_owned())
}
