//! Tier 1 mirrors test_encoder_roi.c, including list state and callback effects.
use obs_output::encoder_roi::{OBS_ENCODER_CAP_ROI, Roi};
use obs_output::encoder_roi_list::{Entries, RoiList, add_roi, enum_roi};

pub struct Memory {
    pub caps: u32,
    pub scaled: (u32, u32),
    pub video: (u32, u32),
    pub rois: Vec<Roi>,
    pub increment: u32,
    pub locks: usize,
    pub locked: bool,
}
impl Default for Memory {
    fn default() -> Self {
        Self {
            caps: OBS_ENCODER_CAP_ROI,
            scaled: (0, 0),
            video: (0, 0),
            rois: vec![],
            increment: 0,
            locks: 0,
            locked: false,
        }
    }
}
impl Entries for Memory {
    fn append(&mut self, roi: &Roi) {
        assert!(self.locked);
        self.rois.push(*roi);
    }
    fn increment(&mut self) -> &mut u32 {
        assert!(self.locked);
        &mut self.increment
    }
    fn count(&self) -> usize {
        assert!(self.locked);
        self.rois.len()
    }
    fn visit(&mut self, index: usize, callback: &mut dyn FnMut(&mut Roi)) {
        assert!(self.locked);
        callback(&mut self.rois[index]);
    }
}
impl RoiList for Memory {
    fn caps(&self) -> u32 {
        self.caps
    }
    fn scaled_size(&self) -> (u32, u32) {
        self.scaled
    }
    fn video_size(&self) -> (u32, u32) {
        assert!(self.scaled.0 != 0 && self.scaled.1 != 0);
        self.video
    }
    fn with_locked(&mut self, action: impl FnOnce(&mut dyn Entries)) {
        assert!(!self.locked);
        self.locked = true;
        self.locks += 1;
        action(self);
        self.locked = false;
    }
}
impl obs_output::encoder_clear_roi::RoiState for Memory {
    fn count(&self) -> usize {
        self.rois.len()
    }
    fn with_locked_fields(&mut self, update: impl FnOnce(&mut usize, &mut u32)) {
        let mut count = self.rois.len();
        update(&mut count, &mut self.increment);
        self.rois.truncate(count);
    }
}
pub fn rect(top: u32, bottom: u32, left: u32, right: u32, priority: f32) -> Roi {
    Roi {
        top,
        bottom,
        left,
        right,
        priority,
    }
}
pub fn bits(r: &Roi) -> [u32; 5] {
    [r.top, r.bottom, r.left, r.right, r.priority.to_bits()]
}
fn expect(m: &mut Memory, r: Roi, accepted: bool) {
    let before = m.increment;
    let count = m.rois.len();
    let locks = m.locks;
    assert_eq!(
        add_roi(Some(m), Some(&r), || panic!("valid encoder")),
        accepted
    );
    assert_eq!(m.increment, before.wrapping_add(u32::from(accepted)));
    assert_eq!(m.rois.len(), count + usize::from(accepted));
    assert_eq!(m.locks, locks + usize::from(accepted));
}
fn seen(m: &mut Memory) -> Vec<[u32; 5]> {
    let mut seen = vec![];
    enum_roi(Some(m), |r| seen.push(bits(r)), || panic!("valid encoder"));
    seen
}
#[test]
fn test_null_roi_is_refused() {
    let mut m = Memory::default();
    assert!(!add_roi(Some(&mut m), None, || panic!()));
    assert_eq!(m.increment, 0);
    assert!(m.rois.is_empty());
    assert_eq!(m.locks, 0);
}
#[test]
fn test_encoder_without_cap_refuses() {
    let mut m = Memory {
        caps: 0,
        ..Memory::default()
    };
    expect(&mut m, rect(0, 64, 0, 64, 0.5), false);
    assert!(m.rois.is_empty());
}
#[test]
fn test_minimum_size() {
    let mut m = Memory::default();
    for (r, ok) in [
        (rect(100, 115, 200, 300, 0.5), false),
        (rect(100, 116, 200, 300, 0.5), true),
        (rect(100, 117, 200, 300, 0.5), true),
        (rect(100, 200, 200, 215, 0.5), false),
        (rect(100, 200, 200, 216, 0.5), true),
        (rect(100, 200, 200, 217, 0.5), true),
        (rect(100, 115, 200, 215, 0.5), false),
        (rect(0, 16, 0, 16, 0.5), true),
        (rect(100, 100, 200, 300, 0.5), false),
        (rect(100, 200, 200, 200, 0.5), false),
        (
            rect(u32::MAX - 16, u32::MAX, u32::MAX - 16, u32::MAX, 0.5),
            true,
        ),
        (rect(u32::MAX - 15, u32::MAX, 0, 16, 0.5), false),
    ] {
        expect(&mut m, r, ok);
    }
}
#[test]
fn test_priority_range() {
    let mut m = Memory::default();
    for (p, ok) in [
        (-1., true),
        (1., true),
        (0., true),
        (-0., true),
        (-f32::from_bits(0x3f800001), false),
        (f32::from_bits(0x3f800001), false),
        (-f32::INFINITY, false),
        (f32::INFINITY, false),
    ] {
        expect(&mut m, rect(0, 16, 0, 16, p), ok);
    }
}
#[test]
fn test_nan_priority() {
    let mut m = Memory::default();
    for p in [f32::NAN, -f32::NAN] {
        expect(&mut m, rect(0, 16, 0, 16, p), false);
    }
    assert!(m.rois.is_empty());
}
#[test]
fn test_upside_down_rect() {
    let mut m = Memory::default();
    for r in [
        rect(200, 100, 0, 64, 0.5),
        rect(0, 64, 200, 100, 0.5),
        rect(200, 100, 200, 100, 0.5),
        rect(101, 100, 0, 64, 0.5),
        rect(0, 64, 101, 100, 0.5),
        rect(u32::MAX, 0, 0, 64, 0.5),
    ] {
        expect(&mut m, r, false);
    }
    assert!(m.rois.is_empty());
}
#[test]
fn test_increment_after_add_and_clear() {
    let mut m = Memory::default();
    assert_eq!(m.increment, 0);
    assert!(m.rois.is_empty());
    obs_output::encoder_clear_roi::clear_roi(Some(&mut m), || panic!());
    assert_eq!(m.increment, 0);
    expect(&mut m, rect(0, 16, 0, 16, 0.5), true);
    expect(&mut m, rect(16, 32, 16, 32, -0.5), true);
    assert_eq!(m.increment, 2);
    assert!(!m.rois.is_empty());
    obs_output::encoder_clear_roi::clear_roi(Some(&mut m), || panic!());
    assert_eq!(m.increment, 3);
    assert!(m.rois.is_empty());
    obs_output::encoder_clear_roi::clear_roi(Some(&mut m), || panic!());
    assert_eq!(m.increment, 3);
    expect(&mut m, rect(0, 16, 0, 16, 0.5), true);
    assert_eq!(m.increment, 4);
}
#[test]
fn test_enum_order_unscaled() {
    let mut m = Memory::default();
    assert!(seen(&mut m).is_empty());
    let rs = [
        rect(0, 16, 0, 16, 1.),
        rect(33, 101, 7, 1001, -1.),
        rect(1000, 2000, 3000, 4000, 0.25),
    ];
    for r in rs {
        expect(&mut m, r, true);
    }
    assert_eq!(seen(&mut m), rs.iter().rev().map(bits).collect::<Vec<_>>());
}
#[test]
fn test_enum_order_scaled() {
    let mut m = Memory {
        scaled: (1280, 720),
        video: (1920, 1080),
        ..Memory::default()
    };
    for r in [
        rect(0, 1080, 0, 1920, 1.),
        rect(33, 101, 7, 1001, -1.),
        rect(100, 200, 300, 400, 0.25),
    ] {
        expect(&mut m, r, true);
    }
    assert_eq!(
        seen(&mut m),
        [
            bits(&rect(66, 133, 200, 266, 0.25)),
            bits(&rect(22, 67, 4, 667, -1.)),
            bits(&rect(0, 720, 0, 1280, 1.))
        ]
    );
    m.rois.clear();
    m.scaled = (3840, 540);
    expect(&mut m, rect(33, 101, 7, 1001, 0.5), true);
    assert_eq!(seen(&mut m), [bits(&rect(16, 50, 14, 2002, 0.5))]);
}
#[test]
fn test_enum_scaled_saturates() {
    let mut m = Memory {
        scaled: (4096, 4096),
        video: (16, 16),
        ..Memory::default()
    };
    expect(&mut m, rect(0, 0xffffff, 16, 0x1000000, 0.5), true);
    expect(
        &mut m,
        rect(0x1000000, u32::MAX, 0xffffff, u32::MAX, -0.5),
        true,
    );
    assert_eq!(
        seen(&mut m),
        [
            bits(&rect(u32::MAX, u32::MAX, 0xffffff00, u32::MAX, -0.5)),
            bits(&rect(0, 0xffffff00, 4096, u32::MAX, 0.5))
        ]
    );
}
#[test]
fn test_null_encoder() {
    let mut logs = 0;
    assert!(!add_roi(
        None::<&mut Memory>,
        Some(&rect(0, 16, 0, 16, 0.5)),
        || logs += 1
    ));
    let mut calls = 0;
    enum_roi(None::<&mut Memory>, |_| calls += 1, || logs += 1);
    assert_eq!(logs, 2);
    assert_eq!(calls, 0);
    obs_output::encoder_clear_roi::clear_roi(None::<&mut Memory>, || logs += 1);
    assert_eq!(logs, 3);
}
#[test]
fn test_enum_without_video_size() {
    let mut m = Memory {
        scaled: (1280, 720),
        ..Memory::default()
    };
    expect(&mut m, rect(0, 16, 0, 16, 0.5), true);
    let locks = m.locks;
    assert!(seen(&mut m).is_empty());
    assert_eq!(m.locks, locks);
}
#[test]
fn callback_mutation_and_partial_scaling_and_wrap() {
    for scaled in [(0, 0), (1280, 0), (0, 720), (1280, 720)] {
        let mut m = Memory {
            scaled,
            video: (1920, 1080),
            increment: u32::MAX,
            ..Memory::default()
        };
        expect(&mut m, rect(0, 16, 0, 16, 0.5), true);
        assert_eq!(m.increment, 0);
        enum_roi(Some(&mut m), |r| r.priority = -0.75, || panic!());
        assert_eq!(
            m.rois[0].priority,
            if scaled.0 != 0 && scaled.1 != 0 {
                0.5
            } else {
                -0.75
            }
        );
    }
}
