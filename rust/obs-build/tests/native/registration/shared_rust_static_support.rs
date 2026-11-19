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
            .dependency_artifacts(&Default::default(), &dependencies)
            .unwrap(),
        ["rust.lib", "obs.lib", "consumer.a", "host.so"].map(std::path::PathBuf::from)
    );
}
