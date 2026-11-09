//! Tier 1: mirrors test_indexbuffer_create.c without FFI or a GPU.
use obs_graphics::indexbuffer_create::{Backend, DUP_BUFFER, UNSIGNED_SHORT, create};

#[derive(Default)]
struct Device {
    allocations: Vec<Vec<u8>>,
    copies: Vec<usize>,
    calls: Vec<(i32, Option<usize>, usize, u32)>,
    reject: bool,
}

impl Backend for Device {
    type Input = usize;
    type Buffer = Option<usize>;

    fn duplicate(&mut self, input: usize, bytes: usize) -> usize {
        self.copies.push(bytes);
        self.allocations
            .push(self.allocations[input][..bytes].to_vec());
        self.allocations.len() - 1
    }

    fn create(
        &mut self,
        kind: i32,
        input: Option<usize>,
        count: usize,
        flags: u32,
    ) -> Self::Buffer {
        self.calls.push((kind, input, count, flags));
        (!self.reject).then_some(self.calls.len())
    }
}

#[test]
fn absent_context_neither_dispatches_nor_takes_ownership() {
    assert_eq!(create::<Device>(None, 1, Some(0), 1, DUP_BUFFER), None);
    assert_eq!(create::<Device>(None, UNSIGNED_SHORT, None, 0, 0), None);
}

#[test]
fn input_count_flags_and_widths() {
    for kind in [UNSIGNED_SHORT, 1] {
        for flags in [0, 2, DUP_BUFFER, DUP_BUFFER | 2] {
            for present in [false, true] {
                for count in [0, 3] {
                    let size = count * if kind == UNSIGNED_SHORT { 2 } else { 4 };
                    let bytes: Vec<_> = (0..size.max(1)).map(|i| (i * 19) as u8).collect();
                    let mut device = Device {
                        allocations: vec![bytes.clone()],
                        ..Device::default()
                    };
                    let input = present.then_some(0);
                    assert_eq!(
                        create(Some(&mut device), kind, input, count, flags),
                        Some(Some(1))
                    );
                    let duplicated = present && count != 0 && flags & DUP_BUFFER != 0;
                    assert_eq!(device.copies, if duplicated { vec![size] } else { vec![] });
                    let transferred = if duplicated { Some(1) } else { input };
                    assert_eq!(device.calls, vec![(kind, transferred, count, flags)]);
                    if duplicated {
                        assert_eq!(device.allocations[1], bytes);
                        device.allocations[0].fill(0);
                        assert_eq!(device.allocations[1], bytes);
                    }
                    if let Some(id) = transferred {
                        device.allocations[id].clear();
                    }
                    if duplicated {
                        assert!(device.allocations[0].iter().all(|b| *b == 0));
                    }
                }
            }
        }
    }
}

#[test]
fn backend_failure_is_propagated() {
    for flags in [0, DUP_BUFFER] {
        let mut device = Device {
            allocations: vec![b"abcdefgh".to_vec()],
            reject: true,
            ..Device::default()
        };
        assert_eq!(create(Some(&mut device), 1, Some(0), 2, flags), Some(None));
        assert_eq!(device.calls.len(), 1);
        let input = device.calls[0].1.unwrap();
        device.allocations[input].clear();
        if flags == DUP_BUFFER {
            assert_eq!(device.allocations[0], b"abcdefgh");
        }
    }
}

#[test]
fn size_t_product_wraps_and_non_short_width_is_four() {
    struct Sizes(Vec<usize>);
    impl Backend for Sizes {
        type Input = ();
        type Buffer = ();
        fn duplicate(&mut self, (): (), bytes: usize) {
            self.0.push(bytes);
        }
        fn create(&mut self, _: i32, _: Option<()>, _: usize, _: u32) {}
    }
    for kind in [UNSIGNED_SHORT, 1, -1, 2] {
        for count in [1, usize::MAX / 2, usize::MAX / 2 + 1, usize::MAX] {
            let mut backend = Sizes(vec![]);
            assert_eq!(
                create(Some(&mut backend), kind, Some(()), count, DUP_BUFFER),
                Some(())
            );
            assert_eq!(
                backend.0,
                vec![count.wrapping_mul(if kind == UNSIGNED_SHORT { 2 } else { 4 })]
            );
        }
    }
}
