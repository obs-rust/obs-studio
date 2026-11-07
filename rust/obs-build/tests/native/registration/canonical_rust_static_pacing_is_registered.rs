use super::*;

/// #1390: preserve the direct Rust-static characterization, without a C core.
#[test]
fn canonical_rust_static_pacing_is_registered() {
    let graph = rust_static_entries::assert_family("pacing");
    let name = "test_async_frame_pacing_steps_rust";
    for os in [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        for available in [false, true] {
            let facts = [
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
                ("OS_LINUX".into(), os.contains("linux")),
                ("OS_MACOS".into(), os.contains("apple")),
                ("OS_WINDOWS".into(), os.contains("windows")),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available, "{os}");
            assert_eq!(selected.tests.contains_key(name), available, "{os}");
        }
    }
}
