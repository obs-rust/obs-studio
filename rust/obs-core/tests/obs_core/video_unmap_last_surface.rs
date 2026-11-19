//! Tier 1: every assertion of test_video_unmap_last_surface.c, without FFI.
use obs_core::video_unmap_last_surface::{MappedSurfaces, unmap_last_surface};

/// `NUM_CHANNELS` in `obs-internal.h`.
pub const CHANNELS: usize = 3;

/// One call the cleanup makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Mapped(usize),
    /// An unmap: the surface, and every slot as the callback sees it.
    Unmap(usize, [Option<usize>; CHANNELS]),
    Clear(usize),
}

/// A mix whose mapped slots hold surface tokens.
#[derive(Default)]
pub struct Model {
    pub slots: [Option<usize>; CHANNELS],
    pub calls: Vec<Call>,
}

impl MappedSurfaces for Model {
    type Surface = usize;
    fn channels(&self) -> usize {
        CHANNELS
    }
    fn mapped(&mut self, channel: usize) -> Option<usize> {
        self.calls.push(Call::Mapped(channel));
        self.slots[channel]
    }
    fn unmap(&mut self, surface: usize) {
        self.calls.push(Call::Unmap(surface, self.slots));
    }
    fn clear(&mut self, channel: usize) {
        self.calls.push(Call::Clear(channel));
        self.slots[channel] = None;
    }
}

/// The token test_video_unmap_last_surface.c maps into slot `channel`
/// (`&unmap_tokens[channel]`), as a distinct value per channel.
pub fn token(channel: usize) -> usize {
    0x1000 + 0x10 * channel
}

/// The slots of presence mask `mask`.
pub fn slots(mask: u32) -> [Option<usize>; CHANNELS] {
    core::array::from_fn(|c| (mask & (1 << c) != 0).then(|| token(c)))
}

/// The unmap calls, in order, with the slots each callback saw.
pub fn unmaps(calls: &[Call]) -> Vec<(usize, [Option<usize>; CHANNELS])> {
    calls
        .iter()
        .filter_map(|call| match call {
            Call::Unmap(surface, seen) => Some((*surface, *seen)),
            _ => None,
        })
        .collect()
}

#[test]
fn unmap_all_channel_masks() {
    for mask in 0..(1u32 << CHANNELS) {
        let mut model = Model {
            slots: slots(mask),
            ..Model::default()
        };
        let expected_calls = mask.count_ones() as usize;
        unmap_last_surface(&mut model);
        let calls = unmaps(&model.calls);
        assert_eq!(calls.len(), expected_calls, "mask {mask:#b}");
        // Ascending channel order, each call naming its own slot's surface.
        let mapped: Vec<usize> = (0..CHANNELS).filter(|c| mask & (1 << c) != 0).collect();
        for (&(surface, seen), &next) in calls.iter().zip(&mapped) {
            assert_eq!(surface, token(next));
            // Earlier slots have reset; the current and later slots retain
            // their original identity until their own unmap has returned.
            for (c, slot) in seen.iter().enumerate() {
                let expected = (c >= next && mask & (1 << c) != 0).then(|| token(c));
                assert_eq!(*slot, expected, "mask {mask:#b}, unmap of {next}, slot {c}");
            }
        }
        // Each slot resets right after its own unmap returns.
        for &c in &mapped {
            let at = model
                .calls
                .iter()
                .position(|call| matches!(call, Call::Unmap(s, _) if *s == token(c)))
                .expect("every mapped slot is unmapped");
            assert_eq!(model.calls.get(at + 1), Some(&Call::Clear(c)));
        }
        assert_eq!(model.slots, [None; CHANNELS]);
        // Empty and repeated cleanup make no graphics calls.
        model.calls.clear();
        unmap_last_surface(&mut model);
        assert!(unmaps(&model.calls).is_empty(), "mask {mask:#b}");
        assert_eq!(model.slots, [None; CHANNELS]);
    }
}

#[test]
fn unmapped_slots_are_never_cleared() {
    // The C writes NULL only into a slot it unmapped.
    for mask in 0..(1u32 << CHANNELS) {
        let mut model = Model {
            slots: slots(mask),
            ..Model::default()
        };
        unmap_last_surface(&mut model);
        let mut expected = Vec::new();
        let mut seen = slots(mask);
        for c in 0..CHANNELS {
            expected.push(Call::Mapped(c));
            if mask & (1 << c) != 0 {
                expected.extend([Call::Unmap(token(c), seen), Call::Clear(c)]);
                seen[c] = None;
            }
        }
        assert_eq!(model.calls, expected, "mask {mask:#b}");
    }
}
