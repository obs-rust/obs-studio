//! Tier 1: the safe core of `libobs/obs-nix-wayland-keys.c.h`.
//!
//! The first eleven tests port `test/cmocka/test_nix_wayland_keys_steps.c`
//! one to one, with the same names, inputs and expectations, in the same
//! order. Each starts from the C test's `clear_maps()`: [`Maps::new`].
//!
//! One intentional difference changes the four focused-key cases: the core
//! records the key each keycode was pressed as and releases that key, where
//! the C leaves the record alone and looks a release up again. Their C
//! teardown (`record_untouched`) becomes a check of the record, and
//! `focused_across_layout_change` releases the key that was pressed.
//!
//! The cases after those pin the edges of each function, and the proptests
//! check the row and focused-key rules against models written from the C
//! and the fix.

#[allow(clippy::wildcard_imports)] // the C test names every key and keysym
use obs_hotkey::name_map::*;
#[allow(clippy::wildcard_imports)]
use obs_hotkey::nix_wayland_keys::keysyms::*;
use obs_hotkey::nix_wayland_keys::{
    self as wk, MAX_KEYCODES, MAX_SHIFT_LEVELS, OBS_KEY_LAST_VALUE,
};
use proptest::prelude::*;

/// Every case of the keysym-to-key switch, in its order.
const FROM_TABLE: &[(u32, i32)] = &[
    (XKB_KEY_0, OBS_KEY_0),
    (XKB_KEY_1, OBS_KEY_1),
    (XKB_KEY_2, OBS_KEY_2),
    (XKB_KEY_3, OBS_KEY_3),
    (XKB_KEY_4, OBS_KEY_4),
    (XKB_KEY_5, OBS_KEY_5),
    (XKB_KEY_6, OBS_KEY_6),
    (XKB_KEY_7, OBS_KEY_7),
    (XKB_KEY_8, OBS_KEY_8),
    (XKB_KEY_9, OBS_KEY_9),
    (XKB_KEY_A, OBS_KEY_A),
    (XKB_KEY_a, OBS_KEY_A),
    (XKB_KEY_Aacute, OBS_KEY_AACUTE),
    (XKB_KEY_aacute, OBS_KEY_AACUTE),
    (XKB_KEY_Acircumflex, OBS_KEY_ACIRCUMFLEX),
    (XKB_KEY_acircumflex, OBS_KEY_ACIRCUMFLEX),
    (XKB_KEY_acute, OBS_KEY_ACUTE),
    (XKB_KEY_Adiaeresis, OBS_KEY_ADIAERESIS),
    (XKB_KEY_adiaeresis, OBS_KEY_ADIAERESIS),
    (XKB_KEY_AE, OBS_KEY_AE),
    (XKB_KEY_ae, OBS_KEY_AE),
    (XKB_KEY_Agrave, OBS_KEY_AGRAVE),
    (XKB_KEY_agrave, OBS_KEY_AGRAVE),
    (XKB_KEY_ampersand, OBS_KEY_AMPERSAND),
    (XKB_KEY_apostrophe, OBS_KEY_APOSTROPHE),
    (XKB_KEY_Aring, OBS_KEY_ARING),
    (XKB_KEY_aring, OBS_KEY_ARING),
    (XKB_KEY_asciicircum, OBS_KEY_ASCIICIRCUM),
    (XKB_KEY_asciitilde, OBS_KEY_ASCIITILDE),
    (XKB_KEY_asterisk, OBS_KEY_ASTERISK),
    (XKB_KEY_at, OBS_KEY_AT),
    (XKB_KEY_Atilde, OBS_KEY_ATILDE),
    (XKB_KEY_atilde, OBS_KEY_ATILDE),
    (XKB_KEY_B, OBS_KEY_B),
    (XKB_KEY_b, OBS_KEY_B),
    (XKB_KEY_backslash, OBS_KEY_BACKSLASH),
    (XKB_KEY_BackSpace, OBS_KEY_BACKSPACE),
    (XKB_KEY_BackTab, OBS_KEY_BACKTAB),
    (XKB_KEY_bar, OBS_KEY_BAR),
    (XKB_KEY_braceleft, OBS_KEY_BRACELEFT),
    (XKB_KEY_braceright, OBS_KEY_BRACERIGHT),
    (XKB_KEY_bracketleft, OBS_KEY_BRACKETLEFT),
    (XKB_KEY_bracketright, OBS_KEY_BRACKETRIGHT),
    (XKB_KEY_brokenbar, OBS_KEY_BROKENBAR),
    (XKB_KEY_C, OBS_KEY_C),
    (XKB_KEY_c, OBS_KEY_C),
    (XKB_KEY_Cancel, OBS_KEY_CANCEL),
    (XKB_KEY_Ccedilla, OBS_KEY_CCEDILLA),
    (XKB_KEY_ccedilla, OBS_KEY_CCEDILLA),
    (XKB_KEY_cedilla, OBS_KEY_CEDILLA),
    (XKB_KEY_cent, OBS_KEY_CENT),
    (XKB_KEY_Clear, OBS_KEY_CLEAR),
    (XKB_KEY_Codeinput, OBS_KEY_CODEINPUT),
    (XKB_KEY_colon, OBS_KEY_COLON),
    (XKB_KEY_comma, OBS_KEY_COMMA),
    (XKB_KEY_copyright, OBS_KEY_COPYRIGHT),
    (XKB_KEY_currency, OBS_KEY_CURRENCY),
    (XKB_KEY_D, OBS_KEY_D),
    (XKB_KEY_d, OBS_KEY_D),
    (XKB_KEY_dead_abovedot, OBS_KEY_DEAD_ABOVEDOT),
    (XKB_KEY_dead_abovering, OBS_KEY_DEAD_ABOVERING),
    (XKB_KEY_dead_acute, OBS_KEY_DEAD_ACUTE),
    (XKB_KEY_dead_belowdot, OBS_KEY_DEAD_BELOWDOT),
    (XKB_KEY_dead_breve, OBS_KEY_DEAD_BREVE),
    (XKB_KEY_dead_caron, OBS_KEY_DEAD_CARON),
    (XKB_KEY_dead_cedilla, OBS_KEY_DEAD_CEDILLA),
    (XKB_KEY_dead_circumflex, OBS_KEY_DEAD_CIRCUMFLEX),
    (XKB_KEY_dead_diaeresis, OBS_KEY_DEAD_DIAERESIS),
    (XKB_KEY_dead_doubleacute, OBS_KEY_DEAD_DOUBLEACUTE),
    (XKB_KEY_dead_grave, OBS_KEY_DEAD_GRAVE),
    (XKB_KEY_dead_hook, OBS_KEY_DEAD_HOOK),
    (XKB_KEY_dead_horn, OBS_KEY_DEAD_HORN),
    (XKB_KEY_dead_iota, OBS_KEY_DEAD_IOTA),
    (XKB_KEY_dead_macron, OBS_KEY_DEAD_MACRON),
    (XKB_KEY_dead_ogonek, OBS_KEY_DEAD_OGONEK),
    (XKB_KEY_dead_semivoiced_sound, OBS_KEY_DEAD_SEMIVOICED_SOUND),
    (XKB_KEY_dead_tilde, OBS_KEY_DEAD_TILDE),
    (XKB_KEY_dead_voiced_sound, OBS_KEY_DEAD_VOICED_SOUND),
    (XKB_KEY_degree, OBS_KEY_DEGREE),
    (XKB_KEY_Delete, OBS_KEY_DELETE),
    (XKB_KEY_diaeresis, OBS_KEY_DIAERESIS),
    (XKB_KEY_division, OBS_KEY_DIVISION),
    (XKB_KEY_dollar, OBS_KEY_DOLLAR),
    (XKB_KEY_Down, OBS_KEY_DOWN),
    (XKB_KEY_E, OBS_KEY_E),
    (XKB_KEY_e, OBS_KEY_E),
    (XKB_KEY_Eacute, OBS_KEY_EACUTE),
    (XKB_KEY_eacute, OBS_KEY_EACUTE),
    (XKB_KEY_Ecircumflex, OBS_KEY_ECIRCUMFLEX),
    (XKB_KEY_ecircumflex, OBS_KEY_ECIRCUMFLEX),
    (XKB_KEY_Ediaeresis, OBS_KEY_EDIAERESIS),
    (XKB_KEY_ediaeresis, OBS_KEY_EDIAERESIS),
    (XKB_KEY_Egrave, OBS_KEY_EGRAVE),
    (XKB_KEY_egrave, OBS_KEY_EGRAVE),
    (XKB_KEY_Eisu_Shift, OBS_KEY_EISU_SHIFT),
    (XKB_KEY_Eisu_toggle, OBS_KEY_EISU_TOGGLE),
    (XKB_KEY_End, OBS_KEY_END),
    (XKB_KEY_equal, OBS_KEY_EQUAL),
    (XKB_KEY_Escape, OBS_KEY_ESCAPE),
    (XKB_KEY_Eth, OBS_KEY_ETH),
    (XKB_KEY_eth, OBS_KEY_ETH),
    (XKB_KEY_exclam, OBS_KEY_EXCLAM),
    (XKB_KEY_exclamdown, OBS_KEY_EXCLAMDOWN),
    (XKB_KEY_Execute, OBS_KEY_EXECUTE),
    (XKB_KEY_F, OBS_KEY_F),
    (XKB_KEY_f, OBS_KEY_F),
    (XKB_KEY_F1, OBS_KEY_F1),
    (XKB_KEY_F10, OBS_KEY_F10),
    (XKB_KEY_F11, OBS_KEY_F11),
    (XKB_KEY_F12, OBS_KEY_F12),
    (XKB_KEY_F13, OBS_KEY_F13),
    (XKB_KEY_F14, OBS_KEY_F14),
    (XKB_KEY_F15, OBS_KEY_F15),
    (XKB_KEY_F16, OBS_KEY_F16),
    (XKB_KEY_F17, OBS_KEY_F17),
    (XKB_KEY_F18, OBS_KEY_F18),
    (XKB_KEY_F19, OBS_KEY_F19),
    (XKB_KEY_F2, OBS_KEY_F2),
    (XKB_KEY_F20, OBS_KEY_F20),
    (XKB_KEY_F21, OBS_KEY_F21),
    (XKB_KEY_F22, OBS_KEY_F22),
    (XKB_KEY_F23, OBS_KEY_F23),
    (XKB_KEY_F24, OBS_KEY_F24),
    (XKB_KEY_F25, OBS_KEY_F25),
    (XKB_KEY_F26, OBS_KEY_F26),
    (XKB_KEY_F27, OBS_KEY_F27),
    (XKB_KEY_F28, OBS_KEY_F28),
    (XKB_KEY_F29, OBS_KEY_F29),
    (XKB_KEY_F3, OBS_KEY_F3),
    (XKB_KEY_F30, OBS_KEY_F30),
    (XKB_KEY_F31, OBS_KEY_F31),
    (XKB_KEY_F32, OBS_KEY_F32),
    (XKB_KEY_F33, OBS_KEY_F33),
    (XKB_KEY_F34, OBS_KEY_F34),
    (XKB_KEY_F35, OBS_KEY_F35),
    (XKB_KEY_F4, OBS_KEY_F4),
    (XKB_KEY_F5, OBS_KEY_F5),
    (XKB_KEY_F6, OBS_KEY_F6),
    (XKB_KEY_F7, OBS_KEY_F7),
    (XKB_KEY_F8, OBS_KEY_F8),
    (XKB_KEY_F9, OBS_KEY_F9),
    (XKB_KEY_Find, OBS_KEY_FIND),
    (XKB_KEY_G, OBS_KEY_G),
    (XKB_KEY_g, OBS_KEY_G),
    (XKB_KEY_greater, OBS_KEY_GREATER),
    (XKB_KEY_guillemotleft, OBS_KEY_GUILLEMOTLEFT),
    (XKB_KEY_guillemotright, OBS_KEY_GUILLEMOTRIGHT),
    (XKB_KEY_H, OBS_KEY_H),
    (XKB_KEY_h, OBS_KEY_H),
    (XKB_KEY_Hangul, OBS_KEY_HANGUL),
    (XKB_KEY_Hangul_Banja, OBS_KEY_HANGUL_BANJA),
    (XKB_KEY_Hangul_End, OBS_KEY_HANGUL_END),
    (XKB_KEY_Hangul_Hanja, OBS_KEY_HANGUL_HANJA),
    (XKB_KEY_Hangul_Jamo, OBS_KEY_HANGUL_JAMO),
    (XKB_KEY_Hangul_Jeonja, OBS_KEY_HANGUL_JEONJA),
    (XKB_KEY_Hangul_PostHanja, OBS_KEY_HANGUL_POSTHANJA),
    (XKB_KEY_Hangul_PreHanja, OBS_KEY_HANGUL_PREHANJA),
    (XKB_KEY_Hangul_Romaja, OBS_KEY_HANGUL_ROMAJA),
    (XKB_KEY_Hangul_Special, OBS_KEY_HANGUL_SPECIAL),
    (XKB_KEY_Hangul_Start, OBS_KEY_HANGUL_START),
    (XKB_KEY_Hankaku, OBS_KEY_HANKAKU),
    (XKB_KEY_Help, OBS_KEY_HELP),
    (XKB_KEY_Henkan, OBS_KEY_HENKAN),
    (XKB_KEY_Hiragana, OBS_KEY_HIRAGANA),
    (XKB_KEY_Hiragana_Katakana, OBS_KEY_HIRAGANA_KATAKANA),
    (XKB_KEY_Home, OBS_KEY_HOME),
    (XKB_KEY_Hyper_L, OBS_KEY_HYPER_L),
    (XKB_KEY_Hyper_R, OBS_KEY_HYPER_R),
    (XKB_KEY_hyphen, OBS_KEY_HYPHEN),
    (XKB_KEY_I, OBS_KEY_I),
    (XKB_KEY_i, OBS_KEY_I),
    (XKB_KEY_Iacute, OBS_KEY_IACUTE),
    (XKB_KEY_iacute, OBS_KEY_IACUTE),
    (XKB_KEY_Icircumflex, OBS_KEY_ICIRCUMFLEX),
    (XKB_KEY_icircumflex, OBS_KEY_ICIRCUMFLEX),
    (XKB_KEY_Idiaeresis, OBS_KEY_IDIAERESIS),
    (XKB_KEY_idiaeresis, OBS_KEY_IDIAERESIS),
    (XKB_KEY_Igrave, OBS_KEY_IGRAVE),
    (XKB_KEY_igrave, OBS_KEY_IGRAVE),
    (XKB_KEY_Insert, OBS_KEY_INSERT),
    (XKB_KEY_J, OBS_KEY_J),
    (XKB_KEY_j, OBS_KEY_J),
    (XKB_KEY_K, OBS_KEY_K),
    (XKB_KEY_k, OBS_KEY_K),
    (XKB_KEY_Kana_Lock, OBS_KEY_KANA_LOCK),
    (XKB_KEY_Kana_Shift, OBS_KEY_KANA_SHIFT),
    (XKB_KEY_Kanji, OBS_KEY_KANJI),
    (XKB_KEY_Katakana, OBS_KEY_KATAKANA),
    (XKB_KEY_L, OBS_KEY_L),
    (XKB_KEY_l, OBS_KEY_L),
    (XKB_KEY_Left, OBS_KEY_LEFT),
    (XKB_KEY_less, OBS_KEY_LESS),
    (XKB_KEY_M, OBS_KEY_M),
    (XKB_KEY_m, OBS_KEY_M),
    (XKB_KEY_macron, OBS_KEY_MACRON),
    (XKB_KEY_masculine, OBS_KEY_MASCULINE),
    (XKB_KEY_Massyo, OBS_KEY_MASSYO),
    (XKB_KEY_Menu, OBS_KEY_MENU),
    (XKB_KEY_minus, OBS_KEY_MINUS),
    (XKB_KEY_Mode_switch, OBS_KEY_MODE_SWITCH),
    (XKB_KEY_mu, OBS_KEY_MU),
    (XKB_KEY_Muhenkan, OBS_KEY_MUHENKAN),
    (XKB_KEY_MultipleCandidate, OBS_KEY_MULTIPLECANDIDATE),
    (XKB_KEY_multiply, OBS_KEY_MULTIPLY),
    (XKB_KEY_Multi_key, OBS_KEY_MULTI_KEY),
    (XKB_KEY_N, OBS_KEY_N),
    (XKB_KEY_n, OBS_KEY_N),
    (XKB_KEY_nobreakspace, OBS_KEY_NOBREAKSPACE),
    (XKB_KEY_notsign, OBS_KEY_NOTSIGN),
    (XKB_KEY_Ntilde, OBS_KEY_NTILDE),
    (XKB_KEY_ntilde, OBS_KEY_NTILDE),
    (XKB_KEY_numbersign, OBS_KEY_NUMBERSIGN),
    (XKB_KEY_O, OBS_KEY_O),
    (XKB_KEY_o, OBS_KEY_O),
    (XKB_KEY_Oacute, OBS_KEY_OACUTE),
    (XKB_KEY_oacute, OBS_KEY_OACUTE),
    (XKB_KEY_Ocircumflex, OBS_KEY_OCIRCUMFLEX),
    (XKB_KEY_ocircumflex, OBS_KEY_OCIRCUMFLEX),
    (XKB_KEY_Odiaeresis, OBS_KEY_ODIAERESIS),
    (XKB_KEY_odiaeresis, OBS_KEY_ODIAERESIS),
    (XKB_KEY_Ograve, OBS_KEY_OGRAVE),
    (XKB_KEY_ograve, OBS_KEY_OGRAVE),
    (XKB_KEY_onehalf, OBS_KEY_ONEHALF),
    (XKB_KEY_onequarter, OBS_KEY_ONEQUARTER),
    (XKB_KEY_onesuperior, OBS_KEY_ONESUPERIOR),
    (XKB_KEY_Ooblique, OBS_KEY_OOBLIQUE),
    (XKB_KEY_ooblique, OBS_KEY_OOBLIQUE),
    (XKB_KEY_ordfeminine, OBS_KEY_ORDFEMININE),
    (XKB_KEY_Otilde, OBS_KEY_OTILDE),
    (XKB_KEY_otilde, OBS_KEY_OTILDE),
    (XKB_KEY_P, OBS_KEY_P),
    (XKB_KEY_p, OBS_KEY_P),
    (XKB_KEY_paragraph, OBS_KEY_PARAGRAPH),
    (XKB_KEY_parenleft, OBS_KEY_PARENLEFT),
    (XKB_KEY_parenright, OBS_KEY_PARENRIGHT),
    (XKB_KEY_Pause, OBS_KEY_PAUSE),
    (XKB_KEY_percent, OBS_KEY_PERCENT),
    (XKB_KEY_period, OBS_KEY_PERIOD),
    (XKB_KEY_periodcentered, OBS_KEY_PERIODCENTERED),
    (XKB_KEY_plus, OBS_KEY_PLUS),
    (XKB_KEY_plusminus, OBS_KEY_PLUSMINUS),
    (XKB_KEY_PreviousCandidate, OBS_KEY_PREVIOUSCANDIDATE),
    (XKB_KEY_Print, OBS_KEY_PRINT),
    (XKB_KEY_Q, OBS_KEY_Q),
    (XKB_KEY_q, OBS_KEY_Q),
    (XKB_KEY_question, OBS_KEY_QUESTION),
    (XKB_KEY_questiondown, OBS_KEY_QUESTIONDOWN),
    (XKB_KEY_quotedbl, OBS_KEY_QUOTEDBL),
    (XKB_KEY_quoteleft, OBS_KEY_QUOTELEFT),
    (XKB_KEY_R, OBS_KEY_R),
    (XKB_KEY_r, OBS_KEY_R),
    (XKB_KEY_Redo, OBS_KEY_REDO),
    (XKB_KEY_registered, OBS_KEY_REGISTERED),
    (XKB_KEY_Return, OBS_KEY_RETURN),
    (XKB_KEY_Right, OBS_KEY_RIGHT),
    (XKB_KEY_Romaji, OBS_KEY_ROMAJI),
    (XKB_KEY_S, OBS_KEY_S),
    (XKB_KEY_s, OBS_KEY_S),
    (XKB_KEY_section, OBS_KEY_SECTION),
    (XKB_KEY_Select, OBS_KEY_SELECT),
    (XKB_KEY_semicolon, OBS_KEY_SEMICOLON),
    (XKB_KEY_SingleCandidate, OBS_KEY_SINGLECANDIDATE),
    (XKB_KEY_slash, OBS_KEY_SLASH),
    (XKB_KEY_space, OBS_KEY_SPACE),
    (XKB_KEY_ssharp, OBS_KEY_SSHARP),
    (XKB_KEY_sterling, OBS_KEY_STERLING),
    (XKB_KEY_T, OBS_KEY_T),
    (XKB_KEY_t, OBS_KEY_T),
    (XKB_KEY_Tab, OBS_KEY_TAB),
    (XKB_KEY_Thorn, OBS_KEY_THORN),
    (XKB_KEY_thorn, OBS_KEY_THORN),
    (XKB_KEY_threequarters, OBS_KEY_THREEQUARTERS),
    (XKB_KEY_threesuperior, OBS_KEY_THREESUPERIOR),
    (XKB_KEY_Touroku, OBS_KEY_TOUROKU),
    (XKB_KEY_twosuperior, OBS_KEY_TWOSUPERIOR),
    (XKB_KEY_U, OBS_KEY_U),
    (XKB_KEY_u, OBS_KEY_U),
    (XKB_KEY_Uacute, OBS_KEY_UACUTE),
    (XKB_KEY_uacute, OBS_KEY_UACUTE),
    (XKB_KEY_Ucircumflex, OBS_KEY_UCIRCUMFLEX),
    (XKB_KEY_ucircumflex, OBS_KEY_UCIRCUMFLEX),
    (XKB_KEY_Udiaeresis, OBS_KEY_UDIAERESIS),
    (XKB_KEY_udiaeresis, OBS_KEY_UDIAERESIS),
    (XKB_KEY_Ugrave, OBS_KEY_UGRAVE),
    (XKB_KEY_ugrave, OBS_KEY_UGRAVE),
    (XKB_KEY_underscore, OBS_KEY_UNDERSCORE),
    (XKB_KEY_Undo, OBS_KEY_UNDO),
    (XKB_KEY_Up, OBS_KEY_UP),
    (XKB_KEY_V, OBS_KEY_V),
    (XKB_KEY_v, OBS_KEY_V),
    (XKB_KEY_W, OBS_KEY_W),
    (XKB_KEY_w, OBS_KEY_W),
    (XKB_KEY_X, OBS_KEY_X),
    (XKB_KEY_x, OBS_KEY_X),
    (XKB_KEY_Y, OBS_KEY_Y),
    (XKB_KEY_y, OBS_KEY_Y),
    (XKB_KEY_Yacute, OBS_KEY_YACUTE),
    (XKB_KEY_yacute, OBS_KEY_YACUTE),
    (XKB_KEY_Ydiaeresis, OBS_KEY_YDIAERESIS),
    (XKB_KEY_ydiaeresis, OBS_KEY_YDIAERESIS),
    (XKB_KEY_yen, OBS_KEY_YEN),
    (XKB_KEY_Z, OBS_KEY_Z),
    (XKB_KEY_z, OBS_KEY_Z),
    (XKB_KEY_Zenkaku, OBS_KEY_ZENKAKU),
    (XKB_KEY_Zenkaku_Hankaku, OBS_KEY_ZENKAKU_HANKAKU),
    (XKB_KEY_Page_Up, OBS_KEY_PAGEUP),
    (XKB_KEY_Page_Down, OBS_KEY_PAGEDOWN),
    (XKB_KEY_KP_Equal, OBS_KEY_NUMEQUAL),
    (XKB_KEY_KP_Multiply, OBS_KEY_NUMASTERISK),
    (XKB_KEY_KP_Add, OBS_KEY_NUMPLUS),
    (XKB_KEY_KP_Separator, OBS_KEY_NUMCOMMA),
    (XKB_KEY_KP_Subtract, OBS_KEY_NUMMINUS),
    (XKB_KEY_KP_Decimal, OBS_KEY_NUMPERIOD),
    (XKB_KEY_KP_Divide, OBS_KEY_NUMSLASH),
    (XKB_KEY_KP_Enter, OBS_KEY_ENTER),
    (XKB_KEY_KP_0, OBS_KEY_NUM0),
    (XKB_KEY_KP_1, OBS_KEY_NUM1),
    (XKB_KEY_KP_2, OBS_KEY_NUM2),
    (XKB_KEY_KP_3, OBS_KEY_NUM3),
    (XKB_KEY_KP_4, OBS_KEY_NUM4),
    (XKB_KEY_KP_5, OBS_KEY_NUM5),
    (XKB_KEY_KP_6, OBS_KEY_NUM6),
    (XKB_KEY_KP_7, OBS_KEY_NUM7),
    (XKB_KEY_KP_8, OBS_KEY_NUM8),
    (XKB_KEY_KP_9, OBS_KEY_NUM9),
    (XKB_KEY_XF86AudioPlay, OBS_KEY_VK_MEDIA_PLAY_PAUSE),
    (XKB_KEY_XF86AudioStop, OBS_KEY_VK_MEDIA_STOP),
    (XKB_KEY_XF86AudioPrev, OBS_KEY_VK_MEDIA_PREV_TRACK),
    (XKB_KEY_XF86AudioNext, OBS_KEY_VK_MEDIA_NEXT_TRACK),
    (XKB_KEY_XF86AudioMute, OBS_KEY_VK_VOLUME_MUTE),
    (XKB_KEY_XF86AudioRaiseVolume, OBS_KEY_VK_VOLUME_UP),
    (XKB_KEY_XF86AudioLowerVolume, OBS_KEY_VK_VOLUME_DOWN),
];

