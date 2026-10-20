use obs_c_oracle as _;

use std::collections::BTreeSet;
use std::fs;

/// Names of fns and statics following a `#[unsafe(no_mangle)]` /
/// `#[no_mangle]` attribute (a static is C data, such as
/// `obs_blend_mode_params`).
fn no_mangle_fns(src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t != "#[unsafe(no_mangle)]" && t != "#[no_mangle]" {
            continue;
        }
        for next in &lines[i + 1..] {
            let item = ["fn ", "static "]
                .iter()
                .filter_map(|kw| next.find(kw).map(|pos| pos + kw.len()))
                .min();
            if let Some(pos) = item {
                let name: String = next[pos..]
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                out.push(name);
                break;
            }
        }
    }
    out
}

fn symbol_list(rel: &str) -> BTreeSet<String> {
    let text = fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../libobs/cmake/").to_owned() + rel,
    )
    .unwrap_or_else(|_| panic!("read {rel}"));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_whitespace().next().map(String::from))
        .collect()
}

const EXPORTS_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libobs/cmake/rust-exports.d"
);

/// `rust-exports.d/<crate>-<ffi module>.txt` fragments (obs-rust/obs-studio#137):
/// fragment stem -> lines (`symbol` or `symbol unix`).
fn export_fragments() -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = fs::read_dir(EXPORTS_DIR)
        .expect("read libobs/cmake/rust-exports.d")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "txt"))
        .map(|p| {
            let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
            let lines = fs::read_to_string(&p)
                .expect("read fragment")
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect();
            (stem, lines)
        })
        .collect();
    out.sort();
    out
}

/// Every exported symbol, from all fragments.
fn exported_symbols() -> BTreeSet<String> {
    export_fragments()
        .into_iter()
        .flat_map(|(_, lines)| lines)
        .filter_map(|l| l.split_whitespace().next().map(String::from))
        .collect()
}

/// Whether `name` is a dependency of `libobs-rust`, the one staticlib libobs
/// links. A crate linked elsewhere (obs-nvenc goes into plugins/obs-nvenc,
/// obs-rust/obs-studio#524) exports its shims from that module, not libobs.
fn linked_into_libobs(name: &str) -> bool {
    let manifest = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../libobs-rust/Cargo.toml"
    ))
    .expect("read libobs-rust/Cargo.toml");
    manifest
        .lines()
        .any(|l| l.trim_start().starts_with(&format!("{name} = ")))
}

/// (crate, ffi module) -> its `#[no_mangle]` shims, for every rust/*/src/ffi/*.rs.
fn ffi_modules() -> Vec<((String, String), Vec<String>)> {
    let mut out = Vec::new();
    for krate in fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).expect("read rust/") {
        let krate = krate.expect("dir entry").path();
        let ffi_dir = krate.join("src/ffi");
        if !ffi_dir.is_dir() {
            continue;
        }
        let name = krate.file_name().unwrap().to_string_lossy().into_owned();
        if !linked_into_libobs(&name) {
            continue;
        }
        for entry in fs::read_dir(&ffi_dir).expect("read src/ffi") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let module = path.file_stem().unwrap().to_string_lossy().into_owned();
                let src = fs::read_to_string(&path).expect("read ffi source");
                out.push(((name.clone(), module), no_mangle_fns(&src)));
            }
        }
    }
    out.sort();
    out
}

/// Each FFI module with exported shims owns exactly one fragment,
/// `rust-exports.d/<crate>-<module>.txt`, listing exactly those shims, so a
/// new port adds a file instead of editing a list every other port edits.
#[test]
fn rust_exports_are_one_fragment_per_ffi_module() {
    let hidden = symbol_list("rust-hidden.txt");
    let fragments: std::collections::BTreeMap<String, BTreeSet<String>> = export_fragments()
        .into_iter()
        .map(|(stem, lines)| {
            let names = lines
                .iter()
                .filter_map(|l| l.split_whitespace().next().map(String::from))
                .collect();
            (stem, names)
        })
        .collect();
    let mut expected = std::collections::BTreeMap::new();
    for ((krate, module), shims) in ffi_modules() {
        let exported: BTreeSet<String> =
            shims.into_iter().filter(|s| !hidden.contains(s)).collect();
        if !exported.is_empty() {
            expected.insert(format!("{krate}-{module}"), exported);
        }
    }
    assert_eq!(
        fragments.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "rust-exports.d must hold one <crate>-<ffi module>.txt per FFI module with exported shims"
    );
    for (stem, names) in &expected {
        assert_eq!(&fragments[stem], names, "rust-exports.d/{stem}.txt");
    }
}

