#[path = "../../oracle_fragments.rs"]
mod reader;

#[test]
fn additions_edits_and_removals_are_discovered_in_order() {
    let directory = crate::common::ScratchDir::new("obs-oracle-fragments");
    let root = directory.path();
    std::fs::write(root.join("z.list"), "z.inc|libobs/z.c.h|auto\n").unwrap();
    std::fs::write(root.join("ignored.txt"), "not a fragment").unwrap();
    assert_eq!(reader::load(root).len(), 1);
    std::fs::write(
        root.join("a.list"),
        "a.inc|libobs/a.c.h|#include \"pch.h\"\n",
    )
    .unwrap();
    let entries = reader::load(root);
    assert_eq!(entries[0].output, "a.inc");
    assert_eq!(entries[0].source, std::path::Path::new("libobs/a.c.h"));
    assert_eq!(entries[0].pch, "#include \"pch.h\"");
    assert_eq!(entries[1].output, "z.inc");
    std::fs::write(root.join("a.list"), "a.inc|libobs/changed.c.h|auto\n").unwrap();
    assert_eq!(
        reader::load(root)[0].source,
        std::path::Path::new("libobs/changed.c.h")
    );
    std::fs::remove_file(root.join("z.list")).unwrap();
    assert_eq!(reader::load(root).len(), 1);
}

#[test]
fn invalid_or_duplicate_entries_fail_instead_of_dropping_coverage() {
    for invalid in [
        "a.inc|libobs/a.c.h\n",
        "a.inc|libobs/a.c.h|\n",
        "../a.inc|libobs/a.c.h|auto\n",
        "a.inc|../libobs/a.c.h|auto\n",
        "a.inc|/libobs/a.c.h|auto\n",
        "a.inc|D:/libobs/a.c.h|auto\n",
        "a.inc|libobs/a.c.h|auto\nb.inc|libobs/b.c.h|auto\n",
    ] {
        let directory = crate::common::ScratchDir::new("obs-oracle-fragments");
        std::fs::write(directory.path().join("a.list"), invalid).unwrap();
        assert!(
            std::panic::catch_unwind(|| reader::load(directory.path())).is_err(),
            "{invalid}"
        );
    }
    let directory = crate::common::ScratchDir::new("obs-oracle-fragments");
    for name in ["a.list", "b.list"] {
        std::fs::write(directory.path().join(name), "same.inc|libobs/a.c.h|auto\n").unwrap();
    }
    assert!(std::panic::catch_unwind(|| reader::load(directory.path())).is_err());
}
