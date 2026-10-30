//! Safe core of `libobs/obs-nix-x11-input.c.h`: the X11 keysym table and
//! the mouse button answers of the X11 hotkey platform (`obs-nix-x11.c`,
//! Linux and FreeBSD).
//!
//! The X11 hotkey platform (still C) asks this code:
//! - [`keysym`], for every key at init (`fill_base_keysyms()`): the X keysym
//!   of the key, which fills the base keysym table that
//!   [`nix_x11_keys`](crate::nix_x11_keys) searches;
//! - [`mouse_pressed`], on each hotkey poll of a mouse key with XInput, after
//!   the C drained the raw button events into the poll's `pressed` and
//!   `update` arrays: whether the key's button is down, pressed in this poll
//!   or held from an earlier one; then [`commit`], which stores the poll's
//!   result in the held array;
//! - [`mask_pressed`], on each hotkey poll of a mouse key without XInput:
//!   whether the key's button bit is set in the QueryPointer reply's mask.
//!
//! It reads the caller's arrays and values, so the C keeps the raw event
//! drain, the array clears, the QueryPointer request and every X call. It
//! keeps no state, takes no lock, allocates nothing and logs nothing.
//!
//! The three per-button arrays (pressed in this poll, updated in this poll,
//! held from earlier polls) have [`MOUSE_LEN`] entries, `XINPUT_MOUSE_LEN`
//! in `obs-hotkey.h`: a mouse key reads [`slot`] in them.
//!
//! No intentional difference from C. Characterized and kept: two pairs of
//! keys share a keysym (Quoteleft and Asciitilde on `XK_grave`, Cedilla and
//! Ccedilla on `XK_cedilla`), and Mouse 29 reads slot 32, which the C's raw
//! event drain never fills (it takes buttons 1 to 32 into slots 0 to 31).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::name_map as k;

/// `XINPUT_MOUSE_LEN`: the entries of each per-button array.
pub const MOUSE_LEN: usize = 33;

/// `XCB_BUTTON_MASK_1`: the left button's bit in a QueryPointer mask.
pub const BUTTON_MASK_1: u16 = 0x100;
/// `XCB_BUTTON_MASK_2`: the middle button's bit.
pub const BUTTON_MASK_2: u16 = 0x200;
/// `XCB_BUTTON_MASK_3`: the right button's bit.
pub const BUTTON_MASK_3: u16 = 0x400;

/// The keysyms `get_keysym()` gives mouse buttons 1 to 5: `n << 16`.
const MOUSE_1: i32 = 0x1_0000;
const MOUSE_2: i32 = 0x2_0000;
const MOUSE_3: i32 = 0x3_0000;
const MOUSE_4: i32 = 0x4_0000;
const MOUSE_5: i32 = 0x5_0000;

