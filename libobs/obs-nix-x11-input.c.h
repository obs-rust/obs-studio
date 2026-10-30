/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2014 by Zachary Lund <admin@computerquip.com>
    Copyright (C) 2019 by Jason Francis <cycl0ps@tuta.io>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs-ported-nix-x11-input.pch.h"

/* Ported to rust/obs-hotkey (nix_x11_input): the hybrid libobs drops this
 * file. */

/* The X11 keysym table and mouse button answers of obs-nix-x11.c, moved: the
 * table and the answers, on plain values. obs-nix-x11.c keeps every X call and
 * the state. Declared in obs-nix-x11-input.h without EXPORT. */

#define MOUSE_1 (1 << 16)
#define MOUSE_2 (2 << 16)
#define MOUSE_3 (3 << 16)
#define MOUSE_4 (4 << 16)
#define MOUSE_5 (5 << 16)

/* get_keysym(), with each keysym written as its value (the X header's name in
 * the comment). */
int obs_nix_x11_input_keysym(obs_key_t key)
{
	switch (key) {
	case OBS_KEY_RETURN:
		return 0xff0d; /* XK_Return */
	case OBS_KEY_ESCAPE:
		return 0xff1b; /* XK_Escape */
	case OBS_KEY_TAB:
		return 0xff09; /* XK_Tab */
	case OBS_KEY_BACKSPACE:
		return 0xff08; /* XK_BackSpace */
	case OBS_KEY_INSERT:
		return 0xff63; /* XK_Insert */
	case OBS_KEY_DELETE:
		return 0xffff; /* XK_Delete */
	case OBS_KEY_PAUSE:
		return 0xff13; /* XK_Pause */
	case OBS_KEY_PRINT:
		return 0xff61; /* XK_Print */
	case OBS_KEY_HOME:
		return 0xff50; /* XK_Home */
	case OBS_KEY_END:
		return 0xff57; /* XK_End */
	case OBS_KEY_LEFT:
		return 0xff51; /* XK_Left */
	case OBS_KEY_UP:
		return 0xff52; /* XK_Up */
	case OBS_KEY_RIGHT:
		return 0xff53; /* XK_Right */
	case OBS_KEY_DOWN:
		return 0xff54; /* XK_Down */
	case OBS_KEY_PAGEUP:
		return 0xff55; /* XK_Prior */
	case OBS_KEY_PAGEDOWN:
		return 0xff56; /* XK_Next */

	case OBS_KEY_SHIFT:
		return 0xffe1; /* XK_Shift_L */
	case OBS_KEY_CONTROL:
		return 0xffe3; /* XK_Control_L */
	case OBS_KEY_ALT:
		return 0xffe9; /* XK_Alt_L */
	case OBS_KEY_CAPSLOCK:
		return 0xffe5; /* XK_Caps_Lock */
	case OBS_KEY_NUMLOCK:
		return 0xff7f; /* XK_Num_Lock */
	case OBS_KEY_SCROLLLOCK:
		return 0xff14; /* XK_Scroll_Lock */

	case OBS_KEY_F1:
		return 0xffbe; /* XK_F1 */
	case OBS_KEY_F2:
		return 0xffbf; /* XK_F2 */
	case OBS_KEY_F3:
		return 0xffc0; /* XK_F3 */
	case OBS_KEY_F4:
		return 0xffc1; /* XK_F4 */
	case OBS_KEY_F5:
		return 0xffc2; /* XK_F5 */
	case OBS_KEY_F6:
		return 0xffc3; /* XK_F6 */
	case OBS_KEY_F7:
		return 0xffc4; /* XK_F7 */
	case OBS_KEY_F8:
		return 0xffc5; /* XK_F8 */
	case OBS_KEY_F9:
		return 0xffc6; /* XK_F9 */
	case OBS_KEY_F10:
		return 0xffc7; /* XK_F10 */
	case OBS_KEY_F11:
		return 0xffc8; /* XK_F11 */
	case OBS_KEY_F12:
		return 0xffc9; /* XK_F12 */
	case OBS_KEY_F13:
		return 0xffca; /* XK_F13 */
	case OBS_KEY_F14:
		return 0xffcb; /* XK_F14 */
	case OBS_KEY_F15:
		return 0xffcc; /* XK_F15 */
	case OBS_KEY_F16:
		return 0xffcd; /* XK_F16 */
	case OBS_KEY_F17:
		return 0xffce; /* XK_F17 */
	case OBS_KEY_F18:
		return 0xffcf; /* XK_F18 */
	case OBS_KEY_F19:
		return 0xffd0; /* XK_F19 */
	case OBS_KEY_F20:
		return 0xffd1; /* XK_F20 */
	case OBS_KEY_F21:
		return 0xffd2; /* XK_F21 */
	case OBS_KEY_F22:
		return 0xffd3; /* XK_F22 */
	case OBS_KEY_F23:
		return 0xffd4; /* XK_F23 */
	case OBS_KEY_F24:
		return 0xffd5; /* XK_F24 */
	case OBS_KEY_F25:
		return 0xffd6; /* XK_F25 */
	case OBS_KEY_F26:
		return 0xffd7; /* XK_F26 */
	case OBS_KEY_F27:
		return 0xffd8; /* XK_F27 */
	case OBS_KEY_F28:
		return 0xffd9; /* XK_F28 */
	case OBS_KEY_F29:
		return 0xffda; /* XK_F29 */
	case OBS_KEY_F30:
		return 0xffdb; /* XK_F30 */
	case OBS_KEY_F31:
		return 0xffdc; /* XK_F31 */
	case OBS_KEY_F32:
		return 0xffdd; /* XK_F32 */
	case OBS_KEY_F33:
		return 0xffde; /* XK_F33 */
	case OBS_KEY_F34:
		return 0xffdf; /* XK_F34 */
	case OBS_KEY_F35:
		return 0xffe0; /* XK_F35 */

	case OBS_KEY_MENU:
		return 0xff67; /* XK_Menu */
	case OBS_KEY_HYPER_L:
		return 0xffed; /* XK_Hyper_L */
	case OBS_KEY_HYPER_R:
		return 0xffee; /* XK_Hyper_R */
	case OBS_KEY_HELP:
		return 0xff6a; /* XK_Help */
	case OBS_KEY_CANCEL:
		return 0xff69; /* XK_Cancel */
	case OBS_KEY_FIND:
		return 0xff68; /* XK_Find */
	case OBS_KEY_REDO:
		return 0xff66; /* XK_Redo */
	case OBS_KEY_UNDO:
		return 0xff65; /* XK_Undo */
	case OBS_KEY_SPACE:
		return 0x20; /* XK_space */

	case OBS_KEY_COPY:
		return 0x1008ff57; /* XF86XK_Copy */
	case OBS_KEY_CUT:
		return 0x1008ff58; /* XF86XK_Cut */
	case OBS_KEY_OPEN:
		return 0x1008ff6b; /* XF86XK_Open */
	case OBS_KEY_PASTE:
		return 0x1008ff6d; /* XF86XK_Paste */
	case OBS_KEY_FRONT:
		return 0x1005ff71; /* SunXK_Front */
	case OBS_KEY_PROPS:
		return 0x1005ff70; /* SunXK_Props */

	case OBS_KEY_EXCLAM:
		return 0x21; /* XK_exclam */
	case OBS_KEY_QUOTEDBL:
		return 0x22; /* XK_quotedbl */
	case OBS_KEY_NUMBERSIGN:
		return 0x23; /* XK_numbersign */
	case OBS_KEY_DOLLAR:
		return 0x24; /* XK_dollar */
	case OBS_KEY_PERCENT:
		return 0x25; /* XK_percent */
	case OBS_KEY_AMPERSAND:
		return 0x26; /* XK_ampersand */
	case OBS_KEY_APOSTROPHE:
		return 0x27; /* XK_apostrophe */
	case OBS_KEY_PARENLEFT:
		return 0x28; /* XK_parenleft */
	case OBS_KEY_PARENRIGHT:
		return 0x29; /* XK_parenright */
	case OBS_KEY_ASTERISK:
		return 0x2a; /* XK_asterisk */
	case OBS_KEY_PLUS:
		return 0x2b; /* XK_plus */
	case OBS_KEY_COMMA:
		return 0x2c; /* XK_comma */
	case OBS_KEY_MINUS:
		return 0x2d; /* XK_minus */
	case OBS_KEY_PERIOD:
		return 0x2e; /* XK_period */
	case OBS_KEY_SLASH:
		return 0x2f; /* XK_slash */
	case OBS_KEY_0:
		return 0x30; /* XK_0 */
	case OBS_KEY_1:
		return 0x31; /* XK_1 */
	case OBS_KEY_2:
		return 0x32; /* XK_2 */
	case OBS_KEY_3:
		return 0x33; /* XK_3 */
	case OBS_KEY_4:
		return 0x34; /* XK_4 */
	case OBS_KEY_5:
		return 0x35; /* XK_5 */
	case OBS_KEY_6:
		return 0x36; /* XK_6 */
	case OBS_KEY_7:
		return 0x37; /* XK_7 */
	case OBS_KEY_8:
		return 0x38; /* XK_8 */
	case OBS_KEY_9:
		return 0x39; /* XK_9 */
	case OBS_KEY_NUMEQUAL:
		return 0xffbd; /* XK_KP_Equal */
	case OBS_KEY_NUMASTERISK:
		return 0xffaa; /* XK_KP_Multiply */
	case OBS_KEY_NUMPLUS:
		return 0xffab; /* XK_KP_Add */
	case OBS_KEY_NUMCOMMA:
		return 0xffac; /* XK_KP_Separator */
	case OBS_KEY_NUMMINUS:
		return 0xffad; /* XK_KP_Subtract */
	case OBS_KEY_NUMPERIOD:
		return 0xffae; /* XK_KP_Decimal */
	case OBS_KEY_NUMSLASH:
		return 0xffaf; /* XK_KP_Divide */
	case OBS_KEY_NUM0:
		return 0xffb0; /* XK_KP_0 */
	case OBS_KEY_NUM1:
		return 0xffb1; /* XK_KP_1 */
	case OBS_KEY_NUM2:
		return 0xffb2; /* XK_KP_2 */
	case OBS_KEY_NUM3:
		return 0xffb3; /* XK_KP_3 */
	case OBS_KEY_NUM4:
		return 0xffb4; /* XK_KP_4 */
	case OBS_KEY_NUM5:
		return 0xffb5; /* XK_KP_5 */
	case OBS_KEY_NUM6:
		return 0xffb6; /* XK_KP_6 */
	case OBS_KEY_NUM7:
		return 0xffb7; /* XK_KP_7 */
	case OBS_KEY_NUM8:
		return 0xffb8; /* XK_KP_8 */
	case OBS_KEY_NUM9:
		return 0xffb9; /* XK_KP_9 */
	case OBS_KEY_COLON:
		return 0x3a; /* XK_colon */
	case OBS_KEY_SEMICOLON:
		return 0x3b; /* XK_semicolon */
	case OBS_KEY_LESS:
		return 0x3c; /* XK_less */
	case OBS_KEY_EQUAL:
		return 0x3d; /* XK_equal */
	case OBS_KEY_GREATER:
		return 0x3e; /* XK_greater */
	case OBS_KEY_QUESTION:
		return 0x3f; /* XK_question */
	case OBS_KEY_AT:
		return 0x40; /* XK_at */
	case OBS_KEY_A:
		return 0x41; /* XK_A */
	case OBS_KEY_B:
		return 0x42; /* XK_B */
	case OBS_KEY_C:
		return 0x43; /* XK_C */
	case OBS_KEY_D:
		return 0x44; /* XK_D */
	case OBS_KEY_E:
		return 0x45; /* XK_E */
	case OBS_KEY_F:
		return 0x46; /* XK_F */
	case OBS_KEY_G:
		return 0x47; /* XK_G */
	case OBS_KEY_H:
		return 0x48; /* XK_H */
	case OBS_KEY_I:
		return 0x49; /* XK_I */
	case OBS_KEY_J:
		return 0x4a; /* XK_J */
	case OBS_KEY_K:
		return 0x4b; /* XK_K */
	case OBS_KEY_L:
		return 0x4c; /* XK_L */
	case OBS_KEY_M:
		return 0x4d; /* XK_M */
	case OBS_KEY_N:
		return 0x4e; /* XK_N */
	case OBS_KEY_O:
		return 0x4f; /* XK_O */
	case OBS_KEY_P:
		return 0x50; /* XK_P */
	case OBS_KEY_Q:
		return 0x51; /* XK_Q */
	case OBS_KEY_R:
		return 0x52; /* XK_R */
	case OBS_KEY_S:
		return 0x53; /* XK_S */
	case OBS_KEY_T:
		return 0x54; /* XK_T */
	case OBS_KEY_U:
		return 0x55; /* XK_U */
	case OBS_KEY_V:
		return 0x56; /* XK_V */
	case OBS_KEY_W:
		return 0x57; /* XK_W */
	case OBS_KEY_X:
		return 0x58; /* XK_X */
	case OBS_KEY_Y:
		return 0x59; /* XK_Y */
	case OBS_KEY_Z:
		return 0x5a; /* XK_Z */
	case OBS_KEY_BRACKETLEFT:
		return 0x5b; /* XK_bracketleft */
	case OBS_KEY_BACKSLASH:
		return 0x5c; /* XK_backslash */
	case OBS_KEY_BRACKETRIGHT:
		return 0x5d; /* XK_bracketright */
	case OBS_KEY_ASCIICIRCUM:
		return 0x5e; /* XK_asciicircum */
	case OBS_KEY_UNDERSCORE:
		return 0x5f; /* XK_underscore */
	case OBS_KEY_QUOTELEFT:
		return 0x60; /* XK_quoteleft */
	case OBS_KEY_BRACELEFT:
		return 0x7b; /* XK_braceleft */
	case OBS_KEY_BAR:
		return 0x7c; /* XK_bar */
	case OBS_KEY_BRACERIGHT:
		return 0x7d; /* XK_braceright */
	case OBS_KEY_ASCIITILDE:
		return 0x60; /* XK_grave */
	case OBS_KEY_NOBREAKSPACE:
		return 0xa0; /* XK_nobreakspace */
	case OBS_KEY_EXCLAMDOWN:
		return 0xa1; /* XK_exclamdown */
	case OBS_KEY_CENT:
		return 0xa2; /* XK_cent */
	case OBS_KEY_STERLING:
		return 0xa3; /* XK_sterling */
	case OBS_KEY_CURRENCY:
		return 0xa4; /* XK_currency */
	case OBS_KEY_YEN:
		return 0xa5; /* XK_yen */
	case OBS_KEY_BROKENBAR:
		return 0xa6; /* XK_brokenbar */
	case OBS_KEY_SECTION:
		return 0xa7; /* XK_section */
	case OBS_KEY_DIAERESIS:
		return 0xa8; /* XK_diaeresis */
	case OBS_KEY_COPYRIGHT:
		return 0xa9; /* XK_copyright */
	case OBS_KEY_ORDFEMININE:
		return 0xaa; /* XK_ordfeminine */
	case OBS_KEY_GUILLEMOTLEFT:
		return 0xab; /* XK_guillemotleft */
	case OBS_KEY_NOTSIGN:
		return 0xac; /* XK_notsign */
	case OBS_KEY_HYPHEN:
		return 0xad; /* XK_hyphen */
	case OBS_KEY_REGISTERED:
		return 0xae; /* XK_registered */
	case OBS_KEY_MACRON:
		return 0xaf; /* XK_macron */
	case OBS_KEY_DEGREE:
		return 0xb0; /* XK_degree */
	case OBS_KEY_PLUSMINUS:
		return 0xb1; /* XK_plusminus */
	case OBS_KEY_TWOSUPERIOR:
		return 0xb2; /* XK_twosuperior */
	case OBS_KEY_THREESUPERIOR:
		return 0xb3; /* XK_threesuperior */
	case OBS_KEY_ACUTE:
		return 0xb4; /* XK_acute */
	case OBS_KEY_MU:
		return 0xb5; /* XK_mu */
	case OBS_KEY_PARAGRAPH:
		return 0xb6; /* XK_paragraph */
	case OBS_KEY_PERIODCENTERED:
		return 0xb7; /* XK_periodcentered */
	case OBS_KEY_CEDILLA:
		return 0xb8; /* XK_cedilla */
	case OBS_KEY_ONESUPERIOR:
		return 0xb9; /* XK_onesuperior */
	case OBS_KEY_MASCULINE:
		return 0xba; /* XK_masculine */
	case OBS_KEY_GUILLEMOTRIGHT:
		return 0xbb; /* XK_guillemotright */
	case OBS_KEY_ONEQUARTER:
		return 0xbc; /* XK_onequarter */
	case OBS_KEY_ONEHALF:
		return 0xbd; /* XK_onehalf */
	case OBS_KEY_THREEQUARTERS:
		return 0xbe; /* XK_threequarters */
	case OBS_KEY_QUESTIONDOWN:
		return 0xbf; /* XK_questiondown */
	case OBS_KEY_AGRAVE:
		return 0xc0; /* XK_Agrave */
	case OBS_KEY_AACUTE:
		return 0xc1; /* XK_Aacute */
	case OBS_KEY_ACIRCUMFLEX:
		return 0xc2; /* XK_Acircumflex */
	case OBS_KEY_ATILDE:
		return 0xc3; /* XK_Atilde */
	case OBS_KEY_ADIAERESIS:
		return 0xc4; /* XK_Adiaeresis */
	case OBS_KEY_ARING:
		return 0xc5; /* XK_Aring */
	case OBS_KEY_AE:
		return 0xc6; /* XK_AE */
	case OBS_KEY_CCEDILLA:
		return 0xb8; /* XK_cedilla */
	case OBS_KEY_EGRAVE:
		return 0xc8; /* XK_Egrave */
	case OBS_KEY_EACUTE:
		return 0xc9; /* XK_Eacute */
	case OBS_KEY_ECIRCUMFLEX:
		return 0xca; /* XK_Ecircumflex */
	case OBS_KEY_EDIAERESIS:
		return 0xcb; /* XK_Ediaeresis */
	case OBS_KEY_IGRAVE:
		return 0xcc; /* XK_Igrave */
	case OBS_KEY_IACUTE:
		return 0xcd; /* XK_Iacute */
	case OBS_KEY_ICIRCUMFLEX:
		return 0xce; /* XK_Icircumflex */
	case OBS_KEY_IDIAERESIS:
		return 0xcf; /* XK_Idiaeresis */
	case OBS_KEY_ETH:
		return 0xd0; /* XK_ETH */
	case OBS_KEY_NTILDE:
		return 0xd1; /* XK_Ntilde */
	case OBS_KEY_OGRAVE:
		return 0xd2; /* XK_Ograve */
	case OBS_KEY_OACUTE:
		return 0xd3; /* XK_Oacute */
	case OBS_KEY_OCIRCUMFLEX:
		return 0xd4; /* XK_Ocircumflex */
	case OBS_KEY_ODIAERESIS:
		return 0xd6; /* XK_Odiaeresis */
	case OBS_KEY_MULTIPLY:
		return 0xd7; /* XK_multiply */
	case OBS_KEY_OOBLIQUE:
		return 0xd8; /* XK_Ooblique */
	case OBS_KEY_UGRAVE:
		return 0xd9; /* XK_Ugrave */
	case OBS_KEY_UACUTE:
		return 0xda; /* XK_Uacute */
	case OBS_KEY_UCIRCUMFLEX:
		return 0xdb; /* XK_Ucircumflex */
	case OBS_KEY_UDIAERESIS:
		return 0xdc; /* XK_Udiaeresis */
	case OBS_KEY_YACUTE:
		return 0xdd; /* XK_Yacute */
	case OBS_KEY_THORN:
		return 0xde; /* XK_Thorn */
	case OBS_KEY_SSHARP:
		return 0xdf; /* XK_ssharp */
	case OBS_KEY_DIVISION:
		return 0xf7; /* XK_division */
	case OBS_KEY_YDIAERESIS:
		return 0x13be; /* XK_Ydiaeresis */
	case OBS_KEY_MULTI_KEY:
		return 0xff20; /* XK_Multi_key */
	case OBS_KEY_CODEINPUT:
		return 0xff37; /* XK_Codeinput */
	case OBS_KEY_SINGLECANDIDATE:
		return 0xff3c; /* XK_SingleCandidate */
	case OBS_KEY_MULTIPLECANDIDATE:
		return 0xff3d; /* XK_MultipleCandidate */
	case OBS_KEY_PREVIOUSCANDIDATE:
		return 0xff3e; /* XK_PreviousCandidate */
	case OBS_KEY_MODE_SWITCH:
		return 0xff7e; /* XK_Mode_switch */
	case OBS_KEY_KANJI:
		return 0xff21; /* XK_Kanji */
	case OBS_KEY_MUHENKAN:
		return 0xff22; /* XK_Muhenkan */
	case OBS_KEY_HENKAN:
		return 0xff23; /* XK_Henkan */
	case OBS_KEY_ROMAJI:
		return 0xff24; /* XK_Romaji */
	case OBS_KEY_HIRAGANA:
		return 0xff25; /* XK_Hiragana */
	case OBS_KEY_KATAKANA:
		return 0xff26; /* XK_Katakana */
	case OBS_KEY_HIRAGANA_KATAKANA:
		return 0xff27; /* XK_Hiragana_Katakana */
	case OBS_KEY_ZENKAKU:
		return 0xff28; /* XK_Zenkaku */
	case OBS_KEY_HANKAKU:
		return 0xff29; /* XK_Hankaku */
	case OBS_KEY_ZENKAKU_HANKAKU:
		return 0xff2a; /* XK_Zenkaku_Hankaku */
	case OBS_KEY_TOUROKU:
		return 0xff2b; /* XK_Touroku */
	case OBS_KEY_MASSYO:
		return 0xff2c; /* XK_Massyo */
	case OBS_KEY_KANA_LOCK:
		return 0xff2d; /* XK_Kana_Lock */
	case OBS_KEY_KANA_SHIFT:
		return 0xff2e; /* XK_Kana_Shift */
	case OBS_KEY_EISU_SHIFT:
		return 0xff2f; /* XK_Eisu_Shift */
	case OBS_KEY_EISU_TOGGLE:
		return 0xff30; /* XK_Eisu_toggle */
	case OBS_KEY_HANGUL:
		return 0xff31; /* XK_Hangul */
	case OBS_KEY_HANGUL_START:
		return 0xff32; /* XK_Hangul_Start */
	case OBS_KEY_HANGUL_END:
		return 0xff33; /* XK_Hangul_End */
	case OBS_KEY_HANGUL_HANJA:
		return 0xff34; /* XK_Hangul_Hanja */
	case OBS_KEY_HANGUL_JAMO:
		return 0xff35; /* XK_Hangul_Jamo */
	case OBS_KEY_HANGUL_ROMAJA:
		return 0xff36; /* XK_Hangul_Romaja */
	case OBS_KEY_HANGUL_BANJA:
		return 0xff39; /* XK_Hangul_Banja */
	case OBS_KEY_HANGUL_PREHANJA:
		return 0xff3a; /* XK_Hangul_PreHanja */
	case OBS_KEY_HANGUL_POSTHANJA:
		return 0xff3b; /* XK_Hangul_PostHanja */
	case OBS_KEY_HANGUL_SPECIAL:
		return 0xff3f; /* XK_Hangul_Special */
	case OBS_KEY_DEAD_GRAVE:
		return 0xfe50; /* XK_dead_grave */
	case OBS_KEY_DEAD_ACUTE:
		return 0xfe51; /* XK_dead_acute */
	case OBS_KEY_DEAD_CIRCUMFLEX:
		return 0xfe52; /* XK_dead_circumflex */
	case OBS_KEY_DEAD_TILDE:
		return 0xfe53; /* XK_dead_tilde */
	case OBS_KEY_DEAD_MACRON:
		return 0xfe54; /* XK_dead_macron */
	case OBS_KEY_DEAD_BREVE:
		return 0xfe55; /* XK_dead_breve */
	case OBS_KEY_DEAD_ABOVEDOT:
		return 0xfe56; /* XK_dead_abovedot */
	case OBS_KEY_DEAD_DIAERESIS:
		return 0xfe57; /* XK_dead_diaeresis */
	case OBS_KEY_DEAD_ABOVERING:
		return 0xfe58; /* XK_dead_abovering */
	case OBS_KEY_DEAD_DOUBLEACUTE:
		return 0xfe59; /* XK_dead_doubleacute */
	case OBS_KEY_DEAD_CARON:
		return 0xfe5a; /* XK_dead_caron */
	case OBS_KEY_DEAD_CEDILLA:
		return 0xfe5b; /* XK_dead_cedilla */
	case OBS_KEY_DEAD_OGONEK:
		return 0xfe5c; /* XK_dead_ogonek */
	case OBS_KEY_DEAD_IOTA:
		return 0xfe5d; /* XK_dead_iota */
	case OBS_KEY_DEAD_VOICED_SOUND:
		return 0xfe5e; /* XK_dead_voiced_sound */
	case OBS_KEY_DEAD_SEMIVOICED_SOUND:
		return 0xfe5f; /* XK_dead_semivoiced_sound */
	case OBS_KEY_DEAD_BELOWDOT:
		return 0xfe60; /* XK_dead_belowdot */
	case OBS_KEY_DEAD_HOOK:
		return 0xfe61; /* XK_dead_hook */
	case OBS_KEY_DEAD_HORN:
		return 0xfe62; /* XK_dead_horn */

	case OBS_KEY_MOUSE1:
		return MOUSE_1;
	case OBS_KEY_MOUSE2:
		return MOUSE_2;
	case OBS_KEY_MOUSE3:
		return MOUSE_3;
	case OBS_KEY_MOUSE4:
		return MOUSE_4;
	case OBS_KEY_MOUSE5:
		return MOUSE_5;

	case OBS_KEY_VK_MEDIA_PLAY_PAUSE:
		return 0x1008ff14; /* XF86XK_AudioPlay */
	case OBS_KEY_VK_MEDIA_STOP:
		return 0x1008ff15; /* XF86XK_AudioStop */
	case OBS_KEY_VK_MEDIA_PREV_TRACK:
		return 0x1008ff16; /* XF86XK_AudioPrev */
	case OBS_KEY_VK_MEDIA_NEXT_TRACK:
		return 0x1008ff17; /* XF86XK_AudioNext */
	case OBS_KEY_VK_VOLUME_MUTE:
		return 0x1008ff12; /* XF86XK_AudioMute */
	case OBS_KEY_VK_VOLUME_DOWN:
		return 0x1008ff11; /* XF86XK_AudioLowerVolume */
	case OBS_KEY_VK_VOLUME_UP:
		return 0x1008ff13; /* XF86XK_AudioRaiseVolume */

	/* TODO: Implement keys for non-US keyboards */
	default:;
	}
	return 0;
}