/// Every case of the key-to-keysym switch, in its order.
const TO_TABLE: &[(i32, u32)] = &[
    (OBS_KEY_0, XKB_KEY_0),
    (OBS_KEY_1, XKB_KEY_1),
    (OBS_KEY_2, XKB_KEY_2),
    (OBS_KEY_3, XKB_KEY_3),
    (OBS_KEY_4, XKB_KEY_4),
    (OBS_KEY_5, XKB_KEY_5),
    (OBS_KEY_6, XKB_KEY_6),
    (OBS_KEY_7, XKB_KEY_7),
    (OBS_KEY_8, XKB_KEY_8),
    (OBS_KEY_9, XKB_KEY_9),
    (OBS_KEY_A, XKB_KEY_A),
    (OBS_KEY_AACUTE, XKB_KEY_Aacute),
    (OBS_KEY_ACIRCUMFLEX, XKB_KEY_Acircumflex),
    (OBS_KEY_ACUTE, XKB_KEY_acute),
    (OBS_KEY_ADIAERESIS, XKB_KEY_Adiaeresis),
    (OBS_KEY_AE, XKB_KEY_AE),
    (OBS_KEY_AGRAVE, XKB_KEY_Agrave),
    (OBS_KEY_AMPERSAND, XKB_KEY_ampersand),
    (OBS_KEY_APOSTROPHE, XKB_KEY_apostrophe),
    (OBS_KEY_ARING, XKB_KEY_Aring),
    (OBS_KEY_ASCIICIRCUM, XKB_KEY_asciicircum),
    (OBS_KEY_ASCIITILDE, XKB_KEY_asciitilde),
    (OBS_KEY_ASTERISK, XKB_KEY_asterisk),
    (OBS_KEY_AT, XKB_KEY_at),
    (OBS_KEY_ATILDE, XKB_KEY_Atilde),
    (OBS_KEY_B, XKB_KEY_B),
    (OBS_KEY_BACKSLASH, XKB_KEY_backslash),
    (OBS_KEY_BACKSPACE, XKB_KEY_BackSpace),
    (OBS_KEY_BACKTAB, XKB_KEY_BackTab),
    (OBS_KEY_BAR, XKB_KEY_bar),
    (OBS_KEY_BRACELEFT, XKB_KEY_braceleft),
    (OBS_KEY_BRACERIGHT, XKB_KEY_braceright),
    (OBS_KEY_BRACKETLEFT, XKB_KEY_bracketleft),
    (OBS_KEY_BRACKETRIGHT, XKB_KEY_bracketright),
    (OBS_KEY_BROKENBAR, XKB_KEY_brokenbar),
    (OBS_KEY_C, XKB_KEY_C),
    (OBS_KEY_CANCEL, XKB_KEY_Cancel),
    (OBS_KEY_CCEDILLA, XKB_KEY_Ccedilla),
    (OBS_KEY_CEDILLA, XKB_KEY_cedilla),
    (OBS_KEY_CENT, XKB_KEY_cent),
    (OBS_KEY_CLEAR, XKB_KEY_Clear),
    (OBS_KEY_CODEINPUT, XKB_KEY_Codeinput),
    (OBS_KEY_COLON, XKB_KEY_colon),
    (OBS_KEY_COMMA, XKB_KEY_comma),
    (OBS_KEY_COPYRIGHT, XKB_KEY_copyright),
    (OBS_KEY_CURRENCY, XKB_KEY_currency),
    (OBS_KEY_D, XKB_KEY_D),
    (OBS_KEY_DEAD_ABOVEDOT, XKB_KEY_dead_abovedot),
    (OBS_KEY_DEAD_ABOVERING, XKB_KEY_dead_abovering),
    (OBS_KEY_DEAD_ACUTE, XKB_KEY_dead_acute),
    (OBS_KEY_DEAD_BELOWDOT, XKB_KEY_dead_belowdot),
    (OBS_KEY_DEAD_BREVE, XKB_KEY_dead_breve),
    (OBS_KEY_DEAD_CARON, XKB_KEY_dead_caron),
    (OBS_KEY_DEAD_CEDILLA, XKB_KEY_dead_cedilla),
    (OBS_KEY_DEAD_CIRCUMFLEX, XKB_KEY_dead_circumflex),
    (OBS_KEY_DEAD_DIAERESIS, XKB_KEY_dead_diaeresis),
    (OBS_KEY_DEAD_DOUBLEACUTE, XKB_KEY_dead_doubleacute),
    (OBS_KEY_DEAD_GRAVE, XKB_KEY_dead_grave),
    (OBS_KEY_DEAD_HOOK, XKB_KEY_dead_hook),
    (OBS_KEY_DEAD_HORN, XKB_KEY_dead_horn),
    (OBS_KEY_DEAD_IOTA, XKB_KEY_dead_iota),
    (OBS_KEY_DEAD_MACRON, XKB_KEY_dead_macron),
    (OBS_KEY_DEAD_OGONEK, XKB_KEY_dead_ogonek),
    (OBS_KEY_DEAD_SEMIVOICED_SOUND, XKB_KEY_dead_semivoiced_sound),
    (OBS_KEY_DEAD_TILDE, XKB_KEY_dead_tilde),
    (OBS_KEY_DEAD_VOICED_SOUND, XKB_KEY_dead_voiced_sound),
    (OBS_KEY_DEGREE, XKB_KEY_degree),
    (OBS_KEY_DELETE, XKB_KEY_Delete),
    (OBS_KEY_DIAERESIS, XKB_KEY_diaeresis),
    (OBS_KEY_DIVISION, XKB_KEY_division),
    (OBS_KEY_DOLLAR, XKB_KEY_dollar),
    (OBS_KEY_DOWN, XKB_KEY_Down),
    (OBS_KEY_E, XKB_KEY_E),
    (OBS_KEY_EACUTE, XKB_KEY_Eacute),
    (OBS_KEY_ECIRCUMFLEX, XKB_KEY_Ecircumflex),
    (OBS_KEY_EDIAERESIS, XKB_KEY_Ediaeresis),
    (OBS_KEY_EGRAVE, XKB_KEY_Egrave),
    (OBS_KEY_EISU_SHIFT, XKB_KEY_Eisu_Shift),
    (OBS_KEY_EISU_TOGGLE, XKB_KEY_Eisu_toggle),
    (OBS_KEY_END, XKB_KEY_End),
    (OBS_KEY_EQUAL, XKB_KEY_equal),
    (OBS_KEY_ESCAPE, XKB_KEY_Escape),
    (OBS_KEY_ETH, XKB_KEY_ETH),
    (OBS_KEY_EXCLAM, XKB_KEY_exclam),
    (OBS_KEY_EXCLAMDOWN, XKB_KEY_exclamdown),
    (OBS_KEY_EXECUTE, XKB_KEY_Execute),
    (OBS_KEY_F, XKB_KEY_F),
    (OBS_KEY_F1, XKB_KEY_F1),
    (OBS_KEY_F10, XKB_KEY_F10),
    (OBS_KEY_F11, XKB_KEY_F11),
    (OBS_KEY_F12, XKB_KEY_F12),
    (OBS_KEY_F13, XKB_KEY_F13),
    (OBS_KEY_F14, XKB_KEY_F14),
    (OBS_KEY_F15, XKB_KEY_F15),
    (OBS_KEY_F16, XKB_KEY_F16),
    (OBS_KEY_F17, XKB_KEY_F17),
    (OBS_KEY_F18, XKB_KEY_F18),
    (OBS_KEY_F19, XKB_KEY_F19),
    (OBS_KEY_F2, XKB_KEY_F2),
    (OBS_KEY_F20, XKB_KEY_F20),
    (OBS_KEY_F21, XKB_KEY_F21),
    (OBS_KEY_F22, XKB_KEY_F22),
    (OBS_KEY_F23, XKB_KEY_F23),
    (OBS_KEY_F24, XKB_KEY_F24),
    (OBS_KEY_F25, XKB_KEY_F25),
    (OBS_KEY_F26, XKB_KEY_F26),
    (OBS_KEY_F27, XKB_KEY_F27),
    (OBS_KEY_F28, XKB_KEY_F28),
    (OBS_KEY_F29, XKB_KEY_F29),
    (OBS_KEY_F3, XKB_KEY_F3),
    (OBS_KEY_F30, XKB_KEY_F30),
    (OBS_KEY_F31, XKB_KEY_F31),
    (OBS_KEY_F32, XKB_KEY_F32),
    (OBS_KEY_F33, XKB_KEY_F33),
    (OBS_KEY_F34, XKB_KEY_F34),
    (OBS_KEY_F35, XKB_KEY_F35),
    (OBS_KEY_F4, XKB_KEY_F4),
    (OBS_KEY_F5, XKB_KEY_F5),
    (OBS_KEY_F6, XKB_KEY_F6),
    (OBS_KEY_F7, XKB_KEY_F7),
    (OBS_KEY_F8, XKB_KEY_F8),
    (OBS_KEY_F9, XKB_KEY_F9),
    (OBS_KEY_FIND, XKB_KEY_Find),
    (OBS_KEY_G, XKB_KEY_G),
    (OBS_KEY_GREATER, XKB_KEY_greater),
    (OBS_KEY_GUILLEMOTLEFT, XKB_KEY_guillemotleft),
    (OBS_KEY_GUILLEMOTRIGHT, XKB_KEY_guillemotright),
    (OBS_KEY_H, XKB_KEY_H),
    (OBS_KEY_HANGUL, XKB_KEY_Hangul),
    (OBS_KEY_HANGUL_BANJA, XKB_KEY_Hangul_Banja),
    (OBS_KEY_HANGUL_END, XKB_KEY_Hangul_End),
    (OBS_KEY_HANGUL_HANJA, XKB_KEY_Hangul_Hanja),
    (OBS_KEY_HANGUL_JAMO, XKB_KEY_Hangul_Jamo),
    (OBS_KEY_HANGUL_JEONJA, XKB_KEY_Hangul_Jeonja),
    (OBS_KEY_HANGUL_POSTHANJA, XKB_KEY_Hangul_PostHanja),
    (OBS_KEY_HANGUL_PREHANJA, XKB_KEY_Hangul_PreHanja),
    (OBS_KEY_HANGUL_ROMAJA, XKB_KEY_Hangul_Romaja),
    (OBS_KEY_HANGUL_SPECIAL, XKB_KEY_Hangul_Special),
    (OBS_KEY_HANGUL_START, XKB_KEY_Hangul_Start),
    (OBS_KEY_HANKAKU, XKB_KEY_Hankaku),
    (OBS_KEY_HELP, XKB_KEY_Help),
    (OBS_KEY_HENKAN, XKB_KEY_Henkan),
    (OBS_KEY_HIRAGANA, XKB_KEY_Hiragana),
    (OBS_KEY_HIRAGANA_KATAKANA, XKB_KEY_Hiragana_Katakana),
    (OBS_KEY_HOME, XKB_KEY_Home),
    (OBS_KEY_HYPER_L, XKB_KEY_Hyper_L),
    (OBS_KEY_HYPER_R, XKB_KEY_Hyper_R),
    (OBS_KEY_HYPHEN, XKB_KEY_hyphen),
    (OBS_KEY_I, XKB_KEY_I),
    (OBS_KEY_IACUTE, XKB_KEY_Iacute),
    (OBS_KEY_ICIRCUMFLEX, XKB_KEY_Icircumflex),
    (OBS_KEY_IDIAERESIS, XKB_KEY_Idiaeresis),
    (OBS_KEY_IGRAVE, XKB_KEY_Igrave),
    (OBS_KEY_INSERT, XKB_KEY_Insert),
    (OBS_KEY_J, XKB_KEY_J),
    (OBS_KEY_K, XKB_KEY_K),
    (OBS_KEY_KANA_LOCK, XKB_KEY_Kana_Lock),
    (OBS_KEY_KANA_SHIFT, XKB_KEY_Kana_Shift),
    (OBS_KEY_KANJI, XKB_KEY_Kanji),
    (OBS_KEY_KATAKANA, XKB_KEY_Katakana),
    (OBS_KEY_L, XKB_KEY_L),
    (OBS_KEY_LEFT, XKB_KEY_Left),
    (OBS_KEY_LESS, XKB_KEY_less),
    (OBS_KEY_M, XKB_KEY_M),
    (OBS_KEY_MACRON, XKB_KEY_macron),
    (OBS_KEY_MASCULINE, XKB_KEY_masculine),
    (OBS_KEY_MASSYO, XKB_KEY_Massyo),
    (OBS_KEY_MENU, XKB_KEY_Menu),
    (OBS_KEY_MINUS, XKB_KEY_minus),
    (OBS_KEY_MODE_SWITCH, XKB_KEY_Mode_switch),
    (OBS_KEY_MU, XKB_KEY_mu),
    (OBS_KEY_MUHENKAN, XKB_KEY_Muhenkan),
    (OBS_KEY_MULTI_KEY, XKB_KEY_Multi_key),
    (OBS_KEY_MULTIPLECANDIDATE, XKB_KEY_MultipleCandidate),
    (OBS_KEY_MULTIPLY, XKB_KEY_multiply),
    (OBS_KEY_N, XKB_KEY_N),
    (OBS_KEY_NOBREAKSPACE, XKB_KEY_nobreakspace),
    (OBS_KEY_NOTSIGN, XKB_KEY_notsign),
    (OBS_KEY_NTILDE, XKB_KEY_Ntilde),
    (OBS_KEY_NUMBERSIGN, XKB_KEY_numbersign),
    (OBS_KEY_O, XKB_KEY_O),
    (OBS_KEY_OACUTE, XKB_KEY_Oacute),
    (OBS_KEY_OCIRCUMFLEX, XKB_KEY_Ocircumflex),
    (OBS_KEY_ODIAERESIS, XKB_KEY_Odiaeresis),
    (OBS_KEY_OGRAVE, XKB_KEY_Ograve),
    (OBS_KEY_ONEHALF, XKB_KEY_onehalf),
    (OBS_KEY_ONEQUARTER, XKB_KEY_onequarter),
    (OBS_KEY_ONESUPERIOR, XKB_KEY_onesuperior),
    (OBS_KEY_OOBLIQUE, XKB_KEY_Ooblique),
    (OBS_KEY_ORDFEMININE, XKB_KEY_ordfeminine),
    (OBS_KEY_OTILDE, XKB_KEY_Otilde),
    (OBS_KEY_P, XKB_KEY_P),
    (OBS_KEY_PARAGRAPH, XKB_KEY_paragraph),
    (OBS_KEY_PARENLEFT, XKB_KEY_parenleft),
    (OBS_KEY_PARENRIGHT, XKB_KEY_parenright),
    (OBS_KEY_PAUSE, XKB_KEY_Pause),
    (OBS_KEY_PERCENT, XKB_KEY_percent),
    (OBS_KEY_PERIOD, XKB_KEY_period),
    (OBS_KEY_PERIODCENTERED, XKB_KEY_periodcentered),
    (OBS_KEY_PLUS, XKB_KEY_plus),
    (OBS_KEY_PLUSMINUS, XKB_KEY_plusminus),
    (OBS_KEY_PREVIOUSCANDIDATE, XKB_KEY_PreviousCandidate),
    (OBS_KEY_PRINT, XKB_KEY_Print),
    (OBS_KEY_Q, XKB_KEY_Q),
    (OBS_KEY_QUESTION, XKB_KEY_question),
    (OBS_KEY_QUESTIONDOWN, XKB_KEY_questiondown),
    (OBS_KEY_QUOTEDBL, XKB_KEY_quotedbl),
    (OBS_KEY_QUOTELEFT, XKB_KEY_quoteleft),
    (OBS_KEY_R, XKB_KEY_R),
    (OBS_KEY_REDO, XKB_KEY_Redo),
    (OBS_KEY_REGISTERED, XKB_KEY_registered),
    (OBS_KEY_RETURN, XKB_KEY_Return),
    (OBS_KEY_RIGHT, XKB_KEY_Right),
    (OBS_KEY_ROMAJI, XKB_KEY_Romaji),
    (OBS_KEY_S, XKB_KEY_S),
    (OBS_KEY_SECTION, XKB_KEY_section),
    (OBS_KEY_SELECT, XKB_KEY_Select),
    (OBS_KEY_SEMICOLON, XKB_KEY_semicolon),
    (OBS_KEY_SINGLECANDIDATE, XKB_KEY_SingleCandidate),
    (OBS_KEY_SLASH, XKB_KEY_slash),
    (OBS_KEY_SPACE, XKB_KEY_space),
    (OBS_KEY_SSHARP, XKB_KEY_ssharp),
    (OBS_KEY_STERLING, XKB_KEY_sterling),
    (OBS_KEY_T, XKB_KEY_T),
    (OBS_KEY_TAB, XKB_KEY_Tab),
    (OBS_KEY_THORN, XKB_KEY_THORN),
    (OBS_KEY_THREEQUARTERS, XKB_KEY_threequarters),
    (OBS_KEY_THREESUPERIOR, XKB_KEY_threesuperior),
    (OBS_KEY_TOUROKU, XKB_KEY_Touroku),
    (OBS_KEY_TWOSUPERIOR, XKB_KEY_twosuperior),
    (OBS_KEY_U, XKB_KEY_U),
    (OBS_KEY_UACUTE, XKB_KEY_Uacute),
    (OBS_KEY_UCIRCUMFLEX, XKB_KEY_Ucircumflex),
    (OBS_KEY_UDIAERESIS, XKB_KEY_Udiaeresis),
    (OBS_KEY_UGRAVE, XKB_KEY_Ugrave),
    (OBS_KEY_UNDERSCORE, XKB_KEY_underscore),
    (OBS_KEY_UNDO, XKB_KEY_Undo),
    (OBS_KEY_UP, XKB_KEY_Up),
    (OBS_KEY_V, XKB_KEY_V),
    (OBS_KEY_W, XKB_KEY_W),
    (OBS_KEY_X, XKB_KEY_X),
    (OBS_KEY_Y, XKB_KEY_Y),
    (OBS_KEY_YACUTE, XKB_KEY_Yacute),
    (OBS_KEY_YDIAERESIS, XKB_KEY_Ydiaeresis),
    (OBS_KEY_YEN, XKB_KEY_yen),
    (OBS_KEY_Z, XKB_KEY_Z),
    (OBS_KEY_ZENKAKU, XKB_KEY_Zenkaku),
    (OBS_KEY_ZENKAKU_HANKAKU, XKB_KEY_Zenkaku_Hankaku),
    (OBS_KEY_PAGEUP, XKB_KEY_Page_Up),
    (OBS_KEY_PAGEDOWN, XKB_KEY_Page_Down),
    (OBS_KEY_NUMEQUAL, XKB_KEY_KP_Equal),
    (OBS_KEY_NUMASTERISK, XKB_KEY_KP_Multiply),
    (OBS_KEY_NUMPLUS, XKB_KEY_KP_Add),
    (OBS_KEY_NUMCOMMA, XKB_KEY_KP_Separator),
    (OBS_KEY_NUMMINUS, XKB_KEY_KP_Subtract),
    (OBS_KEY_NUMPERIOD, XKB_KEY_KP_Decimal),
    (OBS_KEY_NUMSLASH, XKB_KEY_KP_Divide),
    (OBS_KEY_ENTER, XKB_KEY_KP_Enter),
    (OBS_KEY_NUM0, XKB_KEY_KP_0),
    (OBS_KEY_NUM1, XKB_KEY_KP_1),
    (OBS_KEY_NUM2, XKB_KEY_KP_2),
    (OBS_KEY_NUM3, XKB_KEY_KP_3),
    (OBS_KEY_NUM4, XKB_KEY_KP_4),
    (OBS_KEY_NUM5, XKB_KEY_KP_5),
    (OBS_KEY_NUM6, XKB_KEY_KP_6),
    (OBS_KEY_NUM7, XKB_KEY_KP_7),
    (OBS_KEY_NUM8, XKB_KEY_KP_8),
    (OBS_KEY_NUM9, XKB_KEY_KP_9),
    (OBS_KEY_VK_MEDIA_PLAY_PAUSE, XKB_KEY_XF86AudioPlay),
    (OBS_KEY_VK_MEDIA_STOP, XKB_KEY_XF86AudioStop),
    (OBS_KEY_VK_MEDIA_PREV_TRACK, XKB_KEY_XF86AudioPrev),
    (OBS_KEY_VK_MEDIA_NEXT_TRACK, XKB_KEY_XF86AudioNext),
    (OBS_KEY_VK_VOLUME_MUTE, XKB_KEY_XF86AudioMute),
    (OBS_KEY_VK_VOLUME_DOWN, XKB_KEY_XF86AudioLowerVolume),
    (OBS_KEY_VK_VOLUME_UP, XKB_KEY_XF86AudioRaiseVolume),
];