/// `get_keysym()`: the X keysym of `key` (`obs_key_t`), 0 for a key it
/// doesn't name and for a value that isn't a key. Mouse buttons 1 to 5 are
/// their number shifted left by 16.
#[expect(clippy::too_many_lines, reason = "one arm per key, as the C table")]
pub fn keysym(key: i32) -> i32 {
    // Each keysym is the X header's value, named beside it.
    match key {
        k::OBS_KEY_RETURN => 0xff0d,    // `XK_Return`
        k::OBS_KEY_ESCAPE => 0xff1b,    // `XK_Escape`
        k::OBS_KEY_TAB => 0xff09,       // `XK_Tab`
        k::OBS_KEY_BACKSPACE => 0xff08, // `XK_BackSpace`
        k::OBS_KEY_INSERT => 0xff63,    // `XK_Insert`
        k::OBS_KEY_DELETE => 0xffff,    // `XK_Delete`
        k::OBS_KEY_PAUSE => 0xff13,     // `XK_Pause`
        k::OBS_KEY_PRINT => 0xff61,     // `XK_Print`
        k::OBS_KEY_HOME => 0xff50,      // `XK_Home`
        k::OBS_KEY_END => 0xff57,       // `XK_End`
        k::OBS_KEY_LEFT => 0xff51,      // `XK_Left`
        k::OBS_KEY_UP => 0xff52,        // `XK_Up`
        k::OBS_KEY_RIGHT => 0xff53,     // `XK_Right`
        k::OBS_KEY_DOWN => 0xff54,      // `XK_Down`
        k::OBS_KEY_PAGEUP => 0xff55,    // `XK_Prior`
        k::OBS_KEY_PAGEDOWN => 0xff56,  // `XK_Next`

        k::OBS_KEY_SHIFT => 0xffe1,      // `XK_Shift_L`
        k::OBS_KEY_CONTROL => 0xffe3,    // `XK_Control_L`
        k::OBS_KEY_ALT => 0xffe9,        // `XK_Alt_L`
        k::OBS_KEY_CAPSLOCK => 0xffe5,   // `XK_Caps_Lock`
        k::OBS_KEY_NUMLOCK => 0xff7f,    // `XK_Num_Lock`
        k::OBS_KEY_SCROLLLOCK => 0xff14, // `XK_Scroll_Lock`

        k::OBS_KEY_F1 => 0xffbe,  // `XK_F1`
        k::OBS_KEY_F2 => 0xffbf,  // `XK_F2`
        k::OBS_KEY_F3 => 0xffc0,  // `XK_F3`
        k::OBS_KEY_F4 => 0xffc1,  // `XK_F4`
        k::OBS_KEY_F5 => 0xffc2,  // `XK_F5`
        k::OBS_KEY_F6 => 0xffc3,  // `XK_F6`
        k::OBS_KEY_F7 => 0xffc4,  // `XK_F7`
        k::OBS_KEY_F8 => 0xffc5,  // `XK_F8`
        k::OBS_KEY_F9 => 0xffc6,  // `XK_F9`
        k::OBS_KEY_F10 => 0xffc7, // `XK_F10`
        k::OBS_KEY_F11 => 0xffc8, // `XK_F11`
        k::OBS_KEY_F12 => 0xffc9, // `XK_F12`
        k::OBS_KEY_F13 => 0xffca, // `XK_F13`
        k::OBS_KEY_F14 => 0xffcb, // `XK_F14`
        k::OBS_KEY_F15 => 0xffcc, // `XK_F15`
        k::OBS_KEY_F16 => 0xffcd, // `XK_F16`
        k::OBS_KEY_F17 => 0xffce, // `XK_F17`
        k::OBS_KEY_F18 => 0xffcf, // `XK_F18`
        k::OBS_KEY_F19 => 0xffd0, // `XK_F19`
        k::OBS_KEY_F20 => 0xffd1, // `XK_F20`
        k::OBS_KEY_F21 => 0xffd2, // `XK_F21`
        k::OBS_KEY_F22 => 0xffd3, // `XK_F22`
        k::OBS_KEY_F23 => 0xffd4, // `XK_F23`
        k::OBS_KEY_F24 => 0xffd5, // `XK_F24`
        k::OBS_KEY_F25 => 0xffd6, // `XK_F25`
        k::OBS_KEY_F26 => 0xffd7, // `XK_F26`
        k::OBS_KEY_F27 => 0xffd8, // `XK_F27`
        k::OBS_KEY_F28 => 0xffd9, // `XK_F28`
        k::OBS_KEY_F29 => 0xffda, // `XK_F29`
        k::OBS_KEY_F30 => 0xffdb, // `XK_F30`
        k::OBS_KEY_F31 => 0xffdc, // `XK_F31`
        k::OBS_KEY_F32 => 0xffdd, // `XK_F32`
        k::OBS_KEY_F33 => 0xffde, // `XK_F33`
        k::OBS_KEY_F34 => 0xffdf, // `XK_F34`
        k::OBS_KEY_F35 => 0xffe0, // `XK_F35`

        k::OBS_KEY_MENU => 0xff67,    // `XK_Menu`
        k::OBS_KEY_HYPER_L => 0xffed, // `XK_Hyper_L`
        k::OBS_KEY_HYPER_R => 0xffee, // `XK_Hyper_R`
        k::OBS_KEY_HELP => 0xff6a,    // `XK_Help`
        k::OBS_KEY_CANCEL => 0xff69,  // `XK_Cancel`
        k::OBS_KEY_FIND => 0xff68,    // `XK_Find`
        k::OBS_KEY_REDO => 0xff66,    // `XK_Redo`
        k::OBS_KEY_UNDO => 0xff65,    // `XK_Undo`
        k::OBS_KEY_SPACE => 0x20,     // `XK_space`

        k::OBS_KEY_COPY => 0x1008ff57,  // `XF86XK_Copy`
        k::OBS_KEY_CUT => 0x1008ff58,   // `XF86XK_Cut`
        k::OBS_KEY_OPEN => 0x1008ff6b,  // `XF86XK_Open`
        k::OBS_KEY_PASTE => 0x1008ff6d, // `XF86XK_Paste`
        k::OBS_KEY_FRONT => 0x1005ff71, // `SunXK_Front`
        k::OBS_KEY_PROPS => 0x1005ff70, // `SunXK_Props`

        k::OBS_KEY_EXCLAM => 0x21,                  // `XK_exclam`
        k::OBS_KEY_QUOTEDBL => 0x22,                // `XK_quotedbl`
        k::OBS_KEY_NUMBERSIGN => 0x23,              // `XK_numbersign`
        k::OBS_KEY_DOLLAR => 0x24,                  // `XK_dollar`
        k::OBS_KEY_PERCENT => 0x25,                 // `XK_percent`
        k::OBS_KEY_AMPERSAND => 0x26,               // `XK_ampersand`
        k::OBS_KEY_APOSTROPHE => 0x27,              // `XK_apostrophe`
        k::OBS_KEY_PARENLEFT => 0x28,               // `XK_parenleft`
        k::OBS_KEY_PARENRIGHT => 0x29,              // `XK_parenright`
        k::OBS_KEY_ASTERISK => 0x2a,                // `XK_asterisk`
        k::OBS_KEY_PLUS => 0x2b,                    // `XK_plus`
        k::OBS_KEY_COMMA => 0x2c,                   // `XK_comma`
        k::OBS_KEY_MINUS => 0x2d,                   // `XK_minus`
        k::OBS_KEY_PERIOD => 0x2e,                  // `XK_period`
        k::OBS_KEY_SLASH => 0x2f,                   // `XK_slash`
        k::OBS_KEY_0 => 0x30,                       // `XK_0`
        k::OBS_KEY_1 => 0x31,                       // `XK_1`
        k::OBS_KEY_2 => 0x32,                       // `XK_2`
        k::OBS_KEY_3 => 0x33,                       // `XK_3`
        k::OBS_KEY_4 => 0x34,                       // `XK_4`
        k::OBS_KEY_5 => 0x35,                       // `XK_5`
        k::OBS_KEY_6 => 0x36,                       // `XK_6`
        k::OBS_KEY_7 => 0x37,                       // `XK_7`
        k::OBS_KEY_8 => 0x38,                       // `XK_8`
        k::OBS_KEY_9 => 0x39,                       // `XK_9`
        k::OBS_KEY_NUMEQUAL => 0xffbd,              // `XK_KP_Equal`
        k::OBS_KEY_NUMASTERISK => 0xffaa,           // `XK_KP_Multiply`
        k::OBS_KEY_NUMPLUS => 0xffab,               // `XK_KP_Add`
        k::OBS_KEY_NUMCOMMA => 0xffac,              // `XK_KP_Separator`
        k::OBS_KEY_NUMMINUS => 0xffad,              // `XK_KP_Subtract`
        k::OBS_KEY_NUMPERIOD => 0xffae,             // `XK_KP_Decimal`
        k::OBS_KEY_NUMSLASH => 0xffaf,              // `XK_KP_Divide`
        k::OBS_KEY_NUM0 => 0xffb0,                  // `XK_KP_0`
        k::OBS_KEY_NUM1 => 0xffb1,                  // `XK_KP_1`
        k::OBS_KEY_NUM2 => 0xffb2,                  // `XK_KP_2`
        k::OBS_KEY_NUM3 => 0xffb3,                  // `XK_KP_3`
        k::OBS_KEY_NUM4 => 0xffb4,                  // `XK_KP_4`
        k::OBS_KEY_NUM5 => 0xffb5,                  // `XK_KP_5`
        k::OBS_KEY_NUM6 => 0xffb6,                  // `XK_KP_6`
        k::OBS_KEY_NUM7 => 0xffb7,                  // `XK_KP_7`
        k::OBS_KEY_NUM8 => 0xffb8,                  // `XK_KP_8`
        k::OBS_KEY_NUM9 => 0xffb9,                  // `XK_KP_9`
        k::OBS_KEY_COLON => 0x3a,                   // `XK_colon`
        k::OBS_KEY_SEMICOLON => 0x3b,               // `XK_semicolon`
        k::OBS_KEY_LESS => 0x3c,                    // `XK_less`
        k::OBS_KEY_EQUAL => 0x3d,                   // `XK_equal`
        k::OBS_KEY_GREATER => 0x3e,                 // `XK_greater`
        k::OBS_KEY_QUESTION => 0x3f,                // `XK_question`
        k::OBS_KEY_AT => 0x40,                      // `XK_at`
        k::OBS_KEY_A => 0x41,                       // `XK_A`
        k::OBS_KEY_B => 0x42,                       // `XK_B`
        k::OBS_KEY_C => 0x43,                       // `XK_C`
        k::OBS_KEY_D => 0x44,                       // `XK_D`
        k::OBS_KEY_E => 0x45,                       // `XK_E`
        k::OBS_KEY_F => 0x46,                       // `XK_F`
        k::OBS_KEY_G => 0x47,                       // `XK_G`
        k::OBS_KEY_H => 0x48,                       // `XK_H`
        k::OBS_KEY_I => 0x49,                       // `XK_I`
        k::OBS_KEY_J => 0x4a,                       // `XK_J`
        k::OBS_KEY_K => 0x4b,                       // `XK_K`
        k::OBS_KEY_L => 0x4c,                       // `XK_L`
        k::OBS_KEY_M => 0x4d,                       // `XK_M`
        k::OBS_KEY_N => 0x4e,                       // `XK_N`
        k::OBS_KEY_O => 0x4f,                       // `XK_O`
        k::OBS_KEY_P => 0x50,                       // `XK_P`
        k::OBS_KEY_Q => 0x51,                       // `XK_Q`
        k::OBS_KEY_R => 0x52,                       // `XK_R`
        k::OBS_KEY_S => 0x53,                       // `XK_S`
        k::OBS_KEY_T => 0x54,                       // `XK_T`
        k::OBS_KEY_U => 0x55,                       // `XK_U`
        k::OBS_KEY_V => 0x56,                       // `XK_V`
        k::OBS_KEY_W => 0x57,                       // `XK_W`
        k::OBS_KEY_X => 0x58,                       // `XK_X`
        k::OBS_KEY_Y => 0x59,                       // `XK_Y`
        k::OBS_KEY_Z => 0x5a,                       // `XK_Z`
        k::OBS_KEY_BRACKETLEFT => 0x5b,             // `XK_bracketleft`
        k::OBS_KEY_BACKSLASH => 0x5c,               // `XK_backslash`
        k::OBS_KEY_BRACKETRIGHT => 0x5d,            // `XK_bracketright`
        k::OBS_KEY_ASCIICIRCUM => 0x5e,             // `XK_asciicircum`
        k::OBS_KEY_UNDERSCORE => 0x5f,              // `XK_underscore`
        k::OBS_KEY_QUOTELEFT => 0x60,               // `XK_quoteleft`
        k::OBS_KEY_BRACELEFT => 0x7b,               // `XK_braceleft`
        k::OBS_KEY_BAR => 0x7c,                     // `XK_bar`
        k::OBS_KEY_BRACERIGHT => 0x7d,              // `XK_braceright`
        k::OBS_KEY_ASCIITILDE => 0x60,              // `XK_grave`
        k::OBS_KEY_NOBREAKSPACE => 0xa0,            // `XK_nobreakspace`
        k::OBS_KEY_EXCLAMDOWN => 0xa1,              // `XK_exclamdown`
        k::OBS_KEY_CENT => 0xa2,                    // `XK_cent`
        k::OBS_KEY_STERLING => 0xa3,                // `XK_sterling`
        k::OBS_KEY_CURRENCY => 0xa4,                // `XK_currency`
        k::OBS_KEY_YEN => 0xa5,                     // `XK_yen`
        k::OBS_KEY_BROKENBAR => 0xa6,               // `XK_brokenbar`
        k::OBS_KEY_SECTION => 0xa7,                 // `XK_section`
        k::OBS_KEY_DIAERESIS => 0xa8,               // `XK_diaeresis`
        k::OBS_KEY_COPYRIGHT => 0xa9,               // `XK_copyright`
        k::OBS_KEY_ORDFEMININE => 0xaa,             // `XK_ordfeminine`
        k::OBS_KEY_GUILLEMOTLEFT => 0xab,           // `XK_guillemotleft`
        k::OBS_KEY_NOTSIGN => 0xac,                 // `XK_notsign`
        k::OBS_KEY_HYPHEN => 0xad,                  // `XK_hyphen`
        k::OBS_KEY_REGISTERED => 0xae,              // `XK_registered`
        k::OBS_KEY_MACRON => 0xaf,                  // `XK_macron`
        k::OBS_KEY_DEGREE => 0xb0,                  // `XK_degree`
        k::OBS_KEY_PLUSMINUS => 0xb1,               // `XK_plusminus`
        k::OBS_KEY_TWOSUPERIOR => 0xb2,             // `XK_twosuperior`
        k::OBS_KEY_THREESUPERIOR => 0xb3,           // `XK_threesuperior`
        k::OBS_KEY_ACUTE => 0xb4,                   // `XK_acute`
        k::OBS_KEY_MU => 0xb5,                      // `XK_mu`
        k::OBS_KEY_PARAGRAPH => 0xb6,               // `XK_paragraph`
        k::OBS_KEY_PERIODCENTERED => 0xb7,          // `XK_periodcentered`
        k::OBS_KEY_CEDILLA => 0xb8,                 // `XK_cedilla`
        k::OBS_KEY_ONESUPERIOR => 0xb9,             // `XK_onesuperior`
        k::OBS_KEY_MASCULINE => 0xba,               // `XK_masculine`
        k::OBS_KEY_GUILLEMOTRIGHT => 0xbb,          // `XK_guillemotright`
        k::OBS_KEY_ONEQUARTER => 0xbc,              // `XK_onequarter`
        k::OBS_KEY_ONEHALF => 0xbd,                 // `XK_onehalf`
        k::OBS_KEY_THREEQUARTERS => 0xbe,           // `XK_threequarters`
        k::OBS_KEY_QUESTIONDOWN => 0xbf,            // `XK_questiondown`
        k::OBS_KEY_AGRAVE => 0xc0,                  // `XK_Agrave`
        k::OBS_KEY_AACUTE => 0xc1,                  // `XK_Aacute`
        k::OBS_KEY_ACIRCUMFLEX => 0xc2,             // `XK_Acircumflex`
        k::OBS_KEY_ATILDE => 0xc3,                  // `XK_Atilde`
        k::OBS_KEY_ADIAERESIS => 0xc4,              // `XK_Adiaeresis`
        k::OBS_KEY_ARING => 0xc5,                   // `XK_Aring`
        k::OBS_KEY_AE => 0xc6,                      // `XK_AE`
        k::OBS_KEY_CCEDILLA => 0xb8,                // `XK_cedilla`
        k::OBS_KEY_EGRAVE => 0xc8,                  // `XK_Egrave`
        k::OBS_KEY_EACUTE => 0xc9,                  // `XK_Eacute`
        k::OBS_KEY_ECIRCUMFLEX => 0xca,             // `XK_Ecircumflex`
        k::OBS_KEY_EDIAERESIS => 0xcb,              // `XK_Ediaeresis`
        k::OBS_KEY_IGRAVE => 0xcc,                  // `XK_Igrave`
        k::OBS_KEY_IACUTE => 0xcd,                  // `XK_Iacute`
        k::OBS_KEY_ICIRCUMFLEX => 0xce,             // `XK_Icircumflex`
        k::OBS_KEY_IDIAERESIS => 0xcf,              // `XK_Idiaeresis`
        k::OBS_KEY_ETH => 0xd0,                     // `XK_ETH`
        k::OBS_KEY_NTILDE => 0xd1,                  // `XK_Ntilde`
        k::OBS_KEY_OGRAVE => 0xd2,                  // `XK_Ograve`
        k::OBS_KEY_OACUTE => 0xd3,                  // `XK_Oacute`
        k::OBS_KEY_OCIRCUMFLEX => 0xd4,             // `XK_Ocircumflex`
        k::OBS_KEY_ODIAERESIS => 0xd6,              // `XK_Odiaeresis`
        k::OBS_KEY_MULTIPLY => 0xd7,                // `XK_multiply`
        k::OBS_KEY_OOBLIQUE => 0xd8,                // `XK_Ooblique`
        k::OBS_KEY_UGRAVE => 0xd9,                  // `XK_Ugrave`
        k::OBS_KEY_UACUTE => 0xda,                  // `XK_Uacute`
        k::OBS_KEY_UCIRCUMFLEX => 0xdb,             // `XK_Ucircumflex`
        k::OBS_KEY_UDIAERESIS => 0xdc,              // `XK_Udiaeresis`
        k::OBS_KEY_YACUTE => 0xdd,                  // `XK_Yacute`
        k::OBS_KEY_THORN => 0xde,                   // `XK_Thorn`
        k::OBS_KEY_SSHARP => 0xdf,                  // `XK_ssharp`
        k::OBS_KEY_DIVISION => 0xf7,                // `XK_division`
        k::OBS_KEY_YDIAERESIS => 0x13be,            // `XK_Ydiaeresis`
        k::OBS_KEY_MULTI_KEY => 0xff20,             // `XK_Multi_key`
        k::OBS_KEY_CODEINPUT => 0xff37,             // `XK_Codeinput`
        k::OBS_KEY_SINGLECANDIDATE => 0xff3c,       // `XK_SingleCandidate`
        k::OBS_KEY_MULTIPLECANDIDATE => 0xff3d,     // `XK_MultipleCandidate`
        k::OBS_KEY_PREVIOUSCANDIDATE => 0xff3e,     // `XK_PreviousCandidate`
        k::OBS_KEY_MODE_SWITCH => 0xff7e,           // `XK_Mode_switch`
        k::OBS_KEY_KANJI => 0xff21,                 // `XK_Kanji`
        k::OBS_KEY_MUHENKAN => 0xff22,              // `XK_Muhenkan`
        k::OBS_KEY_HENKAN => 0xff23,                // `XK_Henkan`
        k::OBS_KEY_ROMAJI => 0xff24,                // `XK_Romaji`
        k::OBS_KEY_HIRAGANA => 0xff25,              // `XK_Hiragana`
        k::OBS_KEY_KATAKANA => 0xff26,              // `XK_Katakana`
        k::OBS_KEY_HIRAGANA_KATAKANA => 0xff27,     // `XK_Hiragana_Katakana`
        k::OBS_KEY_ZENKAKU => 0xff28,               // `XK_Zenkaku`
        k::OBS_KEY_HANKAKU => 0xff29,               // `XK_Hankaku`
        k::OBS_KEY_ZENKAKU_HANKAKU => 0xff2a,       // `XK_Zenkaku_Hankaku`
        k::OBS_KEY_TOUROKU => 0xff2b,               // `XK_Touroku`
        k::OBS_KEY_MASSYO => 0xff2c,                // `XK_Massyo`
        k::OBS_KEY_KANA_LOCK => 0xff2d,             // `XK_Kana_Lock`
        k::OBS_KEY_KANA_SHIFT => 0xff2e,            // `XK_Kana_Shift`
        k::OBS_KEY_EISU_SHIFT => 0xff2f,            // `XK_Eisu_Shift`
        k::OBS_KEY_EISU_TOGGLE => 0xff30,           // `XK_Eisu_toggle`
        k::OBS_KEY_HANGUL => 0xff31,                // `XK_Hangul`
        k::OBS_KEY_HANGUL_START => 0xff32,          // `XK_Hangul_Start`
        k::OBS_KEY_HANGUL_END => 0xff33,            // `XK_Hangul_End`
        k::OBS_KEY_HANGUL_HANJA => 0xff34,          // `XK_Hangul_Hanja`
        k::OBS_KEY_HANGUL_JAMO => 0xff35,           // `XK_Hangul_Jamo`
        k::OBS_KEY_HANGUL_ROMAJA => 0xff36,         // `XK_Hangul_Romaja`
        k::OBS_KEY_HANGUL_BANJA => 0xff39,          // `XK_Hangul_Banja`
        k::OBS_KEY_HANGUL_PREHANJA => 0xff3a,       // `XK_Hangul_PreHanja`
        k::OBS_KEY_HANGUL_POSTHANJA => 0xff3b,      // `XK_Hangul_PostHanja`
        k::OBS_KEY_HANGUL_SPECIAL => 0xff3f,        // `XK_Hangul_Special`
        k::OBS_KEY_DEAD_GRAVE => 0xfe50,            // `XK_dead_grave`
        k::OBS_KEY_DEAD_ACUTE => 0xfe51,            // `XK_dead_acute`
        k::OBS_KEY_DEAD_CIRCUMFLEX => 0xfe52,       // `XK_dead_circumflex`
        k::OBS_KEY_DEAD_TILDE => 0xfe53,            // `XK_dead_tilde`
        k::OBS_KEY_DEAD_MACRON => 0xfe54,           // `XK_dead_macron`
        k::OBS_KEY_DEAD_BREVE => 0xfe55,            // `XK_dead_breve`
        k::OBS_KEY_DEAD_ABOVEDOT => 0xfe56,         // `XK_dead_abovedot`
        k::OBS_KEY_DEAD_DIAERESIS => 0xfe57,        // `XK_dead_diaeresis`
        k::OBS_KEY_DEAD_ABOVERING => 0xfe58,        // `XK_dead_abovering`
        k::OBS_KEY_DEAD_DOUBLEACUTE => 0xfe59,      // `XK_dead_doubleacute`
        k::OBS_KEY_DEAD_CARON => 0xfe5a,            // `XK_dead_caron`
        k::OBS_KEY_DEAD_CEDILLA => 0xfe5b,          // `XK_dead_cedilla`
        k::OBS_KEY_DEAD_OGONEK => 0xfe5c,           // `XK_dead_ogonek`
        k::OBS_KEY_DEAD_IOTA => 0xfe5d,             // `XK_dead_iota`
        k::OBS_KEY_DEAD_VOICED_SOUND => 0xfe5e,     // `XK_dead_voiced_sound`
        k::OBS_KEY_DEAD_SEMIVOICED_SOUND => 0xfe5f, // `XK_dead_semivoiced_sound`
        k::OBS_KEY_DEAD_BELOWDOT => 0xfe60,         // `XK_dead_belowdot`
        k::OBS_KEY_DEAD_HOOK => 0xfe61,             // `XK_dead_hook`
        k::OBS_KEY_DEAD_HORN => 0xfe62,             // `XK_dead_horn`

        k::OBS_KEY_MOUSE1 => MOUSE_1,
        k::OBS_KEY_MOUSE2 => MOUSE_2,
        k::OBS_KEY_MOUSE3 => MOUSE_3,
        k::OBS_KEY_MOUSE4 => MOUSE_4,
        k::OBS_KEY_MOUSE5 => MOUSE_5,

        k::OBS_KEY_VK_MEDIA_PLAY_PAUSE => 0x1008ff14, // `XF86XK_AudioPlay`
        k::OBS_KEY_VK_MEDIA_STOP => 0x1008ff15,       // `XF86XK_AudioStop`
        k::OBS_KEY_VK_MEDIA_PREV_TRACK => 0x1008ff16, // `XF86XK_AudioPrev`
        k::OBS_KEY_VK_MEDIA_NEXT_TRACK => 0x1008ff17, // `XF86XK_AudioNext`
        k::OBS_KEY_VK_VOLUME_MUTE => 0x1008ff12,      // `XF86XK_AudioMute`
        k::OBS_KEY_VK_VOLUME_DOWN => 0x1008ff11,      // `XF86XK_AudioLowerVolume`
        k::OBS_KEY_VK_VOLUME_UP => 0x1008ff13,        // `XF86XK_AudioRaiseVolume`

        // TODO: Implement keys for non-US keyboards (as in the C).
        _ => 0,
    }
}

