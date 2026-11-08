//! Mirrors retained_channel in the unchanged actual-library C characterization.
use obs_core::canvas_channel_getter::get_channel;
use std::cell::Cell;

#[test]
fn retained_channel() {
    for channel in 0..64 {
        let refs = Cell::new(0);
        let acquire = |view: usize, selected| {
            assert_eq!((view, selected), (17, channel));
            refs.set(refs.get() + 1);
            Some(23)
        };
        assert_eq!(get_channel(17, channel, acquire), Some(23));
        assert_eq!(refs.get(), 1);
        assert_eq!(get_channel(17, channel, acquire), Some(23));
        assert_eq!(refs.get(), 2);
        refs.set(refs.get() - 2);
        assert_eq!(refs.get(), 0);
        assert_eq!(get_channel(17, channel, |_, _| None::<usize>), None);
        // The view owns rejection of an expired source, with no retained reference.
        assert_eq!(get_channel(17, channel, |_, _| None::<usize>), None);
    }
    for channel in [64, u32::MAX] {
        assert_eq!(get_channel(17, channel, |view, selected| {
            assert_eq!((view, selected), (17, channel));
            None::<usize>
        }), None);
    }
    // NULL canvas is C UB, excluded; safe handles are supplied by the FFI boundary.
}