fn idx(key: i32) -> usize {
    usize::try_from(key).unwrap()
}

/// `obs_nix_wayland_keys_from_keysym((int)sym)`.
fn from(sym: u32) -> i32 {
    wk::from_keysym(i32::from_ne_bytes(sym.to_ne_bytes()))
}

/// The C test's static arrays after `clear_maps()`.
struct Maps {
    obs_to_key: Vec<u32>,
    key_to_sym: Vec<[u32; MAX_KEYCODES]>,
    pressed_as: Vec<i32>,
}

impl Maps {
    fn new() -> Self {
        Self {
            obs_to_key: vec![0; OBS_KEY_LAST_VALUE],
            key_to_sym: vec![[0; MAX_KEYCODES]; MAX_SHIFT_LEVELS],
            pressed_as: vec![KEY_NONE; MAX_KEYCODES],
        }
    }

    /// `obs_nix_wayland_keys_row(keycode, fake_level_syms, row, ...)`.
    fn row(&mut self, keycode: u32, row: &mut FakeRow) {
        wk::keymap_row(
            keycode,
            |level| {
                assert!(row.asked.len() < 8);
                row.asked.push(level);
                let level = usize::try_from(level).unwrap();
                (row.count[level] >= 1).then(|| row.syms[level][0])
            },
            &mut self.obs_to_key,
            &mut self.key_to_sym,
        );
    }

