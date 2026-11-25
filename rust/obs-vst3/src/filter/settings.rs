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
