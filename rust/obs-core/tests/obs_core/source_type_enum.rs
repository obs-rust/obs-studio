//! Tier 1 mirrors test_source_type_enum.c's enumeration assertions.
use obs_core::source_type_enum::{self, Output};
use std::cell::Cell;

const SENTINEL: &str = "untouched";

fn source<'a>(registry: &[(&'a str, &'a str)], index: usize, out: Option<&Cell<&'a str>>) -> bool {
    source_type_enum::source(
        registry.len(),
        index,
        |i| registry[i].0,
        |id| {
            out.expect("successful source query needs an output")
                .set(id)
        },
    )
}

fn input<'a>(
    registry: &[(&'a str, &'a str)],
    index: usize,
    out: Option<&Cell<&'a str>>,
    unversioned: Option<&Cell<&'a str>>,
) -> bool {
    source_type_enum::input(
        registry.len(),
        index,
        |i, slot| match slot {
            Output::Versioned => registry[i].0,
            Output::Unversioned => registry[i].1,
        },
        out.is_some(),
        unversioned.is_some(),
        |slot, id| match slot {
            Output::Versioned => out.unwrap().set(id),
            Output::Unversioned => unversioned.unwrap().set(id),
        },
    )
}

fn same(actual: &str, expected: &str) {
    assert!(std::ptr::eq(actual, expected));
}

fn source_end(registry: &[(&str, &str)], index: usize) {
    let id = Cell::new(SENTINEL);
    assert!(!source(registry, index, Some(&id)));
    same(id.get(), SENTINEL);
    assert!(!source(registry, index, None));
}

fn input_end(registry: &[(&str, &str)], index: usize) {
    for mask in 0..4 {
        let id = Cell::new(SENTINEL);
        let unversioned = Cell::new(SENTINEL);
        assert!(!input(
            registry,
            index,
            (mask & 1 != 0).then_some(&id),
            (mask & 2 != 0).then_some(&unversioned)
        ));
        same(id.get(), SENTINEL);
        same(unversioned.get(), SENTINEL);
    }
}

#[test]
fn test_source_type_enum_empty_registries() {
    for index in [0, usize::MAX] {
        source_end(&[], index);
        input_end(&[], index);
    }
}

#[test]
fn omitted_input_outputs_do_not_read_fields() {
    assert!(source_type_enum::input(
        1,
        0,
        |_, _| -> u8 { panic!("absent output read") },
        false,
        false,
        |_, _| panic!("absent output write")
    ));
    for mask in 0..4 {
        let reads = Cell::new(0);
        let writes = Cell::new(0);
        assert!(source_type_enum::input(
            1,
            0,
            |index, slot| {
                assert_eq!(index, 0);
                reads.set(reads.get() + 1);
                match slot {
                    Output::Versioned => {
                        assert_ne!(mask & 1, 0);
                        1
                    }
                    Output::Unversioned => {
                        assert_ne!(mask & 2, 0);
                        2
                    }
                }
            },
            mask & 1 != 0,
            mask & 2 != 0,
            |slot, value| {
                writes.set(writes.get() + 1);
                assert_eq!(value, if slot == Output::Versioned { 1 } else { 2 });
            }
        ));
        assert_eq!(reads.get(), (mask & 1) + ((mask >> 1) & 1));
        assert_eq!(writes.get(), reads.get());
    }
}

#[test]
fn test_source_type_enum_populated_registries() {
    // Registration (retained in C) owns every versioned copy, including v0.
    let originals = [
        "enum_input",
        "enum_filter",
        "enum_input",
        "enum_transition",
        "enum_scene",
    ];
    let expected = [
        "enum_input",
        "enum_filter",
        "enum_input_v3",
        "enum_transition",
        "enum_scene",
    ];
    let owned = expected.map(str::to_owned);
    let sources: Vec<_> = owned
        .iter()
        .zip(originals)
        .map(|(id, original)| (id.as_str(), original))
        .collect();
    let inputs = [sources[0], sources[2]];
    assert_eq!(sources.len(), 5);
    assert_eq!(inputs.len(), 2);
    let borrowed = sources
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let id = Cell::new(SENTINEL);
            assert!(source(&sources, i, Some(&id)));
            assert_eq!(id.get(), expected[i]);
            let again = Cell::new(SENTINEL);
            assert!(source(&sources, i, Some(&again)));
            same(again.get(), id.get());
            id.get()
        })
        .collect::<Vec<_>>();
    for i in 0..2 {
        let id = Cell::new(SENTINEL);
        let unversioned = Cell::new(SENTINEL);
        assert!(input(&inputs, i, Some(&id), Some(&unversioned)));
        same(id.get(), borrowed[i * 2]);
        same(unversioned.get(), originals[0]);
        assert!(!std::ptr::eq(id.get(), originals[0]));
        // Legacy enumeration reads the same registration; its implementation stays C.
        same(inputs[i].0, id.get());
        for mask in 0..4 {
            let only_id = Cell::new(SENTINEL);
            let only_unversioned = Cell::new(SENTINEL);
            assert!(input(
                &inputs,
                i,
                (mask & 1 != 0).then_some(&only_id),
                (mask & 2 != 0).then_some(&only_unversioned)
            ));
            same(
                only_id.get(),
                if mask & 1 != 0 { id.get() } else { SENTINEL },
            );
            same(
                only_unversioned.get(),
                if mask & 2 != 0 {
                    unversioned.get()
                } else {
                    SENTINEL
                },
            );
        }
        let alias = Cell::new(SENTINEL);
        assert!(input(&inputs, i, Some(&alias), Some(&alias)));
        same(alias.get(), unversioned.get());
    }
    for index in [5, 6, u32::MAX as usize, usize::MAX] {
        source_end(&sources, index);
    }
    for index in [2, 3, u32::MAX as usize, usize::MAX] {
        input_end(&inputs, index);
    }
    assert_eq!(borrowed[0], originals[0]);
    assert_eq!(borrowed[2], "enum_input_v3");
}