    /// `obs_nix_wayland_keys_focused(scancode, pressed, key_to_sym[0], pressed_as, &key)`.
    fn focused(&mut self, scancode: u32, pressed: bool) -> Option<i32> {
        wk::focused(scancode, pressed, &self.key_to_sym[0], &mut self.pressed_as)
    }

    fn nonzero_obs_to_key(&self) -> usize {
        self.obs_to_key.iter().filter(|&&v| v != 0).count()
    }

    fn nonzero_key_to_sym(&self) -> usize {
        self.key_to_sym
            .iter()
            .flatten()
            .filter(|&&v| v != 0)
            .count()
    }

    /// The record's entries other than `OBS_KEY_NONE`.
    fn recorded(&self) -> Vec<(usize, i32)> {
        self.pressed_as
            .iter()
            .enumerate()
            .filter(|&(_, &k)| k != KEY_NONE)
            .map(|(i, &k)| (i, k))
            .collect()
    }
}

/// A made-up keymap row: each level's keysym count and keysyms, and the
/// levels asked for, in order.
#[derive(Default)]
struct FakeRow {
    count: [i32; MAX_SHIFT_LEVELS],
    syms: [[u32; 2]; MAX_SHIFT_LEVELS],
    asked: Vec<u32>,
}

impl FakeRow {
    fn new(count: [i32; MAX_SHIFT_LEVELS], first: [u32; MAX_SHIFT_LEVELS]) -> Self {
        Self {
            count,
            syms: first.map(|s| [s, 0]),
            asked: Vec::new(),
        }
    }
}