/// The slot an obs mouse key reads in the per-button arrays: Mouse 1 slot 0
/// (button 1), Mouse 2 slot 2 (the right button, 3), Mouse 3 slot 1 (the
/// middle button, 2), Mouse 4 to 29 slots 7 to 32 (buttons 8 to 33; buttons
/// 4 to 7, the wheel, have no key). `None` for any other key.
pub fn slot(key: i32) -> Option<usize> {
    // Mouse 2 for OBS is Right Click and Mouse 3 is Wheel Click.
    // Mouse Wheel axis clicks (xinput mot->detail 4 5 6 7) are ignored.
    match key {
        k::OBS_KEY_MOUSE1 => Some(0),
        k::OBS_KEY_MOUSE2 => Some(2),
        k::OBS_KEY_MOUSE3 => Some(1),
        k::OBS_KEY_MOUSE4 => Some(7),
        k::OBS_KEY_MOUSE5 => Some(8),
        k::OBS_KEY_MOUSE6 => Some(9),
        k::OBS_KEY_MOUSE7 => Some(10),
        k::OBS_KEY_MOUSE8 => Some(11),
        k::OBS_KEY_MOUSE9 => Some(12),
        k::OBS_KEY_MOUSE10 => Some(13),
        k::OBS_KEY_MOUSE11 => Some(14),
        k::OBS_KEY_MOUSE12 => Some(15),
        k::OBS_KEY_MOUSE13 => Some(16),
        k::OBS_KEY_MOUSE14 => Some(17),
        k::OBS_KEY_MOUSE15 => Some(18),
        k::OBS_KEY_MOUSE16 => Some(19),
        k::OBS_KEY_MOUSE17 => Some(20),
        k::OBS_KEY_MOUSE18 => Some(21),
        k::OBS_KEY_MOUSE19 => Some(22),
        k::OBS_KEY_MOUSE20 => Some(23),
        k::OBS_KEY_MOUSE21 => Some(24),
        k::OBS_KEY_MOUSE22 => Some(25),
        k::OBS_KEY_MOUSE23 => Some(26),
        k::OBS_KEY_MOUSE24 => Some(27),
        k::OBS_KEY_MOUSE25 => Some(28),
        k::OBS_KEY_MOUSE26 => Some(29),
        k::OBS_KEY_MOUSE27 => Some(30),
        k::OBS_KEY_MOUSE28 => Some(31),
        k::OBS_KEY_MOUSE29 => Some(32),
        _ => None,
    }
}

