//! The `BasicWindow/DockSizes` value (decision D6 on #5230): the intended
//! sizes and the hash of the `DockState` blob they belong to.
//!
//! Format: `v1;<hash>` and then one `;`-separated entry per size:
//! `a<area>=<px>` for the depth of a dock area (`L`, `R`, `T`, `B`) and
//! `d<name>=<px>` for the length of a dock in its row. In a name, `%`, `;`
//! and `=` are written as `%25`, `%3B` and `%3D`.

use std::collections::BTreeMap;

use crate::{Area, LIMIT};

/// The intended sizes of one layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sizes {
    /// Depth of each dock area: the width of the left and right areas, the
    /// height of the top and bottom ones.
    pub depth: BTreeMap<Area, i32>,
    /// Length of each dock along its row, by object name.
    pub length: BTreeMap<String, i32>,
}

const VERSION: &str = "v1";

fn area_code(area: Area) -> char {
    match area {
        Area::Left => 'L',
        Area::Right => 'R',
        Area::Top => 'T',
        Area::Bottom => 'B',
    }
}

fn area_from(code: &str) -> Option<Area> {
    match code {
        "L" => Some(Area::Left),
        "R" => Some(Area::Right),
        "T" => Some(Area::Top),
        "B" => Some(Area::Bottom),
        _ => None,
    }
}

fn escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '%' => out.push_str("%25"),
            ';' => out.push_str("%3B"),
            '=' => out.push_str("%3D"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let code = rest.get(at + 1..at + 3)?;
        out.push(match code {
            "25" => '%',
            "3B" => ';',
            "3D" => '=',
            _ => return None,
        });
        rest = &rest[at + 3..];
    }
    out.push_str(rest);
    Some(out)
}

/// The record of `sizes` for the blob with hash `hash`.
pub fn encode(hash: &str, sizes: &Sizes) -> String {
    let mut out = format!("{VERSION};{}", escape(hash));
    for (area, px) in &sizes.depth {
        out.push_str(&format!(";a{}={px}", area_code(*area)));
    }
    for (name, px) in &sizes.length {
        out.push_str(&format!(";d{}={px}", escape(name)));
    }
    out
}

/// The hash and the sizes of a record. `None` for anything this version
/// does not write: another version, a malformed entry, a size below 1 px
/// or above [`LIMIT`].
/// A record that cannot be read is no record: the start is as upstream's.
pub fn decode(record: &str) -> Option<(String, Sizes)> {
    let mut parts = record.split(';');
    if parts.next()? != VERSION {
        return None;
    }
    let hash = unescape(parts.next()?)?;
    let mut sizes = Sizes::default();
    for entry in parts {
        let (key, value) = entry.split_once('=')?;
        let px: i32 = value.parse().ok()?;
        if !(1..=LIMIT).contains(&px) {
            return None;
        }
        if let Some(code) = key.strip_prefix('a') {
            sizes.depth.insert(area_from(code)?, px);
        } else {
            let name = key.strip_prefix('d')?;
            sizes.length.insert(unescape(name)?, px);
        }
    }
    Some((hash, sizes))
}
