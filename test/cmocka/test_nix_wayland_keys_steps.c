#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-nix-wayland-keys.h"

#include <xkbcommon/xkbcommon-keysyms.h>

/*
 * Characterization of the keysym tables and the keymap and focused-key
 * rules of the Wayland hotkey platform (libobs/obs-nix-wayland.c), moved to
 * libobs/obs-nix-wayland-keys.c before their Rust port:
 * - obs_nix_wayland_keys_from_keysym(): obs_nix_wayland_key_from_virtual_key();
 * - obs_nix_wayland_keys_to_keysym(): obs_nix_wayland_key_to_virtual_key();
 * - obs_nix_wayland_keys_row(): load_keymap_data()'s rule for one keycode;
 * - obs_nix_wayland_keys_focused(): set_focused_key()'s rule.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No compositor is needed: the keymaps are
 * made up here, each level's keysyms coming from the callback that stands
 * for xkb_keymap_key_get_syms_by_level(). test_wayland_volume_keys,
 * test_wayland_keymap and test_wayland_multiseat cover the callers through
 * the real platform.
 *
 * The tables below are every case of the two switches, in their order.
 */

struct sym_key {
	uint32_t sym;
	obs_key_t key;
};

static const struct sym_key from_table[] = {
	{XKB_KEY_0, OBS_KEY_0},
	{XKB_KEY_1, OBS_KEY_1},
	{XKB_KEY_2, OBS_KEY_2},
	{XKB_KEY_3, OBS_KEY_3},
	{XKB_KEY_4, OBS_KEY_4},
	{XKB_KEY_5, OBS_KEY_5},
	{XKB_KEY_6, OBS_KEY_6},
	{XKB_KEY_7, OBS_KEY_7},
	{XKB_KEY_8, OBS_KEY_8},
	{XKB_KEY_9, OBS_KEY_9},
	{XKB_KEY_A, OBS_KEY_A},
	{XKB_KEY_a, OBS_KEY_A},
	{XKB_KEY_Aacute, OBS_KEY_AACUTE},
	{XKB_KEY_aacute, OBS_KEY_AACUTE},
	{XKB_KEY_Acircumflex, OBS_KEY_ACIRCUMFLEX},
	{XKB_KEY_acircumflex, OBS_KEY_ACIRCUMFLEX},
	{XKB_KEY_acute, OBS_KEY_ACUTE},
	{XKB_KEY_Adiaeresis, OBS_KEY_ADIAERESIS},
	{XKB_KEY_adiaeresis, OBS_KEY_ADIAERESIS},
	{XKB_KEY_AE, OBS_KEY_AE},
	{XKB_KEY_ae, OBS_KEY_AE},
	{XKB_KEY_Agrave, OBS_KEY_AGRAVE},
	{XKB_KEY_agrave, OBS_KEY_AGRAVE},
	{XKB_KEY_ampersand, OBS_KEY_AMPERSAND},
	{XKB_KEY_apostrophe, OBS_KEY_APOSTROPHE},
	{XKB_KEY_Aring, OBS_KEY_ARING},
	{XKB_KEY_aring, OBS_KEY_ARING},
	{XKB_KEY_asciicircum, OBS_KEY_ASCIICIRCUM},
	{XKB_KEY_asciitilde, OBS_KEY_ASCIITILDE},
	{XKB_KEY_asterisk, OBS_KEY_ASTERISK},
	{XKB_KEY_at, OBS_KEY_AT},
	{XKB_KEY_Atilde, OBS_KEY_ATILDE},
	{XKB_KEY_atilde, OBS_KEY_ATILDE},
	{XKB_KEY_B, OBS_KEY_B},
	{XKB_KEY_b, OBS_KEY_B},
	{XKB_KEY_backslash, OBS_KEY_BACKSLASH},
	{XKB_KEY_BackSpace, OBS_KEY_BACKSPACE},
	{XKB_KEY_BackTab, OBS_KEY_BACKTAB},
	{XKB_KEY_bar, OBS_KEY_BAR},
	{XKB_KEY_braceleft, OBS_KEY_BRACELEFT},
	{XKB_KEY_braceright, OBS_KEY_BRACERIGHT},
	{XKB_KEY_bracketleft, OBS_KEY_BRACKETLEFT},
	{XKB_KEY_bracketright, OBS_KEY_BRACKETRIGHT},
	{XKB_KEY_brokenbar, OBS_KEY_BROKENBAR},
	{XKB_KEY_C, OBS_KEY_C},
	{XKB_KEY_c, OBS_KEY_C},
	{XKB_KEY_Cancel, OBS_KEY_CANCEL},
	{XKB_KEY_Ccedilla, OBS_KEY_CCEDILLA},
	{XKB_KEY_ccedilla, OBS_KEY_CCEDILLA},
	{XKB_KEY_cedilla, OBS_KEY_CEDILLA},
	{XKB_KEY_cent, OBS_KEY_CENT},
	{XKB_KEY_Clear, OBS_KEY_CLEAR},
	{XKB_KEY_Codeinput, OBS_KEY_CODEINPUT},
	{XKB_KEY_colon, OBS_KEY_COLON},
	{XKB_KEY_comma, OBS_KEY_COMMA},
	{XKB_KEY_copyright, OBS_KEY_COPYRIGHT},
	{XKB_KEY_currency, OBS_KEY_CURRENCY},
	{XKB_KEY_D, OBS_KEY_D},
	{XKB_KEY_d, OBS_KEY_D},
	{XKB_KEY_dead_abovedot, OBS_KEY_DEAD_ABOVEDOT},
	{XKB_KEY_dead_abovering, OBS_KEY_DEAD_ABOVERING},
	{XKB_KEY_dead_acute, OBS_KEY_DEAD_ACUTE},
	{XKB_KEY_dead_belowdot, OBS_KEY_DEAD_BELOWDOT},
	{XKB_KEY_dead_breve, OBS_KEY_DEAD_BREVE},
	{XKB_KEY_dead_caron, OBS_KEY_DEAD_CARON},
	{XKB_KEY_dead_cedilla, OBS_KEY_DEAD_CEDILLA},
	{XKB_KEY_dead_circumflex, OBS_KEY_DEAD_CIRCUMFLEX},
	{XKB_KEY_dead_diaeresis, OBS_KEY_DEAD_DIAERESIS},
	{XKB_KEY_dead_doubleacute, OBS_KEY_DEAD_DOUBLEACUTE},
	{XKB_KEY_dead_grave, OBS_KEY_DEAD_GRAVE},
	{XKB_KEY_dead_hook, OBS_KEY_DEAD_HOOK},
	{XKB_KEY_dead_horn, OBS_KEY_DEAD_HORN},
	{XKB_KEY_dead_iota, OBS_KEY_DEAD_IOTA},
	{XKB_KEY_dead_macron, OBS_KEY_DEAD_MACRON},
	{XKB_KEY_dead_ogonek, OBS_KEY_DEAD_OGONEK},
	{XKB_KEY_dead_semivoiced_sound, OBS_KEY_DEAD_SEMIVOICED_SOUND},
	{XKB_KEY_dead_tilde, OBS_KEY_DEAD_TILDE},
	{XKB_KEY_dead_voiced_sound, OBS_KEY_DEAD_VOICED_SOUND},
	{XKB_KEY_degree, OBS_KEY_DEGREE},
	{XKB_KEY_Delete, OBS_KEY_DELETE},
	{XKB_KEY_diaeresis, OBS_KEY_DIAERESIS},
	{XKB_KEY_division, OBS_KEY_DIVISION},
	{XKB_KEY_dollar, OBS_KEY_DOLLAR},
	{XKB_KEY_Down, OBS_KEY_DOWN},
	{XKB_KEY_E, OBS_KEY_E},
	{XKB_KEY_e, OBS_KEY_E},
	{XKB_KEY_Eacute, OBS_KEY_EACUTE},
	{XKB_KEY_eacute, OBS_KEY_EACUTE},
	{XKB_KEY_Ecircumflex, OBS_KEY_ECIRCUMFLEX},
	{XKB_KEY_ecircumflex, OBS_KEY_ECIRCUMFLEX},
	{XKB_KEY_Ediaeresis, OBS_KEY_EDIAERESIS},
	{XKB_KEY_ediaeresis, OBS_KEY_EDIAERESIS},
	{XKB_KEY_Egrave, OBS_KEY_EGRAVE},
	{XKB_KEY_egrave, OBS_KEY_EGRAVE},
	{XKB_KEY_Eisu_Shift, OBS_KEY_EISU_SHIFT},
	{XKB_KEY_Eisu_toggle, OBS_KEY_EISU_TOGGLE},
	{XKB_KEY_End, OBS_KEY_END},
	{XKB_KEY_equal, OBS_KEY_EQUAL},
	{XKB_KEY_Escape, OBS_KEY_ESCAPE},
	{XKB_KEY_Eth, OBS_KEY_ETH},
	{XKB_KEY_eth, OBS_KEY_ETH},
	{XKB_KEY_exclam, OBS_KEY_EXCLAM},
	{XKB_KEY_exclamdown, OBS_KEY_EXCLAMDOWN},
	{XKB_KEY_Execute, OBS_KEY_EXECUTE},
	{XKB_KEY_F, OBS_KEY_F},
	{XKB_KEY_f, OBS_KEY_F},
	{XKB_KEY_F1, OBS_KEY_F1},
	{XKB_KEY_F10, OBS_KEY_F10},
	{XKB_KEY_F11, OBS_KEY_F11},
	{XKB_KEY_F12, OBS_KEY_F12},
	{XKB_KEY_F13, OBS_KEY_F13},
	{XKB_KEY_F14, OBS_KEY_F14},
	{XKB_KEY_F15, OBS_KEY_F15},
	{XKB_KEY_F16, OBS_KEY_F16},
	{XKB_KEY_F17, OBS_KEY_F17},
	{XKB_KEY_F18, OBS_KEY_F18},
	{XKB_KEY_F19, OBS_KEY_F19},
	{XKB_KEY_F2, OBS_KEY_F2},
	{XKB_KEY_F20, OBS_KEY_F20},
	{XKB_KEY_F21, OBS_KEY_F21},
	{XKB_KEY_F22, OBS_KEY_F22},
	{XKB_KEY_F23, OBS_KEY_F23},
	{XKB_KEY_F24, OBS_KEY_F24},
	{XKB_KEY_F25, OBS_KEY_F25},
	{XKB_KEY_F26, OBS_KEY_F26},
	{XKB_KEY_F27, OBS_KEY_F27},
	{XKB_KEY_F28, OBS_KEY_F28},
	{XKB_KEY_F29, OBS_KEY_F29},
	{XKB_KEY_F3, OBS_KEY_F3},
	{XKB_KEY_F30, OBS_KEY_F30},
	{XKB_KEY_F31, OBS_KEY_F31},
	{XKB_KEY_F32, OBS_KEY_F32},
	{XKB_KEY_F33, OBS_KEY_F33},
	{XKB_KEY_F34, OBS_KEY_F34},
	{XKB_KEY_F35, OBS_KEY_F35},
	{XKB_KEY_F4, OBS_KEY_F4},
	{XKB_KEY_F5, OBS_KEY_F5},
	{XKB_KEY_F6, OBS_KEY_F6},
	{XKB_KEY_F7, OBS_KEY_F7},
	{XKB_KEY_F8, OBS_KEY_F8},
	{XKB_KEY_F9, OBS_KEY_F9},
	{XKB_KEY_Find, OBS_KEY_FIND},
	{XKB_KEY_G, OBS_KEY_G},
	{XKB_KEY_g, OBS_KEY_G},
	{XKB_KEY_greater, OBS_KEY_GREATER},
	{XKB_KEY_guillemotleft, OBS_KEY_GUILLEMOTLEFT},
	{XKB_KEY_guillemotright, OBS_KEY_GUILLEMOTRIGHT},
	{XKB_KEY_H, OBS_KEY_H},
	{XKB_KEY_h, OBS_KEY_H},
	{XKB_KEY_Hangul, OBS_KEY_HANGUL},
	{XKB_KEY_Hangul_Banja, OBS_KEY_HANGUL_BANJA},
	{XKB_KEY_Hangul_End, OBS_KEY_HANGUL_END},
	{XKB_KEY_Hangul_Hanja, OBS_KEY_HANGUL_HANJA},
	{XKB_KEY_Hangul_Jamo, OBS_KEY_HANGUL_JAMO},
	{XKB_KEY_Hangul_Jeonja, OBS_KEY_HANGUL_JEONJA},
	{XKB_KEY_Hangul_PostHanja, OBS_KEY_HANGUL_POSTHANJA},
	{XKB_KEY_Hangul_PreHanja, OBS_KEY_HANGUL_PREHANJA},
	{XKB_KEY_Hangul_Romaja, OBS_KEY_HANGUL_ROMAJA},
	{XKB_KEY_Hangul_Special, OBS_KEY_HANGUL_SPECIAL},
	{XKB_KEY_Hangul_Start, OBS_KEY_HANGUL_START},
	{XKB_KEY_Hankaku, OBS_KEY_HANKAKU},
	{XKB_KEY_Help, OBS_KEY_HELP},
	{XKB_KEY_Henkan, OBS_KEY_HENKAN},
	{XKB_KEY_Hiragana, OBS_KEY_HIRAGANA},
	{XKB_KEY_Hiragana_Katakana, OBS_KEY_HIRAGANA_KATAKANA},
	{XKB_KEY_Home, OBS_KEY_HOME},
	{XKB_KEY_Hyper_L, OBS_KEY_HYPER_L},
	{XKB_KEY_Hyper_R, OBS_KEY_HYPER_R},
	{XKB_KEY_hyphen, OBS_KEY_HYPHEN},
	{XKB_KEY_I, OBS_KEY_I},
	{XKB_KEY_i, OBS_KEY_I},
	{XKB_KEY_Iacute, OBS_KEY_IACUTE},
	{XKB_KEY_iacute, OBS_KEY_IACUTE},
	{XKB_KEY_Icircumflex, OBS_KEY_ICIRCUMFLEX},
	{XKB_KEY_icircumflex, OBS_KEY_ICIRCUMFLEX},
	{XKB_KEY_Idiaeresis, OBS_KEY_IDIAERESIS},
	{XKB_KEY_idiaeresis, OBS_KEY_IDIAERESIS},
	{XKB_KEY_Igrave, OBS_KEY_IGRAVE},
	{XKB_KEY_igrave, OBS_KEY_IGRAVE},
	{XKB_KEY_Insert, OBS_KEY_INSERT},
	{XKB_KEY_J, OBS_KEY_J},
	{XKB_KEY_j, OBS_KEY_J},
	{XKB_KEY_K, OBS_KEY_K},
	{XKB_KEY_k, OBS_KEY_K},
	{XKB_KEY_Kana_Lock, OBS_KEY_KANA_LOCK},
	{XKB_KEY_Kana_Shift, OBS_KEY_KANA_SHIFT},
	{XKB_KEY_Kanji, OBS_KEY_KANJI},
	{XKB_KEY_Katakana, OBS_KEY_KATAKANA},
	{XKB_KEY_L, OBS_KEY_L},
	{XKB_KEY_l, OBS_KEY_L},
	{XKB_KEY_Left, OBS_KEY_LEFT},
	{XKB_KEY_less, OBS_KEY_LESS},
	{XKB_KEY_M, OBS_KEY_M},
	{XKB_KEY_m, OBS_KEY_M},
	{XKB_KEY_macron, OBS_KEY_MACRON},
	{XKB_KEY_masculine, OBS_KEY_MASCULINE},
	{XKB_KEY_Massyo, OBS_KEY_MASSYO},
	{XKB_KEY_Menu, OBS_KEY_MENU},
	{XKB_KEY_minus, OBS_KEY_MINUS},
	{XKB_KEY_Mode_switch, OBS_KEY_MODE_SWITCH},
	{XKB_KEY_mu, OBS_KEY_MU},
	{XKB_KEY_Muhenkan, OBS_KEY_MUHENKAN},
	{XKB_KEY_MultipleCandidate, OBS_KEY_MULTIPLECANDIDATE},
	{XKB_KEY_multiply, OBS_KEY_MULTIPLY},
	{XKB_KEY_Multi_key, OBS_KEY_MULTI_KEY},
	{XKB_KEY_N, OBS_KEY_N},
	{XKB_KEY_n, OBS_KEY_N},
	{XKB_KEY_nobreakspace, OBS_KEY_NOBREAKSPACE},
	{XKB_KEY_notsign, OBS_KEY_NOTSIGN},
	{XKB_KEY_Ntilde, OBS_KEY_NTILDE},
	{XKB_KEY_ntilde, OBS_KEY_NTILDE},
	{XKB_KEY_numbersign, OBS_KEY_NUMBERSIGN},
	{XKB_KEY_O, OBS_KEY_O},
	{XKB_KEY_o, OBS_KEY_O},
	{XKB_KEY_Oacute, OBS_KEY_OACUTE},
	{XKB_KEY_oacute, OBS_KEY_OACUTE},
	{XKB_KEY_Ocircumflex, OBS_KEY_OCIRCUMFLEX},
	{XKB_KEY_ocircumflex, OBS_KEY_OCIRCUMFLEX},
	{XKB_KEY_Odiaeresis, OBS_KEY_ODIAERESIS},
	{XKB_KEY_odiaeresis, OBS_KEY_ODIAERESIS},
	{XKB_KEY_Ograve, OBS_KEY_OGRAVE},
	{XKB_KEY_ograve, OBS_KEY_OGRAVE},
	{XKB_KEY_onehalf, OBS_KEY_ONEHALF},
	{XKB_KEY_onequarter, OBS_KEY_ONEQUARTER},
	{XKB_KEY_onesuperior, OBS_KEY_ONESUPERIOR},
	{XKB_KEY_Ooblique, OBS_KEY_OOBLIQUE},
	{XKB_KEY_ooblique, OBS_KEY_OOBLIQUE},
	{XKB_KEY_ordfeminine, OBS_KEY_ORDFEMININE},
	{XKB_KEY_Otilde, OBS_KEY_OTILDE},
	{XKB_KEY_otilde, OBS_KEY_OTILDE},
	{XKB_KEY_P, OBS_KEY_P},
	{XKB_KEY_p, OBS_KEY_P},
	{XKB_KEY_paragraph, OBS_KEY_PARAGRAPH},
	{XKB_KEY_parenleft, OBS_KEY_PARENLEFT},
	{XKB_KEY_parenright, OBS_KEY_PARENRIGHT},
	{XKB_KEY_Pause, OBS_KEY_PAUSE},
	{XKB_KEY_percent, OBS_KEY_PERCENT},
	{XKB_KEY_period, OBS_KEY_PERIOD},
	{XKB_KEY_periodcentered, OBS_KEY_PERIODCENTERED},
	{XKB_KEY_plus, OBS_KEY_PLUS},
	{XKB_KEY_plusminus, OBS_KEY_PLUSMINUS},
	{XKB_KEY_PreviousCandidate, OBS_KEY_PREVIOUSCANDIDATE},
	{XKB_KEY_Print, OBS_KEY_PRINT},
	{XKB_KEY_Q, OBS_KEY_Q},
	{XKB_KEY_q, OBS_KEY_Q},
	{XKB_KEY_question, OBS_KEY_QUESTION},
	{XKB_KEY_questiondown, OBS_KEY_QUESTIONDOWN},
	{XKB_KEY_quotedbl, OBS_KEY_QUOTEDBL},
	{XKB_KEY_quoteleft, OBS_KEY_QUOTELEFT},
	{XKB_KEY_R, OBS_KEY_R},
	{XKB_KEY_r, OBS_KEY_R},
	{XKB_KEY_Redo, OBS_KEY_REDO},
	{XKB_KEY_registered, OBS_KEY_REGISTERED},
	{XKB_KEY_Return, OBS_KEY_RETURN},
	{XKB_KEY_Right, OBS_KEY_RIGHT},
	{XKB_KEY_Romaji, OBS_KEY_ROMAJI},
	{XKB_KEY_S, OBS_KEY_S},
	{XKB_KEY_s, OBS_KEY_S},
	{XKB_KEY_section, OBS_KEY_SECTION},
	{XKB_KEY_Select, OBS_KEY_SELECT},
	{XKB_KEY_semicolon, OBS_KEY_SEMICOLON},
	{XKB_KEY_SingleCandidate, OBS_KEY_SINGLECANDIDATE},
	{XKB_KEY_slash, OBS_KEY_SLASH},
	{XKB_KEY_space, OBS_KEY_SPACE},
	{XKB_KEY_ssharp, OBS_KEY_SSHARP},
	{XKB_KEY_sterling, OBS_KEY_STERLING},
	{XKB_KEY_T, OBS_KEY_T},
	{XKB_KEY_t, OBS_KEY_T},
	{XKB_KEY_Tab, OBS_KEY_TAB},
	{XKB_KEY_Thorn, OBS_KEY_THORN},
	{XKB_KEY_thorn, OBS_KEY_THORN},
	{XKB_KEY_threequarters, OBS_KEY_THREEQUARTERS},
	{XKB_KEY_threesuperior, OBS_KEY_THREESUPERIOR},
	{XKB_KEY_Touroku, OBS_KEY_TOUROKU},
	{XKB_KEY_twosuperior, OBS_KEY_TWOSUPERIOR},
	{XKB_KEY_U, OBS_KEY_U},
	{XKB_KEY_u, OBS_KEY_U},
	{XKB_KEY_Uacute, OBS_KEY_UACUTE},
	{XKB_KEY_uacute, OBS_KEY_UACUTE},
	{XKB_KEY_Ucircumflex, OBS_KEY_UCIRCUMFLEX},
	{XKB_KEY_ucircumflex, OBS_KEY_UCIRCUMFLEX},
	{XKB_KEY_Udiaeresis, OBS_KEY_UDIAERESIS},
	{XKB_KEY_udiaeresis, OBS_KEY_UDIAERESIS},
	{XKB_KEY_Ugrave, OBS_KEY_UGRAVE},
	{XKB_KEY_ugrave, OBS_KEY_UGRAVE},
	{XKB_KEY_underscore, OBS_KEY_UNDERSCORE},
	{XKB_KEY_Undo, OBS_KEY_UNDO},
	{XKB_KEY_Up, OBS_KEY_UP},
	{XKB_KEY_V, OBS_KEY_V},
	{XKB_KEY_v, OBS_KEY_V},
	{XKB_KEY_W, OBS_KEY_W},
	{XKB_KEY_w, OBS_KEY_W},
	{XKB_KEY_X, OBS_KEY_X},
	{XKB_KEY_x, OBS_KEY_X},
	{XKB_KEY_Y, OBS_KEY_Y},
	{XKB_KEY_y, OBS_KEY_Y},
	{XKB_KEY_Yacute, OBS_KEY_YACUTE},
	{XKB_KEY_yacute, OBS_KEY_YACUTE},
	{XKB_KEY_Ydiaeresis, OBS_KEY_YDIAERESIS},
	{XKB_KEY_ydiaeresis, OBS_KEY_YDIAERESIS},
	{XKB_KEY_yen, OBS_KEY_YEN},
	{XKB_KEY_Z, OBS_KEY_Z},
	{XKB_KEY_z, OBS_KEY_Z},
	{XKB_KEY_Zenkaku, OBS_KEY_ZENKAKU},
	{XKB_KEY_Zenkaku_Hankaku, OBS_KEY_ZENKAKU_HANKAKU},
	{XKB_KEY_Page_Up, OBS_KEY_PAGEUP},
	{XKB_KEY_Page_Down, OBS_KEY_PAGEDOWN},
	{XKB_KEY_KP_Equal, OBS_KEY_NUMEQUAL},
	{XKB_KEY_KP_Multiply, OBS_KEY_NUMASTERISK},
	{XKB_KEY_KP_Add, OBS_KEY_NUMPLUS},
	{XKB_KEY_KP_Separator, OBS_KEY_NUMCOMMA},
	{XKB_KEY_KP_Subtract, OBS_KEY_NUMMINUS},
	{XKB_KEY_KP_Decimal, OBS_KEY_NUMPERIOD},
	{XKB_KEY_KP_Divide, OBS_KEY_NUMSLASH},
	{XKB_KEY_KP_Enter, OBS_KEY_ENTER},
	{XKB_KEY_KP_0, OBS_KEY_NUM0},
	{XKB_KEY_KP_1, OBS_KEY_NUM1},
	{XKB_KEY_KP_2, OBS_KEY_NUM2},
	{XKB_KEY_KP_3, OBS_KEY_NUM3},
	{XKB_KEY_KP_4, OBS_KEY_NUM4},
	{XKB_KEY_KP_5, OBS_KEY_NUM5},
	{XKB_KEY_KP_6, OBS_KEY_NUM6},
	{XKB_KEY_KP_7, OBS_KEY_NUM7},
	{XKB_KEY_KP_8, OBS_KEY_NUM8},
	{XKB_KEY_KP_9, OBS_KEY_NUM9},
	{XKB_KEY_XF86AudioPlay, OBS_KEY_VK_MEDIA_PLAY_PAUSE},
	{XKB_KEY_XF86AudioStop, OBS_KEY_VK_MEDIA_STOP},
	{XKB_KEY_XF86AudioPrev, OBS_KEY_VK_MEDIA_PREV_TRACK},
	{XKB_KEY_XF86AudioNext, OBS_KEY_VK_MEDIA_NEXT_TRACK},
	{XKB_KEY_XF86AudioMute, OBS_KEY_VK_VOLUME_MUTE},
	{XKB_KEY_XF86AudioRaiseVolume, OBS_KEY_VK_VOLUME_UP},
	{XKB_KEY_XF86AudioLowerVolume, OBS_KEY_VK_VOLUME_DOWN},
};

