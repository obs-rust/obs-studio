//! Tier 1 mirror of test_output_starting_queries.c::flag_matrix.
use obs_output::output_starting_queries::{Flag, needs_encoders, starting};

#[test]
fn c_flag_matrix_reads_only_the_ordered_prefix_and_preserves_storage() {
    let fields = [
        Flag::Active,
        Flag::Starting,
        Flag::Reconnecting,
        Flag::DelayActive,
    ];
    for mask in 0u8..16 {
        // The two unrelated true flags mirror delay_capturing and data_active.
        let storage = [
            mask & 1 != 0,
            mask & 2 != 0,
            mask & 4 != 0,
            mask & 8 != 0,
            true,
            true,
        ];
        let before = storage;
        for first in 0..2 {
            let mut reads = Vec::new();
            let load = |flag| {
                let index = fields.iter().position(|&field| field == flag).unwrap();
                reads.push(index);
                storage[index]
            };
            let actual = if first == 0 {
                needs_encoders(load)
            } else {
                starting(load)
            };
            let last = (first..4).find(|&i| storage[i]).unwrap_or(3);
            assert_eq!(actual, mask & if first == 0 { 15 } else { 14 } != 0);
            assert_eq!(reads, (first..=last).collect::<Vec<_>>());
            assert_eq!(storage, before);
        }
    }
}

#[test]
fn later_loads_observe_changes_after_the_first_load() {
    let mut flags = [false; 4];
    let mut reads = Vec::new();
    assert!(needs_encoders(|flag| {
        reads.push(flag);
        match flag {
            Flag::Active => {
                flags[1] = true;
                false
            }
            Flag::Starting => flags[1],
            _ => panic!("load after true"),
        }
    }));
    assert_eq!(reads, [Flag::Active, Flag::Starting]);
}
