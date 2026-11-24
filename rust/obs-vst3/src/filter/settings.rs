#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location<'a> {
    pub path: &'a str,
    pub name: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SelectionChange<'a> {
    Clear,
    Retain,
    Install {
        location: Location<'a>,
        initial: bool,
    },
}

pub fn change<'a>(
    current: &str,
    has_processor: bool,
    requested: &str,
    cached: Location<'a>,
    scan: impl FnOnce() -> Option<Location<'a>>,
) -> SelectionChange<'a> {
    if requested.is_empty() {
        return SelectionChange::Clear;
    }
    if current == requested {
        return SelectionChange::Retain;
    }
    SelectionChange::Install {
        location: scan()
            .filter(|location| !location.path.is_empty())
            .unwrap_or(cached),
        initial: current.is_empty() && !has_processor,
    }
}

pub fn encode_state(state: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(state.len() * 2);
    for &byte in state {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0xf) as usize]));
    }
    result
}

pub fn decode_state(state: &str) -> Result<Vec<u8>, InvalidHex> {
    if state.is_empty() || !state.len().is_multiple_of(2) {
        return Err(InvalidHex);
    }
    state
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16).ok_or(InvalidHex)?;
            let low = char::from(pair[1]).to_digit(16).ok_or(InvalidHex)?;
            Ok((high * 16 + low) as u8)
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub struct InvalidHex;

#[cfg(test)]
mod tests {
    use super::*;

    const SAVED: Location<'_> = Location {
        path: "/old/Gain.vst3",
        name: "Old name",
    };
    const SCANNED: Location<'_> = Location {
        path: "/new/Gain.vst3",
        name: "Gain",
    };

    #[test]
    fn initial_scene_load_prefers_scanner_and_falls_back_to_saved_metadata() {
        assert_eq!(
            change("", false, "gain", SAVED, || Some(SCANNED)),
            SelectionChange::Install {
                location: SCANNED,
                initial: true
            }
        );
        assert_eq!(
            change("", false, "gain", SAVED, || None),
            SelectionChange::Install {
                location: SAVED,
                initial: true
            }
        );
        assert_eq!(
            change("", false, "gain", SAVED, || Some(Location {
                path: "",
                name: "Invalid"
            })),
            SelectionChange::Install {
                location: SAVED,
                initial: true
            }
        );
    }

    #[test]
    fn clear_and_same_class_avoid_scanner_and_swap_does_not_restore_old_state() {
        assert_eq!(
            change("gain", true, "", SAVED, || panic!("unexpected scan")),
            SelectionChange::Clear
        );
        assert_eq!(
            change("gain", false, "gain", SAVED, || panic!("unexpected scan")),
            SelectionChange::Retain
        );
        assert_eq!(
            change("gain", false, "other", SAVED, || Some(SCANNED)),
            SelectionChange::Install {
                location: SCANNED,
                initial: false
            }
        );
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