struct key_sym {
	obs_key_t key;
	uint32_t sym;
};

static const struct key_sym to_table[] = {
	{OBS_KEY_0, XKB_KEY_0},
	{OBS_KEY_1, XKB_KEY_1},
	{OBS_KEY_2, XKB_KEY_2},
	{OBS_KEY_3, XKB_KEY_3},
	{OBS_KEY_4, XKB_KEY_4},
	{OBS_KEY_5, XKB_KEY_5},
	{OBS_KEY_6, XKB_KEY_6},
	{OBS_KEY_7, XKB_KEY_7},
	{OBS_KEY_8, XKB_KEY_8},
	{OBS_KEY_9, XKB_KEY_9},
	{OBS_KEY_A, XKB_KEY_A},
	{OBS_KEY_AACUTE, XKB_KEY_Aacute},
	{OBS_KEY_ACIRCUMFLEX, XKB_KEY_Acircumflex},
	{OBS_KEY_ACUTE, XKB_KEY_acute},
	{OBS_KEY_ADIAERESIS, XKB_KEY_Adiaeresis},
	{OBS_KEY_AE, XKB_KEY_AE},
	{OBS_KEY_AGRAVE, XKB_KEY_Agrave},
	{OBS_KEY_AMPERSAND, XKB_KEY_ampersand},
	{OBS_KEY_APOSTROPHE, XKB_KEY_apostrophe},
	{OBS_KEY_ARING, XKB_KEY_Aring},
	{OBS_KEY_ASCIICIRCUM, XKB_KEY_asciicircum},
	{OBS_KEY_ASCIITILDE, XKB_KEY_asciitilde},
	{OBS_KEY_ASTERISK, XKB_KEY_asterisk},
	{OBS_KEY_AT, XKB_KEY_at},
	{OBS_KEY_ATILDE, XKB_KEY_Atilde},
	{OBS_KEY_B, XKB_KEY_B},
	{OBS_KEY_BACKSLASH, XKB_KEY_backslash},
	{OBS_KEY_BACKSPACE, XKB_KEY_BackSpace},
	{OBS_KEY_BACKTAB, XKB_KEY_BackTab},
	{OBS_KEY_BAR, XKB_KEY_bar},
	{OBS_KEY_BRACELEFT, XKB_KEY_braceleft},
	{OBS_KEY_BRACERIGHT, XKB_KEY_braceright},
	{OBS_KEY_BRACKETLEFT, XKB_KEY_bracketleft},
	{OBS_KEY_BRACKETRIGHT, XKB_KEY_bracketright},
	{OBS_KEY_BROKENBAR, XKB_KEY_brokenbar},
	{OBS_KEY_C, XKB_KEY_C},
	{OBS_KEY_CANCEL, XKB_KEY_Cancel},
	{OBS_KEY_CCEDILLA, XKB_KEY_Ccedilla},
	{OBS_KEY_CEDILLA, XKB_KEY_cedilla},
	{OBS_KEY_CENT, XKB_KEY_cent},
	{OBS_KEY_CLEAR, XKB_KEY_Clear},
	{OBS_KEY_CODEINPUT, XKB_KEY_Codeinput},
	{OBS_KEY_COLON, XKB_KEY_colon},
	{OBS_KEY_COMMA, XKB_KEY_comma},
	{OBS_KEY_COPYRIGHT, XKB_KEY_copyright},
	{OBS_KEY_CURRENCY, XKB_KEY_currency},
	{OBS_KEY_D, XKB_KEY_D},
	{OBS_KEY_DEAD_ABOVEDOT, XKB_KEY_dead_abovedot},
	{OBS_KEY_DEAD_ABOVERING, XKB_KEY_dead_abovering},
	{OBS_KEY_DEAD_ACUTE, XKB_KEY_dead_acute},
	{OBS_KEY_DEAD_BELOWDOT, XKB_KEY_dead_belowdot},
	{OBS_KEY_DEAD_BREVE, XKB_KEY_dead_breve},
	{OBS_KEY_DEAD_CARON, XKB_KEY_dead_caron},
	{OBS_KEY_DEAD_CEDILLA, XKB_KEY_dead_cedilla},
	{OBS_KEY_DEAD_CIRCUMFLEX, XKB_KEY_dead_circumflex},
	{OBS_KEY_DEAD_DIAERESIS, XKB_KEY_dead_diaeresis},
	{OBS_KEY_DEAD_DOUBLEACUTE, XKB_KEY_dead_doubleacute},
	{OBS_KEY_DEAD_GRAVE, XKB_KEY_dead_grave},
	{OBS_KEY_DEAD_HOOK, XKB_KEY_dead_hook},
	{OBS_KEY_DEAD_HORN, XKB_KEY_dead_horn},
	{OBS_KEY_DEAD_IOTA, XKB_KEY_dead_iota},
	{OBS_KEY_DEAD_MACRON, XKB_KEY_dead_macron},
	{OBS_KEY_DEAD_OGONEK, XKB_KEY_dead_ogonek},
	{OBS_KEY_DEAD_SEMIVOICED_SOUND, XKB_KEY_dead_semivoiced_sound},
	{OBS_KEY_DEAD_TILDE, XKB_KEY_dead_tilde},
	{OBS_KEY_DEAD_VOICED_SOUND, XKB_KEY_dead_voiced_sound},
	{OBS_KEY_DEGREE, XKB_KEY_degree},
	{OBS_KEY_DELETE, XKB_KEY_Delete},
	{OBS_KEY_DIAERESIS, XKB_KEY_diaeresis},
	{OBS_KEY_DIVISION, XKB_KEY_division},
	{OBS_KEY_DOLLAR, XKB_KEY_dollar},
	{OBS_KEY_DOWN, XKB_KEY_Down},
	{OBS_KEY_E, XKB_KEY_E},
	{OBS_KEY_EACUTE, XKB_KEY_Eacute},
	{OBS_KEY_ECIRCUMFLEX, XKB_KEY_Ecircumflex},
	{OBS_KEY_EDIAERESIS, XKB_KEY_Ediaeresis},
	{OBS_KEY_EGRAVE, XKB_KEY_Egrave},
	{OBS_KEY_EISU_SHIFT, XKB_KEY_Eisu_Shift},
	{OBS_KEY_EISU_TOGGLE, XKB_KEY_Eisu_toggle},
	{OBS_KEY_END, XKB_KEY_End},
	{OBS_KEY_EQUAL, XKB_KEY_equal},
	{OBS_KEY_ESCAPE, XKB_KEY_Escape},
	{OBS_KEY_ETH, XKB_KEY_ETH},
	{OBS_KEY_EXCLAM, XKB_KEY_exclam},
	{OBS_KEY_EXCLAMDOWN, XKB_KEY_exclamdown},
	{OBS_KEY_EXECUTE, XKB_KEY_Execute},
	{OBS_KEY_F, XKB_KEY_F},
	{OBS_KEY_F1, XKB_KEY_F1},
	{OBS_KEY_F10, XKB_KEY_F10},
	{OBS_KEY_F11, XKB_KEY_F11},
	{OBS_KEY_F12, XKB_KEY_F12},
	{OBS_KEY_F13, XKB_KEY_F13},
	{OBS_KEY_F14, XKB_KEY_F14},
	{OBS_KEY_F15, XKB_KEY_F15},
	{OBS_KEY_F16, XKB_KEY_F16},
	{OBS_KEY_F17, XKB_KEY_F17},
	{OBS_KEY_F18, XKB_KEY_F18},
	{OBS_KEY_F19, XKB_KEY_F19},
	{OBS_KEY_F2, XKB_KEY_F2},
	{OBS_KEY_F20, XKB_KEY_F20},
	{OBS_KEY_F21, XKB_KEY_F21},
	{OBS_KEY_F22, XKB_KEY_F22},
	{OBS_KEY_F23, XKB_KEY_F23},
	{OBS_KEY_F24, XKB_KEY_F24},
	{OBS_KEY_F25, XKB_KEY_F25},
	{OBS_KEY_F26, XKB_KEY_F26},
	{OBS_KEY_F27, XKB_KEY_F27},
	{OBS_KEY_F28, XKB_KEY_F28},
	{OBS_KEY_F29, XKB_KEY_F29},
	{OBS_KEY_F3, XKB_KEY_F3},
	{OBS_KEY_F30, XKB_KEY_F30},
	{OBS_KEY_F31, XKB_KEY_F31},
	{OBS_KEY_F32, XKB_KEY_F32},
	{OBS_KEY_F33, XKB_KEY_F33},
	{OBS_KEY_F34, XKB_KEY_F34},
	{OBS_KEY_F35, XKB_KEY_F35},
	{OBS_KEY_F4, XKB_KEY_F4},
	{OBS_KEY_F5, XKB_KEY_F5},
	{OBS_KEY_F6, XKB_KEY_F6},
	{OBS_KEY_F7, XKB_KEY_F7},
	{OBS_KEY_F8, XKB_KEY_F8},
	{OBS_KEY_F9, XKB_KEY_F9},
	{OBS_KEY_FIND, XKB_KEY_Find},
	{OBS_KEY_G, XKB_KEY_G},
	{OBS_KEY_GREATER, XKB_KEY_greater},
	{OBS_KEY_GUILLEMOTLEFT, XKB_KEY_guillemotleft},
	{OBS_KEY_GUILLEMOTRIGHT, XKB_KEY_guillemotright},
	{OBS_KEY_H, XKB_KEY_H},
	{OBS_KEY_HANGUL, XKB_KEY_Hangul},
	{OBS_KEY_HANGUL_BANJA, XKB_KEY_Hangul_Banja},
	{OBS_KEY_HANGUL_END, XKB_KEY_Hangul_End},
	{OBS_KEY_HANGUL_HANJA, XKB_KEY_Hangul_Hanja},
	{OBS_KEY_HANGUL_JAMO, XKB_KEY_Hangul_Jamo},
	{OBS_KEY_HANGUL_JEONJA, XKB_KEY_Hangul_Jeonja},
	{OBS_KEY_HANGUL_POSTHANJA, XKB_KEY_Hangul_PostHanja},
	{OBS_KEY_HANGUL_PREHANJA, XKB_KEY_Hangul_PreHanja},
	{OBS_KEY_HANGUL_ROMAJA, XKB_KEY_Hangul_Romaja},
	{OBS_KEY_HANGUL_SPECIAL, XKB_KEY_Hangul_Special},
	{OBS_KEY_HANGUL_START, XKB_KEY_Hangul_Start},
	{OBS_KEY_HANKAKU, XKB_KEY_Hankaku},
	{OBS_KEY_HELP, XKB_KEY_Help},
	{OBS_KEY_HENKAN, XKB_KEY_Henkan},
	{OBS_KEY_HIRAGANA, XKB_KEY_Hiragana},
	{OBS_KEY_HIRAGANA_KATAKANA, XKB_KEY_Hiragana_Katakana},
	{OBS_KEY_HOME, XKB_KEY_Home},
	{OBS_KEY_HYPER_L, XKB_KEY_Hyper_L},
	{OBS_KEY_HYPER_R, XKB_KEY_Hyper_R},
	{OBS_KEY_HYPHEN, XKB_KEY_hyphen},
	{OBS_KEY_I, XKB_KEY_I},
	{OBS_KEY_IACUTE, XKB_KEY_Iacute},
	{OBS_KEY_ICIRCUMFLEX, XKB_KEY_Icircumflex},
	{OBS_KEY_IDIAERESIS, XKB_KEY_Idiaeresis},
	{OBS_KEY_IGRAVE, XKB_KEY_Igrave},
	{OBS_KEY_INSERT, XKB_KEY_Insert},
	{OBS_KEY_J, XKB_KEY_J},
	{OBS_KEY_K, XKB_KEY_K},
	{OBS_KEY_KANA_LOCK, XKB_KEY_Kana_Lock},
	{OBS_KEY_KANA_SHIFT, XKB_KEY_Kana_Shift},
	{OBS_KEY_KANJI, XKB_KEY_Kanji},
	{OBS_KEY_KATAKANA, XKB_KEY_Katakana},
	{OBS_KEY_L, XKB_KEY_L},
	{OBS_KEY_LEFT, XKB_KEY_Left},
	{OBS_KEY_LESS, XKB_KEY_less},
	{OBS_KEY_M, XKB_KEY_M},
	{OBS_KEY_MACRON, XKB_KEY_macron},
	{OBS_KEY_MASCULINE, XKB_KEY_masculine},
	{OBS_KEY_MASSYO, XKB_KEY_Massyo},
	{OBS_KEY_MENU, XKB_KEY_Menu},
	{OBS_KEY_MINUS, XKB_KEY_minus},
	{OBS_KEY_MODE_SWITCH, XKB_KEY_Mode_switch},
	{OBS_KEY_MU, XKB_KEY_mu},
	{OBS_KEY_MUHENKAN, XKB_KEY_Muhenkan},
	{OBS_KEY_MULTI_KEY, XKB_KEY_Multi_key},
	{OBS_KEY_MULTIPLECANDIDATE, XKB_KEY_MultipleCandidate},
	{OBS_KEY_MULTIPLY, XKB_KEY_multiply},
	{OBS_KEY_N, XKB_KEY_N},
	{OBS_KEY_NOBREAKSPACE, XKB_KEY_nobreakspace},
	{OBS_KEY_NOTSIGN, XKB_KEY_notsign},
	{OBS_KEY_NTILDE, XKB_KEY_Ntilde},
	{OBS_KEY_NUMBERSIGN, XKB_KEY_numbersign},
	{OBS_KEY_O, XKB_KEY_O},
	{OBS_KEY_OACUTE, XKB_KEY_Oacute},
	{OBS_KEY_OCIRCUMFLEX, XKB_KEY_Ocircumflex},
	{OBS_KEY_ODIAERESIS, XKB_KEY_Odiaeresis},
	{OBS_KEY_OGRAVE, XKB_KEY_Ograve},
	{OBS_KEY_ONEHALF, XKB_KEY_onehalf},
	{OBS_KEY_ONEQUARTER, XKB_KEY_onequarter},
	{OBS_KEY_ONESUPERIOR, XKB_KEY_onesuperior},
	{OBS_KEY_OOBLIQUE, XKB_KEY_Ooblique},
	{OBS_KEY_ORDFEMININE, XKB_KEY_ordfeminine},
	{OBS_KEY_OTILDE, XKB_KEY_Otilde},
	{OBS_KEY_P, XKB_KEY_P},
	{OBS_KEY_PARAGRAPH, XKB_KEY_paragraph},
	{OBS_KEY_PARENLEFT, XKB_KEY_parenleft},
	{OBS_KEY_PARENRIGHT, XKB_KEY_parenright},
	{OBS_KEY_PAUSE, XKB_KEY_Pause},
	{OBS_KEY_PERCENT, XKB_KEY_percent},
	{OBS_KEY_PERIOD, XKB_KEY_period},
	{OBS_KEY_PERIODCENTERED, XKB_KEY_periodcentered},
	{OBS_KEY_PLUS, XKB_KEY_plus},
	{OBS_KEY_PLUSMINUS, XKB_KEY_plusminus},
	{OBS_KEY_PREVIOUSCANDIDATE, XKB_KEY_PreviousCandidate},
	{OBS_KEY_PRINT, XKB_KEY_Print},
	{OBS_KEY_Q, XKB_KEY_Q},
	{OBS_KEY_QUESTION, XKB_KEY_question},
	{OBS_KEY_QUESTIONDOWN, XKB_KEY_questiondown},
	{OBS_KEY_QUOTEDBL, XKB_KEY_quotedbl},
	{OBS_KEY_QUOTELEFT, XKB_KEY_quoteleft},
	{OBS_KEY_R, XKB_KEY_R},
	{OBS_KEY_REDO, XKB_KEY_Redo},
	{OBS_KEY_REGISTERED, XKB_KEY_registered},
	{OBS_KEY_RETURN, XKB_KEY_Return},
	{OBS_KEY_RIGHT, XKB_KEY_Right},
	{OBS_KEY_ROMAJI, XKB_KEY_Romaji},
	{OBS_KEY_S, XKB_KEY_S},
	{OBS_KEY_SECTION, XKB_KEY_section},
	{OBS_KEY_SELECT, XKB_KEY_Select},
	{OBS_KEY_SEMICOLON, XKB_KEY_semicolon},
	{OBS_KEY_SINGLECANDIDATE, XKB_KEY_SingleCandidate},
	{OBS_KEY_SLASH, XKB_KEY_slash},
	{OBS_KEY_SPACE, XKB_KEY_space},
	{OBS_KEY_SSHARP, XKB_KEY_ssharp},
	{OBS_KEY_STERLING, XKB_KEY_sterling},
	{OBS_KEY_T, XKB_KEY_T},
	{OBS_KEY_TAB, XKB_KEY_Tab},
	{OBS_KEY_THORN, XKB_KEY_THORN},
	{OBS_KEY_THREEQUARTERS, XKB_KEY_threequarters},
	{OBS_KEY_THREESUPERIOR, XKB_KEY_threesuperior},
	{OBS_KEY_TOUROKU, XKB_KEY_Touroku},
	{OBS_KEY_TWOSUPERIOR, XKB_KEY_twosuperior},
	{OBS_KEY_U, XKB_KEY_U},
	{OBS_KEY_UACUTE, XKB_KEY_Uacute},
	{OBS_KEY_UCIRCUMFLEX, XKB_KEY_Ucircumflex},
	{OBS_KEY_UDIAERESIS, XKB_KEY_Udiaeresis},
	{OBS_KEY_UGRAVE, XKB_KEY_Ugrave},
	{OBS_KEY_UNDERSCORE, XKB_KEY_underscore},
	{OBS_KEY_UNDO, XKB_KEY_Undo},
	{OBS_KEY_UP, XKB_KEY_Up},
	{OBS_KEY_V, XKB_KEY_V},
	{OBS_KEY_W, XKB_KEY_W},
	{OBS_KEY_X, XKB_KEY_X},
	{OBS_KEY_Y, XKB_KEY_Y},
	{OBS_KEY_YACUTE, XKB_KEY_Yacute},
	{OBS_KEY_YDIAERESIS, XKB_KEY_Ydiaeresis},
	{OBS_KEY_YEN, XKB_KEY_yen},
	{OBS_KEY_Z, XKB_KEY_Z},
	{OBS_KEY_ZENKAKU, XKB_KEY_Zenkaku},
	{OBS_KEY_ZENKAKU_HANKAKU, XKB_KEY_Zenkaku_Hankaku},
	{OBS_KEY_PAGEUP, XKB_KEY_Page_Up},
	{OBS_KEY_PAGEDOWN, XKB_KEY_Page_Down},
	{OBS_KEY_NUMEQUAL, XKB_KEY_KP_Equal},
	{OBS_KEY_NUMASTERISK, XKB_KEY_KP_Multiply},
	{OBS_KEY_NUMPLUS, XKB_KEY_KP_Add},
	{OBS_KEY_NUMCOMMA, XKB_KEY_KP_Separator},
	{OBS_KEY_NUMMINUS, XKB_KEY_KP_Subtract},
	{OBS_KEY_NUMPERIOD, XKB_KEY_KP_Decimal},
	{OBS_KEY_NUMSLASH, XKB_KEY_KP_Divide},
	{OBS_KEY_ENTER, XKB_KEY_KP_Enter},
	{OBS_KEY_NUM0, XKB_KEY_KP_0},
	{OBS_KEY_NUM1, XKB_KEY_KP_1},
	{OBS_KEY_NUM2, XKB_KEY_KP_2},
	{OBS_KEY_NUM3, XKB_KEY_KP_3},
	{OBS_KEY_NUM4, XKB_KEY_KP_4},
	{OBS_KEY_NUM5, XKB_KEY_KP_5},
	{OBS_KEY_NUM6, XKB_KEY_KP_6},
	{OBS_KEY_NUM7, XKB_KEY_KP_7},
	{OBS_KEY_NUM8, XKB_KEY_KP_8},
	{OBS_KEY_NUM9, XKB_KEY_KP_9},
	{OBS_KEY_VK_MEDIA_PLAY_PAUSE, XKB_KEY_XF86AudioPlay},
	{OBS_KEY_VK_MEDIA_STOP, XKB_KEY_XF86AudioStop},
	{OBS_KEY_VK_MEDIA_PREV_TRACK, XKB_KEY_XF86AudioPrev},
	{OBS_KEY_VK_MEDIA_NEXT_TRACK, XKB_KEY_XF86AudioNext},
	{OBS_KEY_VK_VOLUME_MUTE, XKB_KEY_XF86AudioMute},
	{OBS_KEY_VK_VOLUME_DOWN, XKB_KEY_XF86AudioLowerVolume},
	{OBS_KEY_VK_VOLUME_UP, XKB_KEY_XF86AudioRaiseVolume},
};

