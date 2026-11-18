//! Check the actual Cargo source resolver against every CMake host without SDKs.
#[path = "../src/libobs/source_plan.rs"]
mod production;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    process::Command,
};

fn fixture_sources() -> (PathBuf, Vec<PathBuf>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sets: BTreeMap<String, Vec<String>> = ["LIBOBS_SOURCES", "LIBOBS_TEST_SOURCES"]
        .into_iter()
        .map(|name| {
            let path = root.join(format!("libobs/build/source-sets.d/{name}.json"));
            (
                name.into(),
                serde_json::from_slice(&fs::read(path).unwrap()).unwrap(),
            )
        })
        .collect();
    let normal = production::common_sources(&sets, &root, false).unwrap();
    let enabled = production::common_sources(&sets, &root, true).unwrap();
    let fixtures: Vec<_> = enabled
        .iter()
        .filter(|source| !normal.contains(source))
        .map(|source| source.canonicalize().unwrap())
        .collect();
    (root, fixtures)
}

#[test]
fn canonical_fixture_sources_match_cmake_on_every_host() {
    let (root, fixtures) = fixture_sources();
    let temp = tempfile::tempdir().unwrap();
    for host in ["Linux", "Darwin", "Windows"] {
        let build = temp.path().join(host);
        let cmake = Command::new("cmake")
            .args(["-G", "Ninja"])
            .arg("-S")
            .arg(root.join("ci/fixtures/locale_registration"))
            .arg("-B")
            .arg(&build)
            .arg(format!("-DCMAKE_SYSTEM_NAME={host}"))
            .arg(format!(
                "-DFRAGMENT={}",
                root.join("test/cmocka/tests.d/test_set_locale.cmake")
                    .display()
            ))
            .output()
            .unwrap();
        assert!(
            cmake.status.success(),
            "{}",
            String::from_utf8_lossy(&cmake.stderr)
        );
        for library in ["libobs", "libobs-c-reference"] {
            let selected = fs::read_to_string(build.join(format!("{library}.sources"))).unwrap();
            let cmake_sources: Vec<_> = selected
                .lines()
                .map(|line| PathBuf::from(line).canonicalize().unwrap())
                .collect();
            assert_eq!(
                fixtures, cmake_sources,
                "{host} {library}: Cargo/CMake fixture inputs"
            );
        }
    }
}

#[test]
fn canonical_fixture_exports_match_failed_queue_diff() {
    let (_, fixtures) = fixture_sources();
    let temp = tempfile::tempdir().unwrap();
    // This fixture only swaps an opaque pointer. Compile its actual C body with
    // that declaration, without pulling platform SDKs or the rest of libobs in.
    fs::write(
        temp.path().join("obs-internal.h"),
        "struct obs_core;\nextern struct obs_core *obs;\n#define EXPORT __attribute__((visibility(\"default\")))\n",
    )
    .unwrap();
    let mut exported = BTreeSet::new();
    for (index, source) in fixtures.iter().enumerate() {
        let object = temp.path().join(format!("fixture-{index}.o"));
        let compile = Command::new("cc")
            .args(["-std=c17", "-fvisibility=hidden", "-I"])
            .arg(temp.path())
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let symbols = Command::new("nm")
            .args(["-g", "--defined-only"])
            .arg(&object)
            .output()
            .unwrap();
        assert!(symbols.status.success());
        exported.extend(
            String::from_utf8(symbols.stdout)
                .unwrap()
                .lines()
                .map(|line| line.split_whitespace().last().unwrap().to_owned()),
        );
    }
    assert_eq!(
        exported,
        BTreeSet::from(["obs_test_set_locale_swap_core".to_owned()])
    );
}
