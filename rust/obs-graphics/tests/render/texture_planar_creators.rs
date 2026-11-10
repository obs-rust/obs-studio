//! Tier 1: `obs_graphics::texture_planar_creators` over a model host.
//!
//! `rejected_inputs` and `native_and_fallback` port the assertions of
//! `test/cmocka/test_texture_planar_creators.c` 1:1: the model's device
//! writes tokens 2 and 3 to the outputs and returns its answer, as the C
//! test's `native_create` does, and its plane creator returns token 0 for
//! luma and 1 for chroma unless that plane's failure bit is set. The other
//! tests pin the order of every call, read and write, and two outputs that
//! are the same slot.

use obs_graphics::texture_planar_creators::{
    GS_R8, GS_R8G8, GS_R16, GS_RG16, Host, Line, Planar, Plane, create,
};
use obs_graphics::texture_render::ColorFormat;

const GS_DYNAMIC: u32 = 1 << 1;
const GS_RENDER_TARGET: u32 = 1 << 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Context,
    Log(Line),
    Device {
        planar: Planar,
        width: u32,
        height: u32,
        flags: u32,
    },
    Create {
        width: u32,
        height: u32,
        format: ColorFormat,
        flags: u32,
    },
    Destroy(u32),
    Store(Plane, Option<u32>),
    Load(Plane),
}

struct Model {
    context: bool,
    /// [`None`]: the device has no planar creator; else its answer.
    device: Option<bool>,
    /// Bit 0 fails the luma plane, bit 1 the chroma plane.
    failures: u32,
    creates: u32,
    /// The outputs. With `alias`, both are slot 0.
    slots: [Option<u32>; 2],
    alias: bool,
    calls: Vec<Call>,
}

impl Model {
    fn new(context: bool, device: Option<bool>, failures: u32) -> Self {
        Self {
            context,
            device,
            failures,
            creates: 0,
            slots: [Some(2), Some(3)],
            alias: false,
            calls: Vec::new(),
        }
    }

    fn slot(&self, plane: Plane) -> usize {
        match plane {
            Plane::Uv if !self.alias => 1,
            _ => 0,
        }
    }

    fn count(&self, pick: impl Fn(&Call) -> bool) -> usize {
        self.calls.iter().filter(|call| pick(call)).count()
    }

    fn logs(&self) -> Vec<Line> {
        self.calls
            .iter()
            .filter_map(|call| match call {
                Call::Log(line) => Some(*line),
                _ => None,
            })
            .collect()
    }

    fn destroyed(&self) -> Vec<u32> {
        self.calls
            .iter()
            .filter_map(|call| match call {
                Call::Destroy(token) => Some(*token),
                _ => None,
            })
            .collect()
    }
}

impl Host for Model {
    type Texture = u32;

    fn has_context(&mut self) -> bool {
        self.calls.push(Call::Context);
        self.context
    }

    fn log(&mut self, line: Line) {
        self.calls.push(Call::Log(line));
    }

    fn device_create(
        &mut self,
        planar: Planar,
        width: u32,
        height: u32,
        flags: u32,
    ) -> Option<bool> {
        let answer = self.device?;
        self.calls.push(Call::Device {
            planar,
            width,
            height,
            flags,
        });
        // Even on decline the fallback must replace backend-written outputs.
        let (y, uv) = (self.slot(Plane::Y), self.slot(Plane::Uv));
        self.slots[y] = Some(2);
        self.slots[uv] = Some(3);
        Some(answer)
    }

    fn texture_create(
        &mut self,
        width: u32,
        height: u32,
        format: ColorFormat,
        flags: u32,
    ) -> Option<u32> {
        self.calls.push(Call::Create {
            width,
            height,
            format,
            flags,
        });
        let plane = self.creates;
        self.creates += 1;
        assert!(plane < 2, "a third plane");
        (self.failures & (1 << plane) == 0).then_some(plane)
    }

    fn texture_destroy(&mut self, texture: u32) {
        self.calls.push(Call::Destroy(texture));
    }

    fn store(&mut self, plane: Plane, texture: Option<u32>) {
        self.calls.push(Call::Store(plane, texture));
        let slot = self.slot(plane);
        self.slots[slot] = texture;
    }

    fn load(&mut self, plane: Plane) -> Option<u32> {
        self.calls.push(Call::Load(plane));
        self.slots[self.slot(plane)]
    }
}

const BOTH: [Planar; 2] = [Planar::Nv12, Planar::P010];