/// `mouse_button_pressed()`'s answer with XInput: the key's button pressed
/// in this poll or held from an earlier one; false for a key that isn't a
/// mouse key. `held` is read only when the button wasn't pressed in this
/// poll.
pub fn mouse_pressed(key: i32, pressed: &[bool; MOUSE_LEN], held: &[bool; MOUSE_LEN]) -> bool {
    slot(key).is_some_and(|s| {
        pressed.get(s).copied().unwrap_or(false) || held.get(s).copied().unwrap_or(false)
    })
}

/// `mouse_button_pressed()`'s commit with XInput: each slot updated in this
/// poll is held when it was pressed in this poll and released otherwise;
/// every other slot keeps its held state.
pub fn commit(
    update: &[bool; MOUSE_LEN],
    pressed: &[bool; MOUSE_LEN],
    held: &mut [bool; MOUSE_LEN],
) {
    // A slot's pressed flag is read only when it was updated, as in C.
    for ((held, &update), pressed) in held.iter_mut().zip(update).zip(pressed) {
        if update {
            *held = *pressed;
        }
    }
}

/// `mouse_button_pressed()`'s answer without XInput: whether the key's
/// button bit is set in `buttons`, a QueryPointer reply's mask. Mouse 1 is
/// [`BUTTON_MASK_1`], Mouse 2 [`BUTTON_MASK_3`] and Mouse 3
/// [`BUTTON_MASK_2`]; any other key is never pressed.
pub fn mask_pressed(key: i32, buttons: u16) -> bool {
    let bit = match key {
        k::OBS_KEY_MOUSE1 => BUTTON_MASK_1,
        k::OBS_KEY_MOUSE2 => BUTTON_MASK_3,
        k::OBS_KEY_MOUSE3 => BUTTON_MASK_2,
        _ => return false,
    };
    buttons & bit != 0
}
