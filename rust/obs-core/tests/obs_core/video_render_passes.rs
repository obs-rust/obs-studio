//! Tier 1 mirrors test_video_render_passes.c, including all adapter arguments.
use obs_core::video_render_passes::{self as render, Operation};

struct Recording {
    trace: String,
    index: usize,
    passes: usize,
    width: u32,
    height: u32,
    name: usize,
    loop_index: usize,
    pass_index: usize,
}

impl Recording {
    fn call(&mut self, op: Operation, a: usize, b: usize) -> usize {
        let texture = 10 + self.index;
        let (code, result) = match op {
            Operation::MixTexture => {
                assert_eq!((a, b), (self.index, 0));
                ('M', texture)
            }
            Operation::DefaultEffect => {
                assert_eq!((a, b), (0, 0));
                ('E', 20)
            }
            Operation::ImageParameter => {
                assert_eq!((a, b), (20, 0));
                ('P', 30)
            }
            Operation::BindTexture => {
                assert_eq!((a, b), (30, texture));
                ('I', 0)
            }
            Operation::PushBlend => {
                assert_eq!((a, b), (0, 0));
                ('[', 0)
            }
            Operation::DisableBlend => {
                assert_eq!((a, b), (0, 0));
                ('B', 0)
            }
            Operation::EnableSrgb => {
                assert_eq!((a, b), (0, 0));
                ('+', 0)
            }
            Operation::EffectLoop => {
                assert_eq!((a, b), (20, 0));
                let more = self.loop_index < self.passes;
                self.loop_index += 1;
                ('L', usize::from(more))
            }
            Operation::Sprite => {
                assert_eq!((a, b), (texture, 0));
                ('S', 0)
            }
            Operation::DisableSrgb => {
                assert_eq!((a, b), (0, 0));
                ('-', 0)
            }
            Operation::PopBlend => {
                assert_eq!((a, b), (0, 0));
                (']', 0)
            }
            Operation::Technique => {
                assert_eq!((a, b), (20, self.name));
                ('T', 40)
            }
            Operation::Width => {
                assert_eq!((a, b), (texture, 0));
                ('W', self.width as usize)
            }
            Operation::Height => {
                assert_eq!((a, b), (texture, 0));
                ('H', self.height as usize)
            }
            Operation::Target => {
                assert_eq!((a, b), (texture, 0));
                ('R', 0)
            }
            Operation::Size => {
                assert_eq!((a, b), (self.width as usize, self.height as usize));
                ('Z', 0)
            }
            Operation::Begin => {
                assert_eq!((a, b), (40, 0));
                ('A', self.passes)
            }
            Operation::BeginPass => {
                assert_eq!((a, b), (40, self.pass_index));
                self.pass_index += 1;
                ('b', 0)
            }
            Operation::Triangle => {
                assert_eq!((a, b), (0, 0));
                ('D', 0)
            }
            Operation::EndPass => {
                assert_eq!((a, b), (40, 0));
                ('e', 0)
            }
            Operation::End => {
                assert_eq!((a, b), (40, 0));
                ('F', 0)
            }
        };
        self.trace.push(code);
        result
    }
}

pub(super) fn run(
    convert: bool,
    index: usize,
    passes: usize,
    width: u32,
    height: u32,
    name: usize,
) -> String {
    let mut recording = Recording {
        trace: String::new(),
        index,
        passes,
        width,
        height,
        name,
        loop_index: 0,
        pass_index: 0,
    };
    let mut call = |op, a, b| recording.call(op, a, b);
    if convert {
        render::render_convert_plane(20, 10 + index, name, &mut call);
        assert_eq!(recording.pass_index, passes);
    } else {
        render::draw_mix_texture(index, &mut call);
        assert_eq!(recording.loop_index, passes + 1);
    }
    recording.trace
}

#[test]
fn test_video_render_passes_c_mix_passes() {
    for index in 0..2 {
        for count in 0..3 {
            let expected = ["MEPI[B+L-]", "MEPI[B+LSL-]", "MEPI[B+LSLSL-]"];
            assert_eq!(run(false, index, count, 0, 0, 0), expected[count]);
        }
    }
}

#[test]
fn test_video_render_passes_c_conversion_passes() {
    for (plane, (width, height)) in [(0, 0), (1920, 1080), (u32::MAX, 1)]
        .into_iter()
        .enumerate()
    {
        for count in 0..3 {
            let expected = ["TWHRZAF", "TWHRZAbDeF", "TWHRZAbDebDeF"];
            assert_eq!(
                run(true, plane % 2, count, width, height, 100 + plane),
                expected[count]
            );
        }
    }
}