#define ARRAY_LEN(a) (sizeof(a) / sizeof((a)[0]))

/* Every keysym the from-table names maps to its key, and no other keysym in
 * the Latin-1, keypad, function, vendor (BackTab) and XF86 ranges maps to
 * any. */
static void from_keysym_table(void **state)
{
	(void)state;
	assert_int_equal(ARRAY_LEN(from_table), 332);
	for (size_t i = 0; i < ARRAY_LEN(from_table); i++)
		assert_int_equal(obs_nix_wayland_keys_from_keysym((int)from_table[i].sym), from_table[i].key);

	size_t mapped = 0;
	for (int sym = 0; sym < 0x20000; sym++)
		mapped += obs_nix_wayland_keys_from_keysym(sym) != OBS_KEY_NONE;
	for (int sym = 0x1000FF00; sym < 0x10010000; sym++)
		mapped += obs_nix_wayland_keys_from_keysym(sym) != OBS_KEY_NONE;
	for (int sym = 0x1008FE00; sym < 0x10090000; sym++)
		mapped += obs_nix_wayland_keys_from_keysym(sym) != OBS_KEY_NONE;
	assert_int_equal(mapped, ARRAY_LEN(from_table));

	assert_int_equal(obs_nix_wayland_keys_from_keysym(0), OBS_KEY_NONE);
	assert_int_equal(obs_nix_wayland_keys_from_keysym(-1), OBS_KEY_NONE);
	assert_int_equal(obs_nix_wayland_keys_from_keysym(INT32_MAX), OBS_KEY_NONE);
	assert_int_equal(obs_nix_wayland_keys_from_keysym(INT32_MIN), OBS_KEY_NONE);
	assert_int_equal(obs_nix_wayland_keys_from_keysym(XKB_KEY_NoSymbol), OBS_KEY_NONE);
	assert_int_equal(obs_nix_wayland_keys_from_keysym(XKB_KEY_VoidSymbol), OBS_KEY_NONE);
}

