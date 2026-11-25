//! Mirrors test_sceneitem_matrix_getters.c, including captured-parent order.
use obs_scene::item_matrix_getters::{FieldAccess, copy_field};

struct Access<const N: usize> {
    stored: [u32; N],
    output: [u32; N],
    parent: Option<u32>,
    captured: Option<u32>,
    phase: u32,
    caller_holds: u32,
}

impl<const N: usize> FieldAccess<N> for Access<N> {
    type Parent = Option<u32>;
    fn lock(&mut self) -> Self::Parent {
        assert_eq!(self.phase, 0);
        assert_ne!(self.output, self.stored);
        self.phase = 1;
        self.captured = self.parent;
        self.parent = Some(99);
        self.captured
    }
    fn read(&self) -> [u32; N] {
        assert_eq!(self.phase, 1);
        self.stored
    }
    fn write(&mut self, bits: [u32; N]) {
        assert_eq!(self.phase, 1);
        self.output = bits;
    }
    fn unlock(&mut self, parent: Self::Parent) {
        assert_eq!(parent, self.captured);
        assert_eq!(self.output, self.stored);
        assert_eq!(self.phase, 1);
        self.phase = 2;
    }
}

#[test]
fn test_sceneitem_matrix_getters_null_item_is_noop() {
    copy_field::<16>(None::<&mut Access<16>>);
    copy_field::<2>(None::<&mut Access<2>>);
}

const EDGES: [u32; 8] = [
    0x80000000, 0, 0x7fc00001, 0xffc12345, 0x7f800000, 0xff800000, 1, 0x3f800000,
];

fn check<const N: usize>(seed: usize, parent: Option<u32>, caller_holds: u32) {
    let stored = core::array::from_fn(|i| {
        if i % 2 == 0 { EDGES[(i / 2 + seed) % 8] }
        else { 0x40000000 + seed as u32 * 97 + i as u32 }
    });
    let mut access = Access {
        stored, output: [0xa5a5a5a5; N], parent, captured: None,
        phase: 0, caller_holds,
    };
    copy_field(Some(&mut access));
    assert_eq!(access.output, stored);
    assert_eq!(access.stored, stored);
    assert_eq!(access.parent, Some(99));
    assert_eq!(access.phase, 2);
    assert_eq!(access.caller_holds, caller_holds);
}

#[test]
fn test_sceneitem_matrix_getters_captured_parent_and_exact_fields() {
    for seed in 0..8 {
        for parent in [None, Some(1)] {
            // Draw and box are independent stored matrices, with different seeds.
            check::<16>(seed, parent, 0);
            check::<16>((seed + 3) % 8, parent, 0);
            check::<2>(seed, parent, 0);
        }
    }
}

#[test]
fn test_sceneitem_matrix_getters_live_reads_under_parent_lock() {
    check::<16>(1, Some(1), 1);
    check::<2>(1, Some(1), 1);
}
