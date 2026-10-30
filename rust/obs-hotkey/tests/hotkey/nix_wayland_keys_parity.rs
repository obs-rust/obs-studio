//! Tier 3 parity: the shims against the original
//! `libobs/obs-nix-wayland-keys.c.h` (oracle/nix_wayland_keys.c).
//!
//! `test_nix_wayland_keys_steps` compiles the C into itself, so it never
//! sees the Rust swap (a variant linked against `libobs_rust-static` does
//! not link on its own, as for `test_portal_shortcut_sets`). This file is
//! what compares the two:
//! - the values the C uses, and every keysym the tables name, against
//!   xkbcommon's header (where the oracle has it: on a host without it the
//!   oracle is built with a stand-in written from the Rust list, and only
//!   the names are compared);
//! - both tables, exhaustive over every keysym in the Latin-1, keypad,
//!   function, vendor and XF86 ranges, one above and one below every keysym
//!   either table names, and every key from -100 to `OBS_KEY_LAST_VALUE`
//!   + 100 and the `int` edges; proptests over every `int`;
//! - keymap rows: whole generated keymaps (keycodes 0 to 300, counts -1 to
//!   3 per level, keysyms from the tables, 0, random ones and one repeated
//!   across keycodes), one row at a time on both sides with the same level
//!   callback: the levels each side asks for and the two arrays;
//! - focused keys: every scancode from 0 to 300 and near `u32::MAX`,
//!   pressed and released, and press and release sequences across layout
//!   changes, the shim against the C and the core.
//!
//! Intentional difference (#3069), excluded here only where it applies and
//! checked against a model: the shim records the key each press reported
//! and releases that key, where the C leaves the record alone and looks a
//! release up again on the current layout. So the C and the shim agree on
//! every press, and on every release whose keycode still has the key it
//! was pressed as; on any other release the shim reports the recorded key,
//! or nothing when none is recorded. Null arrays, which the C dereferences,
//! are not passed. Nothing else is excluded.
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use std::ffi::{c_int, c_void};

use obs_c_oracle::nix_wayland_keys as c;
use obs_hotkey::ffi::nix_wayland_keys as shim;
use obs_hotkey::name_map::KEY_NONE;
#[allow(clippy::wildcard_imports)] // every keysym the tables name
use obs_hotkey::nix_wayland_keys::keysyms::*;
use obs_hotkey::nix_wayland_keys::{
    self as core, MAX_KEYCODES, MAX_SHIFT_LEVELS, OBS_KEY_LAST_VALUE,
};
use proptest::prelude::*;

