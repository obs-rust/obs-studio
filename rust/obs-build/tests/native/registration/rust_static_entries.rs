//! Independent, checked-in expectations; never derive expected fields from the real graph.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use obs_build::validation::graph::{ConditionalValue, Graph};
use serde_json::{Value, json};

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../rust/obs-build/tests/native/registration/rust-static")
}

fn sorted_paths(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(directory)
        .map_err(|e| format!("{}: {e}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    paths.sort();
    Ok(paths)
}

fn read(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", path.display()))
}

struct Entries {
    graph: Graph,
    families: BTreeMap<String, Vec<String>>,
}

fn load(directory: &Path) -> Result<Entries, String> {
    let mut raw = json!({"schema": 1, "targets": {}, "tests": {}});
    let mut families = BTreeMap::new();
    for family in sorted_paths(&directory.join("cases"))? {
        let family_name = family.file_name().unwrap().to_str().unwrap().to_owned();
        if !matches!(
            family_name.as_str(),
            "source"
                | "header"
                | "decision"
                | "conversion"
                | "pacing"
                | "transition-start"
                | "monitor"
                | "graphics"
                | "private-source"
                | "private-steps"
                | "private-rules"
                | "private-link-rules"
        ) {
            return Err(format!("unknown registration family {family_name}"));
        }
        let mut cases = Vec::new();
        for path in sorted_paths(&family)? {
            let entry = read(&path)?;
            let graph = Graph::parse(&entry.to_string())?;
            let base = path.file_stem().unwrap().to_str().unwrap().to_owned();
            let name = format!("{base}_rust");
            if path.extension().is_none_or(|ext| ext != "json")
                || graph.targets.len() != 1
                || graph.tests.len() != 1
                || !graph.targets.contains_key(&name)
                || !graph.tests.contains_key(&name)
                || graph.tests[&name].target != name
            {
                return Err(format!(
                    "{}: expected one matching target/test",
                    path.display()
                ));
            }
            for section in ["targets", "tests"] {
                if raw[section].as_object().unwrap().contains_key(&name) {
                    return Err(format!("duplicate expectation {name}"));
                }
                raw[section][&name] = entry[section][&name].clone();
            }
            cases.push(base);
        }
        if cases.is_empty() {
            return Err(format!("missing entries for {family_name}"));
        }
        if matches!(family_name.as_str(), "conversion" | "pacing") && cases.len() != 1 {
            return Err(format!(
                "{family_name}: extend its singleton runtime before adding cases"
            ));
        }
        families.insert(family_name, cases);
    }
    append_support(directory, &mut raw)?;
    Ok(Entries {
        graph: Graph::parse(&raw.to_string())?,
        families,
    })
}

fn append_support(directory: &Path, raw: &mut Value) -> Result<(), String> {
    // Each port contributes an explicit target list and exact additional fields.
    // Stable file order and first-occurrence deduplication preserve array order.
    for path in sorted_paths(&directory.join("support"))? {
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let entry = read(&path)?;
        let targets = entry.as_object().ok_or("support entry must be an object")?;
        if targets.is_empty() {
            return Err("support entry has no targets".into());
        }
        for (name, fields) in targets {
            let target = raw["targets"]
                .as_object_mut()
                .unwrap()
                .get_mut(name)
                .ok_or_else(|| format!("support references missing entry {name}"))?;
            let fields = fields
                .as_object()
                .ok_or("support fields must be an object")?;
            if fields.is_empty() {
                return Err("support entry has no fields".into());
            }
            for (field, values) in fields {
                if !matches!(
                    field.as_str(),
                    "sources"
                        | "includes"
                        | "includes_before"
                        | "header_libraries"
                        | "header_packages"
                        | "packages"
                        | "libraries"
                        | "archives"
                        | "definitions"
                        | "compile_options"
                        | "compile_options_if_supported"
                        | "link_options"
                        | "watch_inputs"
                ) {
                    return Err(format!("unsupported support field {field}"));
                }
                let values = values.as_array().ok_or("support field must be an array")?;
                if values.is_empty() {
                    return Err(format!("empty support field {field}"));
                }
                let expected = target
                    .as_object_mut()
                    .unwrap()
                    .entry(field.clone())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .ok_or("expected array")?;
                for value in values {
                    if !expected.contains(value) {
                        expected.push(value.clone());
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn cases(family: &str) -> Vec<String> {
    load(&directory())
        .unwrap()
        .families
        .remove(family)
        .expect("missing family")
}

pub(super) fn assert_family(family: &str) -> Graph {
    let entries = load(&directory()).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut actual = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let names: BTreeSet<_> = entries.families[family]
        .iter()
        .map(|base| format!("{base}_rust"))
        .collect();
    for name in &names {
        // Debug covers every deserialized field, including defaults, ordered conditional
        // libraries/options, requirements and test properties. No graph-derived defaults.
        assert_eq!(
            format!("{:#?}", actual.targets.get(name).expect("missing target")),
            format!("{:#?}", entries.graph.targets[name]),
            "{name}: target"
        );
        assert_eq!(
            format!("{:#?}", actual.tests.get(name).expect("missing test")),
            format!("{:#?}", entries.graph.tests[name]),
            "{name}: test"
        );
        let base = name.strip_suffix("_rust").unwrap();
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{base}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    actual.targets.retain(|name, _| names.contains(name));
    actual.tests.retain(|name, _| names.contains(name));
    actual
}

fn assert_coverage(expected: &Graph, actual: &Graph) {
    let actual_names: BTreeSet<_> = actual
        .targets
        .iter()
        .filter(|(_, target)| {
            target.libraries.iter().any(|library| {
                matches!(library,
            ConditionalValue::Name(name) | ConditionalValue::Conditional { name, .. }
                if name == "libobs_rust-static")
            })
        })
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        expected.targets.keys().collect::<BTreeSet<_>>(),
        actual_names,
        "every Rust-static target needs an independent expectation"
    );
}

#[test]
fn entries_cover_every_rust_static_target() {
    let entries = load(&directory()).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let actual = Graph::load(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    assert_coverage(&entries.graph, &actual);
}

#[test]
fn missing_malformed_and_unknown_support_entries_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let family = directory.path().join("cases/source");
    fs::create_dir_all(&family).unwrap();
    fs::create_dir(directory.path().join("support")).unwrap();
    assert!(
        load(directory.path())
            .err()
            .unwrap()
            .contains("missing entries")
    );
    let path = family.join("test_fixture.json");
    fs::write(&path, "{malformed").unwrap();
    assert!(load(directory.path()).is_err());
    fs::write(
        &path,
        json!({"schema":1,"targets":{"test_fixture_rust":{
        "sources":["fixture.c"],"libraries":["libobs_rust-static"]}},
        "tests":{"test_fixture_rust":{"target":"test_fixture_rust"}}})
        .to_string(),
    )
    .unwrap();
    let entries = load(directory.path()).unwrap();
    // Create in reverse order: discovery must sort filenames, not directory order.
    fs::write(
        directory.path().join("support/z-port.json"),
        r#"{"test_fixture_rust":{"sources":["z.c","common.c"],"header_libraries":["headers"]}}"#,
    )
    .unwrap();
    fs::write(directory.path().join("support/a-port.json"),
        r#"{"test_fixture_rust":{"sources":["a.c","common.c"],"header_libraries":["headers"],"libraries":[{"name":"m","requires":{"not":"MSVC"}}]}}"#).unwrap();
    let extended = load(directory.path()).unwrap();
    let target = &extended.graph.targets["test_fixture_rust"];
    let sources: Vec<_> = target
        .sources
        .iter()
        .map(|source| match source {
            obs_build::validation::graph::Source::Path(path) => path.as_str(),
            _ => panic!("unexpected conditional source"),
        })
        .collect();
    assert_eq!(sources, ["fixture.c", "a.c", "common.c", "z.c"]);
    assert_eq!(target.header_libraries, ["headers"]);
    assert_eq!(target.libraries.len(), 2);
    assert!(
        matches!(&target.libraries[1], ConditionalValue::Conditional { name, requires }
        if name == "m" && format!("{requires:?}") == "Not { not: Fact(\"MSVC\") }")
    );
    fs::write(
        directory.path().join("support/port.json"),
        r#"{"missing_rust":{"sources":["support.c"]}}"#,
    )
    .unwrap();
    assert!(load(directory.path()).is_err());
    let mut missing = Graph::parse(r#"{"schema":1,"targets":{"test_fixture_rust":{"sources":["fixture.c"],"libraries":["libobs_rust-static"]}},"tests":{"test_fixture_rust":{"target":"test_fixture_rust"}}}"#).unwrap();
    missing.targets.clear();
    assert!(std::panic::catch_unwind(|| assert_coverage(&missing, &entries.graph)).is_err());
    let singleton = directory.path().join("cases/pacing");
    fs::create_dir(&singleton).unwrap();
    let template = fs::read_to_string(&path).unwrap();
    for base in ["test_first", "test_second"] {
        fs::write(
            singleton.join(format!("{base}.json")),
            template.replace("test_fixture_rust", &format!("{base}_rust")),
        )
        .unwrap();
    }
    assert!(
        load(directory.path())
            .err()
            .unwrap()
            .contains("singleton runtime")
    );
}

#[test]
fn rust_static_consumers_preserve_private_adapter_link_support() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raw = read(&root.join("test/cmocka/build/test-graph.json")).unwrap();
    let mut missing = Vec::new();
    for (name, target) in raw["targets"].as_object().unwrap() {
        if !target["libraries"]
            .as_array()
            .is_some_and(|libraries| libraries.contains(&json!("libobs_rust-static")))
        {
            continue;
        }
        // Mirror the two legacy deferred fragments' archive-consumer predicates.
        let support = if name.starts_with("test_scene_") {
            Some("test/cmocka/scene-item-transition-getter-link-stub.c")
        } else if [
            "test_async_",
            "test_deinterlace_",
            "test_source_",
            "test_transition_",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        {
            Some("test/cmocka/source-identity-getter-link-stubs.c")
        } else {
            None
        };
        if let Some(support) = support
            && !target["sources"]
                .as_array()
                .unwrap()
                .contains(&json!(support))
        {
            missing.push(format!("{name}: {support}"));
        }
    }
    assert!(
        missing.is_empty(),
        "missing private adapter support: {missing:#?}"
    );
}
