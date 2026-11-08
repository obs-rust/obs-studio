//! Tier 1 mirrors test_core_texture_activity.c, plus each absent-owner case.
use obs_core::core_texture_activity::activity;

#[test]
fn test_core_texture_activity_lifecycle_and_stored_flags() {
    assert_eq!(activity(None), (false, false));
    assert_eq!(activity(None), (false, false));
    for nv12 in [false, true] {
        for p010 in [false, true] {
            let before = (nv12, p010);
            for _ in 0..2 {
                assert_eq!(activity(Some((&nv12, &p010))), before);
                assert_eq!((nv12, p010), before);
            }
        }
    }
    assert_eq!(activity(None), (false, false));
}

#[test]
fn every_absent_owner_yields_inactive() {
    for _ in 0..3 {
        assert_eq!(activity(None), (false, false));
    }
}
