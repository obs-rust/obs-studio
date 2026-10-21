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
    // The codes the Wayland key state (obs-nix-hotkey-state.c) and the portal
    // shortcut ids (obs-nix-portal-shortcut-ids.c) and the binding codec
    // (obs-hotkey-bindings.c) need by name.
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
            if matches!(
                name,
                "OBS_KEY_SHIFT"
                    | "OBS_KEY_CONTROL"
                    | "OBS_KEY_ALT"
                    | "OBS_KEY_META"
                    | "OBS_KEY_MOUSE1"
                    | "OBS_KEY_MOUSE2"
                    | "OBS_KEY_MOUSE29"
            ) {
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
