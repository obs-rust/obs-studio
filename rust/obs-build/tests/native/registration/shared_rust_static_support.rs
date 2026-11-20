#[test]
fn rust_static_precedes_import_providers_and_their_host_closure() {
    use obs_build::validation::graph::{Dependency, Graph};
    let graph = Graph::parse(
        r#"{"schema":1,"targets":{"fixture":{"sources":["main.c"],
        "libraries":["OBS::libobs","host_consumer","libobs_rust-static"]}},
        "tests":{"fixture":{"target":"fixture"}}}"#,
    )
    .unwrap();
    let dependencies = [
        (
            "OBS::libobs".into(),
            Dependency {
                artifacts: vec!["obs.lib".into()],
                ..Default::default()
            },
        ),
        (
            "host_consumer".into(),
            Dependency {
                artifacts: vec!["consumer.a".into(), "host.so".into()],
                ..Default::default()
            },
        ),
        (
            "libobs_rust-static".into(),
            Dependency {
                artifacts: vec!["rust.lib".into()],
                ..Default::default()
            },
        ),
    ]
    .into();
    assert_eq!(
        graph.targets["fixture"]
            .dependency_artifacts(&[("MSVC".into(), true)].into(), &dependencies)
            .unwrap(),
        ["rust.lib", "obs.lib", "consumer.a", "host.so"].map(std::path::PathBuf::from)
    );
    assert_eq!(
        graph.targets["fixture"]
            .dependency_artifacts(&[("MSVC".into(), false)].into(), &dependencies)
            .unwrap(),
        ["obs.lib", "consumer.a", "host.so", "rust.lib"].map(std::path::PathBuf::from)
    );
    let mut standalone = dependencies.clone();
    standalone
        .get_mut("libobs_rust-static")
        .unwrap()
        .rust_static_support = true;
    assert_eq!(
        graph.targets["fixture"]
            .dependency_artifacts(&[("MSVC".into(), true)].into(), &standalone)
            .unwrap(),
        ["rust.lib", "consumer.a", "host.so"].map(std::path::PathBuf::from)
    );
}

#[test]
fn rust_archive_owns_transitive_sdk_interfaces_and_target_facts() {
    use obs_build::validation::graph::{Dependency, Graph};
    let graph = Graph::parse(
        r#"{"schema":1,"targets":{"fixture":{"sources":["main.c"],
        "libraries":[{"name":"host_sdk","requires":"OS_LINUX"},"libobs_rust-static"]}},
        "tests":{"fixture":{"target":"fixture"}}}"#,
    )
    .unwrap();
    let owner = Dependency {
        artifacts: vec!["rust.a".into()],
        native_facts: Dependency::target_facts("x86_64-unknown-linux-gnu"),
        native_libraries: [(
            "host_sdk".into(),
            Dependency {
                artifacts: vec!["host.a".into()],
                ..Default::default()
            },
        )]
        .into(),
        ..Default::default()
    };
    let libraries = [("libobs_rust-static".into(), owner)].into();
    assert_eq!(
        graph.targets["fixture"]
            .dependency_artifacts(&Default::default(), &libraries)
            .unwrap(),
        ["host.a", "rust.a"].map(std::path::PathBuf::from)
    );
    assert_eq!(
        graph.targets["fixture"]
            .dependency_artifacts(&[("OS_LINUX".into(), false)].into(), &libraries)
            .unwrap(),
        ["rust.a"].map(std::path::PathBuf::from)
    );
    for feature in ["ENABLE_HEVC", "UNDECLARED_SDK"] {
        let graph = Graph::parse(
            &serde_json::json!({
                "schema": 1,
                "targets": {"fixture": {"sources": ["main.c"], "libraries": [
                    {"name": "host_sdk", "requires": feature}, "libobs_rust-static"
                ]}},
                "tests": {"fixture": {"target": "fixture"}}
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(
            graph.targets["fixture"]
                .dependency_artifacts(&Default::default(), &libraries)
                .unwrap_err(),
            format!("unknown native test requirement {feature}")
        );
    }
}
