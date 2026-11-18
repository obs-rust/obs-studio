//! Check the actual Cargo source resolver against every CMake host without SDKs.
#[path = "../src/libobs/source_plan.rs"]
mod production;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    process::Command,
};

#[test]
fn canonical_fixture_exports_match_cmake_on_every_host() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sets: BTreeMap<String, Vec<String>> = ["LIBOBS_SOURCES", "LIBOBS_TEST_SOURCES"]
        .into_iter()
        .map(|name| {
            let path = root.join(format!("libobs/build/source-sets.d/{name}.json"));
            (name.into(), serde_json::from_slice(&fs::read(path).unwrap()).unwrap())
        })
        .collect();
    let normal = production::common_sources(&sets, &root, false).unwrap();
    let enabled = production::common_sources(&sets, &root, true).unwrap();
    let fixtures: Vec<_> = enabled
        .iter()
        .filter(|source| !normal.contains(source))
        .map(|source| source.canonicalize().unwrap())
        .collect();
    let temp = tempfile::tempdir().unwrap();
    let plan = temp.path().join("cargo-fixtures.json");
    fs::write(&plan, serde_json::to_vec(&fixtures).unwrap()).unwrap();
    let cmake = Command::new("uv")
        .args(["run", "--no-project"])
        .arg(root.join("ci/test_set_locale_registration.py"))
        .env("OBS_BUILD_NATIVE_FIXTURE_PLAN", &plan)
        .output()
        .unwrap();
    assert!(
        cmake.status.success(),
        "{}{}",
        String::from_utf8_lossy(&cmake.stdout),
        String::from_utf8_lossy(&cmake.stderr)
    );
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
        assert!(compile.status.success(), "{}", String::from_utf8_lossy(&compile.stderr));
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
    assert_eq!(exported, BTreeSet::from(["obs_test_set_locale_swap_core".to_owned()]));
}