/// Every keysym of `keysyms.rs`, in its order (`KEYSYM_NAMES` of the
/// oracle).
const KEYSYMS: [(&str, u32); 334] = [
    ("XKB_KEY_space", XKB_KEY_space),
    ("XKB_KEY_exclam", XKB_KEY_exclam),
    ("XKB_KEY_quotedbl", XKB_KEY_quotedbl),
    ("XKB_KEY_numbersign", XKB_KEY_numbersign),
    ("XKB_KEY_dollar", XKB_KEY_dollar),
    ("XKB_KEY_percent", XKB_KEY_percent),
    ("XKB_KEY_ampersand", XKB_KEY_ampersand),
    ("XKB_KEY_apostrophe", XKB_KEY_apostrophe),
    ("XKB_KEY_parenleft", XKB_KEY_parenleft),
    ("XKB_KEY_parenright", XKB_KEY_parenright),
    ("XKB_KEY_asterisk", XKB_KEY_asterisk),
    ("XKB_KEY_plus", XKB_KEY_plus),
    ("XKB_KEY_comma", XKB_KEY_comma),
    ("XKB_KEY_minus", XKB_KEY_minus),
    ("XKB_KEY_period", XKB_KEY_period),
    ("XKB_KEY_slash", XKB_KEY_slash),
    ("XKB_KEY_0", XKB_KEY_0),
    ("XKB_KEY_1", XKB_KEY_1),
    ("XKB_KEY_2", XKB_KEY_2),
    ("XKB_KEY_3", XKB_KEY_3),
    ("XKB_KEY_4", XKB_KEY_4),
    ("XKB_KEY_5", XKB_KEY_5),
    ("XKB_KEY_6", XKB_KEY_6),
    ("XKB_KEY_7", XKB_KEY_7),
    ("XKB_KEY_8", XKB_KEY_8),
    ("XKB_KEY_9", XKB_KEY_9),
    ("XKB_KEY_colon", XKB_KEY_colon),
    ("XKB_KEY_semicolon", XKB_KEY_semicolon),
    ("XKB_KEY_less", XKB_KEY_less),
    ("XKB_KEY_equal", XKB_KEY_equal),
    ("XKB_KEY_greater", XKB_KEY_greater),
    ("XKB_KEY_question", XKB_KEY_question),
    ("XKB_KEY_at", XKB_KEY_at),
    ("XKB_KEY_A", XKB_KEY_A),
    ("XKB_KEY_B", XKB_KEY_B),
    ("XKB_KEY_C", XKB_KEY_C),
    ("XKB_KEY_D", XKB_KEY_D),
    ("XKB_KEY_E", XKB_KEY_E),
    ("XKB_KEY_F", XKB_KEY_F),
    ("XKB_KEY_G", XKB_KEY_G),
    ("XKB_KEY_H", XKB_KEY_H),
    ("XKB_KEY_I", XKB_KEY_I),
    ("XKB_KEY_J", XKB_KEY_J),
    ("XKB_KEY_K", XKB_KEY_K),
    ("XKB_KEY_L", XKB_KEY_L),
    ("XKB_KEY_M", XKB_KEY_M),
    ("XKB_KEY_N", XKB_KEY_N),
    ("XKB_KEY_O", XKB_KEY_O),
    ("XKB_KEY_P", XKB_KEY_P),
    ("XKB_KEY_Q", XKB_KEY_Q),
    ("XKB_KEY_R", XKB_KEY_R),
    ("XKB_KEY_S", XKB_KEY_S),
    ("XKB_KEY_T", XKB_KEY_T),
    ("XKB_KEY_U", XKB_KEY_U),
    ("XKB_KEY_V", XKB_KEY_V),
    ("XKB_KEY_W", XKB_KEY_W),
    ("XKB_KEY_X", XKB_KEY_X),
    ("XKB_KEY_Y", XKB_KEY_Y),
    ("XKB_KEY_Z", XKB_KEY_Z),
    ("XKB_KEY_bracketleft", XKB_KEY_bracketleft),
    ("XKB_KEY_backslash", XKB_KEY_backslash),
    ("XKB_KEY_bracketright", XKB_KEY_bracketright),
    ("XKB_KEY_asciicircum", XKB_KEY_asciicircum),
    ("XKB_KEY_underscore", XKB_KEY_underscore),
    ("XKB_KEY_quoteleft", XKB_KEY_quoteleft),
    ("XKB_KEY_a", XKB_KEY_a),
    ("XKB_KEY_b", XKB_KEY_b),
    ("XKB_KEY_c", XKB_KEY_c),
    ("XKB_KEY_d", XKB_KEY_d),
    ("XKB_KEY_e", XKB_KEY_e),
    ("XKB_KEY_f", XKB_KEY_f),
    ("XKB_KEY_g", XKB_KEY_g),
    ("XKB_KEY_h", XKB_KEY_h),
    ("XKB_KEY_i", XKB_KEY_i),
    ("XKB_KEY_j", XKB_KEY_j),
    ("XKB_KEY_k", XKB_KEY_k),
    ("XKB_KEY_l", XKB_KEY_l),
    ("XKB_KEY_m", XKB_KEY_m),
    ("XKB_KEY_n", XKB_KEY_n),
    ("XKB_KEY_o", XKB_KEY_o),
    ("XKB_KEY_p", XKB_KEY_p),
    ("XKB_KEY_q", XKB_KEY_q),
    ("XKB_KEY_r", XKB_KEY_r),
    ("XKB_KEY_s", XKB_KEY_s),
    ("XKB_KEY_t", XKB_KEY_t),
    ("XKB_KEY_u", XKB_KEY_u),
    ("XKB_KEY_v", XKB_KEY_v),
    ("XKB_KEY_w", XKB_KEY_w),
    ("XKB_KEY_x", XKB_KEY_x),
    ("XKB_KEY_y", XKB_KEY_y),
    ("XKB_KEY_z", XKB_KEY_z),
    ("XKB_KEY_braceleft", XKB_KEY_braceleft),
    ("XKB_KEY_bar", XKB_KEY_bar),
    ("XKB_KEY_braceright", XKB_KEY_braceright),
    ("XKB_KEY_asciitilde", XKB_KEY_asciitilde),
    ("XKB_KEY_nobreakspace", XKB_KEY_nobreakspace),
    ("XKB_KEY_exclamdown", XKB_KEY_exclamdown),
    ("XKB_KEY_cent", XKB_KEY_cent),
    ("XKB_KEY_sterling", XKB_KEY_sterling),
    ("XKB_KEY_currency", XKB_KEY_currency),
    ("XKB_KEY_yen", XKB_KEY_yen),
    ("XKB_KEY_brokenbar", XKB_KEY_brokenbar),
    ("XKB_KEY_section", XKB_KEY_section),
    ("XKB_KEY_diaeresis", XKB_KEY_diaeresis),
    ("XKB_KEY_copyright", XKB_KEY_copyright),
    ("XKB_KEY_ordfeminine", XKB_KEY_ordfeminine),
    ("XKB_KEY_guillemotleft", XKB_KEY_guillemotleft),
    ("XKB_KEY_notsign", XKB_KEY_notsign),
    ("XKB_KEY_hyphen", XKB_KEY_hyphen),
    ("XKB_KEY_registered", XKB_KEY_registered),
    ("XKB_KEY_macron", XKB_KEY_macron),
    ("XKB_KEY_degree", XKB_KEY_degree),
    ("XKB_KEY_plusminus", XKB_KEY_plusminus),
    ("XKB_KEY_twosuperior", XKB_KEY_twosuperior),
    ("XKB_KEY_threesuperior", XKB_KEY_threesuperior),
    ("XKB_KEY_acute", XKB_KEY_acute),
    ("XKB_KEY_mu", XKB_KEY_mu),
    ("XKB_KEY_paragraph", XKB_KEY_paragraph),
    ("XKB_KEY_periodcentered", XKB_KEY_periodcentered),
    ("XKB_KEY_cedilla", XKB_KEY_cedilla),
    ("XKB_KEY_onesuperior", XKB_KEY_onesuperior),
    ("XKB_KEY_masculine", XKB_KEY_masculine),
    ("XKB_KEY_guillemotright", XKB_KEY_guillemotright),
    ("XKB_KEY_onequarter", XKB_KEY_onequarter),
    ("XKB_KEY_onehalf", XKB_KEY_onehalf),
    ("XKB_KEY_threequarters", XKB_KEY_threequarters),
    ("XKB_KEY_questiondown", XKB_KEY_questiondown),
    ("XKB_KEY_Agrave", XKB_KEY_Agrave),
    ("XKB_KEY_Aacute", XKB_KEY_Aacute),
    ("XKB_KEY_Acircumflex", XKB_KEY_Acircumflex),
    ("XKB_KEY_Atilde", XKB_KEY_Atilde),
    ("XKB_KEY_Adiaeresis", XKB_KEY_Adiaeresis),
    ("XKB_KEY_Aring", XKB_KEY_Aring),
    ("XKB_KEY_AE", XKB_KEY_AE),
    ("XKB_KEY_Ccedilla", XKB_KEY_Ccedilla),
    ("XKB_KEY_Egrave", XKB_KEY_Egrave),
    ("XKB_KEY_Eacute", XKB_KEY_Eacute),
    ("XKB_KEY_Ecircumflex", XKB_KEY_Ecircumflex),
    ("XKB_KEY_Ediaeresis", XKB_KEY_Ediaeresis),
    ("XKB_KEY_Igrave", XKB_KEY_Igrave),
    ("XKB_KEY_Iacute", XKB_KEY_Iacute),
    ("XKB_KEY_Icircumflex", XKB_KEY_Icircumflex),
    ("XKB_KEY_Idiaeresis", XKB_KEY_Idiaeresis),
    ("XKB_KEY_Eth", XKB_KEY_Eth),
    ("XKB_KEY_ETH", XKB_KEY_ETH),
    ("XKB_KEY_Ntilde", XKB_KEY_Ntilde),
    ("XKB_KEY_Ograve", XKB_KEY_Ograve),
    ("XKB_KEY_Oacute", XKB_KEY_Oacute),
    ("XKB_KEY_Ocircumflex", XKB_KEY_Ocircumflex),
    ("XKB_KEY_Otilde", XKB_KEY_Otilde),
    ("XKB_KEY_Odiaeresis", XKB_KEY_Odiaeresis),
    ("XKB_KEY_multiply", XKB_KEY_multiply),
    ("XKB_KEY_Ooblique", XKB_KEY_Ooblique),
    ("XKB_KEY_Ugrave", XKB_KEY_Ugrave),
    ("XKB_KEY_Uacute", XKB_KEY_Uacute),
    ("XKB_KEY_Ucircumflex", XKB_KEY_Ucircumflex),
    ("XKB_KEY_Udiaeresis", XKB_KEY_Udiaeresis),
    ("XKB_KEY_Yacute", XKB_KEY_Yacute),
    ("XKB_KEY_Thorn", XKB_KEY_Thorn),
    ("XKB_KEY_THORN", XKB_KEY_THORN),
    ("XKB_KEY_ssharp", XKB_KEY_ssharp),
    ("XKB_KEY_agrave", XKB_KEY_agrave),
    ("XKB_KEY_aacute", XKB_KEY_aacute),
    ("XKB_KEY_acircumflex", XKB_KEY_acircumflex),
    ("XKB_KEY_atilde", XKB_KEY_atilde),
    ("XKB_KEY_adiaeresis", XKB_KEY_adiaeresis),
    ("XKB_KEY_aring", XKB_KEY_aring),
    ("XKB_KEY_ae", XKB_KEY_ae),
    ("XKB_KEY_ccedilla", XKB_KEY_ccedilla),
    ("XKB_KEY_egrave", XKB_KEY_egrave),
    ("XKB_KEY_eacute", XKB_KEY_eacute),
    ("XKB_KEY_ecircumflex", XKB_KEY_ecircumflex),
    ("XKB_KEY_ediaeresis", XKB_KEY_ediaeresis),
    ("XKB_KEY_igrave", XKB_KEY_igrave),
    ("XKB_KEY_iacute", XKB_KEY_iacute),
    ("XKB_KEY_icircumflex", XKB_KEY_icircumflex),
    ("XKB_KEY_idiaeresis", XKB_KEY_idiaeresis),
    ("XKB_KEY_eth", XKB_KEY_eth),
    ("XKB_KEY_ntilde", XKB_KEY_ntilde),
    ("XKB_KEY_ograve", XKB_KEY_ograve),
    ("XKB_KEY_oacute", XKB_KEY_oacute),
    ("XKB_KEY_ocircumflex", XKB_KEY_ocircumflex),
    ("XKB_KEY_otilde", XKB_KEY_otilde),
    ("XKB_KEY_odiaeresis", XKB_KEY_odiaeresis),
    ("XKB_KEY_division", XKB_KEY_division),
    ("XKB_KEY_ooblique", XKB_KEY_ooblique),
    ("XKB_KEY_ugrave", XKB_KEY_ugrave),
    ("XKB_KEY_uacute", XKB_KEY_uacute),
    ("XKB_KEY_ucircumflex", XKB_KEY_ucircumflex),
    ("XKB_KEY_udiaeresis", XKB_KEY_udiaeresis),
    ("XKB_KEY_yacute", XKB_KEY_yacute),
    ("XKB_KEY_thorn", XKB_KEY_thorn),
    ("XKB_KEY_ydiaeresis", XKB_KEY_ydiaeresis),
    ("XKB_KEY_Ydiaeresis", XKB_KEY_Ydiaeresis),
    ("XKB_KEY_dead_grave", XKB_KEY_dead_grave),
    ("XKB_KEY_dead_acute", XKB_KEY_dead_acute),
    ("XKB_KEY_dead_circumflex", XKB_KEY_dead_circumflex),
    ("XKB_KEY_dead_tilde", XKB_KEY_dead_tilde),
    ("XKB_KEY_dead_macron", XKB_KEY_dead_macron),
    ("XKB_KEY_dead_breve", XKB_KEY_dead_breve),
    ("XKB_KEY_dead_abovedot", XKB_KEY_dead_abovedot),
    ("XKB_KEY_dead_diaeresis", XKB_KEY_dead_diaeresis),
    ("XKB_KEY_dead_abovering", XKB_KEY_dead_abovering),
    ("XKB_KEY_dead_doubleacute", XKB_KEY_dead_doubleacute),
    ("XKB_KEY_dead_caron", XKB_KEY_dead_caron),
    ("XKB_KEY_dead_cedilla", XKB_KEY_dead_cedilla),
    ("XKB_KEY_dead_ogonek", XKB_KEY_dead_ogonek),
    ("XKB_KEY_dead_iota", XKB_KEY_dead_iota),
    ("XKB_KEY_dead_voiced_sound", XKB_KEY_dead_voiced_sound),
    (
        "XKB_KEY_dead_semivoiced_sound",
        XKB_KEY_dead_semivoiced_sound,
    ),
    ("XKB_KEY_dead_belowdot", XKB_KEY_dead_belowdot),
    ("XKB_KEY_dead_hook", XKB_KEY_dead_hook),
    ("XKB_KEY_dead_horn", XKB_KEY_dead_horn),
    ("XKB_KEY_BackSpace", XKB_KEY_BackSpace),
    ("XKB_KEY_Tab", XKB_KEY_Tab),
    ("XKB_KEY_Clear", XKB_KEY_Clear),
    ("XKB_KEY_Return", XKB_KEY_Return),
    ("XKB_KEY_Pause", XKB_KEY_Pause),
    ("XKB_KEY_Escape", XKB_KEY_Escape),
    ("XKB_KEY_Multi_key", XKB_KEY_Multi_key),
    ("XKB_KEY_Kanji", XKB_KEY_Kanji),
    ("XKB_KEY_Muhenkan", XKB_KEY_Muhenkan),
    ("XKB_KEY_Henkan", XKB_KEY_Henkan),
    ("XKB_KEY_Romaji", XKB_KEY_Romaji),
    ("XKB_KEY_Hiragana", XKB_KEY_Hiragana),
    ("XKB_KEY_Katakana", XKB_KEY_Katakana),
    ("XKB_KEY_Hiragana_Katakana", XKB_KEY_Hiragana_Katakana),
    ("XKB_KEY_Zenkaku", XKB_KEY_Zenkaku),
    ("XKB_KEY_Hankaku", XKB_KEY_Hankaku),
    ("XKB_KEY_Zenkaku_Hankaku", XKB_KEY_Zenkaku_Hankaku),
    ("XKB_KEY_Touroku", XKB_KEY_Touroku),
    ("XKB_KEY_Massyo", XKB_KEY_Massyo),
    ("XKB_KEY_Kana_Lock", XKB_KEY_Kana_Lock),
    ("XKB_KEY_Kana_Shift", XKB_KEY_Kana_Shift),
    ("XKB_KEY_Eisu_Shift", XKB_KEY_Eisu_Shift),
    ("XKB_KEY_Eisu_toggle", XKB_KEY_Eisu_toggle),
    ("XKB_KEY_Hangul", XKB_KEY_Hangul),
    ("XKB_KEY_Hangul_Start", XKB_KEY_Hangul_Start),
    ("XKB_KEY_Hangul_End", XKB_KEY_Hangul_End),
    ("XKB_KEY_Hangul_Hanja", XKB_KEY_Hangul_Hanja),
    ("XKB_KEY_Hangul_Jamo", XKB_KEY_Hangul_Jamo),
    ("XKB_KEY_Hangul_Romaja", XKB_KEY_Hangul_Romaja),
    ("XKB_KEY_Codeinput", XKB_KEY_Codeinput),
    ("XKB_KEY_Hangul_Jeonja", XKB_KEY_Hangul_Jeonja),
    ("XKB_KEY_Hangul_Banja", XKB_KEY_Hangul_Banja),
    ("XKB_KEY_Hangul_PreHanja", XKB_KEY_Hangul_PreHanja),
    ("XKB_KEY_Hangul_PostHanja", XKB_KEY_Hangul_PostHanja),
    ("XKB_KEY_SingleCandidate", XKB_KEY_SingleCandidate),
    ("XKB_KEY_MultipleCandidate", XKB_KEY_MultipleCandidate),
    ("XKB_KEY_PreviousCandidate", XKB_KEY_PreviousCandidate),
    ("XKB_KEY_Hangul_Special", XKB_KEY_Hangul_Special),
    ("XKB_KEY_Home", XKB_KEY_Home),
    ("XKB_KEY_Left", XKB_KEY_Left),
    ("XKB_KEY_Up", XKB_KEY_Up),
    ("XKB_KEY_Right", XKB_KEY_Right),
    ("XKB_KEY_Down", XKB_KEY_Down),
    ("XKB_KEY_Page_Up", XKB_KEY_Page_Up),
    ("XKB_KEY_Page_Down", XKB_KEY_Page_Down),
    ("XKB_KEY_End", XKB_KEY_End),
    ("XKB_KEY_Select", XKB_KEY_Select),
    ("XKB_KEY_Print", XKB_KEY_Print),
    ("XKB_KEY_Execute", XKB_KEY_Execute),
    ("XKB_KEY_Insert", XKB_KEY_Insert),
    ("XKB_KEY_Undo", XKB_KEY_Undo),
    ("XKB_KEY_Redo", XKB_KEY_Redo),
    ("XKB_KEY_Menu", XKB_KEY_Menu),
    ("XKB_KEY_Find", XKB_KEY_Find),
    ("XKB_KEY_Cancel", XKB_KEY_Cancel),
    ("XKB_KEY_Help", XKB_KEY_Help),
    ("XKB_KEY_Mode_switch", XKB_KEY_Mode_switch),
    ("XKB_KEY_KP_Enter", XKB_KEY_KP_Enter),
    ("XKB_KEY_KP_Multiply", XKB_KEY_KP_Multiply),
    ("XKB_KEY_KP_Add", XKB_KEY_KP_Add),
    ("XKB_KEY_KP_Separator", XKB_KEY_KP_Separator),
    ("XKB_KEY_KP_Subtract", XKB_KEY_KP_Subtract),
    ("XKB_KEY_KP_Decimal", XKB_KEY_KP_Decimal),
    ("XKB_KEY_KP_Divide", XKB_KEY_KP_Divide),
    ("XKB_KEY_KP_0", XKB_KEY_KP_0),
    ("XKB_KEY_KP_1", XKB_KEY_KP_1),
    ("XKB_KEY_KP_2", XKB_KEY_KP_2),
    ("XKB_KEY_KP_3", XKB_KEY_KP_3),
    ("XKB_KEY_KP_4", XKB_KEY_KP_4),
    ("XKB_KEY_KP_5", XKB_KEY_KP_5),
    ("XKB_KEY_KP_6", XKB_KEY_KP_6),
    ("XKB_KEY_KP_7", XKB_KEY_KP_7),
    ("XKB_KEY_KP_8", XKB_KEY_KP_8),
    ("XKB_KEY_KP_9", XKB_KEY_KP_9),
    ("XKB_KEY_KP_Equal", XKB_KEY_KP_Equal),
    ("XKB_KEY_F1", XKB_KEY_F1),
    ("XKB_KEY_F2", XKB_KEY_F2),
    ("XKB_KEY_F3", XKB_KEY_F3),
    ("XKB_KEY_F4", XKB_KEY_F4),
    ("XKB_KEY_F5", XKB_KEY_F5),
    ("XKB_KEY_F6", XKB_KEY_F6),
    ("XKB_KEY_F7", XKB_KEY_F7),
    ("XKB_KEY_F8", XKB_KEY_F8),
    ("XKB_KEY_F9", XKB_KEY_F9),
    ("XKB_KEY_F10", XKB_KEY_F10),
    ("XKB_KEY_F11", XKB_KEY_F11),
    ("XKB_KEY_F12", XKB_KEY_F12),
    ("XKB_KEY_F13", XKB_KEY_F13),
    ("XKB_KEY_F14", XKB_KEY_F14),
    ("XKB_KEY_F15", XKB_KEY_F15),
    ("XKB_KEY_F16", XKB_KEY_F16),
    ("XKB_KEY_F17", XKB_KEY_F17),
    ("XKB_KEY_F18", XKB_KEY_F18),
    ("XKB_KEY_F19", XKB_KEY_F19),
    ("XKB_KEY_F20", XKB_KEY_F20),
    ("XKB_KEY_F21", XKB_KEY_F21),
    ("XKB_KEY_F22", XKB_KEY_F22),
    ("XKB_KEY_F23", XKB_KEY_F23),
    ("XKB_KEY_F24", XKB_KEY_F24),
    ("XKB_KEY_F25", XKB_KEY_F25),
    ("XKB_KEY_F26", XKB_KEY_F26),
    ("XKB_KEY_F27", XKB_KEY_F27),
    ("XKB_KEY_F28", XKB_KEY_F28),
    ("XKB_KEY_F29", XKB_KEY_F29),
    ("XKB_KEY_F30", XKB_KEY_F30),
    ("XKB_KEY_F31", XKB_KEY_F31),
    ("XKB_KEY_F32", XKB_KEY_F32),
    ("XKB_KEY_F33", XKB_KEY_F33),
    ("XKB_KEY_F34", XKB_KEY_F34),
    ("XKB_KEY_F35", XKB_KEY_F35),
    ("XKB_KEY_Hyper_L", XKB_KEY_Hyper_L),
    ("XKB_KEY_Hyper_R", XKB_KEY_Hyper_R),
    ("XKB_KEY_Delete", XKB_KEY_Delete),
    ("XKB_KEY_BackTab", XKB_KEY_BackTab),
    ("XKB_KEY_XF86AudioLowerVolume", XKB_KEY_XF86AudioLowerVolume),
    ("XKB_KEY_XF86AudioMute", XKB_KEY_XF86AudioMute),
    ("XKB_KEY_XF86AudioRaiseVolume", XKB_KEY_XF86AudioRaiseVolume),
    ("XKB_KEY_XF86AudioPlay", XKB_KEY_XF86AudioPlay),
    ("XKB_KEY_XF86AudioStop", XKB_KEY_XF86AudioStop),
    ("XKB_KEY_XF86AudioPrev", XKB_KEY_XF86AudioPrev),
    ("XKB_KEY_XF86AudioNext", XKB_KEY_XF86AudioNext),
];

