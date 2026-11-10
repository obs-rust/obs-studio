//! Tier 3: original C switch over real private storage; no intentional differences.
use obs_core::base_effect::{Field, get};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use std::ffi::c_void;
use std::ptr::without_provenance_mut;

unsafe extern "C" {
    fn oracle_base_effect_set(effects: *const *mut c_void) -> *mut c_void;
    fn oracle_base_effect_clear();
    fn oracle_base_effect_unchanged() -> bool;
    fn oracle_obs_get_base_effect(effect: i32) -> *mut c_void;
}

fn compare(fields: [usize; 10]) {
    let pointers = fields.map(without_provenance_mut::<c_void>);
    // SAFETY: fixture copies ten tokens, never dereferences them. This test is
    // isolated in a child, so only this test accesses the fixture's global.
    unsafe { oracle_base_effect_set(pointers.as_ptr()) };
    for id in (0..10).chain([-1, i32::MIN, 10, 11, i32::MAX]) {
        for _ in 0..4 {
            let actual = get(id, |field| {
                let index = match field {
                    Field::Default => 0,
                    Field::DefaultRect => 1,
                    Field::Opaque => 2,
                    Field::Solid => 3,
                    Field::Bicubic => 4,
                    Field::Lanczos => 5,
                    Field::BilinearLowres => 6,
                    Field::PremultipliedAlpha => 7,
                    Field::Repeat => 8,
                    Field::Area => 9,
                };
                fields[index]
            });
            // SAFETY: known IDs have a live fixture; unknown IDs don't read it.
            assert_eq!(actual, unsafe { oracle_obs_get_base_effect(id) } as usize);
        }
    }
    // SAFETY: fixture remains alive and is only accessed in this child.
    assert!(unsafe { oracle_base_effect_unchanged() });
}

#[test]
fn characterized_edges_and_random_fields_match_c() {
    const CHILD: &str = "OBS_BASE_EFFECT_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!("{}::characterized_edges_and_random_fields_match_c",
            module_path!().split_once("::").unwrap().1);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1").output().unwrap();
        assert!(output.status.success(), "{}\n{}",
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    for _ in 0..2 {
        // SAFETY: unknown enums do not access the core, including before startup
        // and after shutdown. Known enums without a core are outside C's domain.
        unsafe { oracle_base_effect_clear() };
        for id in [-1, i32::MIN, 10, 11, i32::MAX] {
            assert_eq!(get::<usize>(id, |_| panic!("unknown ID read core")), 0);
            // SAFETY: unknown ID does not read the null core.
            assert!(unsafe { oracle_obs_get_base_effect(id) }.is_null());
        }
        compare([0; 10]);
        compare(std::array::from_fn(|i| i + 1));
        compare([17; 10]);
        for index in 0..10 {
            let mut fields = [0; 10];
            fields[index] = usize::MAX;
            compare(fields);
        }
    }
    let mut runner = TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    });
    runner.run(&any::<[usize; 10]>(), |fields| { compare(fields); Ok(()) }).unwrap();
}
