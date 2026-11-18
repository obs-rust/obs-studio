//! Tier 1 mirrors test_source_default_flags.c using borrowed safe state.
use obs_source::default_flags::set_default_flags;

#[test]
fn test_source_default_flags_null_source() {
    let mut diagnostics = Vec::new();
    set_default_flags(None, u32::MAX, || {
        diagnostics.push((400, "obs_source_set_default_flags: Null 'source' parameter"));
    });
    assert_eq!(
        diagnostics,
        [(400, "obs_source_set_default_flags: Null 'source' parameter")]
    );
}

#[derive(Debug, PartialEq)]
struct Source {
    flags: u32,
    default_flags: u32,
    last_obs_ver: u32,
}

#[test]
fn test_source_default_flags_exact_storage_and_replacement() {
    let mut source = Source {
        flags: 0xa55a_a55a,
        default_flags: u32::MAX,
        last_obs_ver: 0x1234_5678,
    };
    for flags in [0, u32::MAX, 0xaaaa_aaaa, 0x5555_5555, 0]
        .into_iter()
        .chain((0..32).map(|bit| 1 << bit))
    {
        set_default_flags(Some(&mut source.default_flags), flags, || {
            panic!("valid source")
        });
        assert_eq!(source.default_flags, flags);
        assert_eq!(
            source,
            Source {
                flags: 0xa55a_a55a,
                default_flags: flags,
                last_obs_ver: 0x1234_5678
            }
        );
    }
}

#[test]
fn test_source_default_flags_independent_sources_and_current_flags() {
    let mut first = Source {
        flags: 2,
        default_flags: 0,
        last_obs_ver: 0,
    };
    let mut second = Source {
        flags: 0x4000_0000,
        default_flags: 0x2468_ace0,
        last_obs_ver: 0,
    };
    set_default_flags(Some(&mut first.default_flags), 0x8000_0001, || {
        panic!("valid source")
    });
    assert_eq!((first.default_flags, first.flags), (0x8000_0001, 2));
    assert_eq!(
        (second.default_flags, second.flags),
        (0x2468_ace0, 0x4000_0000)
    );
    first.flags = u32::MAX;
    set_default_flags(Some(&mut second.default_flags), 0, || {
        panic!("valid source")
    });
    assert_eq!((first.default_flags, first.flags), (0x8000_0001, u32::MAX));
    assert_eq!((second.default_flags, second.flags), (0, 0x4000_0000));
}