#[test]
fn rust_exports_list_matches_no_mangle_shims() {
    let listed = exported_symbols();
    let hidden = symbol_list("rust-hidden.txt");
    let overlap: Vec<_> = listed.intersection(&hidden).collect();
    assert!(
        overlap.is_empty(),
        "symbol is both exported and hidden: {overlap:?}"
    );

    let mut found = BTreeSet::new();
    let mut visited = Vec::new();
    for krate in fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).expect("read rust/") {
        let krate = krate.expect("dir entry").path();
        let ffi_dir = krate.join("src/ffi");
        if !ffi_dir.is_dir() || !linked_into_libobs(&krate.file_name().unwrap().to_string_lossy()) {
            continue;
        }
        for entry in fs::read_dir(&ffi_dir).expect("read src/ffi") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let src = fs::read_to_string(&path).expect("read ffi source");
                found.extend(no_mangle_fns(&src));
            }
        }
        visited.push(ffi_dir);
    }
    for krate in ["obs-util", "obs-graphics", "obs-codec", "obs-media-io"] {
        assert!(
            visited
                .iter()
                .any(|d| d.ends_with(format!("{krate}/src/ffi"))),
            "{krate}/src/ffi was not scanned: {visited:?}"
        );
    }

    let mut accounted = listed.clone();
    accounted.extend(hidden.iter().cloned());
    let missing_from_list: Vec<_> = found.difference(&accounted).collect();
    let missing_from_src: Vec<_> = accounted.difference(&found).collect();
    assert!(
        missing_from_list.is_empty() && missing_from_src.is_empty(),
        "shims missing from rust-exports.d and rust-hidden.txt: {missing_from_list:?}; \
         listed but no #[no_mangle] shim in rust/*/src/ffi:{missing_from_src:?}"
    );

    let map = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../libobs/cmake/rust-exports.map"
    ))
    .expect("read rust-exports.map");
    let macos = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../libobs/cmake/rust-unexports-macos.txt"
    ))
    .expect("read rust-unexports-macos.txt");
    for name in &hidden {
        assert!(
            map.contains(&format!("{name};")),
            "{name} is hidden but rust-exports.map does not list it as local"
        );
        assert!(
            macos.lines().any(|line| line.trim() == format!("_{name}")),
            "{name} is hidden but rust-unexports-macos.txt has no _{name}"
        );
    }
}

/// Names of no_mangle fns inside a `#[cfg(...)] mod name { ... }` block
/// (closed by a `}` in column 0).
fn cfg_mod_fns(src: &str, cfg: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == cfg
            && lines
                .get(i + 1)
                .is_some_and(|l| l.starts_with("mod ") && l.trim_end().ends_with('{'))
        {
            let start = i + 2;
            let end = lines[start..]
                .iter()
                .position(|l| *l == "}")
                .map_or(lines.len(), |p| start + p);
            out.extend(no_mangle_fns(&lines[start..end].join("\n")));
            i = end;
        }
        i += 1;
    }
    out
}

/// Names that exist only inside a `#[cfg(unix)]` module. A symbol that also
/// has a Windows shim is compiled on both platforms, so rust.cmake must
/// `/EXPORT` it and the fragment must not mark it `unix`.
fn unix_only_fns(src: &str) -> Vec<String> {
    let windows: BTreeSet<String> = cfg_mod_fns(src, "#[cfg(windows)]").into_iter().collect();
    cfg_mod_fns(src, "#[cfg(unix)]")
        .into_iter()
        .filter(|name| !windows.contains(name))
        .collect()
}

/// `rust-exports.d` lines are `symbol` or `symbol unix`. The `unix`
/// marker keeps libobs/cmake/rust.cmake from emitting `/EXPORT:symbol` on
/// Windows, where the shim is not compiled and the C file (e.g.
/// util/pipe-windows.c) still provides the dllexport'ed symbol.
#[test]
fn unix_only_shims_are_marked_unix_in_rust_exports() {
    let marked: BTreeSet<String> = export_fragments()
        .into_iter()
        .flat_map(|(_, lines)| lines)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let name = it.next()?;
            (it.next() == Some("unix")).then(|| name.to_owned())
        })
        .collect();
    let mut unix_only = BTreeSet::new();
    for krate in fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/..")).expect("read rust/") {
        let ffi_dir = krate.expect("dir entry").path().join("src/ffi");
        if !ffi_dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&ffi_dir).expect("read src/ffi") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let src = fs::read_to_string(&path).expect("read ffi source");
                unix_only.extend(unix_only_fns(&src));
            }
        }
    }
    // No shim is Unix-only since the pipe-windows.c port, so the scan
    // finding nothing is expected; the check stays for per-OS ports.
    assert_eq!(
        marked, unix_only,
        "rust-exports.d `unix` markers must match the cfg(unix) shims"
    );
}
