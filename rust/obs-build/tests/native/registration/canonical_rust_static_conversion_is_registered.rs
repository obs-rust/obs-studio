use super::*;

/// #1390: the conversion table must exercise the production Rust archive.
#[test]
fn canonical_rust_static_conversion_is_registered() {
    let graph = rust_static_entries::assert_family("conversion");
    let name = "test_async_conversion_plan_rust";
    for os in [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ] {
        for available in [false, true] {
            let msvc = os.contains("msvc");
            let facts = [
                ("HAVE_LIBOBS_RUST_STATIC".into(), available),
                ("MSVC".into(), msvc),
            ]
            .into();
            let selected = graph.select(&facts).unwrap();
            assert_eq!(selected.targets.contains_key(name), available, "{os}");
            assert_eq!(selected.tests.contains_key(name), available, "{os}");
        }
    }
}