#[test]
fn names_lines_and_formats() {
    assert_eq!(Planar::Nv12.function(), "gs_texture_create_nv12");
    assert_eq!(Planar::P010.function(), "gs_texture_create_p010");
    assert_eq!(
        Planar::Nv12.odd_size_error(),
        "NV12 textures must have dimensions divisible by 2."
    );
    assert_eq!(
        Planar::P010.odd_size_error(),
        "P010 textures must have dimensions divisible by 2."
    );
    assert_eq!(
        (Planar::Nv12.luma_format(), Planar::Nv12.chroma_format()),
        (GS_R8, GS_R8G8)
    );
    assert_eq!(
        (Planar::P010.luma_format(), Planar::P010.chroma_format()),
        (GS_R16, GS_RG16)
    );
    assert_eq!(
        [GS_R8.0, GS_R16.0, GS_R8G8.0, GS_RG16.0],
        [2, 8, 18, 22],
        "enum gs_color_format"
    );
}

/// `test_texture_planar_creators.c` `rejected_inputs`.
#[test]
fn rejected_inputs() {
    for planar in BOTH {
        let mut host = Model::new(false, Some(true), 0);
        assert!(!create(&mut host, planar, 3, 5, GS_DYNAMIC));
        assert_eq!(host.slots, [Some(2), Some(3)]);
        assert_eq!(host.logs(), [Line::NoContext(planar)]);
        assert_eq!(
            host.calls,
            [Call::Context, Call::Log(Line::NoContext(planar))]
        );

        let dimensions = [(3, 4), (4, 3), (3, 3), (u32::MAX, 2), (2, u32::MAX)];
        for (width, height) in dimensions {
            let mut host = Model::new(true, Some(true), 0);
            assert!(!create(&mut host, planar, width, height, GS_DYNAMIC));
            assert_eq!(host.slots, [Some(2), Some(3)]);
            assert_eq!(host.logs(), [Line::OddSize(planar)]);
            assert_eq!(host.count(|c| matches!(c, Call::Device { .. })), 0);
            assert_eq!(host.count(|c| matches!(c, Call::Create { .. })), 0);
            assert_eq!(host.count(|c| matches!(c, Call::Destroy(_))), 0);
            assert_eq!(
                host.calls,
                [Call::Context, Call::Log(Line::OddSize(planar))]
            );
        }
    }
}

/// `test_texture_planar_creators.c` `native_and_fallback`.
#[test]
fn native_and_fallback() {
    let dimensions = [(1920, 1080), (2, 2), (0, 0), (0, 2), (2, 0)];
    let flag_sets = [0, GS_DYNAMIC, GS_RENDER_TARGET];
    for planar in BOTH {
        for native in 0..3 {
            let device = match native {
                0 => None,
                n => Some(n == 2),
            };
            let native_success = native == 2;
            for (width, height) in dimensions {
                for flags in flag_sets {
                    for failures in 0..4 {
                        let mut host = Model::new(true, device, failures);
                        let success = create(&mut host, planar, width, height, flags);
                        assert_eq!(success, native_success || failures == 0);
                        let devices: Vec<Call> = host
                            .calls
                            .iter()
                            .copied()
                            .filter(|c| matches!(c, Call::Device { .. }))
                            .collect();
                        if native == 0 {
                            assert!(devices.is_empty());
                        } else {
                            assert_eq!(
                                devices,
                                [Call::Device {
                                    planar,
                                    width,
                                    height,
                                    flags
                                }]
                            );
                        }
                        let creates: Vec<Call> = host
                            .calls
                            .iter()
                            .copied()
                            .filter(|c| matches!(c, Call::Create { .. }))
                            .collect();
                        if native_success {
                            assert!(creates.is_empty());
                        } else {
                            assert_eq!(
                                creates,
                                [
                                    Call::Create {
                                        width,
                                        height,
                                        format: planar.luma_format(),
                                        flags
                                    },
                                    Call::Create {
                                        width: width / 2,
                                        height: height / 2,
                                        format: planar.chroma_format(),
                                        flags
                                    },
                                ]
                            );
                        }
                        assert!(host.logs().is_empty());
                        if success {
                            let expected = if native_success {
                                [Some(2), Some(3)]
                            } else {
                                [Some(0), Some(1)]
                            };
                            assert_eq!(host.slots, expected);
                            assert!(host.destroyed().is_empty());
                        } else {
                            assert_eq!(host.slots, [None, None]);
                            let expected: &[u32] = match failures {
                                1 => &[1],
                                2 => &[0],
                                _ => &[],
                            };
                            assert_eq!(host.destroyed(), expected);
                        }
                    }
                }
            }
        }
    }
    // Zero dimensions reaching the backend is characterized, not endorsed.
}

