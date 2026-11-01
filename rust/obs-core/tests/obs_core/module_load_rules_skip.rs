//! Tier 1: `load_all_callback()`'s skip ladder in
//! `libobs/obs-module-load-rules-skip.c.h`.
//!
//! The first four tests mirror the cases of
//! `test/cmocka/test_module_load_rules_skip_steps.c` one to one, in the same
//! order. The case marked "characterized, not endorsed" is odd but real
//! (#3409) and kept as the C has it. The tests after those pin the order of
//! the reads, and a proptest covers any type with every step and value.

use obs_core::module_load_rules::{Skip, TYPE_CORE, TYPE_LEGACY_PLUGIN, skip_step};
use proptest::prelude::*;

/// The undefined type, core, an unnamed one, legacy, one past it and the
/// largest word.
const TYPES: [u32; 6] = [0, TYPE_CORE, 2, TYPE_LEGACY_PLUGIN, 4, u32::MAX];

/// The steps, which read a value.
const STEPS: [Skip; 5] = [
    Skip::Plugin,
    Skip::Safe,
    Skip::Disabled,
    Skip::CoreName,
    Skip::Loaded,
];

/// The routes, which come back unchanged.
const ROUTES: [Skip; 8] = [
    Skip::NotPlugin,
    Skip::DisableSafe,
    Skip::Disable,
    Skip::LegacyCore,
    Skip::LoadedCore,
    Skip::LoadedLegacy,
    Skip::LoadedPlugin,
    Skip::Load,
];

/// The answer `answers` gives a step: one flag per step, in [`STEPS`]
/// order.
fn answer(answers: [bool; 5], step: Skip) -> bool {
    STEPS
        .iter()
        .zip(answers)
        .find_map(|(&s, a)| (s == step).then_some(a))
        .unwrap_or(false)
}

/// The reads `load_all_callback()` makes, in its order, each answered from
/// `answers`; the steps read, in order, and the route.
fn walk(module_type: u32, answers: [bool; 5]) -> (Skip, Vec<Skip>) {
    let mut step = skip_step(Skip::Start, module_type, false);
    let mut asked = Vec::new();
    while STEPS.contains(&step) {
        assert!(!asked.contains(&step), "{step:?} read twice");
        assert!(asked.len() < STEPS.len());
        asked.push(step);
        step = skip_step(step, module_type, answer(answers, step));
    }
    (step, asked)
}

fn answers_of(m: u32) -> [bool; 5] {
    core::array::from_fn(|i| (m >> i) & 1 == 1)
}

/// What the C's ladder reads and where it ends, as `load_all_callback()`
/// read before the move.
fn by_c(module_type: u32, answers: [bool; 5]) -> (Skip, Vec<Skip>) {
    let [plugin, safe, disabled, core_name, loaded] = answers;
    let core = module_type == TYPE_CORE;
    let legacy = module_type == TYPE_LEGACY_PLUGIN;
    let mut asked = vec![Skip::Plugin];
    if !plugin {
        return (Skip::NotPlugin, asked);
    }
    asked.push(Skip::Safe);
    if !safe {
        return (Skip::DisableSafe, asked);
    }
    if !core {
        asked.push(Skip::Disabled);
        if disabled {
            return (Skip::Disable, asked);
        }
    }
    if legacy {
        asked.push(Skip::CoreName);
        if core_name {
            return (Skip::LegacyCore, asked);
        }
    }
    asked.push(Skip::Loaded);
    let route = match (loaded, core, legacy) {
        (false, _, _) => Skip::Load,
        (true, true, _) => Skip::LoadedCore,
        (true, false, true) => Skip::LoadedLegacy,
        (true, false, false) => Skip::LoadedPlugin,
    };
    (route, asked)
}

// ------------------------------------------------- the C test, one to one

/// `test_constants`.
#[test]
fn test_constants() {
    assert_eq!(TYPE_CORE, 1);
    assert_eq!(TYPE_LEGACY_PLUGIN, 3);
    let all = [
        Skip::Start,
        Skip::Plugin,
        Skip::Safe,
        Skip::Disabled,
        Skip::CoreName,
        Skip::Loaded,
        Skip::NotPlugin,
        Skip::DisableSafe,
        Skip::Disable,
        Skip::LegacyCore,
        Skip::LoadedCore,
        Skip::LoadedLegacy,
        Skip::LoadedPlugin,
        Skip::Load,
    ];
    for (i, s) in all.iter().enumerate() {
        assert_eq!(*s as usize, i, "{s:?}");
    }
}