fn last() -> i32 {
    i32::try_from(OBS_KEY_LAST_VALUE).unwrap()
}

/// The keysym as the C's `int`.
fn int(sym: u32) -> i32 {
    i32::from_ne_bytes(sym.to_ne_bytes())
}

#[test]
fn values_and_keysyms() {
    let rust = [
        i64::try_from(OBS_KEY_LAST_VALUE).unwrap(),
        i64::from(KEY_NONE),
        i64::try_from(MAX_KEYCODES).unwrap(),
        i64::try_from(MAX_SHIFT_LEVELS).unwrap(),
        i64::try_from(size_of::<c_int>()).unwrap(),
    ];
    for (which, (name, value)) in c::VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a plain getter.
        assert_eq!(
            unsafe { c::oracle_wayland_keys_value(which) },
            value,
            "{name}"
        );
    }

    // SAFETY: a plain getter.
    let real = unsafe { c::oracle_wayland_keys_real_keysyms() } == 1;
    if !real {
        eprintln!("the oracle has no <xkbcommon/xkbcommon-keysyms.h>: keysym names only");
    }
    for (which, (name, value)) in KEYSYMS.iter().enumerate() {
        assert_eq!(c::KEYSYM_NAMES[which], *name);
        // SAFETY: a plain getter.
        assert_eq!(
            unsafe { c::oracle_wayland_keys_keysym(which) },
            i64::from(*value),
            "{name}"
        );
    }
}

