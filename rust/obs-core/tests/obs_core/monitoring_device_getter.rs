//! Tier 1: monitoring getter assertions from test_core_setup_rules_steps.c.
use obs_core::monitoring_device_getter::{Field, get};

#[test]
fn core_setup_monitoring_device_values() {
    for (name, id) in [
        ("Default", "default"),
        ("Default", "default"), // refused and same-ID updates preserve the fields
        ("Core setup rules", "core_setup_rules_dev"),
        ("Default", "default"),
    ] {
        let mut got_name = "sentinel";
        let mut got_id = "sentinel";
        get(
            Some(&mut got_name),
            Some(&mut got_id),
            |field| match field {
                Field::Name => name,
                Field::Id => id,
            },
        );
        assert_eq!(got_name, name);
        assert_eq!(got_id, id);
        assert!(std::ptr::eq(got_name.as_ptr(), name.as_ptr()));
        assert!(std::ptr::eq(got_id.as_ptr(), id.as_ptr()));
    }
}

#[test]
fn outputs_are_independent_and_lazy() {
    for mask in 0..4 {
        let mut name = Some(11);
        let mut id = Some(22);
        let mut reads = Vec::new();
        get(
            (mask & 1 != 0).then_some(&mut name),
            (mask & 2 != 0).then_some(&mut id),
            |field| {
                let index = match field {
                    Field::Name => 0,
                    Field::Id => 1,
                };
                reads.push(index);
                None
            },
        );
        assert_eq!(name, if mask & 1 != 0 { None } else { Some(11) });
        assert_eq!(id, if mask & 2 != 0 { None } else { Some(22) });
        assert_eq!(
            reads,
            (0..2).filter(|i| mask & (1 << i) != 0).collect::<Vec<_>>()
        );
    }
}
