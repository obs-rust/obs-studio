//! Generates the key table from `libobs/obs-hotkeys.h`, the list the C
//! expands with `OBS_HOTKEY(x)` (and `OBS_MOUSE_BUTTON(x)`), so the Rust names cannot drift from it.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let header = manifest.join("../../libobs/obs-hotkeys.h");
    println!("cargo:rerun-if-changed={}", header.display());
    let text = std::fs::read_to_string(&header).expect("read obs-hotkeys.h");
    let mut out = String::from(
        "/// `obs-hotkeys.h` in enum order.\npub(crate) static KEY_NAMES: &[&std::ffi::CStr] = &[\n",
    );
    // The codes the Wayland key state (obs-nix-hotkey-state.c), the portal
    // shortcut ids (obs-nix-portal-shortcut-ids.c), the binding codec
    // (obs-hotkey-bindings.c) and the key-name translations
    // (obs-hotkey-translations.c) need by name.
    let mut consts = String::new();
    let mut count = 0usize;
    for line in text.lines() {
        let line = line.trim();
        // OBS_MOUSE_BUTTON(x) expands to OBS_HOTKEY(x) unless the includer
        // defines it, and obs-hotkey-name-map.c does not.
        let rest = line
            .strip_prefix("OBS_HOTKEY(")
            .or_else(|| line.strip_prefix("OBS_MOUSE_BUTTON("));
        if let Some(rest) = rest {
            let name = rest.strip_suffix(')').expect("OBS_HOTKEY(x) on one line");
            writeln!(out, "    c\"{name}\",").unwrap();
            if named_by_code(name) {
                writeln!(consts, "/// `{name}`.\npub const {name}: i32 = {count};").unwrap();
            }
            count += 1;
        }
    }
    out.push_str("];\n");
    writeln!(
        out,
        "/// `OBS_KEY_LAST_VALUE`: the number of keys.\npub const OBS_KEY_LAST_VALUE: usize = {count};"
    )
    .unwrap();
    out.push_str(&consts);
    let dest = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("keys.rs");
    std::fs::write(dest, out).expect("write keys.rs");
}

/// Whether the crate needs `name`'s code as a `pub const`: the modifiers,
/// the keys the translations name, NUM0-NUM9 and every mouse button.
fn named_by_code(name: &str) -> bool {
    let digits = |rest: Option<&str>| {
        rest.is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
    };
    matches!(
        name,
        "OBS_KEY_SHIFT"
            | "OBS_KEY_CONTROL"
            | "OBS_KEY_ALT"
            | "OBS_KEY_META"
            | "OBS_KEY_INSERT"
            | "OBS_KEY_DELETE"
            | "OBS_KEY_HOME"
            | "OBS_KEY_END"
            | "OBS_KEY_PAGEUP"
            | "OBS_KEY_PAGEDOWN"
            | "OBS_KEY_NUMLOCK"
            | "OBS_KEY_SCROLLLOCK"
            | "OBS_KEY_CAPSLOCK"
            | "OBS_KEY_BACKSPACE"
            | "OBS_KEY_TAB"
            | "OBS_KEY_PRINT"
            | "OBS_KEY_PAUSE"
            | "OBS_KEY_LEFT"
            | "OBS_KEY_RIGHT"
            | "OBS_KEY_UP"
            | "OBS_KEY_DOWN"
            | "OBS_KEY_MENU"
            | "OBS_KEY_SPACE"
            | "OBS_KEY_ESCAPE"
            | "OBS_KEY_NUMSLASH"
            | "OBS_KEY_NUMASTERISK"
            | "OBS_KEY_NUMMINUS"
            | "OBS_KEY_NUMPLUS"
            | "OBS_KEY_NUMPERIOD"
            | "OBS_KEY_NUMEQUAL"
    ) || (digits(name.strip_prefix("OBS_KEY_NUM")) && name.len() == "OBS_KEY_NUM0".len())
        || digits(name.strip_prefix("OBS_KEY_MOUSE"))
}