#[test]
fn from_keysym_table() {
    assert_eq!(FROM_TABLE.len(), 332);
    for &(sym, key) in FROM_TABLE {
        assert_eq!(from(sym), key, "keysym {sym:#x}");
    }

    let mut mapped = 0;
    for sym in (0..0x20000)
        .chain(0x1000FF00..0x10010000)
        .chain(0x1008FE00..0x10090000)
    {
        mapped += usize::from(wk::from_keysym(sym) != KEY_NONE);
    }
    assert_eq!(mapped, FROM_TABLE.len());

    assert_eq!(wk::from_keysym(0), KEY_NONE);
    assert_eq!(wk::from_keysym(-1), KEY_NONE);
    assert_eq!(wk::from_keysym(i32::MAX), KEY_NONE);
    assert_eq!(wk::from_keysym(i32::MIN), KEY_NONE);
    assert_eq!(from(0), KEY_NONE); // XKB_KEY_NoSymbol
    assert_eq!(from(0xffffff), KEY_NONE); // XKB_KEY_VoidSymbol
}

#[test]
fn to_keysym_table() {
    assert_eq!(TO_TABLE.len(), 275);
    let mut expected = vec![0u32; OBS_KEY_LAST_VALUE];
    for &(key, sym) in TO_TABLE {
        assert_eq!(expected[idx(key)], 0);
        expected[idx(key)] = sym;
    }
    for (key, &sym) in expected.iter().enumerate() {
        assert_eq!(wk::to_keysym(i32::try_from(key).unwrap()), sym, "key {key}");
    }

    assert_eq!(wk::to_keysym(KEY_NONE), 0);
    assert_eq!(wk::to_keysym(-1), 0);
    assert_eq!(wk::to_keysym(i32::try_from(OBS_KEY_LAST_VALUE).unwrap()), 0);
}

