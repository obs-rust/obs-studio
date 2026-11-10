//! Tier 1 mirrors test_base_effect.c without FFI or graphics ownership.
use obs_core::base_effect::{Field, get};

const FIELDS: [Field; 10] = [
    Field::Default,
    Field::DefaultRect,
    Field::Opaque,
    Field::Solid,
    Field::Bicubic,
    Field::Lanczos,
    Field::BilinearLowres,
    Field::PremultipliedAlpha,
    Field::Repeat,
    Field::Area,
];

#[test]
fn test_base_effect_loaded_mappings_and_identity() {
    let effects: [String; 10] = std::array::from_fn(|i| format!("effect-{i}"));
    for pass in 0..4 {
        for (id, expected) in FIELDS.iter().enumerate() {
            let mut reads = 0;
            let result = get(id as i32, |field| {
                reads += 1;
                assert_eq!(&field, expected);
                Some(&effects[id])
            });
            assert_eq!(reads, 1, "pass {pass}");
            assert!(std::ptr::eq(result.unwrap(), &effects[id]));
        }
        for i in 0..10 {
            for j in 0..i {
                assert!(!std::ptr::eq(&effects[i], &effects[j]));
            }
        }
    }
}

#[test]
fn test_base_effect_unloaded_effects_and_replacements() {
    for id in 0..10 {
        assert_eq!(get::<Option<&str>>(id, |_| None), None);
        for effect in [Some("loaded"), None, Some("replacement"), Some("alias")] {
            assert_eq!(get(id, |_| effect), effect);
        }
    }
}

#[test]
fn test_base_effect_unknown_enums_never_read_storage() {
    // Before startup, live core, and after shutdown all have the same result.
    for _ in 0..3 {
        for id in [-1, i32::MIN, 10, 11, i32::MAX] {
            assert_eq!(get::<Option<&str>>(id, |_| panic!("unknown ID read core")), None);
        }
    }
}
