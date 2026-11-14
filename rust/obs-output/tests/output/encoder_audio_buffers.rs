use obs_output::encoder_audio_buffers::{self as core, Host, PLANES};

#[derive(Default)]
struct Encoder {
    inputs: [Vec<u8>; PLANES],
    outputs: [Option<Vec<u8>>; PLANES],
    rate: u32,
    planes: usize,
    blocksize: usize,
    frames: usize,
    bytes: usize,
    callback_frames: usize,
    calls: usize,
    /// Runs after each allocation, as a stand-in for state that changes
    /// between the reset loop's reads.
    on_allocate: Option<fn(&mut Encoder)>,
}
impl Encoder {
    fn populated() -> Self {
        Self {
            inputs: std::array::from_fn(|_| vec![1, 2, 3, 4, 5]),
            outputs: std::array::from_fn(|i| Some(vec![0; 19 + i])),
            ..Self::default()
        }
    }
    fn assert_empty_inputs(&self) {
        assert!(self.inputs.iter().all(Vec::is_empty));
    }
    fn allocations(&self) -> usize {
        self.inputs.iter().filter(|v| !v.is_empty()).count() + self.outputs.iter().flatten().count()
    }
    fn assert_outputs(&mut self) {
        for (i, output) in self.outputs.iter_mut().enumerate() {
            if i < self.planes {
                let bytes = output.as_mut().unwrap();
                assert_eq!(bytes.len(), self.bytes);
                bytes.fill(0x5a);
            } else {
                assert!(output.is_none());
            }
        }
    }
}
impl Host for Encoder {
    fn free_plane(&mut self, i: usize) {
        self.inputs[i].clear();
        self.outputs[i] = None;
    }
    fn allocate_plane(&mut self, i: usize, bytes: usize) {
        self.outputs[i] = Some(vec![0; bytes]);
        if let Some(f) = self.on_allocate {
            f(self);
        }
    }
    fn planes(&self) -> usize {
        self.planes
    }
    fn bytes(&self) -> usize {
        self.bytes
    }
    fn set_format(&mut self, rate: u32, planes: usize, blocksize: usize) {
        self.rate = rate;
        self.planes = planes;
        self.blocksize = blocksize;
    }
    fn frame_size(&mut self) -> usize {
        assert_ne!(self.rate, 0);
        assert_ne!(self.blocksize, 0);
        // Negotiated fields are visible, but old buffers are still alive.
        assert_eq!(self.allocations(), 16);
        self.calls += 1;
        self.callback_frames
    }
    fn blocksize(&self) -> usize {
        self.blocksize
    }
    fn set_frame_size(&mut self, frames: usize, bytes: usize) {
        self.frames = frames;
        self.bytes = bytes;
    }
}
#[test]
fn mirrors_c_cleanup_all_planes() {
    let mut e = Encoder {
        planes: 1,
        bytes: 37,
        ..Encoder::populated()
    };
    core::free(&mut e);
    e.assert_empty_inputs();
    assert!(e.outputs.iter().all(Option::is_none));
    assert_eq!((e.planes, e.bytes, e.allocations()), (1, 37, 0));
    core::free(&mut e);
    assert_eq!(e.allocations(), 0);
}
#[test]
fn mirrors_c_reset_active_planes() {
    for planes in [0, 1, 2, PLANES] {
        let mut e = Encoder {
            planes,
            bytes: 37,
            ..Encoder::populated()
        };
        for _ in 0..2 {
            core::reset(&mut e);
            e.assert_empty_inputs();
            e.assert_outputs();
            assert_eq!(e.allocations(), planes);
        }
        core::free(&mut e);
        assert_eq!(e.allocations(), 0);
    }
}
#[test]
fn mirrors_c_initialize_geometry() {
    for (format, speakers, rate, frames, planes, blocksize) in [
        (8, 2, 48000, 1024, 2, 4),
        (2, 2, 44100, 960, 1, 4),
        (6, 6, 32000, 17, 6, 2),
        (1, 1, 22050, 1, 1, 1),
    ] {
        let mut e = Encoder {
            callback_frames: frames,
            ..Encoder::populated()
        };
        core::initialize(&mut e, rate, format, speakers);
        assert_eq!(e.calls, 1);
        assert_eq!(
            (e.rate, e.planes, e.blocksize, e.frames, e.bytes),
            (rate, planes, blocksize, frames, blocksize * frames)
        );
        e.assert_empty_inputs();
        e.assert_outputs();
        assert_eq!(e.allocations(), planes);
        core::free(&mut e);
        assert_eq!(e.allocations(), 0);
    }
}
#[test]
fn reset_rereads_geometry_each_plane() {
    // C reads `planes` at every loop check and `framesize_bytes` in each
    // iteration, so a change between them is seen at the next read.
    let mut e = Encoder {
        planes: 4,
        bytes: 8,
        on_allocate: Some(|e| {
            e.planes = 2;
            e.bytes = 3;
        }),
        ..Encoder::populated()
    };
    core::reset(&mut e);
    let sizes: Vec<_> = e.outputs.iter().map(|o| o.as_ref().map(Vec::len)).collect();
    assert_eq!(sizes[..3], [Some(8), Some(3), None]);
}