/* Every key the to-table names gives its keysym; every other key, and a
 * value outside the enum, gives 0. */
static void to_keysym_table(void **state)
{
	(void)state;
	assert_int_equal(ARRAY_LEN(to_table), 275);
	int expected[OBS_KEY_LAST_VALUE] = {0};
	for (size_t i = 0; i < ARRAY_LEN(to_table); i++) {
		assert_int_equal(expected[to_table[i].key], 0);
		expected[to_table[i].key] = (int)to_table[i].sym;
	}
	for (int key = 0; key < OBS_KEY_LAST_VALUE; key++)
		assert_int_equal(obs_nix_wayland_keys_to_keysym((obs_key_t)key), expected[key]);

	assert_int_equal(obs_nix_wayland_keys_to_keysym(OBS_KEY_NONE), 0);
	assert_int_equal(obs_nix_wayland_keys_to_keysym((obs_key_t)-1), 0);
	assert_int_equal(obs_nix_wayland_keys_to_keysym(OBS_KEY_LAST_VALUE), 0);
}

/* Every key with a keysym comes back as itself. A keysym comes back as
 * itself too, except the 57 lowercase Latin-1 letters, which map to the
 * key of their uppercase letter and so come back uppercase. */
static void round_trips(void **state)
{
	(void)state;
	for (int key = 0; key < OBS_KEY_LAST_VALUE; key++) {
		int sym = obs_nix_wayland_keys_to_keysym((obs_key_t)key);
		if (sym)
			assert_int_equal(obs_nix_wayland_keys_from_keysym(sym), key);
	}

	size_t lowercase = 0;
	for (size_t i = 0; i < ARRAY_LEN(from_table); i++) {
		int back = obs_nix_wayland_keys_to_keysym(from_table[i].key);
		if (back == (int)from_table[i].sym)
			continue;
		lowercase++;
		assert_true((from_table[i].sym >= XKB_KEY_a && from_table[i].sym <= XKB_KEY_z) ||
			    (from_table[i].sym >= XKB_KEY_agrave && from_table[i].sym <= XKB_KEY_ydiaeresis));
	}
	assert_int_equal(lowercase, 57);
	assert_int_equal(obs_nix_wayland_keys_to_keysym(obs_nix_wayland_keys_from_keysym(XKB_KEY_q)), XKB_KEY_Q);
	assert_int_equal(obs_nix_wayland_keys_to_keysym(obs_nix_wayland_keys_from_keysym(XKB_KEY_oslash)),
			 XKB_KEY_Oslash);
	assert_int_equal(obs_nix_wayland_keys_to_keysym(obs_nix_wayland_keys_from_keysym(XKB_KEY_ydiaeresis)),
			 XKB_KEY_Ydiaeresis);
}

