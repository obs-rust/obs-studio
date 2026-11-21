//! Tier 1 mirrors test_service_info_accessors.c (#3840) on the safe core,
//! without FFI or unsafe code: a table stand-in of pointer-sized words and
//! an `Option` plugin-data slot (`None` is NULL).
use obs_output::service_info_accessors::{info_table, plugin_data, set_info, set_plugin_data};

/// Stands in for `struct obs_service_info`: only words, no padding.
type Table = [usize; 26];

fn table(seed: usize) -> Table {
    core::array::from_fn(|i| seed.wrapping_add(i * 7))
}

/// A service stand-in: the two fields beside bytes that must not change.
#[derive(Clone, PartialEq, Debug)]
struct Service<D> {
    before: [u8; 16],
    info: Table,
    data: D,
    after: [u8; 16],
}

fn service<D>(data: D) -> Service<D> {
    Service {
        before: core::array::from_fn(|i| 0x11 + i as u8 * 7),
        info: table(0x11),
        data,
        after: core::array::from_fn(|i| 0x70 + i as u8 * 7),
    }
}

/// set_info copies the whole table by value into the service and touches
/// nothing else; info_table borrows the embedded table, not the source.
#[test]
fn info_table_copy_and_borrow() {
    let mut service = service(0_u8);
    let mut first = table(0x5a);
    let second = table(0xc3);
    let first_copy = first;

    let before = service.clone();
    set_info(&mut service.info, first);
    assert_eq!(service.info, first_copy);
    assert_eq!(first, first_copy);
    assert_eq!(
        Service {
            info: before.info,
            ..service.clone()
        },
        before
    );

    // The service keeps its own copy: a later change to the source does
    // not reach it.
    first = table(0x99);
    assert_ne!(first, first_copy);
    assert_eq!(service.info, first_copy);

    let before = service.clone();
    let borrowed: *const Table = info_table(&service.info);
    assert!(core::ptr::eq(borrowed, &service.info));
    assert!(!core::ptr::eq(borrowed, &first_copy));
    assert!(core::ptr::eq(info_table(&service.info), borrowed));
    assert_eq!(service, before);

    // A second set overwrites the whole table in place.
    set_info(&mut service.info, second);
    assert_eq!(service.info, second);
    assert_eq!(
        Service {
            info: before.info,
            ..service.clone()
        },
        before
    );
    assert!(core::ptr::eq(info_table(&service.info), borrowed));

    // An all-NULL table is copied the same way.
    set_info(&mut service.info, [0; 26]);
    assert_eq!(service.info, [0; 26]);
    assert_eq!(info_table(&service.info)[0], 0);
}

/// set_plugin_data stores the pointer as it is, NULL included;
/// plugin_data returns it without touching the service.
#[test]
fn plugin_data_round_trip() {
    let token_a = 0_u8;
    let token_b = 0_u8;
    let (a, b) = (&raw const token_a, &raw const token_b);
    let values = [a, core::ptr::null(), b, b];
    let mut service = service(b);
    for value in values {
        let before = service.clone();
        set_plugin_data(&mut service.data, value);
        assert_eq!(service.data, value);
        assert_eq!(
            Service {
                data: before.data,
                ..service.clone()
            },
            before
        );

        let before = service.clone();
        assert_eq!(plugin_data(&service.data), value);
        assert_eq!(plugin_data(&service.data), value);
        assert_eq!(service, before);
    }
    // The tokens are borrowed; neither accessor reads through them.
    assert_eq!((token_a, token_b), (0, 0));
}
