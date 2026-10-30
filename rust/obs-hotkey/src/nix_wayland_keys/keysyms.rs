//! The XKB keysyms the Wayland tables name, with the names and values of
//! `<xkbcommon/xkbcommon-keysyms.h>` (the X11 keysym values). The Tier 3
//! parity tests check each one against that header.

#![allow(non_upper_case_globals)]

/// `XKB_KEY_space`.
pub const XKB_KEY_space: u32 = 0x20;
/// `XKB_KEY_exclam`.
pub const XKB_KEY_exclam: u32 = 0x21;
/// `XKB_KEY_quotedbl`.
pub const XKB_KEY_quotedbl: u32 = 0x22;
/// `XKB_KEY_numbersign`.
pub const XKB_KEY_numbersign: u32 = 0x23;
/// `XKB_KEY_dollar`.
pub const XKB_KEY_dollar: u32 = 0x24;
/// `XKB_KEY_percent`.
pub const XKB_KEY_percent: u32 = 0x25;
/// `XKB_KEY_ampersand`.
pub const XKB_KEY_ampersand: u32 = 0x26;
/// `XKB_KEY_apostrophe`.
pub const XKB_KEY_apostrophe: u32 = 0x27;
/// `XKB_KEY_parenleft`.
pub const XKB_KEY_parenleft: u32 = 0x28;
/// `XKB_KEY_parenright`.
pub const XKB_KEY_parenright: u32 = 0x29;
/// `XKB_KEY_asterisk`.
pub const XKB_KEY_asterisk: u32 = 0x2a;
/// `XKB_KEY_plus`.
pub const XKB_KEY_plus: u32 = 0x2b;
/// `XKB_KEY_comma`.
pub const XKB_KEY_comma: u32 = 0x2c;
/// `XKB_KEY_minus`.
pub const XKB_KEY_minus: u32 = 0x2d;
/// `XKB_KEY_period`.
pub const XKB_KEY_period: u32 = 0x2e;
/// `XKB_KEY_slash`.
pub const XKB_KEY_slash: u32 = 0x2f;
/// `XKB_KEY_0`.
pub const XKB_KEY_0: u32 = 0x30;
/// `XKB_KEY_1`.
pub const XKB_KEY_1: u32 = 0x31;
/// `XKB_KEY_2`.
pub const XKB_KEY_2: u32 = 0x32;
/// `XKB_KEY_3`.
pub const XKB_KEY_3: u32 = 0x33;
/// `XKB_KEY_4`.
pub const XKB_KEY_4: u32 = 0x34;
/// `XKB_KEY_5`.
pub const XKB_KEY_5: u32 = 0x35;
/// `XKB_KEY_6`.
pub const XKB_KEY_6: u32 = 0x36;
/// `XKB_KEY_7`.
pub const XKB_KEY_7: u32 = 0x37;
/// `XKB_KEY_8`.
pub const XKB_KEY_8: u32 = 0x38;
/// `XKB_KEY_9`.
pub const XKB_KEY_9: u32 = 0x39;
/// `XKB_KEY_colon`.
pub const XKB_KEY_colon: u32 = 0x3a;
/// `XKB_KEY_semicolon`.
pub const XKB_KEY_semicolon: u32 = 0x3b;
/// `XKB_KEY_less`.
pub const XKB_KEY_less: u32 = 0x3c;
/// `XKB_KEY_equal`.
pub const XKB_KEY_equal: u32 = 0x3d;
/// `XKB_KEY_greater`.
pub const XKB_KEY_greater: u32 = 0x3e;
/// `XKB_KEY_question`.
pub const XKB_KEY_question: u32 = 0x3f;
/// `XKB_KEY_at`.
pub const XKB_KEY_at: u32 = 0x40;
/// `XKB_KEY_A`.
pub const XKB_KEY_A: u32 = 0x41;
/// `XKB_KEY_B`.
pub const XKB_KEY_B: u32 = 0x42;
/// `XKB_KEY_C`.
pub const XKB_KEY_C: u32 = 0x43;
/// `XKB_KEY_D`.
pub const XKB_KEY_D: u32 = 0x44;
/// `XKB_KEY_E`.
pub const XKB_KEY_E: u32 = 0x45;
/// `XKB_KEY_F`.
pub const XKB_KEY_F: u32 = 0x46;
/// `XKB_KEY_G`.
pub const XKB_KEY_G: u32 = 0x47;
/// `XKB_KEY_H`.
pub const XKB_KEY_H: u32 = 0x48;
/// `XKB_KEY_I`.
pub const XKB_KEY_I: u32 = 0x49;
/// `XKB_KEY_J`.
pub const XKB_KEY_J: u32 = 0x4a;
/// `XKB_KEY_K`.
pub const XKB_KEY_K: u32 = 0x4b;
/// `XKB_KEY_L`.
pub const XKB_KEY_L: u32 = 0x4c;
/// `XKB_KEY_M`.
pub const XKB_KEY_M: u32 = 0x4d;
/// `XKB_KEY_N`.
pub const XKB_KEY_N: u32 = 0x4e;
/// `XKB_KEY_O`.
pub const XKB_KEY_O: u32 = 0x4f;
/// `XKB_KEY_P`.
pub const XKB_KEY_P: u32 = 0x50;
/// `XKB_KEY_Q`.
pub const XKB_KEY_Q: u32 = 0x51;
/// `XKB_KEY_R`.
pub const XKB_KEY_R: u32 = 0x52;
/// `XKB_KEY_S`.
pub const XKB_KEY_S: u32 = 0x53;
/// `XKB_KEY_T`.
pub const XKB_KEY_T: u32 = 0x54;
/// `XKB_KEY_U`.
pub const XKB_KEY_U: u32 = 0x55;
/// `XKB_KEY_V`.
pub const XKB_KEY_V: u32 = 0x56;
/// `XKB_KEY_W`.
pub const XKB_KEY_W: u32 = 0x57;
/// `XKB_KEY_X`.
pub const XKB_KEY_X: u32 = 0x58;
/// `XKB_KEY_Y`.
pub const XKB_KEY_Y: u32 = 0x59;
/// `XKB_KEY_Z`.
pub const XKB_KEY_Z: u32 = 0x5a;
/// `XKB_KEY_bracketleft`.
pub const XKB_KEY_bracketleft: u32 = 0x5b;
/// `XKB_KEY_backslash`.
pub const XKB_KEY_backslash: u32 = 0x5c;
/// `XKB_KEY_bracketright`.
pub const XKB_KEY_bracketright: u32 = 0x5d;
/// `XKB_KEY_asciicircum`.
pub const XKB_KEY_asciicircum: u32 = 0x5e;
/// `XKB_KEY_underscore`.
pub const XKB_KEY_underscore: u32 = 0x5f;
/// `XKB_KEY_quoteleft`.
pub const XKB_KEY_quoteleft: u32 = 0x60;
/// `XKB_KEY_a`.
pub const XKB_KEY_a: u32 = 0x61;
/// `XKB_KEY_b`.
pub const XKB_KEY_b: u32 = 0x62;
/// `XKB_KEY_c`.
pub const XKB_KEY_c: u32 = 0x63;
/// `XKB_KEY_d`.
pub const XKB_KEY_d: u32 = 0x64;
/// `XKB_KEY_e`.
pub const XKB_KEY_e: u32 = 0x65;
/// `XKB_KEY_f`.
pub const XKB_KEY_f: u32 = 0x66;
/// `XKB_KEY_g`.
pub const XKB_KEY_g: u32 = 0x67;
/// `XKB_KEY_h`.
pub const XKB_KEY_h: u32 = 0x68;
/// `XKB_KEY_i`.
pub const XKB_KEY_i: u32 = 0x69;
/// `XKB_KEY_j`.
pub const XKB_KEY_j: u32 = 0x6a;
/// `XKB_KEY_k`.
pub const XKB_KEY_k: u32 = 0x6b;
/// `XKB_KEY_l`.
pub const XKB_KEY_l: u32 = 0x6c;
/// `XKB_KEY_m`.
pub const XKB_KEY_m: u32 = 0x6d;
/// `XKB_KEY_n`.
pub const XKB_KEY_n: u32 = 0x6e;
/// `XKB_KEY_o`.
pub const XKB_KEY_o: u32 = 0x6f;
/// `XKB_KEY_p`.
pub const XKB_KEY_p: u32 = 0x70;
/// `XKB_KEY_q`.
pub const XKB_KEY_q: u32 = 0x71;
/// `XKB_KEY_r`.
pub const XKB_KEY_r: u32 = 0x72;
/// `XKB_KEY_s`.
pub const XKB_KEY_s: u32 = 0x73;
/// `XKB_KEY_t`.
pub const XKB_KEY_t: u32 = 0x74;
/// `XKB_KEY_u`.
pub const XKB_KEY_u: u32 = 0x75;
/// `XKB_KEY_v`.
pub const XKB_KEY_v: u32 = 0x76;
/// `XKB_KEY_w`.
pub const XKB_KEY_w: u32 = 0x77;
/// `XKB_KEY_x`.
pub const XKB_KEY_x: u32 = 0x78;
/// `XKB_KEY_y`.
pub const XKB_KEY_y: u32 = 0x79;
/// `XKB_KEY_z`.
pub const XKB_KEY_z: u32 = 0x7a;
/// `XKB_KEY_braceleft`.
pub const XKB_KEY_braceleft: u32 = 0x7b;
/// `XKB_KEY_bar`.
pub const XKB_KEY_bar: u32 = 0x7c;
/// `XKB_KEY_braceright`.
pub const XKB_KEY_braceright: u32 = 0x7d;
/// `XKB_KEY_asciitilde`.
pub const XKB_KEY_asciitilde: u32 = 0x7e;
/// `XKB_KEY_nobreakspace`.
pub const XKB_KEY_nobreakspace: u32 = 0xa0;
/// `XKB_KEY_exclamdown`.
pub const XKB_KEY_exclamdown: u32 = 0xa1;
/// `XKB_KEY_cent`.
pub const XKB_KEY_cent: u32 = 0xa2;
/// `XKB_KEY_sterling`.
pub const XKB_KEY_sterling: u32 = 0xa3;
/// `XKB_KEY_currency`.
pub const XKB_KEY_currency: u32 = 0xa4;
/// `XKB_KEY_yen`.
pub const XKB_KEY_yen: u32 = 0xa5;
/// `XKB_KEY_brokenbar`.
pub const XKB_KEY_brokenbar: u32 = 0xa6;
/// `XKB_KEY_section`.
pub const XKB_KEY_section: u32 = 0xa7;
/// `XKB_KEY_diaeresis`.
pub const XKB_KEY_diaeresis: u32 = 0xa8;
/// `XKB_KEY_copyright`.
pub const XKB_KEY_copyright: u32 = 0xa9;
/// `XKB_KEY_ordfeminine`.
pub const XKB_KEY_ordfeminine: u32 = 0xaa;
/// `XKB_KEY_guillemotleft`.
pub const XKB_KEY_guillemotleft: u32 = 0xab;
/// `XKB_KEY_notsign`.
pub const XKB_KEY_notsign: u32 = 0xac;
/// `XKB_KEY_hyphen`.
pub const XKB_KEY_hyphen: u32 = 0xad;
/// `XKB_KEY_registered`.
pub const XKB_KEY_registered: u32 = 0xae;
/// `XKB_KEY_macron`.
pub const XKB_KEY_macron: u32 = 0xaf;
/// `XKB_KEY_degree`.
pub const XKB_KEY_degree: u32 = 0xb0;
/// `XKB_KEY_plusminus`.
pub const XKB_KEY_plusminus: u32 = 0xb1;
/// `XKB_KEY_twosuperior`.
pub const XKB_KEY_twosuperior: u32 = 0xb2;
/// `XKB_KEY_threesuperior`.
pub const XKB_KEY_threesuperior: u32 = 0xb3;
/// `XKB_KEY_acute`.
pub const XKB_KEY_acute: u32 = 0xb4;
/// `XKB_KEY_mu`.
pub const XKB_KEY_mu: u32 = 0xb5;
/// `XKB_KEY_paragraph`.
pub const XKB_KEY_paragraph: u32 = 0xb6;
/// `XKB_KEY_periodcentered`.
pub const XKB_KEY_periodcentered: u32 = 0xb7;
/// `XKB_KEY_cedilla`.
pub const XKB_KEY_cedilla: u32 = 0xb8;
/// `XKB_KEY_onesuperior`.
pub const XKB_KEY_onesuperior: u32 = 0xb9;
/// `XKB_KEY_masculine`.
pub const XKB_KEY_masculine: u32 = 0xba;
/// `XKB_KEY_guillemotright`.
pub const XKB_KEY_guillemotright: u32 = 0xbb;
/// `XKB_KEY_onequarter`.
pub const XKB_KEY_onequarter: u32 = 0xbc;
/// `XKB_KEY_onehalf`.
pub const XKB_KEY_onehalf: u32 = 0xbd;
/// `XKB_KEY_threequarters`.
pub const XKB_KEY_threequarters: u32 = 0xbe;
/// `XKB_KEY_questiondown`.
pub const XKB_KEY_questiondown: u32 = 0xbf;
/// `XKB_KEY_Agrave`.
pub const XKB_KEY_Agrave: u32 = 0xc0;
/// `XKB_KEY_Aacute`.
pub const XKB_KEY_Aacute: u32 = 0xc1;
/// `XKB_KEY_Acircumflex`.
pub const XKB_KEY_Acircumflex: u32 = 0xc2;
/// `XKB_KEY_Atilde`.
pub const XKB_KEY_Atilde: u32 = 0xc3;
/// `XKB_KEY_Adiaeresis`.
pub const XKB_KEY_Adiaeresis: u32 = 0xc4;
/// `XKB_KEY_Aring`.
pub const XKB_KEY_Aring: u32 = 0xc5;
/// `XKB_KEY_AE`.
pub const XKB_KEY_AE: u32 = 0xc6;
/// `XKB_KEY_Ccedilla`.
pub const XKB_KEY_Ccedilla: u32 = 0xc7;
/// `XKB_KEY_Egrave`.
pub const XKB_KEY_Egrave: u32 = 0xc8;
/// `XKB_KEY_Eacute`.
pub const XKB_KEY_Eacute: u32 = 0xc9;
/// `XKB_KEY_Ecircumflex`.
pub const XKB_KEY_Ecircumflex: u32 = 0xca;
/// `XKB_KEY_Ediaeresis`.
pub const XKB_KEY_Ediaeresis: u32 = 0xcb;
/// `XKB_KEY_Igrave`.
pub const XKB_KEY_Igrave: u32 = 0xcc;
/// `XKB_KEY_Iacute`.
pub const XKB_KEY_Iacute: u32 = 0xcd;
/// `XKB_KEY_Icircumflex`.
pub const XKB_KEY_Icircumflex: u32 = 0xce;
/// `XKB_KEY_Idiaeresis`.
pub const XKB_KEY_Idiaeresis: u32 = 0xcf;
/// `XKB_KEY_Eth`.
pub const XKB_KEY_Eth: u32 = 0xd0;
/// `XKB_KEY_ETH`.
pub const XKB_KEY_ETH: u32 = 0xd0;
/// `XKB_KEY_Ntilde`.
pub const XKB_KEY_Ntilde: u32 = 0xd1;
/// `XKB_KEY_Ograve`.
pub const XKB_KEY_Ograve: u32 = 0xd2;
/// `XKB_KEY_Oacute`.
pub const XKB_KEY_Oacute: u32 = 0xd3;
/// `XKB_KEY_Ocircumflex`.
pub const XKB_KEY_Ocircumflex: u32 = 0xd4;
/// `XKB_KEY_Otilde`.
pub const XKB_KEY_Otilde: u32 = 0xd5;
/// `XKB_KEY_Odiaeresis`.
pub const XKB_KEY_Odiaeresis: u32 = 0xd6;
/// `XKB_KEY_multiply`.
pub const XKB_KEY_multiply: u32 = 0xd7;
/// `XKB_KEY_Ooblique`.
pub const XKB_KEY_Ooblique: u32 = 0xd8;
/// `XKB_KEY_Ugrave`.
pub const XKB_KEY_Ugrave: u32 = 0xd9;
/// `XKB_KEY_Uacute`.
pub const XKB_KEY_Uacute: u32 = 0xda;
/// `XKB_KEY_Ucircumflex`.
pub const XKB_KEY_Ucircumflex: u32 = 0xdb;
/// `XKB_KEY_Udiaeresis`.
pub const XKB_KEY_Udiaeresis: u32 = 0xdc;
/// `XKB_KEY_Yacute`.
pub const XKB_KEY_Yacute: u32 = 0xdd;
/// `XKB_KEY_Thorn`.
pub const XKB_KEY_Thorn: u32 = 0xde;
/// `XKB_KEY_THORN`.
pub const XKB_KEY_THORN: u32 = 0xde;
/// `XKB_KEY_ssharp`.
pub const XKB_KEY_ssharp: u32 = 0xdf;
/// `XKB_KEY_agrave`.
pub const XKB_KEY_agrave: u32 = 0xe0;
/// `XKB_KEY_aacute`.
pub const XKB_KEY_aacute: u32 = 0xe1;
/// `XKB_KEY_acircumflex`.
pub const XKB_KEY_acircumflex: u32 = 0xe2;
/// `XKB_KEY_atilde`.
pub const XKB_KEY_atilde: u32 = 0xe3;
/// `XKB_KEY_adiaeresis`.
pub const XKB_KEY_adiaeresis: u32 = 0xe4;
/// `XKB_KEY_aring`.
pub const XKB_KEY_aring: u32 = 0xe5;
/// `XKB_KEY_ae`.
pub const XKB_KEY_ae: u32 = 0xe6;
/// `XKB_KEY_ccedilla`.
pub const XKB_KEY_ccedilla: u32 = 0xe7;
/// `XKB_KEY_egrave`.
pub const XKB_KEY_egrave: u32 = 0xe8;
/// `XKB_KEY_eacute`.
pub const XKB_KEY_eacute: u32 = 0xe9;
/// `XKB_KEY_ecircumflex`.
pub const XKB_KEY_ecircumflex: u32 = 0xea;
/// `XKB_KEY_ediaeresis`.
pub const XKB_KEY_ediaeresis: u32 = 0xeb;
/// `XKB_KEY_igrave`.
pub const XKB_KEY_igrave: u32 = 0xec;
/// `XKB_KEY_iacute`.
pub const XKB_KEY_iacute: u32 = 0xed;
/// `XKB_KEY_icircumflex`.
pub const XKB_KEY_icircumflex: u32 = 0xee;
/// `XKB_KEY_idiaeresis`.
pub const XKB_KEY_idiaeresis: u32 = 0xef;
/// `XKB_KEY_eth`.
pub const XKB_KEY_eth: u32 = 0xf0;
/// `XKB_KEY_ntilde`.
pub const XKB_KEY_ntilde: u32 = 0xf1;
/// `XKB_KEY_ograve`.
pub const XKB_KEY_ograve: u32 = 0xf2;
/// `XKB_KEY_oacute`.
pub const XKB_KEY_oacute: u32 = 0xf3;
/// `XKB_KEY_ocircumflex`.
pub const XKB_KEY_ocircumflex: u32 = 0xf4;
/// `XKB_KEY_otilde`.
pub const XKB_KEY_otilde: u32 = 0xf5;
/// `XKB_KEY_odiaeresis`.
pub const XKB_KEY_odiaeresis: u32 = 0xf6;
/// `XKB_KEY_division`.
pub const XKB_KEY_division: u32 = 0xf7;
/// `XKB_KEY_ooblique`.
pub const XKB_KEY_ooblique: u32 = 0xf8;
/// `XKB_KEY_ugrave`.
pub const XKB_KEY_ugrave: u32 = 0xf9;
/// `XKB_KEY_uacute`.
pub const XKB_KEY_uacute: u32 = 0xfa;
/// `XKB_KEY_ucircumflex`.
pub const XKB_KEY_ucircumflex: u32 = 0xfb;
/// `XKB_KEY_udiaeresis`.
pub const XKB_KEY_udiaeresis: u32 = 0xfc;
/// `XKB_KEY_yacute`.
pub const XKB_KEY_yacute: u32 = 0xfd;
/// `XKB_KEY_thorn`.
pub const XKB_KEY_thorn: u32 = 0xfe;
/// `XKB_KEY_ydiaeresis`.
pub const XKB_KEY_ydiaeresis: u32 = 0xff;
/// `XKB_KEY_Ydiaeresis`.
pub const XKB_KEY_Ydiaeresis: u32 = 0x13be;
/// `XKB_KEY_dead_grave`.
pub const XKB_KEY_dead_grave: u32 = 0xfe50;
/// `XKB_KEY_dead_acute`.
pub const XKB_KEY_dead_acute: u32 = 0xfe51;
/// `XKB_KEY_dead_circumflex`.
pub const XKB_KEY_dead_circumflex: u32 = 0xfe52;
/// `XKB_KEY_dead_tilde`.
pub const XKB_KEY_dead_tilde: u32 = 0xfe53;
/// `XKB_KEY_dead_macron`.
pub const XKB_KEY_dead_macron: u32 = 0xfe54;
/// `XKB_KEY_dead_breve`.
pub const XKB_KEY_dead_breve: u32 = 0xfe55;
/// `XKB_KEY_dead_abovedot`.
pub const XKB_KEY_dead_abovedot: u32 = 0xfe56;
/// `XKB_KEY_dead_diaeresis`.
pub const XKB_KEY_dead_diaeresis: u32 = 0xfe57;
/// `XKB_KEY_dead_abovering`.
pub const XKB_KEY_dead_abovering: u32 = 0xfe58;
/// `XKB_KEY_dead_doubleacute`.
pub const XKB_KEY_dead_doubleacute: u32 = 0xfe59;
/// `XKB_KEY_dead_caron`.
pub const XKB_KEY_dead_caron: u32 = 0xfe5a;
/// `XKB_KEY_dead_cedilla`.
pub const XKB_KEY_dead_cedilla: u32 = 0xfe5b;
/// `XKB_KEY_dead_ogonek`.
pub const XKB_KEY_dead_ogonek: u32 = 0xfe5c;
/// `XKB_KEY_dead_iota`.
pub const XKB_KEY_dead_iota: u32 = 0xfe5d;
/// `XKB_KEY_dead_voiced_sound`.
pub const XKB_KEY_dead_voiced_sound: u32 = 0xfe5e;
/// `XKB_KEY_dead_semivoiced_sound`.
pub const XKB_KEY_dead_semivoiced_sound: u32 = 0xfe5f;
/// `XKB_KEY_dead_belowdot`.
pub const XKB_KEY_dead_belowdot: u32 = 0xfe60;
/// `XKB_KEY_dead_hook`.
pub const XKB_KEY_dead_hook: u32 = 0xfe61;
/// `XKB_KEY_dead_horn`.
pub const XKB_KEY_dead_horn: u32 = 0xfe62;
/// `XKB_KEY_BackSpace`.
pub const XKB_KEY_BackSpace: u32 = 0xff08;
/// `XKB_KEY_Tab`.
pub const XKB_KEY_Tab: u32 = 0xff09;
/// `XKB_KEY_Clear`.
pub const XKB_KEY_Clear: u32 = 0xff0b;
/// `XKB_KEY_Return`.
pub const XKB_KEY_Return: u32 = 0xff0d;
/// `XKB_KEY_Pause`.
pub const XKB_KEY_Pause: u32 = 0xff13;
/// `XKB_KEY_Escape`.
pub const XKB_KEY_Escape: u32 = 0xff1b;
/// `XKB_KEY_Multi_key`.
pub const XKB_KEY_Multi_key: u32 = 0xff20;
/// `XKB_KEY_Kanji`.
pub const XKB_KEY_Kanji: u32 = 0xff21;
/// `XKB_KEY_Muhenkan`.
pub const XKB_KEY_Muhenkan: u32 = 0xff22;
/// `XKB_KEY_Henkan`.
pub const XKB_KEY_Henkan: u32 = 0xff23;
/// `XKB_KEY_Romaji`.
pub const XKB_KEY_Romaji: u32 = 0xff24;
/// `XKB_KEY_Hiragana`.
pub const XKB_KEY_Hiragana: u32 = 0xff25;
/// `XKB_KEY_Katakana`.
pub const XKB_KEY_Katakana: u32 = 0xff26;
/// `XKB_KEY_Hiragana_Katakana`.
pub const XKB_KEY_Hiragana_Katakana: u32 = 0xff27;
/// `XKB_KEY_Zenkaku`.
pub const XKB_KEY_Zenkaku: u32 = 0xff28;
/// `XKB_KEY_Hankaku`.
pub const XKB_KEY_Hankaku: u32 = 0xff29;
/// `XKB_KEY_Zenkaku_Hankaku`.
pub const XKB_KEY_Zenkaku_Hankaku: u32 = 0xff2a;
/// `XKB_KEY_Touroku`.
pub const XKB_KEY_Touroku: u32 = 0xff2b;
/// `XKB_KEY_Massyo`.
pub const XKB_KEY_Massyo: u32 = 0xff2c;
/// `XKB_KEY_Kana_Lock`.
pub const XKB_KEY_Kana_Lock: u32 = 0xff2d;
/// `XKB_KEY_Kana_Shift`.
pub const XKB_KEY_Kana_Shift: u32 = 0xff2e;
/// `XKB_KEY_Eisu_Shift`.
pub const XKB_KEY_Eisu_Shift: u32 = 0xff2f;
/// `XKB_KEY_Eisu_toggle`.
pub const XKB_KEY_Eisu_toggle: u32 = 0xff30;
/// `XKB_KEY_Hangul`.
pub const XKB_KEY_Hangul: u32 = 0xff31;
/// `XKB_KEY_Hangul_Start`.
pub const XKB_KEY_Hangul_Start: u32 = 0xff32;
/// `XKB_KEY_Hangul_End`.
pub const XKB_KEY_Hangul_End: u32 = 0xff33;
/// `XKB_KEY_Hangul_Hanja`.
pub const XKB_KEY_Hangul_Hanja: u32 = 0xff34;
/// `XKB_KEY_Hangul_Jamo`.
pub const XKB_KEY_Hangul_Jamo: u32 = 0xff35;
/// `XKB_KEY_Hangul_Romaja`.
pub const XKB_KEY_Hangul_Romaja: u32 = 0xff36;
/// `XKB_KEY_Codeinput`.
pub const XKB_KEY_Codeinput: u32 = 0xff37;
/// `XKB_KEY_Hangul_Jeonja`.
pub const XKB_KEY_Hangul_Jeonja: u32 = 0xff38;
/// `XKB_KEY_Hangul_Banja`.
pub const XKB_KEY_Hangul_Banja: u32 = 0xff39;
/// `XKB_KEY_Hangul_PreHanja`.
pub const XKB_KEY_Hangul_PreHanja: u32 = 0xff3a;
/// `XKB_KEY_Hangul_PostHanja`.
pub const XKB_KEY_Hangul_PostHanja: u32 = 0xff3b;
/// `XKB_KEY_SingleCandidate`.
pub const XKB_KEY_SingleCandidate: u32 = 0xff3c;
/// `XKB_KEY_MultipleCandidate`.
pub const XKB_KEY_MultipleCandidate: u32 = 0xff3d;
/// `XKB_KEY_PreviousCandidate`.
pub const XKB_KEY_PreviousCandidate: u32 = 0xff3e;
/// `XKB_KEY_Hangul_Special`.
pub const XKB_KEY_Hangul_Special: u32 = 0xff3f;
/// `XKB_KEY_Home`.
pub const XKB_KEY_Home: u32 = 0xff50;
/// `XKB_KEY_Left`.
pub const XKB_KEY_Left: u32 = 0xff51;
/// `XKB_KEY_Up`.
pub const XKB_KEY_Up: u32 = 0xff52;
/// `XKB_KEY_Right`.
pub const XKB_KEY_Right: u32 = 0xff53;
/// `XKB_KEY_Down`.
pub const XKB_KEY_Down: u32 = 0xff54;
/// `XKB_KEY_Page_Up`.
pub const XKB_KEY_Page_Up: u32 = 0xff55;
/// `XKB_KEY_Page_Down`.
pub const XKB_KEY_Page_Down: u32 = 0xff56;
/// `XKB_KEY_End`.
pub const XKB_KEY_End: u32 = 0xff57;
/// `XKB_KEY_Select`.
pub const XKB_KEY_Select: u32 = 0xff60;
/// `XKB_KEY_Print`.
pub const XKB_KEY_Print: u32 = 0xff61;
/// `XKB_KEY_Execute`.
pub const XKB_KEY_Execute: u32 = 0xff62;
/// `XKB_KEY_Insert`.
pub const XKB_KEY_Insert: u32 = 0xff63;
/// `XKB_KEY_Undo`.
pub const XKB_KEY_Undo: u32 = 0xff65;
/// `XKB_KEY_Redo`.
pub const XKB_KEY_Redo: u32 = 0xff66;
/// `XKB_KEY_Menu`.
pub const XKB_KEY_Menu: u32 = 0xff67;
/// `XKB_KEY_Find`.
pub const XKB_KEY_Find: u32 = 0xff68;
/// `XKB_KEY_Cancel`.
pub const XKB_KEY_Cancel: u32 = 0xff69;
/// `XKB_KEY_Help`.
pub const XKB_KEY_Help: u32 = 0xff6a;
/// `XKB_KEY_Mode_switch`.
pub const XKB_KEY_Mode_switch: u32 = 0xff7e;
/// `XKB_KEY_KP_Enter`.
pub const XKB_KEY_KP_Enter: u32 = 0xff8d;
/// `XKB_KEY_KP_Multiply`.
pub const XKB_KEY_KP_Multiply: u32 = 0xffaa;
/// `XKB_KEY_KP_Add`.
pub const XKB_KEY_KP_Add: u32 = 0xffab;
/// `XKB_KEY_KP_Separator`.
pub const XKB_KEY_KP_Separator: u32 = 0xffac;
/// `XKB_KEY_KP_Subtract`.
pub const XKB_KEY_KP_Subtract: u32 = 0xffad;
/// `XKB_KEY_KP_Decimal`.
pub const XKB_KEY_KP_Decimal: u32 = 0xffae;
/// `XKB_KEY_KP_Divide`.
pub const XKB_KEY_KP_Divide: u32 = 0xffaf;
/// `XKB_KEY_KP_0`.
pub const XKB_KEY_KP_0: u32 = 0xffb0;
/// `XKB_KEY_KP_1`.
pub const XKB_KEY_KP_1: u32 = 0xffb1;
/// `XKB_KEY_KP_2`.
pub const XKB_KEY_KP_2: u32 = 0xffb2;
/// `XKB_KEY_KP_3`.
pub const XKB_KEY_KP_3: u32 = 0xffb3;
/// `XKB_KEY_KP_4`.
pub const XKB_KEY_KP_4: u32 = 0xffb4;
/// `XKB_KEY_KP_5`.
pub const XKB_KEY_KP_5: u32 = 0xffb5;
/// `XKB_KEY_KP_6`.
pub const XKB_KEY_KP_6: u32 = 0xffb6;
/// `XKB_KEY_KP_7`.
pub const XKB_KEY_KP_7: u32 = 0xffb7;
/// `XKB_KEY_KP_8`.
pub const XKB_KEY_KP_8: u32 = 0xffb8;
/// `XKB_KEY_KP_9`.
pub const XKB_KEY_KP_9: u32 = 0xffb9;
/// `XKB_KEY_KP_Equal`.
pub const XKB_KEY_KP_Equal: u32 = 0xffbd;
/// `XKB_KEY_F1`.
pub const XKB_KEY_F1: u32 = 0xffbe;
/// `XKB_KEY_F2`.
pub const XKB_KEY_F2: u32 = 0xffbf;
/// `XKB_KEY_F3`.
pub const XKB_KEY_F3: u32 = 0xffc0;
/// `XKB_KEY_F4`.
pub const XKB_KEY_F4: u32 = 0xffc1;
/// `XKB_KEY_F5`.
pub const XKB_KEY_F5: u32 = 0xffc2;
/// `XKB_KEY_F6`.
pub const XKB_KEY_F6: u32 = 0xffc3;
/// `XKB_KEY_F7`.
pub const XKB_KEY_F7: u32 = 0xffc4;
/// `XKB_KEY_F8`.
pub const XKB_KEY_F8: u32 = 0xffc5;
/// `XKB_KEY_F9`.
pub const XKB_KEY_F9: u32 = 0xffc6;
/// `XKB_KEY_F10`.
pub const XKB_KEY_F10: u32 = 0xffc7;
/// `XKB_KEY_F11`.
pub const XKB_KEY_F11: u32 = 0xffc8;
/// `XKB_KEY_F12`.
pub const XKB_KEY_F12: u32 = 0xffc9;
/// `XKB_KEY_F13`.
pub const XKB_KEY_F13: u32 = 0xffca;
/// `XKB_KEY_F14`.
pub const XKB_KEY_F14: u32 = 0xffcb;
/// `XKB_KEY_F15`.
pub const XKB_KEY_F15: u32 = 0xffcc;
/// `XKB_KEY_F16`.
pub const XKB_KEY_F16: u32 = 0xffcd;
/// `XKB_KEY_F17`.
pub const XKB_KEY_F17: u32 = 0xffce;
/// `XKB_KEY_F18`.
pub const XKB_KEY_F18: u32 = 0xffcf;
/// `XKB_KEY_F19`.
pub const XKB_KEY_F19: u32 = 0xffd0;
/// `XKB_KEY_F20`.
pub const XKB_KEY_F20: u32 = 0xffd1;
/// `XKB_KEY_F21`.
pub const XKB_KEY_F21: u32 = 0xffd2;
/// `XKB_KEY_F22`.
pub const XKB_KEY_F22: u32 = 0xffd3;
/// `XKB_KEY_F23`.
pub const XKB_KEY_F23: u32 = 0xffd4;
/// `XKB_KEY_F24`.
pub const XKB_KEY_F24: u32 = 0xffd5;
/// `XKB_KEY_F25`.
pub const XKB_KEY_F25: u32 = 0xffd6;
/// `XKB_KEY_F26`.
pub const XKB_KEY_F26: u32 = 0xffd7;
/// `XKB_KEY_F27`.
pub const XKB_KEY_F27: u32 = 0xffd8;
/// `XKB_KEY_F28`.
pub const XKB_KEY_F28: u32 = 0xffd9;
/// `XKB_KEY_F29`.
pub const XKB_KEY_F29: u32 = 0xffda;
/// `XKB_KEY_F30`.
pub const XKB_KEY_F30: u32 = 0xffdb;
/// `XKB_KEY_F31`.
pub const XKB_KEY_F31: u32 = 0xffdc;
/// `XKB_KEY_F32`.
pub const XKB_KEY_F32: u32 = 0xffdd;
/// `XKB_KEY_F33`.
pub const XKB_KEY_F33: u32 = 0xffde;
/// `XKB_KEY_F34`.
pub const XKB_KEY_F34: u32 = 0xffdf;
/// `XKB_KEY_F35`.
pub const XKB_KEY_F35: u32 = 0xffe0;
/// `XKB_KEY_Hyper_L`.
pub const XKB_KEY_Hyper_L: u32 = 0xffed;
/// `XKB_KEY_Hyper_R`.
pub const XKB_KEY_Hyper_R: u32 = 0xffee;
/// `XKB_KEY_Delete`.
pub const XKB_KEY_Delete: u32 = 0xffff;
/// `XKB_KEY_BackTab`.
pub const XKB_KEY_BackTab: u32 = 0x1000ff74;
/// `XKB_KEY_XF86AudioLowerVolume`.
pub const XKB_KEY_XF86AudioLowerVolume: u32 = 0x1008ff11;
/// `XKB_KEY_XF86AudioMute`.
pub const XKB_KEY_XF86AudioMute: u32 = 0x1008ff12;
/// `XKB_KEY_XF86AudioRaiseVolume`.
pub const XKB_KEY_XF86AudioRaiseVolume: u32 = 0x1008ff13;
/// `XKB_KEY_XF86AudioPlay`.
pub const XKB_KEY_XF86AudioPlay: u32 = 0x1008ff14;
/// `XKB_KEY_XF86AudioStop`.
pub const XKB_KEY_XF86AudioStop: u32 = 0x1008ff15;
/// `XKB_KEY_XF86AudioPrev`.
pub const XKB_KEY_XF86AudioPrev: u32 = 0x1008ff16;
/// `XKB_KEY_XF86AudioNext`.
pub const XKB_KEY_XF86AudioNext: u32 = 0x1008ff17;