/* A made-up keymap row: each level's keysym count and keysyms. */
struct fake_row {
	int count[OBS_NIX_WAYLAND_KEYS_MAX_SHIFT_LEVELS];
	uint32_t syms[OBS_NIX_WAYLAND_KEYS_MAX_SHIFT_LEVELS][2];
	/* The levels asked for, in order. */
	uint32_t asked[8];
	size_t num_asked;
};

static int fake_level_syms(void *data, uint32_t level, const uint32_t **syms)
{
	struct fake_row *row = data;
	assert_true(row->num_asked < 8);
	row->asked[row->num_asked++] = level;
	*syms = row->syms[level];
	return row->count[level];
}

static uint32_t obs_to_key[OBS_KEY_LAST_VALUE];
static uint32_t key_to_sym[OBS_NIX_WAYLAND_KEYS_MAX_SHIFT_LEVELS][OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES];
/* The platform's record of the key each keycode was pressed as. */
static obs_key_t pressed_as[OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES];

static int clear_maps(void **state)
{
	(void)state;
	memset(obs_to_key, 0, sizeof(obs_to_key));
	memset(key_to_sym, 0, sizeof(key_to_sym));
	memset(pressed_as, 0, sizeof(pressed_as));
	return 0;
}

/* The C rule leaves the record alone. */
static int record_untouched(void **state)
{
	(void)state;
	for (size_t i = 0; i < OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES; i++)
		assert_int_equal(pressed_as[i], OBS_KEY_NONE);
	return 0;
}

