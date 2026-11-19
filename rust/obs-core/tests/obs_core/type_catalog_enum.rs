//! Tier 1 mirrors test_type_catalog_enum.c using only test-owned IDs.
use obs_core::type_catalog_enum;
use std::cell::Cell;

const IDS: [[&str; 2]; 6] = [
    ["catalog_input_z", "catalog_input_a"],
    ["catalog_filter_z", "catalog_filter_a"],
    ["catalog_transition_z", "catalog_transition_a"],
    ["catalog_output_z", "catalog_output_a"],
    ["catalog_encoder_z", "catalog_encoder_a"],
    ["catalog_service_z", "catalog_service_a"],
];
const SENTINELS: [&str; 6] = [
    "input untouched",
    "filter untouched",
    "transition untouched",
    "output untouched",
    "encoder untouched",
    "service untouched",
];

fn query<'a>(registry: &[&'a str], index: usize, output: Option<&Cell<&'a str>>) -> bool {
    type_catalog_enum::enumerate(
        registry.len(),
        index,
        |i| registry[i],
        |id| output.expect("success requires output").set(id),
    )
}
fn end(registry: &[&str], index: usize, sentinel: &str) {
    let id = Cell::new(sentinel);
    assert!(!query(registry, index, Some(&id)));
    assert!(std::ptr::eq(id.get(), sentinel));
    assert!(!query(registry, index, None));
}
fn count(registry: &[&str]) -> usize {
    let output = Cell::new("");
    (0..=registry.len())
        .take_while(|&i| query(registry, i, Some(&output)))
        .count()
}
#[test]
fn test_type_catalog_enum_empty_catalogs() {
    for sentinel in SENTINELS {
        for index in [0, usize::MAX] {
            end(&[], index, sentinel);
        }
    }
}
#[test]
fn test_type_catalog_enum_populated_catalogs() {
    // Source registration owns a versioned copy; the source and per-kind
    // catalogs share that ID. Other catalogs borrow the caller's ID.
    let source_ids: Vec<Vec<String>> = IDS[..3]
        .iter()
        .map(|ids| ids.iter().map(|id| (*id).to_owned()).collect())
        .collect();
    let mut catalogs: [Vec<&str>; 6] = std::array::from_fn(|_| Vec::new());
    let mut added = [0; 6];
    let mut borrowed = [[""; 2]; 6];
    for entry in 0..2 {
        for c in 0..6 {
            let id = if c < 3 {
                source_ids[c][entry].as_str()
            } else {
                IDS[c][entry]
            };
            catalogs[c].push(id);
            added[c] += 1;
            for other in 0..6 {
                assert_eq!(count(&catalogs[other]), added[other]);
            }
            let output = Cell::new(SENTINELS[c]);
            assert!(query(&catalogs[c], entry, Some(&output)));
            assert_eq!(output.get(), IDS[c][entry]);
            borrowed[c][entry] = output.get();
        }
    }
    for c in 0..6 {
        for entry in 0..2 {
            let output = Cell::new(SENTINELS[c]);
            assert!(query(&catalogs[c], entry, Some(&output)));
            assert!(std::ptr::eq(output.get(), borrowed[c][entry]));
            assert_eq!(output.get(), IDS[c][entry]);
            let origin = if c < 3 {
                source_ids[c][entry].as_str()
            } else {
                IDS[c][entry]
            };
            assert!(std::ptr::eq(output.get(), origin));
        }
        for index in [2, 3, usize::MAX] {
            end(&catalogs[c], index, SENTINELS[c]);
        }
        assert_eq!(borrowed[c][0], IDS[c][0]);
    }
}