#[test]
fn round_trips() {
    for key in 0..i32::try_from(OBS_KEY_LAST_VALUE).unwrap() {
        let sym = wk::to_keysym(key);
        if sym != 0 {
            assert_eq!(from(sym), key);
        }
    }

    let mut lowercase = 0;
    for &(sym, key) in FROM_TABLE {
        if wk::to_keysym(key) == sym {
            continue;
        }
        lowercase += 1;
        assert!(
            (XKB_KEY_a..=XKB_KEY_z).contains(&sym)
                || (XKB_KEY_agrave..=XKB_KEY_ydiaeresis).contains(&sym),
            "keysym {sym:#x}"
        );
    }
    assert_eq!(lowercase, 57);
    assert_eq!(wk::to_keysym(from(XKB_KEY_q)), XKB_KEY_Q);
    assert_eq!(wk::to_keysym(from(XKB_KEY_ooblique)), XKB_KEY_Ooblique);
    assert_eq!(wk::to_keysym(from(XKB_KEY_ydiaeresis)), XKB_KEY_Ydiaeresis);
}

#[test]
fn row_levels() {
    let mut m = Maps::new();
    let mut row = FakeRow {
        count: [1, 0, 2, -1],
        syms: [
            [XKB_KEY_a, 0],
            [XKB_KEY_b, 0],
            [XKB_KEY_c, XKB_KEY_d],
            [XKB_KEY_e, 0],
        ],
        asked: Vec::new(),
    };
    m.row(38, &mut row);

    assert_eq!(row.asked, [0, 1, 2, 3]);
    assert_eq!(m.key_to_sym[0][38], XKB_KEY_a);
    assert_eq!(m.key_to_sym[1][38], 0);
    assert_eq!(m.key_to_sym[2][38], XKB_KEY_c);
    assert_eq!(m.key_to_sym[3][38], 0);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_A)], 38);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_C)], 38);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_B)], 0);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_D)], 0);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_E)], 0);
    assert_eq!(m.nonzero_obs_to_key(), 2);
    assert_eq!(m.nonzero_key_to_sym(), 2);
}