static size_t nonzero(const uint32_t *values, size_t num)
{
	size_t n = 0;
	for (size_t i = 0; i < num; i++)
		n += values[i] != 0;
	return n;
}

/* Each level is asked for once, in order; one with a count below 1 stores
 * nothing; any other stores its first keysym. */
static void row_levels(void **state)
{
	(void)state;
	struct fake_row row = {
		.count = {1, 0, 2, -1},
		.syms = {{XKB_KEY_a, 0}, {XKB_KEY_b, 0}, {XKB_KEY_c, XKB_KEY_d}, {XKB_KEY_e, 0}},
	};
	obs_nix_wayland_keys_row(38, fake_level_syms, &row, obs_to_key, key_to_sym);

	assert_int_equal(row.num_asked, 4);
	for (uint32_t level = 0; level < 4; level++)
		assert_int_equal(row.asked[level], level);
	assert_int_equal(key_to_sym[0][38], XKB_KEY_a);
	assert_int_equal(key_to_sym[1][38], 0);
	assert_int_equal(key_to_sym[2][38], XKB_KEY_c);
	assert_int_equal(key_to_sym[3][38], 0);
	assert_int_equal(obs_to_key[OBS_KEY_A], 38);
	assert_int_equal(obs_to_key[OBS_KEY_C], 38);
	assert_int_equal(obs_to_key[OBS_KEY_B], 0);
	assert_int_equal(obs_to_key[OBS_KEY_D], 0);
	assert_int_equal(obs_to_key[OBS_KEY_E], 0);
	assert_int_equal(nonzero(obs_to_key, OBS_KEY_LAST_VALUE), 2);
	assert_int_equal(nonzero(&key_to_sym[0][0], 4 * OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES), 2);
}