fn from_all(sym: i32) {
    // SAFETY: a pure function of its argument.
    let want = unsafe { c::oracle_obs_nix_wayland_keys_from_keysym(sym) };
    assert_eq!(
        shim::obs_nix_wayland_keys_from_keysym(sym),
        want,
        "keysym {sym:#x}"
    );
    assert_eq!(core::from_keysym(sym), want, "keysym {sym:#x}");
}

fn to_all(key: i32) {
    // SAFETY: a pure function of its argument.
    let want = unsafe { c::oracle_obs_nix_wayland_keys_to_keysym(key) };
    assert_eq!(shim::obs_nix_wayland_keys_to_keysym(key), want, "key {key}");
    assert_eq!(int(core::to_keysym(key)), want, "key {key}");
}

#[test]
fn tables_exhaustive() {
    for sym in (0..0x2_0000)
        .chain(0x1000_FF00..0x1001_0000)
        .chain(0x1008_FE00..0x1009_0000)
    {
        from_all(sym);
    }
    for &(_, sym) in &KEYSYMS {
        for near in [int(sym).wrapping_sub(1), int(sym), int(sym).wrapping_add(1)] {
            from_all(near);
        }
    }
    for sym in [
        i32::MIN,
        i32::MIN + 1,
        -2,
        -1,
        0x00ff_ffff,
        0x0100_0000,
        i32::MAX - 1,
        i32::MAX,
    ] {
        from_all(sym);
    }
    for key in (-100..last() + 100).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]) {
        to_all(key);
    }
}