/// The device's success returns before any plane is made or any output
/// is touched by the fallback.
#[test]
fn device_success_is_final() {
    for planar in BOTH {
        let mut host = Model::new(true, Some(true), 3);
        assert!(create(&mut host, planar, 4, 6, GS_DYNAMIC));
        assert_eq!(
            host.calls,
            [
                Call::Context,
                Call::Device {
                    planar,
                    width: 4,
                    height: 6,
                    flags: GS_DYNAMIC
                },
            ]
        );
    }
}

/// The full fallback, in the C order: both creates and stores, then the
/// reads of the check.
#[test]
fn fallback_order() {
    for planar in BOTH {
        for device in [None, Some(false)] {
            let mut host = Model::new(true, device, 0);
            assert!(create(&mut host, planar, 6, 4, 0));
            let mut expected = vec![Call::Context];
            if device.is_some() {
                expected.push(Call::Device {
                    planar,
                    width: 6,
                    height: 4,
                    flags: 0,
                });
            }
            expected.extend([
                Call::Create {
                    width: 6,
                    height: 4,
                    format: planar.luma_format(),
                    flags: 0,
                },
                Call::Store(Plane::Y, Some(0)),
                Call::Create {
                    width: 3,
                    height: 2,
                    format: planar.chroma_format(),
                    flags: 0,
                },
                Call::Store(Plane::Uv, Some(1)),
                Call::Load(Plane::Y),
                Call::Load(Plane::Uv),
            ]);
            assert_eq!(host.calls, expected);
        }
    }
}

/// A failure reads each output once to check it (the chroma read only
/// when luma is there), then each once more for its `if`, and a set one
/// again as the destroy's argument, as the C writes `if (*tex_y)
/// gs_texture_destroy(*tex_y);`, then clears both.
#[test]
fn failure_order() {
    let tail = |failures: u32| -> Vec<Call> {
        let mut calls = vec![Call::Load(Plane::Y)];
        if failures & 1 == 0 {
            calls.push(Call::Load(Plane::Uv));
        }
        calls.push(Call::Load(Plane::Y));
        if failures & 1 == 0 {
            calls.extend([Call::Load(Plane::Y), Call::Destroy(0)]);
        }
        calls.push(Call::Load(Plane::Uv));
        if failures & 2 == 0 {
            calls.extend([Call::Load(Plane::Uv), Call::Destroy(1)]);
        }
        calls.extend([Call::Store(Plane::Y, None), Call::Store(Plane::Uv, None)]);
        calls
    };
    for planar in BOTH {
        for failures in 1..4 {
            let mut host = Model::new(true, None, failures);
            assert!(!create(&mut host, planar, 2, 2, GS_DYNAMIC));
            let start = host
                .calls
                .iter()
                .position(|c| *c == Call::Store(Plane::Uv, (failures & 2 == 0).then_some(1)))
                .expect("chroma store")
                + 1;
            assert_eq!(
                host.calls[start..],
                tail(failures)[..],
                "failures {failures}"
            );
            assert_eq!(host.slots, [None, None]);
        }
    }
}

/// Two outputs that are the same slot: the reads see the chroma plane, so
/// a success leaves it there, and a failed luma plane goes unseen.
#[test]
fn aliased_outputs() {
    for planar in BOTH {
        let mut host = Model::new(true, None, 0);
        host.alias = true;
        assert!(create(&mut host, planar, 2, 2, 0));
        assert_eq!(host.slots[0], Some(1));
        assert!(host.destroyed().is_empty());

        // Luma failed, chroma made: the check sees chroma twice and passes.
        let mut host = Model::new(true, None, 1);
        host.alias = true;
        assert!(create(&mut host, planar, 2, 2, 0));
        assert_eq!(host.slots[0], Some(1));

        // Chroma failed: the luma plane is overwritten by null, not
        // destroyed, and both outputs end null.
        let mut host = Model::new(true, None, 2);
        host.alias = true;
        assert!(!create(&mut host, planar, 2, 2, 0));
        assert_eq!(host.slots[0], None);
        assert!(host.destroyed().is_empty());
    }
}

/// Every even size passes the check, the largest included, and chroma gets
/// half of each.
#[test]
fn even_sizes_reach_the_planes() {
    for planar in BOTH {
        for (width, height) in [(u32::MAX - 1, 2), (2, u32::MAX - 1), (0x8000_0000, 4)] {
            let mut host = Model::new(true, None, 0);
            assert!(create(&mut host, planar, width, height, 0));
            assert!(host.calls.contains(&Call::Create {
                width: width / 2,
                height: height / 2,
                format: planar.chroma_format(),
                flags: 0,
            }));
        }
    }
}