/* A keycode at or above the bound stores nothing and asks for no level;
 * 255 is the last one stored. */
static void row_bound(void **state)
{
	(void)state;
	struct fake_row row = {.count = {1, 1, 1, 1}, .syms = {{XKB_KEY_a}, {XKB_KEY_A}, {XKB_KEY_b}, {XKB_KEY_B}}};
	obs_nix_wayland_keys_row(256, fake_level_syms, &row, obs_to_key, key_to_sym);
	obs_nix_wayland_keys_row(UINT32_MAX, fake_level_syms, &row, obs_to_key, key_to_sym);
	assert_int_equal(row.num_asked, 0);
	assert_int_equal(nonzero(obs_to_key, OBS_KEY_LAST_VALUE), 0);
	assert_int_equal(nonzero(&key_to_sym[0][0], 4 * OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES), 0);

	obs_nix_wayland_keys_row(255, fake_level_syms, &row, obs_to_key, key_to_sym);
	assert_int_equal(row.num_asked, 4);
	assert_int_equal(key_to_sym[0][255], XKB_KEY_a);
	assert_int_equal(key_to_sym[1][255], XKB_KEY_A);
	assert_int_equal(key_to_sym[2][255], XKB_KEY_b);
	assert_int_equal(key_to_sym[3][255], XKB_KEY_B);
	assert_int_equal(obs_to_key[OBS_KEY_A], 255);
	assert_int_equal(obs_to_key[OBS_KEY_B], 255);
}