/* The switch of mouse_button_pressed() with XInput, after the raw event
 * drain. */
bool obs_nix_x11_input_mouse_pressed(obs_key_t key, const bool *pressed, const bool *held)
{
	bool ret = false;

	// Mouse 2 for OBS is Right Click and Mouse 3 is Wheel Click.
	// Mouse Wheel axis clicks (xinput mot->detail 4 5 6 7) are ignored.
	switch (key) {
	case OBS_KEY_MOUSE1:
		ret = pressed[0] || held[0];
		break;
	case OBS_KEY_MOUSE2:
		ret = pressed[2] || held[2];
		break;
	case OBS_KEY_MOUSE3:
		ret = pressed[1] || held[1];
		break;
	case OBS_KEY_MOUSE4:
		ret = pressed[7] || held[7];
		break;
	case OBS_KEY_MOUSE5:
		ret = pressed[8] || held[8];
		break;
	case OBS_KEY_MOUSE6:
		ret = pressed[9] || held[9];
		break;
	case OBS_KEY_MOUSE7:
		ret = pressed[10] || held[10];
		break;
	case OBS_KEY_MOUSE8:
		ret = pressed[11] || held[11];
		break;
	case OBS_KEY_MOUSE9:
		ret = pressed[12] || held[12];
		break;
	case OBS_KEY_MOUSE10:
		ret = pressed[13] || held[13];
		break;
	case OBS_KEY_MOUSE11:
		ret = pressed[14] || held[14];
		break;
	case OBS_KEY_MOUSE12:
		ret = pressed[15] || held[15];
		break;
	case OBS_KEY_MOUSE13:
		ret = pressed[16] || held[16];
		break;
	case OBS_KEY_MOUSE14:
		ret = pressed[17] || held[17];
		break;
	case OBS_KEY_MOUSE15:
		ret = pressed[18] || held[18];
		break;
	case OBS_KEY_MOUSE16:
		ret = pressed[19] || held[19];
		break;
	case OBS_KEY_MOUSE17:
		ret = pressed[20] || held[20];
		break;
	case OBS_KEY_MOUSE18:
		ret = pressed[21] || held[21];
		break;
	case OBS_KEY_MOUSE19:
		ret = pressed[22] || held[22];
		break;
	case OBS_KEY_MOUSE20:
		ret = pressed[23] || held[23];
		break;
	case OBS_KEY_MOUSE21:
		ret = pressed[24] || held[24];
		break;
	case OBS_KEY_MOUSE22:
		ret = pressed[25] || held[25];
		break;
	case OBS_KEY_MOUSE23:
		ret = pressed[26] || held[26];
		break;
	case OBS_KEY_MOUSE24:
		ret = pressed[27] || held[27];
		break;
	case OBS_KEY_MOUSE25:
		ret = pressed[28] || held[28];
		break;
	case OBS_KEY_MOUSE26:
		ret = pressed[29] || held[29];
		break;
	case OBS_KEY_MOUSE27:
		ret = pressed[30] || held[30];
		break;
	case OBS_KEY_MOUSE28:
		ret = pressed[31] || held[31];
		break;
	case OBS_KEY_MOUSE29:
		ret = pressed[32] || held[32];
		break;
	default:
		break;
	}

	return ret;
}

/* The commit loop of mouse_button_pressed() with XInput. */
void obs_nix_x11_input_commit(const bool *update, const bool *pressed, bool *held)
{
	for (int i = 0; i != XINPUT_MOUSE_LEN; i++)
		if (update[i])
			held[i] = pressed[i];
}

/* The switch of mouse_button_pressed() without XInput, on the reply's mask. */
bool obs_nix_x11_input_mask_pressed(obs_key_t key, uint16_t buttons)
{
	bool ret = false;

	switch (key) {
	case OBS_KEY_MOUSE1:
		ret = buttons & OBS_NIX_X11_INPUT_BUTTON_MASK_1;
		break;
	case OBS_KEY_MOUSE2:
		ret = buttons & OBS_NIX_X11_INPUT_BUTTON_MASK_3;
		break;
	case OBS_KEY_MOUSE3:
		ret = buttons & OBS_NIX_X11_INPUT_BUTTON_MASK_2;
		break;
	default:;
	}

	return ret;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef MOUSE_1
#undef MOUSE_2
#undef MOUSE_3
#undef MOUSE_4
#undef MOUSE_5