#[test]
fn row_bound() {
    let mut m = Maps::new();
    let mut row = FakeRow::new([1, 1, 1, 1], [XKB_KEY_a, XKB_KEY_A, XKB_KEY_b, XKB_KEY_B]);
    m.row(256, &mut row);
    m.row(u32::MAX, &mut row);
    assert!(row.asked.is_empty());
    assert_eq!(m.nonzero_obs_to_key(), 0);
    assert_eq!(m.nonzero_key_to_sym(), 0);

    m.row(255, &mut row);
    assert_eq!(row.asked.len(), 4);
    assert_eq!(m.key_to_sym[0][255], XKB_KEY_a);
    assert_eq!(m.key_to_sym[1][255], XKB_KEY_A);
    assert_eq!(m.key_to_sym[2][255], XKB_KEY_b);
    assert_eq!(m.key_to_sym[3][255], XKB_KEY_B);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_A)], 255);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_B)], 255);
}

#[test]
fn row_first_keycode_wins() {
    let mut m = Maps::new();
    let mut less = FakeRow::new([1, 0, 0, 0], [XKB_KEY_less, 0, 0, 0]);
    let mut comma = FakeRow::new([1, 1, 0, 0], [XKB_KEY_comma, XKB_KEY_less, 0, 0]);
    m.row(94, &mut less);
    m.row(59, &mut comma);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_LESS)], 94);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_COMMA)], 59);
    assert_eq!(m.key_to_sym[0][94], XKB_KEY_less);
    assert_eq!(m.key_to_sym[1][59], XKB_KEY_less);

    let mut upper = FakeRow::new([1, 0, 0, 0], [XKB_KEY_A, 0, 0, 0]);
    let mut lower = FakeRow::new([1, 0, 0, 0], [XKB_KEY_a, 0, 0, 0]);
    m.row(40, &mut upper);
    m.row(38, &mut lower);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_A)], 40);

    // Keycode 0 is stored, and a later keycode then overwrites it.
    let mut zero = FakeRow::new([1, 0, 0, 0], [XKB_KEY_z, 0, 0, 0]);
    m.row(0, &mut zero);
    assert_eq!(m.key_to_sym[0][0], XKB_KEY_z);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_Z)], 0);
    let mut zero = FakeRow::new([1, 0, 0, 0], [XKB_KEY_z, 0, 0, 0]);
    m.row(52, &mut zero);
    assert_eq!(m.obs_to_key[idx(OBS_KEY_Z)], 52);
}

/// `XKB_KEY_ISO_Level3_Shift` and `XKB_KEY_VoidSymbol`: keysyms with no key.
const ISO_LEVEL3_SHIFT: u32 = 0xfe03;
const VOID_SYMBOL: u32 = 0xffffff;

/// Characterized, not endorsed: a keysym with no key stores its keycode as
/// `OBS_KEY_NONE`'s, first wins.
#[test]
fn row_unknown_keysym() {
    let mut m = Maps::new();
    let mut first = FakeRow::new([1, 0, 0, 0], [ISO_LEVEL3_SHIFT, 0, 0, 0]);
    let mut second = FakeRow::new([1, 0, 0, 0], [VOID_SYMBOL, 0, 0, 0]);
    m.row(92, &mut first);
    m.row(108, &mut second);
    assert_eq!(m.obs_to_key[idx(KEY_NONE)], 92);
    assert_eq!(m.key_to_sym[0][92], ISO_LEVEL3_SHIFT);
    assert_eq!(m.key_to_sym[0][108], VOID_SYMBOL);
    assert_eq!(m.nonzero_obs_to_key(), 1);
}

#[test]
fn focused_key() {
    let mut m = Maps::new();
    m.key_to_sym[0][24] = XKB_KEY_q;
    m.key_to_sym[1][24] = XKB_KEY_Q;
    m.key_to_sym[0][65] = XKB_KEY_space;
    m.key_to_sym[0][122] = XKB_KEY_XF86AudioLowerVolume;

    assert_eq!(m.focused(16, true), Some(OBS_KEY_Q));
    assert_eq!(m.recorded(), [(24, OBS_KEY_Q)]);
    assert_eq!(m.focused(16, false), Some(OBS_KEY_Q));
    assert_eq!(m.recorded(), []);
    assert_eq!(m.focused(57, true), Some(OBS_KEY_SPACE));
    assert_eq!(m.focused(114, true), Some(OBS_KEY_VK_VOLUME_DOWN));
    assert_eq!(
        m.recorded(),
        [(65, OBS_KEY_SPACE), (122, OBS_KEY_VK_VOLUME_DOWN)]
    );
}

#[test]
fn focused_nothing() {
    let mut m = Maps::new();
    m.key_to_sym[1][30] = XKB_KEY_A;
    m.key_to_sym[0][255] = XKB_KEY_b;

    assert_eq!(m.focused(22, true), None);
    assert_eq!(m.focused(248, true), None);
    assert_eq!(m.focused(0xFFFF_FFF0, true), None);
    assert_eq!(m.recorded(), []);
    assert_eq!(m.focused(247, true), Some(OBS_KEY_B));
    assert_eq!(m.recorded(), [(255, OBS_KEY_B)]);
}

/// Characterized, not endorsed: the scancode plus 8 wraps as a `u32`, and a
/// level-0 keysym with no key reports `OBS_KEY_NONE`.
#[test]
fn focused_odd() {
    let mut m = Maps::new();
    m.key_to_sym[0][0] = XKB_KEY_c;
    m.key_to_sym[0][7] = XKB_KEY_d;
    m.key_to_sym[0][92] = ISO_LEVEL3_SHIFT;

    assert_eq!(m.focused(0xFFFF_FFF8, true), Some(OBS_KEY_C));
    assert_eq!(m.focused(0xFFFF_FFFF, true), Some(OBS_KEY_D));
    assert_eq!(m.focused(84, true), Some(KEY_NONE));
    assert_eq!(m.recorded(), [(0, OBS_KEY_C), (7, OBS_KEY_D)]);
}

