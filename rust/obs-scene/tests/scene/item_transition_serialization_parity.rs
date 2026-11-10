//! Differential serialization orchestration against the unmodified C unit.
//! Source collaborators count references; real libobs behavior is Tier 2.
use super::item_transition_serialization::Model;
use obs_scene::item_transition_serialization::{self as engine, Host};
use proptest::prelude::*;
use std::sync::Mutex;
static ORACLE: Mutex<()> = Mutex::new(());
unsafe extern "C" {
    fn oracle_sits_run(
        duration: i64,
        show: bool,
        has_id: bool,
        initial_show: bool,
        initial_hide: bool,
        null_item: bool,
        null_data: bool,
        out: *mut u64,
    );
}
fn check(
    duration: i64,
    show: bool,
    has_id: bool,
    initial_show: bool,
    initial_hide: bool,
    null_item: bool,
    null_data: bool,
) {
    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    let mut expected = [0; 11];
    // SAFETY: the oracle writes eleven words; the mutex protects its source fixture.
    unsafe {
        oracle_sits_run(
            duration,
            show,
            has_id,
            initial_show,
            initial_hide,
            null_item,
            null_data,
            expected.as_mut_ptr(),
        )
    };
    let mut m = Model::new();
    m.durations = [22, 11];
    let settings = m.create_data();
    m.data[settings].text.insert("marker", "kept".into());
    for (i, initial) in [initial_hide, initial_show].into_iter().enumerate() {
        if initial {
            let source = m.source("old", "old", "Previous", settings);
            m.slots[i] = Some(source);
        }
    }
    let d = m.create_data();
    m.data[d]
        .text
        .insert("id", if has_id { "new" } else { "" }.into());
    m.data[d].text.insert("versioned_id", "ignored".into());
    m.data[d].text.insert("name", "Loaded".into());
    m.data[d].object = settings;
    m.data[d].duration = duration;
    engine::load(
        &mut m,
        (!null_item).then_some(()),
        (!null_data).then_some(d),
        show,
    );
    let mut actual = [0; 11];
    actual[0] = u64::from(m.durations[0]);
    actual[1] = u64::from(m.durations[1]);
    actual[2] = u64::from(m.slots[0].is_some());
    actual[3] = u64::from(m.slots[1].is_some());
    actual[4] = m.created as u64;
    actual[5] = m.destroyed as u64;
    actual[6] = m.sources.iter().map(|s| s.refs as u64).sum();
    for i in 0..2 {
        let saved = engine::save(&mut m, (), i != 0);
        let d = &m.data[saved];
        actual[7 + i] = if let Some(source) = m.slots[i] {
            let s = &m.sources[source];
            u64::from(d.text.get("id") == Some(&s.id))
                | u64::from(d.text.get("versioned_id") == Some(&s.versioned)) << 1
                | u64::from(d.text.get("name") == Some(&s.name)) << 2
                | u64::from(d.object == settings) << 3
                | u64::from(m.data[d.object].text.get("marker").map(String::as_str) == Some("kept"))
                    << 4
        } else {
            u64::from(!d.text.contains_key("id"))
                | u64::from(!d.text.contains_key("versioned_id")) << 1
                | u64::from(!d.text.contains_key("name")) << 2
                | u64::from(d.object == 0) << 3
        };
        actual[9 + i] = d.duration as u64;
    }
    assert_eq!(actual, expected);
}
#[test]
fn characterized_edges() {
    for duration in [
        0,
        1,
        -1,
        i64::from(u32::MAX),
        i64::from(u32::MAX) + 2,
        i64::MIN,
        i64::MAX,
    ] {
        for bits in 0u8..64 {
            check(
                duration,
                bits & 1 != 0,
                bits & 2 != 0,
                bits & 4 != 0,
                bits & 8 != 0,
                bits & 16 != 0,
                bits & 32 != 0,
            );
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn serialization_matches_c(duration in any::<i64>(), flags in any::<u8>()) {
        check(duration,flags&1!=0,flags&2!=0,flags&4!=0,flags&8!=0,flags&16!=0,flags&32!=0);
    }
}
