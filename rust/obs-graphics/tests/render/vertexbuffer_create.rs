//! Tier 1 mirrors test_vertexbuffer_create.c's ownership characterization.
use obs_graphics::vertexbuffer_create::{Backend, DUP_BUFFER, Texture, VertexData, create};

#[derive(Default)]
struct Device {
    arrays: Vec<Vec<u8>>,
    data: Vec<VertexData<usize>>,
    seen: Vec<(Option<usize>, u32)>,
    reject: bool,
}

impl Backend for Device {
    type Data = usize;
    type Array = usize;
    type Buffer = Option<u32>;
    fn count(&self, data: usize) -> usize { self.data[data].num }
    fn read(&self, data: usize) -> VertexData<usize> { self.data[data].clone() }
    fn duplicate(&mut self, array: usize, bytes: usize) -> usize {
        let copy = self.arrays[array][..bytes].to_vec();
        self.arrays.push(copy);
        self.arrays.len() - 1
    }
    fn build(&mut self, data: VertexData<usize>) -> usize {
        self.data.push(data);
        self.data.len() - 1
    }
    fn dispatch(&mut self, data: Option<usize>, flags: u32) -> Option<u32> {
        self.seen.push((data, flags));
        (!self.reject).then_some(42)
    }
}

fn fixture(mask: usize, textures: bool) -> Device {
    let widths = [16, 16, 16, 4, 4, 8, 16];
    Device {
        arrays: widths.iter().enumerate().map(|(i, width)| vec![i as u8 + 1; width * 3]).collect(),
        data: vec![VertexData {
            num: 3,
            arrays: std::array::from_fn(|i| (mask & (1 << i) != 0).then_some(i)),
            textures: if textures {
                [1, 2, 4].into_iter().enumerate().map(|(i, width)| Texture { width, array: i + 4 }).collect()
            } else { vec![] },
        }],
        ..Device::default()
    }
}

#[test]
fn ownership_no_context_and_passthrough() {
    assert_eq!(create::<Device>(None, None, DUP_BUFFER), None);
    assert_eq!(create::<Device>(None, Some(0), DUP_BUFFER), None);
    let mut device = fixture(15, true);
    assert_eq!(create(Some(&mut device), None, DUP_BUFFER | 2), Some(Some(42)));
    assert_eq!(device.seen.last(), Some(&(None, DUP_BUFFER | 2)));
    device.data[0].num = 0;
    assert_eq!(create(Some(&mut device), Some(0), DUP_BUFFER), Some(Some(42)));
    assert_eq!(device.seen.last(), Some(&(Some(0), DUP_BUFFER)));
    device.data[0].num = 3;
    assert_eq!(create(Some(&mut device), Some(0), 2), Some(Some(42)));
    assert_eq!(device.seen.last(), Some(&(Some(0), 2)));
    assert_eq!((device.data.len(), device.arrays.len()), (1, 7));
}

#[test]
fn ownership_all_optional_arrays_and_texture_widths() {
    for mask in 0..16 {
        let mut device = fixture(mask, true);
        let original = device.data[0].clone();
        assert_eq!(create(Some(&mut device), Some(0), DUP_BUFFER | 2), Some(Some(42)));
        assert_eq!(device.seen, [(Some(1), DUP_BUFFER | 2)]);
        let copy = device.data[1].clone();
        assert_eq!(copy.num, 3);
        for (src, dst) in original.arrays.into_iter().zip(copy.arrays) {
            assert_eq!(src.is_some(), dst.is_some());
            if let (Some(src), Some(dst)) = (src, dst) {
                assert_ne!(src, dst);
                assert_eq!(device.arrays[src], device.arrays[dst]);
            }
        }
        assert_eq!(copy.textures.len(), 3);
        for (src, dst) in original.textures.iter().zip(&copy.textures) {
            assert_eq!(src.width, dst.width);
            assert_ne!(src.array, dst.array);
            assert_eq!(device.arrays[src.array], device.arrays[dst.array]);
            device.arrays[dst.array][0] = 99;
            assert_ne!(device.arrays[src.array][0], 99);
            device.arrays[src.array][1] = 100;
            assert_ne!(device.arrays[dst.array][1], 100);
        }
        if let Some(colors) = copy.arrays[3] {
            device.arrays[colors][0] = 99;
            assert_eq!(device.arrays[3][0], 4);
        }
        assert_eq!(device.data[0], original);
    }
}

#[test]
fn ownership_missing_textures_and_backend_failure() {
    for reject in [false, true] {
        let mut device = fixture(0, false);
        device.reject = reject;
        assert_eq!(create(Some(&mut device), Some(0), DUP_BUFFER), Some((!reject).then_some(42)));
        assert_eq!(device.seen, [(Some(1), DUP_BUFFER)]);
        assert_eq!(device.data[1].num, 3);
        assert!(device.data[1].textures.is_empty());
    }
}