/// `test_each_step`.
#[test]
fn test_each_step() {
    for t in TYPES {
        let core = t == TYPE_CORE;
        let legacy = t == TYPE_LEGACY_PLUGIN;

        for v in [false, true] {
            assert_eq!(skip_step(Skip::Start, t, v), Skip::Plugin);
        }

        assert_eq!(skip_step(Skip::Plugin, t, false), Skip::NotPlugin);
        assert_eq!(skip_step(Skip::Plugin, t, true), Skip::Safe);

        // Characterized, not endorsed: the safe list does not look at the
        // type, so a core module left out of a safe-mode list is disabled,
        // although the disabled list never disables one.
        assert_eq!(skip_step(Skip::Safe, t, false), Skip::DisableSafe);
        // A core module never reads the disabled list, and is never legacy,
        // so it goes on to the loaded check.
        assert_eq!(
            skip_step(Skip::Safe, t, true),
            if core { Skip::Loaded } else { Skip::Disabled }
        );

        assert_eq!(skip_step(Skip::Disabled, t, true), Skip::Disable);
        assert_eq!(
            skip_step(Skip::Disabled, t, false),
            if legacy { Skip::CoreName } else { Skip::Loaded }
        );

        assert_eq!(skip_step(Skip::CoreName, t, true), Skip::LegacyCore);
        assert_eq!(skip_step(Skip::CoreName, t, false), Skip::Loaded);

        assert_eq!(skip_step(Skip::Loaded, t, false), Skip::Load);
        let loaded = if core {
            Skip::LoadedCore
        } else if legacy {
            Skip::LoadedLegacy
        } else {
            Skip::LoadedPlugin
        };
        assert_eq!(skip_step(Skip::Loaded, t, true), loaded);

        // A route comes back unchanged.
        for r in ROUTES {
            for v in [false, true] {
                assert_eq!(skip_step(r, t, v), r);
            }
        }
    }
}

/// `test_routes`.
#[test]
fn test_routes() {
    for t in TYPES {
        for m in 0..32 {
            let a = answers_of(m);
            assert_eq!(walk(t, a), by_c(t, a), "type {t} answers {a:?}");
        }
    }
}

/// `test_steps_per_type`.
#[test]
fn test_steps_per_type() {
    // A plugin on the safe list, not disabled, no core module's name and not
    // loaded yet.
    let load = [true, true, false, false, false];
    for t in TYPES {
        let mut want = vec![Skip::Plugin, Skip::Safe];
        if t != TYPE_CORE {
            want.push(Skip::Disabled);
        }
        if t == TYPE_LEGACY_PLUGIN {
            want.push(Skip::CoreName);
        }
        want.push(Skip::Loaded);
        assert_eq!(walk(t, load), (Skip::Load, want), "type {t}");
    }
}

// ----------------------------------------------------------------- edges

/// The plugin check comes first and the loaded check last, for every type
/// that gets that far: the read order `obs-module.c` keeps.
#[test]
fn plugin_first_loaded_last() {
    for t in TYPES {
        for m in 0..32 {
            let (route, asked) = walk(t, answers_of(m));
            assert_eq!(asked.first(), Some(&Skip::Plugin));
            if matches!(
                route,
                Skip::Load | Skip::LoadedCore | Skip::LoadedLegacy | Skip::LoadedPlugin
            ) {
                assert_eq!(asked.last(), Some(&Skip::Loaded));
            } else {
                assert!(!asked.contains(&Skip::Loaded));
            }
        }
    }
}

/// A core type never reads the disabled or core module lists, so it never
/// reaches [`Skip::Disable`] or [`Skip::LegacyCore`].
#[test]
fn core_is_never_disabled_by_the_list() {
    for m in 0..32 {
        let (route, asked) = walk(TYPE_CORE, answers_of(m));
        assert_ne!(route, Skip::Disable);
        assert_ne!(route, Skip::LegacyCore);
        assert!(!asked.contains(&Skip::Disabled));
        assert!(!asked.contains(&Skip::CoreName));
    }
}

fn any_type() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => prop::sample::select(TYPES.to_vec()),
        1 => any::<u32>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(512)))]

    #[test]
    fn ladder_matches_the_c(t in any_type(), m in 0..32_u32) {
        let a = answers_of(m);
        prop_assert_eq!(walk(t, a), by_c(t, a));
    }

    #[test]
    fn routes_stay(t in any::<u32>(), r in prop::sample::select(ROUTES.to_vec()), v in any::<bool>()) {
        prop_assert_eq!(skip_step(r, t, v), r);
    }
}
