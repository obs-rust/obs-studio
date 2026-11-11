use super::*;

/// #1390: preserve the two server-free PulseAudio monitoring decisions.
#[test]
fn canonical_pulse_monitor_decisions_are_registered() {
    use obs_build::validation::graph::{ConditionalValue, Graph, Source};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut graph = Graph::load(&root.join("test/cmocka/build/test-graph.d")).unwrap();
    let names = ["test_pulse_monitor_backlog", "test_pulse_monitor_flags"];
    for name in names {
        let target = graph
            .targets
            .get(name)
            .expect("missing PulseAudio decision target");
        assert!(
            matches!(&target.sources[0], Source::Path(path) if path == &format!("test/cmocka/{name}.c"))
        );
        assert!(
            matches!(&target.sources[1], Source::Path(path) if path == "libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c.h")
        );
        assert_eq!(target.sources.len(), 2);
        assert_eq!(target.includes, ["libobs", "libobs/audio-monitoring/pulse"]);
        assert_eq!(
            target.watch_inputs,
            [
                "libobs/build/libobs-ported-pulse.pch.h",
                "libobs/audio-monitoring/pulse/pulseaudio-stream-flags.h"
            ]
        );
        let libraries: Vec<_> = target
            .libraries
            .iter()
            .map(|library| match library {
                ConditionalValue::Name(name) => name.as_str(),
                _ => panic!("unconditional dependency expected"),
            })
            .collect();
        assert_eq!(
            libraries,
            ["PulseAudio::PulseAudio", "OBS::native-test-cmocka"]
        );
        let test = &graph.tests[name];
        assert_eq!(test.target, name);
        assert_eq!(test.command, ["${target}"]);
        assert_eq!(test.suite, "core");
        assert_eq!(test.timeout_seconds, None);
        assert_eq!(test.skip_return_code, None);
        assert!(test.optional_prefixes.is_empty());
        let fragment =
            fs::read_to_string(root.join(format!("test/cmocka/tests.d/{name}.cmake"))).unwrap();
        assert!(!fragment.contains("add_executable("));
        assert!(!fragment.contains("add_test("));
    }
    graph
        .targets
        .retain(|name, _| names.contains(&name.as_str()));
    graph.tests.retain(|name, _| names.contains(&name.as_str()));
    for os in ["OS_LINUX", "OS_FREEBSD", "OS_MACOS", "OS_WINDOWS"] {
        for enabled in [false, true] {
            let facts = [
                ("OS_LINUX".into(), os == "OS_LINUX"),
                ("OS_FREEBSD".into(), os == "OS_FREEBSD"),
                ("ENABLE_PULSEAUDIO".into(), enabled),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            let expected = usize::from(enabled && ["OS_LINUX", "OS_FREEBSD"].contains(&os)) * 2;
            assert_eq!(selected.targets.len(), expected, "{os}/{enabled}");
            assert_eq!(selected.tests.len(), expected, "{os}/{enabled}");
        }
    }
}
