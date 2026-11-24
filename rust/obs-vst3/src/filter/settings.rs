#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    pub path: &'a str,
    pub name: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SelectionChange<'a> {
    Clear,
    Retain,
    Install { location: Location<'a>, initial: bool },
}

pub fn change<'a>(
    _current: &str,
    _has_processor: bool,
    _requested: &str,
    _cached: Location<'a>,
    _scan: impl FnOnce() -> Option<Location<'a>>,
) -> SelectionChange<'a> {
    todo!()
}

pub fn encode_state(_state: &[u8]) -> String {
    todo!()
}

pub fn decode_state(_state: &str) -> Result<Vec<u8>, InvalidHex> {
    todo!()
}

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidHex;

#[cfg(test)]
mod tests {
    use super::*;

    const SAVED: Location<'_> = Location { path: "/old/Gain.vst3", name: "Old name" };
    const SCANNED: Location<'_> = Location { path: "/new/Gain.vst3", name: "Gain" };

    #[test]
    fn initial_scene_load_prefers_scanner_and_falls_back_to_saved_metadata() {
        assert_eq!(change("", false, "gain", SAVED, || Some(SCANNED)),
            SelectionChange::Install { location: SCANNED, initial: true });
        assert_eq!(change("", false, "gain", SAVED, || None),
            SelectionChange::Install { location: SAVED, initial: true });
        assert_eq!(change("", false, "gain", SAVED, || Some(Location { path: "", name: "Invalid" })),
            SelectionChange::Install { location: SAVED, initial: true });
    }

    #[test]
    fn clear_and_same_class_avoid_scanner_and_swap_does_not_restore_old_state() {
        assert_eq!(change("gain", true, "", SAVED, || panic!("unexpected scan")), SelectionChange::Clear);
        assert_eq!(change("gain", false, "gain", SAVED, || panic!("unexpected scan")), SelectionChange::Retain);
        assert_eq!(change("gain", false, "other", SAVED, || Some(SCANNED)),
            SelectionChange::Install { location: SCANNED, initial: false });
    }

    #[test]
    fn scene_state_hex_is_lowercase_on_save_and_strict_on_load() {
        assert_eq!(encode_state(&[0, 15, 16, 255]), "000f10ff");
        assert_eq!(decode_state("000F10ff"), Ok(vec![0, 15, 16, 255]));
        for invalid in ["", "0", "x1", "Å0", "12 3"] {
            assert_eq!(decode_state(invalid), Err(InvalidHex));
        }
    }
}
