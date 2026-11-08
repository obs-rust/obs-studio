//! Tier 1 mirrors the public C command-line snapshot characterization.
use obs_core::core_cmdline_getter::snapshot;

#[test]
fn test_core_cmdline_getter_initial_snapshot() {
    assert_eq!(snapshot(&0, &None::<&[&[u8]]>), (0, None));
    assert_eq!(snapshot(&0, &None::<&[&[u8]]>), (0, None));
}

#[test]
fn test_core_cmdline_getter_stored_snapshot() {
    let input: &[&[u8]] = &[b"obs", b"", b"--profile=\xc3\xa9", b"\xff\x80"];
    let stored = input
        .iter()
        .map(|arg| {
            let mut copied = arg.to_vec();
            copied.push(0);
            copied
        })
        .collect::<Vec<_>>();
    let table = stored
        .iter()
        .map(|arg| &arg[..arg.len() - 1])
        .collect::<Vec<_>>();
    let argc = 4;
    let (mut count, args) = snapshot(&argc, &table.as_slice());
    assert_eq!(count, 4);
    assert_eq!(args, input);
    assert!(std::ptr::eq(args.as_ptr(), table.as_ptr()));
    for (actual, original) in args.iter().zip(input) {
        assert_eq!(actual, original);
        assert_ne!(actual.as_ptr(), original.as_ptr());
    }
    count = 99;
    assert_eq!(count, 99);
    let again = snapshot(&argc, &table.as_slice());
    assert_eq!(again.0, 4);
    assert!(std::ptr::eq(again.1.as_ptr(), args.as_ptr()));
    assert_eq!(again.1, input);
}

#[test]
fn count_is_copied_without_normalizing() {
    for argc in [i32::MIN, -1, 0, 1, i32::MAX] {
        assert_eq!(snapshot(&argc, &17_u8), (argc, 17));
    }
}