/* The first keycode stored for a key wins, whether a later one has it at
 * the same level or another, and whichever case of the letter it is (LSGT
 * and Shift+AB08 both give `<` on the US layout). A keycode stored 0
 * counts as none stored. */
static void row_first_keycode_wins(void **state)
{
	(void)state;
	struct fake_row less = {.count = {1}, .syms = {{XKB_KEY_less}}};
	struct fake_row comma = {.count = {1, 1}, .syms = {{XKB_KEY_comma}, {XKB_KEY_less}}};
	obs_nix_wayland_keys_row(94, fake_level_syms, &less, obs_to_key, key_to_sym);
	obs_nix_wayland_keys_row(59, fake_level_syms, &comma, obs_to_key, key_to_sym);
	assert_int_equal(obs_to_key[OBS_KEY_LESS], 94);
	assert_int_equal(obs_to_key[OBS_KEY_COMMA], 59);
	assert_int_equal(key_to_sym[0][94], XKB_KEY_less);
	assert_int_equal(key_to_sym[1][59], XKB_KEY_less);

	struct fake_row upper = {.count = {1}, .syms = {{XKB_KEY_A}}};
	struct fake_row lower = {.count = {1}, .syms = {{XKB_KEY_a}}};
	obs_nix_wayland_keys_row(40, fake_level_syms, &upper, obs_to_key, key_to_sym);
	obs_nix_wayland_keys_row(38, fake_level_syms, &lower, obs_to_key, key_to_sym);
	assert_int_equal(obs_to_key[OBS_KEY_A], 40);

	/* Keycode 0 is stored, and a later keycode then overwrites it. */
	struct fake_row zero = {.count = {1}, .syms = {{XKB_KEY_z}}};
	obs_nix_wayland_keys_row(0, fake_level_syms, &zero, obs_to_key, key_to_sym);
	assert_int_equal(key_to_sym[0][0], XKB_KEY_z);
	assert_int_equal(obs_to_key[OBS_KEY_Z], 0);
	obs_nix_wayland_keys_row(52, fake_level_syms, &zero, obs_to_key, key_to_sym);
	assert_int_equal(obs_to_key[OBS_KEY_Z], 52);
}

/* A keysym with no key stores its keycode as OBS_KEY_NONE's, first wins
 * (characterized, not endorsed: nothing reads that entry for a key), and
 * still stores the keysym. */
static void row_unknown_keysym(void **state)
{
	(void)state;
	struct fake_row first = {.count = {1}, .syms = {{XKB_KEY_ISO_Level3_Shift}}};
	struct fake_row second = {.count = {1}, .syms = {{XKB_KEY_VoidSymbol}}};
	obs_nix_wayland_keys_row(92, fake_level_syms, &first, obs_to_key, key_to_sym);
	obs_nix_wayland_keys_row(108, fake_level_syms, &second, obs_to_key, key_to_sym);
	assert_int_equal(obs_to_key[OBS_KEY_NONE], 92);
	assert_int_equal(key_to_sym[0][92], XKB_KEY_ISO_Level3_Shift);
	assert_int_equal(key_to_sym[0][108], XKB_KEY_VoidSymbol);
	assert_int_equal(nonzero(obs_to_key, OBS_KEY_LAST_VALUE), 1);
}

/* The keycode is the scancode plus 8: a key is reported as the key of its
 * level-0 keysym, the same for a press and its release on one layout. */
static void focused_key(void **state)
{
	(void)state;
	key_to_sym[0][24] = XKB_KEY_q;
	key_to_sym[1][24] = XKB_KEY_Q;
	key_to_sym[0][65] = XKB_KEY_space;
	key_to_sym[0][122] = XKB_KEY_XF86AudioLowerVolume;

	obs_key_t key = OBS_KEY_NONE;
	assert_true(obs_nix_wayland_keys_focused(16, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_Q);
	key = OBS_KEY_NONE;
	assert_true(obs_nix_wayland_keys_focused(16, false, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_Q);
	assert_true(obs_nix_wayland_keys_focused(57, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_SPACE);
	assert_true(obs_nix_wayland_keys_focused(114, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_VK_VOLUME_DOWN);
}

/* A keycode at or above the bound and a keycode with no level-0 keysym
 * report nothing and leave *key alone; a level-1 keysym is not used. */
static void focused_nothing(void **state)
{
	(void)state;
	key_to_sym[1][30] = XKB_KEY_A;
	key_to_sym[0][255] = XKB_KEY_b;

	obs_key_t key = OBS_KEY_F13;
	assert_false(obs_nix_wayland_keys_focused(22, true, key_to_sym[0], pressed_as, &key));
	assert_false(obs_nix_wayland_keys_focused(248, true, key_to_sym[0], pressed_as, &key));
	assert_false(obs_nix_wayland_keys_focused(0xFFFFFFF0u, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_F13);
	assert_true(obs_nix_wayland_keys_focused(247, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_B);
}

/* Odd cases, characterized, not endorsed: the scancode plus 8 wraps as a
 * uint32_t, so 0xFFFFFFF8 to 0xFFFFFFFF are keycodes 0 to 7; a level-0
 * keysym with no key reports OBS_KEY_NONE. */
static void focused_odd(void **state)
{
	(void)state;
	key_to_sym[0][0] = XKB_KEY_c;
	key_to_sym[0][7] = XKB_KEY_d;
	key_to_sym[0][92] = XKB_KEY_ISO_Level3_Shift;

	obs_key_t key = OBS_KEY_F13;
	assert_true(obs_nix_wayland_keys_focused(0xFFFFFFF8u, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_C);
	assert_true(obs_nix_wayland_keys_focused(0xFFFFFFFFu, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_D);
	assert_true(obs_nix_wayland_keys_focused(84, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_NONE);
}

/* A press, a layout change that moves another keysym onto the keycode, and
 * the release: each is looked up on the layout of its own moment, so the
 * release reports another key than the press did (characterized, not
 * endorsed: the pressed key's hotkey state stays set). */
static void focused_across_layout_change(void **state)
{
	(void)state;
	key_to_sym[0][24] = XKB_KEY_q;
	obs_key_t key = OBS_KEY_NONE;
	assert_true(obs_nix_wayland_keys_focused(16, true, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_Q);

	key_to_sym[0][24] = XKB_KEY_a;
	assert_true(obs_nix_wayland_keys_focused(16, false, key_to_sym[0], pressed_as, &key));
	assert_int_equal(key, OBS_KEY_A);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(from_keysym_table),
		cmocka_unit_test(to_keysym_table),
		cmocka_unit_test(round_trips),
		cmocka_unit_test_setup(row_levels, clear_maps),
		cmocka_unit_test_setup(row_bound, clear_maps),
		cmocka_unit_test_setup(row_first_keycode_wins, clear_maps),
		cmocka_unit_test_setup(row_unknown_keysym, clear_maps),
		cmocka_unit_test_setup_teardown(focused_key, clear_maps, record_untouched),
		cmocka_unit_test_setup_teardown(focused_nothing, clear_maps, record_untouched),
		cmocka_unit_test_setup_teardown(focused_odd, clear_maps, record_untouched),
		cmocka_unit_test_setup_teardown(focused_across_layout_change, clear_maps, record_untouched),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