/// The intentional difference: the C reports this release as `OBS_KEY_A`,
/// the key the new layout puts on the keycode, and leaves Q held.
#[test]
fn focused_across_layout_change() {
    let mut m = Maps::new();
    m.key_to_sym[0][24] = XKB_KEY_q;
    assert_eq!(m.focused(16, true), Some(OBS_KEY_Q));

    m.key_to_sym[0][24] = XKB_KEY_a;
    assert_eq!(m.focused(16, false), Some(OBS_KEY_Q));
    assert_eq!(m.recorded(), []);
}

// Edges.

#[test]
fn edge_from_keysym_every_int_edge() {
    for sym in [
        i32::MIN,
        i32::MIN + 1,
        -2,
        -1,
        0,
        1,
        0x1f,
        0x7f,
        0xff00,
        i32::MAX - 1,
        i32::MAX,
    ] {
        let expected = FROM_TABLE
            .iter()
            .find(|&&(s, _)| i64::from(s) == i64::from(sym))
            .map_or(KEY_NONE, |&(_, k)| k);
        assert_eq!(wk::from_keysym(sym), expected, "keysym {sym}");
    }
}

#[test]
fn edge_to_keysym_outside_the_enum() {
    for key in [i32::MIN, -100, -1, 636, 1000, i32::MAX] {
        if (0..i32::try_from(OBS_KEY_LAST_VALUE).unwrap()).contains(&key) {
            continue;
        }
        assert_eq!(wk::to_keysym(key), 0, "key {key}");
    }
}

/// Short arrays: what is out of their range is not written, and every level
/// is still asked for.
#[test]
fn edge_row_short_arrays() {
    let mut obs_to_key = vec![0u32; 3];
    let mut key_to_sym = vec![[0u32; MAX_KEYCODES]; 2];
    let mut asked = Vec::new();
    wk::keymap_row(
        10,
        |level| {
            asked.push(level);
            Some([XKB_KEY_a, XKB_KEY_A, XKB_KEY_b, XKB_KEY_B][usize::try_from(level).unwrap()])
        },
        &mut obs_to_key,
        &mut key_to_sym,
    );
    assert_eq!(asked, [0, 1, 2, 3]);
    assert_eq!(key_to_sym[0][10], XKB_KEY_a);
    assert_eq!(key_to_sym[1][10], XKB_KEY_A);
    assert_eq!(obs_to_key, [0, 0, 0]);

    wk::keymap_row(10, |_| Some(XKB_KEY_a), &mut [], &mut []);
}

/// A release with nothing recorded reports nothing, even when the keycode
/// has a key now (C reports that key).
#[test]
fn edge_release_without_press() {
    let mut m = Maps::new();
    m.key_to_sym[0][24] = XKB_KEY_q;
    assert_eq!(m.focused(16, false), None);
    assert_eq!(m.recorded(), []);
}

/// A press that reports `OBS_KEY_NONE` records nothing, so its release
/// reports nothing, whatever the layout puts on the keycode by then.
#[test]
fn edge_release_after_a_press_with_no_key() {
    let mut m = Maps::new();
    m.key_to_sym[0][92] = ISO_LEVEL3_SHIFT;
    assert_eq!(m.focused(84, true), Some(KEY_NONE));
    m.key_to_sym[0][92] = XKB_KEY_a;
    assert_eq!(m.focused(84, false), None);
}

/// A second press replaces the record; a press with no level-0 keysym
/// reports nothing and leaves it.
#[test]
fn edge_press_again() {
    let mut m = Maps::new();
    m.key_to_sym[0][24] = XKB_KEY_q;
    assert_eq!(m.focused(16, true), Some(OBS_KEY_Q));
    m.key_to_sym[0][24] = XKB_KEY_a;
    assert_eq!(m.focused(16, true), Some(OBS_KEY_A));
    m.key_to_sym[0][24] = 0;
    assert_eq!(m.focused(16, true), None);
    assert_eq!(m.recorded(), [(24, OBS_KEY_A)]);
    assert_eq!(m.focused(16, false), Some(OBS_KEY_A));
    assert_eq!(m.focused(16, false), None);
}

/// Keycodes past short arrays read and record nothing.
#[test]
fn edge_focused_short_arrays() {
    let mut record = vec![KEY_NONE; 30];
    let table = vec![XKB_KEY_q; 20];
    assert_eq!(wk::focused(16, true, &table, &mut record), None);
    assert_eq!(wk::focused(4, true, &table, &mut record), Some(OBS_KEY_Q));
    assert_eq!(record[12], OBS_KEY_Q);
    assert_eq!(wk::focused(4, false, &table, &mut record), Some(OBS_KEY_Q));
    let mut short = vec![KEY_NONE; 5];
    assert_eq!(wk::focused(4, true, &table, &mut short), Some(OBS_KEY_Q));
    assert_eq!(wk::focused(4, false, &table, &mut short), None);
}

// Models written from the C (and, for a release, the fix).

fn model_row(keycode: u32, levels: &[Option<u32>; MAX_SHIFT_LEVELS], m: &mut Maps) -> Vec<u32> {
    let mut asked = Vec::new();
    let Ok(kc) = usize::try_from(keycode) else {
        return asked;
    };
    if kc >= MAX_KEYCODES {
        return asked;
    }
    for (level, sym) in levels.iter().enumerate() {
        asked.push(u32::try_from(level).unwrap());
        let Some(sym) = *sym else { continue };
        let key = idx(from(sym));
        if m.obs_to_key[key] == 0 {
            m.obs_to_key[key] = keycode;
        }
        m.key_to_sym[level][kc] = sym;
    }
    asked
}

fn model_focused(scancode: u32, pressed: bool, m: &mut Maps) -> Option<i32> {
    let kc = usize::try_from(scancode.wrapping_add(8)).unwrap();
    if kc >= MAX_KEYCODES {
        return None;
    }
    if !pressed {
        let key = std::mem::replace(&mut m.pressed_as[kc], KEY_NONE);
        return (key != KEY_NONE).then_some(key);
    }
    let sym = m.key_to_sym[0][kc];
    if sym == 0 {
        return None;
    }
    let key = from(sym);
    m.pressed_as[kc] = key;
    Some(key)
}

fn keysym() -> impl Strategy<Value = u32> {
    prop_oneof![
        3 => prop::sample::select(FROM_TABLE).prop_map(|(s, _)| s),
        1 => Just(0u32),
        1 => any::<u32>(),
    ]
}

fn level() -> impl Strategy<Value = Option<u32>> {
    prop::option::weighted(0.7, keysym())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(256)))]

    /// A whole keymap, one row at a time, against the model.
    #[test]
    fn prop_keymap_rows(rows in prop::collection::vec(
        (0u32..300, prop::array::uniform4(level())),
        0..80,
    )) {
        let mut got = Maps::new();
        let mut want = Maps::new();
        for (keycode, levels) in &rows {
            let mut asked = Vec::new();
            wk::keymap_row(
                *keycode,
                |level| {
                    asked.push(level);
                    levels[usize::try_from(level).unwrap()]
                },
                &mut got.obs_to_key,
                &mut got.key_to_sym,
            );
            prop_assert_eq!(asked, model_row(*keycode, levels, &mut want));
        }
        prop_assert_eq!(&got.obs_to_key, &want.obs_to_key);
        prop_assert_eq!(&got.key_to_sym, &want.key_to_sym);
    }

    /// Presses and releases with layout changes between them, against the
    /// model.
    #[test]
    fn prop_focused_events(
        layouts in prop::collection::vec(prop::collection::vec(keysym(), 16), 1..4),
        events in prop::collection::vec((0u32..16, any::<bool>(), 0usize..4), 0..60),
        far in prop::sample::select(vec![240u32, 247, 248, 0xFFFF_FFF7, 0xFFFF_FFF8, u32::MAX]),
    ) {
        let mut got = Maps::new();
        let mut want = Maps::new();
        for (i, (code, pressed, layout)) in events.iter().enumerate() {
            let layout = &layouts[layout % layouts.len()];
            for (j, &sym) in layout.iter().enumerate() {
                got.key_to_sym[0][j + 20] = sym;
                want.key_to_sym[0][j + 20] = sym;
            }
            let scancode = if i % 7 == 6 { far } else { code + 12 };
            prop_assert_eq!(
                got.focused(scancode, *pressed),
                model_focused(scancode, *pressed, &mut want)
            );
            prop_assert_eq!(&got.pressed_as, &want.pressed_as);
        }
    }
}