/// One keycode's levels for the level callback: the count and keysyms of
/// each level, and the levels asked for.
struct Row<'a> {
    levels: &'a [(c_int, [u32; 2]); MAX_SHIFT_LEVELS],
    asked: Vec<u32>,
}

unsafe extern "C" fn level_syms(data: *mut c_void, level: u32, syms: *mut *const u32) -> c_int {
    // SAFETY: `data` is the `Row` the caller passed, alive for the call.
    let row = unsafe { &mut *data.cast::<Row<'_>>() };
    row.asked.push(level);
    let Some((count, values)) = usize::try_from(level).ok().and_then(|l| row.levels.get(l)) else {
        return 0;
    };
    // SAFETY: the callee passes a writable pointer.
    unsafe { syms.write(values.as_ptr()) };
    *count
}

type Keymap = Vec<(u32, [(c_int, [u32; 2]); MAX_SHIFT_LEVELS])>;

/// A whole keymap, one row at a time, on the C and on the shim: the levels
/// each asks for and both arrays.
fn keymap_both(keymap: &Keymap) {
    let mut c_obs = vec![0u32; OBS_KEY_LAST_VALUE];
    let mut c_syms = vec![[0u32; MAX_KEYCODES]; MAX_SHIFT_LEVELS];
    let mut r_obs = c_obs.clone();
    let mut r_syms = c_syms.clone();
    for (keycode, levels) in keymap {
        let mut c_row = Row {
            levels,
            asked: Vec::new(),
        };
        let mut r_row = Row {
            levels,
            asked: Vec::new(),
        };
        // SAFETY: the arrays have the sizes the C reads; the callback and
        // its row outlive the call.
        unsafe {
            c::oracle_obs_nix_wayland_keys_row(
                *keycode,
                Some(level_syms),
                (&raw mut c_row).cast(),
                c_obs.as_mut_ptr(),
                c_syms.as_mut_ptr(),
            );
            shim::obs_nix_wayland_keys_row(
                *keycode,
                Some(level_syms),
                (&raw mut r_row).cast(),
                r_obs.as_mut_ptr(),
                r_syms.as_mut_ptr(),
            );
        }
        assert_eq!(r_row.asked, c_row.asked, "keycode {keycode}");
    }
    assert_eq!(r_obs, c_obs);
    assert_eq!(r_syms, c_syms);
}

/// Keysyms for generated keymaps: the tables' keysyms mostly, then 0 and
/// any value.
fn pick(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    let r = *seed;
    let n = usize::try_from(r >> 40).unwrap();
    match r % 8 {
        0 => 0,
        1 => u32::try_from(r >> 32).unwrap(),
        _ => KEYSYMS[n % KEYSYMS.len()].1,
    }
}

fn generated_keymap(seed: u64) -> Keymap {
    let mut s = seed | 1;
    let repeated = pick(&mut s);
    let first = u32::try_from(seed % 9).unwrap();
    let end = if seed.is_multiple_of(3) { 301 } else { 256 };
    (first..end)
        .map(|keycode| {
            let levels = std::array::from_fn(|_| {
                let count = c_int::try_from(pick(&mut s) % 5).unwrap() - 1;
                let sym = if pick(&mut s).is_multiple_of(4) {
                    repeated
                } else {
                    pick(&mut s)
                };
                (count, [sym, pick(&mut s)])
            });
            (keycode, levels)
        })
        .collect()
}

#[test]
fn keymap_rows_generated() {
    for seed in 0..200u64 {
        if obs_c_oracle::parity::keep(seed) || seed < 8 {
            keymap_both(&generated_keymap(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        }
    }
}

/// A US-like row set: every key of the to-table on its own keycode, its
/// lowercase letter or shifted keysym on level 1.
#[test]
fn keymap_rows_every_key() {
    let mut keymap: Keymap = Vec::new();
    for key in 0..last() {
        let sym = core::to_keysym(key);
        if sym == 0 {
            continue;
        }
        let keycode = u32::try_from(keymap.len() % 300).unwrap();
        let lower = if (XKB_KEY_A..=XKB_KEY_Z).contains(&sym) {
            sym + 0x20
        } else {
            sym
        };
        keymap.push((
            keycode,
            [
                (1, [lower, 0]),
                (1, [sym, 0]),
                (0, [0, 0]),
                (2, [sym, lower]),
            ],
        ));
    }
    keymap_both(&keymap);
}

/// One focused key event on the C, on the shim and on the core: the shim
/// and the core agree; the shim agrees with the C on every press, and on a
/// release reports what the C reported for its press (`pressed_as`, the
/// model, kept from the C's results), or nothing.
struct Focus {
    key_to_sym0: Vec<u32>,
    c_record: Vec<c_int>,
    r_record: Vec<c_int>,
    core_record: Vec<c_int>,
    pressed_as: Vec<Option<c_int>>,
    /// Releases where the shim and the C differ.
    differences: usize,
}

impl Focus {
    fn new() -> Self {
        Self {
            key_to_sym0: vec![0; MAX_KEYCODES],
            c_record: vec![KEY_NONE; MAX_KEYCODES],
            r_record: vec![KEY_NONE; MAX_KEYCODES],
            core_record: vec![KEY_NONE; MAX_KEYCODES],
            pressed_as: vec![None; MAX_KEYCODES],
            differences: 0,
        }
    }

    fn event(&mut self, scancode: u32, pressed: bool) {
        let mut c_key: c_int = -7;
        let mut r_key: c_int = -7;
        // SAFETY: the arrays have the sizes the C reads.
        let (c_hit, r_hit) = unsafe {
            (
                c::oracle_obs_nix_wayland_keys_focused(
                    scancode,
                    pressed,
                    self.key_to_sym0.as_ptr(),
                    self.c_record.as_mut_ptr(),
                    &raw mut c_key,
                ),
                shim::obs_nix_wayland_keys_focused(
                    scancode,
                    pressed,
                    self.key_to_sym0.as_ptr(),
                    self.r_record.as_mut_ptr(),
                    &raw mut r_key,
                ),
            )
        };
        let c_got = c_hit.then_some(c_key);
        let r_got = r_hit.then_some(r_key);
        if !c_hit {
            assert_eq!(c_key, -7, "the C wrote *key for nothing");
        }
        if !r_hit {
            assert_eq!(r_key, -7, "the shim wrote *key for nothing");
        }
        assert_eq!(
            self.c_record,
            vec![KEY_NONE; MAX_KEYCODES],
            "the C left the record alone"
        );

        let core_got = core::focused(scancode, pressed, &self.key_to_sym0, &mut self.core_record);
        assert_eq!(
            r_got, core_got,
            "shim and core, scancode {scancode} pressed {pressed}"
        );
        assert_eq!(self.r_record, self.core_record);

        let Some(keycode) = usize::try_from(scancode.wrapping_add(8))
            .ok()
            .filter(|&k| k < MAX_KEYCODES)
        else {
            assert_eq!(r_got, None);
            assert_eq!(c_got, None);
            return;
        };
        if pressed {
            assert_eq!(r_got, c_got, "scancode {scancode} pressed");
            if c_got.is_some() {
                self.pressed_as[keycode] = c_got;
            }
            return;
        }
        // The fix: the key the press reported (none for OBS_KEY_NONE), once.
        let want = self.pressed_as[keycode].take().filter(|&k| k != KEY_NONE);
        assert_eq!(r_got, want, "scancode {scancode} released");
        if r_got != c_got {
            self.differences += 1;
        }
    }
}

#[test]
fn focused_every_scancode() {
    let mut f = Focus::new();
    for (i, slot) in f.key_to_sym0.iter_mut().enumerate() {
        *slot = KEYSYMS[i * 7 % KEYSYMS.len()].1;
    }
    f.key_to_sym0[30] = 0;
    f.key_to_sym0[92] = 0xfe03; // ISO_Level3_Shift: no key
    let scancodes: Vec<u32> = (0..=300).chain(u32::MAX - 16..=u32::MAX).collect();
    for &s in &scancodes {
        f.event(s, true);
    }
    for &s in &scancodes {
        f.event(s, false);
    }
    for &s in &scancodes {
        f.event(s, false);
    }
}

#[test]
fn focused_across_layout_changes() {
    let mut f = Focus::new();
    let qwerty = [XKB_KEY_q, XKB_KEY_w, XKB_KEY_e, XKB_KEY_space];
    let azerty = [XKB_KEY_a, XKB_KEY_z, XKB_KEY_e, XKB_KEY_space];
    let set = |f: &mut Focus, layout: &[u32; 4]| f.key_to_sym0[24..28].copy_from_slice(layout);
    set(&mut f, &qwerty);
    f.event(16, true); // Q
    f.event(17, true); // W
    f.event(19, true); // Space
    set(&mut f, &azerty);
    f.event(16, false); // C: A, shim: Q
    f.event(17, false); // C: Z, shim: W
    f.event(19, false); // Space on both
    f.event(18, true); // E
    set(&mut f, &qwerty);
    f.event(18, false); // E on both
    assert_eq!(f.differences, 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    #[test]
    fn prop_from_keysym_any_int(sym in any::<i32>()) {
        from_all(sym);
    }

    #[test]
    fn prop_to_keysym_any_int(key in any::<i32>()) {
        to_all(key);
    }

    #[test]
    fn prop_keymap_rows(seed in any::<u64>()) {
        keymap_both(&generated_keymap(seed));
    }

    #[test]
    fn prop_focused_sequences(
        layouts in prop::collection::vec(
            prop::collection::vec(prop::sample::select(KEYSYMS.map(|(_, s)| s).to_vec()), 8),
            1..4,
        ),
        events in prop::collection::vec((0u32..8, any::<bool>(), 0usize..4), 0..80),
    ) {
        let mut f = Focus::new();
        for (code, pressed, layout) in events {
            let layout = &layouts[layout % layouts.len()];
            f.key_to_sym0[20..28].copy_from_slice(layout);
            f.event(code + 12, pressed);
        }
    }
}
