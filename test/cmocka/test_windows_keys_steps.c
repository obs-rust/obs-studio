#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-windows-keys.h"

/*
 * Characterization of the Windows key map and key name route of
 * libobs/obs-windows.c, moved to libobs/obs-windows-keys.c before their Rust
 * port:
 * - obs_windows_keys_virtual_key(): get_virtual_key(), the table
 *   obs_hotkeys_platform_init() fills vk_codes from;
 * - obs_windows_keys_either_win(): obs_hotkeys_platform_is_pressed()'s Meta
 *   rule;
 * - obs_windows_keys_name(), obs_windows_keys_scan_code() and
 *   obs_windows_keys_system_name(): obs_key_to_str()'s route;
 * - obs_windows_keys_from_virtual_key(): obs_key_from_virtual_key();
 * - obs_windows_keys_to_virtual_key(): obs_key_to_virtual_key();
 * - obs_windows_keys_combination(): obs_key_combination_to_str()'s order.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No Windows header is needed: the virtual
 * keys are written out below as winuser.h defines them, so it runs on every
 * platform.
 *
 * Not characterized here: obs_windows_keys_to_virtual_key() for a key
 * outside the table, which C reads past its end. The Rust port gives 0 for
 * it; its Tier 3 tests list that as an intentional difference.
 */

/* Every key the table maps, with its virtual key (winuser.h's VK_* value,
 * or the raw code of an OBS_KEY_0x* key). Every other key maps to 0. */
static const struct {
	obs_key_t key;
	int vk;
} mapped[] = {
	{OBS_KEY_RETURN, 0x0D},
	{OBS_KEY_ESCAPE, 0x1B},
	{OBS_KEY_TAB, 0x09},
	{OBS_KEY_BACKTAB, 0xF5},
	{OBS_KEY_BACKSPACE, 0x08},
	{OBS_KEY_INSERT, 0x2D},
	{OBS_KEY_DELETE, 0x2E},
	{OBS_KEY_PAUSE, 0x13},
	{OBS_KEY_PRINT, 0x2C},
	{OBS_KEY_CLEAR, 0x0C},
	{OBS_KEY_HOME, 0x24},
	{OBS_KEY_END, 0x23},
	{OBS_KEY_LEFT, 0x25},
	{OBS_KEY_UP, 0x26},
	{OBS_KEY_RIGHT, 0x27},
	{OBS_KEY_DOWN, 0x28},
	{OBS_KEY_PAGEUP, 0x21},
	{OBS_KEY_PAGEDOWN, 0x22},
	{OBS_KEY_SHIFT, 0x10},
	{OBS_KEY_CONTROL, 0x11},
	{OBS_KEY_ALT, 0x12},
	{OBS_KEY_CAPSLOCK, 0x14},
	{OBS_KEY_NUMLOCK, 0x90},
	{OBS_KEY_SCROLLLOCK, 0x91},
	{OBS_KEY_F1, 0x70},
	{OBS_KEY_F2, 0x71},
	{OBS_KEY_F3, 0x72},
	{OBS_KEY_F4, 0x73},
	{OBS_KEY_F5, 0x74},
	{OBS_KEY_F6, 0x75},
	{OBS_KEY_F7, 0x76},
	{OBS_KEY_F8, 0x77},
	{OBS_KEY_F9, 0x78},
	{OBS_KEY_F10, 0x79},
	{OBS_KEY_F11, 0x7A},
	{OBS_KEY_F12, 0x7B},
	{OBS_KEY_F13, 0x7C},
	{OBS_KEY_F14, 0x7D},
	{OBS_KEY_F15, 0x7E},
	{OBS_KEY_F16, 0x7F},
	{OBS_KEY_F17, 0x80},
	{OBS_KEY_F18, 0x81},
	{OBS_KEY_F19, 0x82},
	{OBS_KEY_F20, 0x83},
	{OBS_KEY_F21, 0x84},
	{OBS_KEY_F22, 0x85},
	{OBS_KEY_F23, 0x86},
	{OBS_KEY_F24, 0x87},
	{OBS_KEY_SPACE, 0x20},
	{OBS_KEY_APOSTROPHE, 0xDE},
	{OBS_KEY_PLUS, 0xBB},
	{OBS_KEY_COMMA, 0xBC},
	{OBS_KEY_MINUS, 0xBD},
	{OBS_KEY_PERIOD, 0xBE},
	{OBS_KEY_SLASH, 0xBF},
	{OBS_KEY_0, 0x30},
	{OBS_KEY_1, 0x31},
	{OBS_KEY_2, 0x32},
	{OBS_KEY_3, 0x33},
	{OBS_KEY_4, 0x34},
	{OBS_KEY_5, 0x35},
	{OBS_KEY_6, 0x36},
	{OBS_KEY_7, 0x37},
	{OBS_KEY_8, 0x38},
	{OBS_KEY_9, 0x39},
	{OBS_KEY_NUMASTERISK, 0x6A},
	{OBS_KEY_NUMPLUS, 0x6B},
	{OBS_KEY_NUMMINUS, 0x6D},
	{OBS_KEY_NUMPERIOD, 0x6E},
	{OBS_KEY_NUMSLASH, 0x6F},
	{OBS_KEY_NUM0, 0x60},
	{OBS_KEY_NUM1, 0x61},
	{OBS_KEY_NUM2, 0x62},
	{OBS_KEY_NUM3, 0x63},
	{OBS_KEY_NUM4, 0x64},
	{OBS_KEY_NUM5, 0x65},
	{OBS_KEY_NUM6, 0x66},
	{OBS_KEY_NUM7, 0x67},
	{OBS_KEY_NUM8, 0x68},
	{OBS_KEY_NUM9, 0x69},
	{OBS_KEY_SEMICOLON, 0xBA},
	{OBS_KEY_A, 0x41},
	{OBS_KEY_B, 0x42},
	{OBS_KEY_C, 0x43},
	{OBS_KEY_D, 0x44},
	{OBS_KEY_E, 0x45},
	{OBS_KEY_F, 0x46},
	{OBS_KEY_G, 0x47},
	{OBS_KEY_H, 0x48},
	{OBS_KEY_I, 0x49},
	{OBS_KEY_J, 0x4A},
	{OBS_KEY_K, 0x4B},
	{OBS_KEY_L, 0x4C},
	{OBS_KEY_M, 0x4D},
	{OBS_KEY_N, 0x4E},
	{OBS_KEY_O, 0x4F},
	{OBS_KEY_P, 0x50},
	{OBS_KEY_Q, 0x51},
	{OBS_KEY_R, 0x52},
	{OBS_KEY_S, 0x53},
	{OBS_KEY_T, 0x54},
	{OBS_KEY_U, 0x55},
	{OBS_KEY_V, 0x56},
	{OBS_KEY_W, 0x57},
	{OBS_KEY_X, 0x58},
	{OBS_KEY_Y, 0x59},
	{OBS_KEY_Z, 0x5A},
	{OBS_KEY_BRACKETLEFT, 0xDB},
	{OBS_KEY_BACKSLASH, 0xDC},
	{OBS_KEY_BRACKETRIGHT, 0xDD},
	{OBS_KEY_ASCIITILDE, 0xC0},
	{OBS_KEY_KANJI, 0x19},
	{OBS_KEY_MUHENKAN, 0x1D},
	{OBS_KEY_HENKAN, 0x1C},
	{OBS_KEY_TOUROKU, 0x94},
	{OBS_KEY_MASSYO, 0x93},
	{OBS_KEY_HANGUL, 0x15},
	{OBS_KEY_MOUSE1, 0x01},
	{OBS_KEY_MOUSE2, 0x02},
	{OBS_KEY_MOUSE3, 0x04},
	{OBS_KEY_MOUSE4, 0x05},
	{OBS_KEY_MOUSE5, 0x06},
	{OBS_KEY_BACKSLASH_RT102, 0xE2},
	{OBS_KEY_VK_CANCEL, 0x03},
	{OBS_KEY_0x07, 0x07},
	{OBS_KEY_0x0A, 0x0A},
	{OBS_KEY_0x0B, 0x0B},
	{OBS_KEY_0x0E, 0x0E},
	{OBS_KEY_0x0F, 0x0F},
	{OBS_KEY_0x16, 0x16},
	{OBS_KEY_VK_JUNJA, 0x17},
	{OBS_KEY_VK_FINAL, 0x18},
	{OBS_KEY_0x1A, 0x1A},
	{OBS_KEY_VK_ACCEPT, 0x1E},
	{OBS_KEY_VK_MODECHANGE, 0x1F},
	{OBS_KEY_VK_SELECT, 0x29},
	{OBS_KEY_VK_PRINT, 0x2A},
	{OBS_KEY_VK_EXECUTE, 0x2B},
	{OBS_KEY_VK_HELP, 0x2F},
	{OBS_KEY_0x30, 0x30},
	{OBS_KEY_0x31, 0x31},
	{OBS_KEY_0x32, 0x32},
	{OBS_KEY_0x33, 0x33},
	{OBS_KEY_0x34, 0x34},
	{OBS_KEY_0x35, 0x35},
	{OBS_KEY_0x36, 0x36},
	{OBS_KEY_0x37, 0x37},
	{OBS_KEY_0x38, 0x38},
	{OBS_KEY_0x39, 0x39},
	{OBS_KEY_0x3A, 0x3A},
	{OBS_KEY_0x3B, 0x3B},
	{OBS_KEY_0x3C, 0x3C},
	{OBS_KEY_0x3D, 0x3D},
	{OBS_KEY_0x3E, 0x3E},
	{OBS_KEY_0x3F, 0x3F},
	{OBS_KEY_0x40, 0x40},
	{OBS_KEY_0x41, 0x41},
	{OBS_KEY_0x42, 0x42},
	{OBS_KEY_0x43, 0x43},
	{OBS_KEY_0x44, 0x44},
	{OBS_KEY_0x45, 0x45},
	{OBS_KEY_0x46, 0x46},
	{OBS_KEY_0x47, 0x47},
	{OBS_KEY_0x48, 0x48},
	{OBS_KEY_0x49, 0x49},
	{OBS_KEY_0x4A, 0x4A},
	{OBS_KEY_0x4B, 0x4B},
	{OBS_KEY_0x4C, 0x4C},
	{OBS_KEY_0x4D, 0x4D},
	{OBS_KEY_0x4E, 0x4E},
	{OBS_KEY_0x4F, 0x4F},
	{OBS_KEY_0x50, 0x50},
	{OBS_KEY_0x51, 0x51},
	{OBS_KEY_0x52, 0x52},
	{OBS_KEY_0x53, 0x53},
	{OBS_KEY_0x54, 0x54},
	{OBS_KEY_0x55, 0x55},
	{OBS_KEY_0x56, 0x56},
	{OBS_KEY_0x57, 0x57},
	{OBS_KEY_0x58, 0x58},
	{OBS_KEY_0x59, 0x59},
	{OBS_KEY_0x5A, 0x5A},
	{OBS_KEY_VK_LWIN, 0x5B},
	{OBS_KEY_VK_RWIN, 0x5C},
	{OBS_KEY_VK_APPS, 0x5D},
	{OBS_KEY_0x5E, 0x5E},
	{OBS_KEY_VK_SLEEP, 0x5F},
	{OBS_KEY_VK_SEPARATOR, 0x6C},
	{OBS_KEY_0x88, 0x88},
	{OBS_KEY_0x89, 0x89},
	{OBS_KEY_0x8A, 0x8A},
	{OBS_KEY_0x8B, 0x8B},
	{OBS_KEY_0x8C, 0x8C},
	{OBS_KEY_0x8D, 0x8D},
	{OBS_KEY_0x8E, 0x8E},
	{OBS_KEY_0x8F, 0x8F},
	{OBS_KEY_VK_OEM_FJ_JISHO, 0x92},
	{OBS_KEY_VK_OEM_FJ_LOYA, 0x95},
	{OBS_KEY_VK_OEM_FJ_ROYA, 0x96},
	{OBS_KEY_0x97, 0x97},
	{OBS_KEY_0x98, 0x98},
	{OBS_KEY_0x99, 0x99},
	{OBS_KEY_0x9A, 0x9A},
	{OBS_KEY_0x9B, 0x9B},
	{OBS_KEY_0x9C, 0x9C},
	{OBS_KEY_0x9D, 0x9D},
	{OBS_KEY_0x9E, 0x9E},
	{OBS_KEY_0x9F, 0x9F},
	{OBS_KEY_VK_LSHIFT, 0xA0},
	{OBS_KEY_VK_RSHIFT, 0xA1},
	{OBS_KEY_VK_LCONTROL, 0xA2},
	{OBS_KEY_VK_RCONTROL, 0xA3},
	{OBS_KEY_VK_LMENU, 0xA4},
	{OBS_KEY_VK_RMENU, 0xA5},
	{OBS_KEY_VK_BROWSER_BACK, 0xA6},
	{OBS_KEY_VK_BROWSER_FORWARD, 0xA7},
	{OBS_KEY_VK_BROWSER_REFRESH, 0xA8},
	{OBS_KEY_VK_BROWSER_STOP, 0xA9},
	{OBS_KEY_VK_BROWSER_SEARCH, 0xAA},
	{OBS_KEY_VK_BROWSER_FAVORITES, 0xAB},
	{OBS_KEY_VK_BROWSER_HOME, 0xAC},
	{OBS_KEY_VK_VOLUME_MUTE, 0xAD},
	{OBS_KEY_VK_VOLUME_DOWN, 0xAE},
	{OBS_KEY_VK_VOLUME_UP, 0xAF},
	{OBS_KEY_VK_MEDIA_NEXT_TRACK, 0xB0},
	{OBS_KEY_VK_MEDIA_PREV_TRACK, 0xB1},
	{OBS_KEY_VK_MEDIA_STOP, 0xB2},
	{OBS_KEY_VK_MEDIA_PLAY_PAUSE, 0xB3},
	{OBS_KEY_VK_LAUNCH_MAIL, 0xB4},
	{OBS_KEY_VK_LAUNCH_MEDIA_SELECT, 0xB5},
	{OBS_KEY_VK_LAUNCH_APP1, 0xB6},
	{OBS_KEY_VK_LAUNCH_APP2, 0xB7},
	{OBS_KEY_0xB8, 0xB8},
	{OBS_KEY_0xB9, 0xB9},
	{OBS_KEY_0xC1, 0xC1},
	{OBS_KEY_0xC2, 0xC2},
	{OBS_KEY_0xC3, 0xC3},
	{OBS_KEY_0xC4, 0xC4},
	{OBS_KEY_0xC5, 0xC5},
	{OBS_KEY_0xC6, 0xC6},
	{OBS_KEY_0xC7, 0xC7},
	{OBS_KEY_0xC8, 0xC8},
	{OBS_KEY_0xC9, 0xC9},
	{OBS_KEY_0xCA, 0xCA},
	{OBS_KEY_0xCB, 0xCB},
	{OBS_KEY_0xCC, 0xCC},
	{OBS_KEY_0xCD, 0xCD},
	{OBS_KEY_0xCE, 0xCE},
	{OBS_KEY_0xCF, 0xCF},
	{OBS_KEY_0xD0, 0xD0},
	{OBS_KEY_0xD1, 0xD1},
	{OBS_KEY_0xD2, 0xD2},
	{OBS_KEY_0xD3, 0xD3},
	{OBS_KEY_0xD4, 0xD4},
	{OBS_KEY_0xD5, 0xD5},
	{OBS_KEY_0xD6, 0xD6},
	{OBS_KEY_0xD7, 0xD7},
	{OBS_KEY_0xD8, 0xD8},
	{OBS_KEY_0xD9, 0xD9},
	{OBS_KEY_0xDA, 0xDA},
	{OBS_KEY_VK_OEM_8, 0xDF},
	{OBS_KEY_0xE0, 0xE0},
	{OBS_KEY_VK_OEM_AX, 0xE1},
	{OBS_KEY_VK_ICO_HELP, 0xE3},
	{OBS_KEY_VK_ICO_00, 0xE4},
	{OBS_KEY_VK_PROCESSKEY, 0xE5},
	{OBS_KEY_VK_ICO_CLEAR, 0xE6},
	{OBS_KEY_VK_PACKET, 0xE7},
	{OBS_KEY_0xE8, 0xE8},
	{OBS_KEY_VK_OEM_RESET, 0xE9},
	{OBS_KEY_VK_OEM_JUMP, 0xEA},
	{OBS_KEY_VK_OEM_PA1, 0xEB},
	{OBS_KEY_VK_OEM_PA2, 0xEC},
	{OBS_KEY_VK_OEM_PA3, 0xED},
	{OBS_KEY_VK_OEM_WSCTRL, 0xEE},
	{OBS_KEY_VK_OEM_CUSEL, 0xEF},
	{OBS_KEY_VK_OEM_ATTN, 0xF0},
	{OBS_KEY_VK_OEM_FINISH, 0xF1},
	{OBS_KEY_VK_OEM_COPY, 0xF2},
	{OBS_KEY_VK_OEM_AUTO, 0xF3},
	{OBS_KEY_VK_OEM_ENLW, 0xF4},
	{OBS_KEY_VK_ATTN, 0xF6},
	{OBS_KEY_VK_CRSEL, 0xF7},
	{OBS_KEY_VK_EXSEL, 0xF8},
	{OBS_KEY_VK_EREOF, 0xF9},
	{OBS_KEY_VK_PLAY, 0xFA},
	{OBS_KEY_VK_ZOOM, 0xFB},
	{OBS_KEY_VK_NONAME, 0xFC},
	{OBS_KEY_VK_PA1, 0xFD},
	{OBS_KEY_VK_OEM_CLEAR, 0xFE},
};

#define NUM_MAPPED (sizeof(mapped) / sizeof(mapped[0]))

static int vk_codes[OBS_KEY_LAST_VALUE];
static int table_asked;

static int fill(void **state)
{
	(void)state;
	/* As obs_hotkeys_platform_init() fills it. */
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++)
		vk_codes[i] = obs_windows_keys_virtual_key((obs_key_t)i);
	table_asked = 0;
	return 0;
}

static const int *table(void)
{
	table_asked++;
	return vk_codes;
}

static void virtual_key_table(void **state)
{
	(void)state;

	assert_int_equal(NUM_MAPPED, 290);
	for (size_t i = 0; i < NUM_MAPPED; i++)
		assert_int_equal(obs_windows_keys_virtual_key(mapped[i].key), mapped[i].vk);

	/* Every key not listed is 0. */
	size_t nonzero = 0;
	for (int key = 0; key < OBS_KEY_LAST_VALUE; key++) {
		if (obs_windows_keys_virtual_key((obs_key_t)key) != 0)
			nonzero++;
	}
	assert_int_equal(nonzero, NUM_MAPPED);
}

static void virtual_key_named(void **state)
{
	(void)state;

	/* The mouse buttons: VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1 and
	 * VK_XBUTTON2. Mouse 6 to 29 have none. */
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE1), 0x01);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE2), 0x02);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE3), 0x04);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE4), 0x05);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE5), 0x06);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE6), 0);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_MOUSE29), 0);

	/* A letter and its raw code share a virtual key. */
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_A), 0x41);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_0x41), 0x41);

	/* Meta has none of its own (characterized, not endorsed): the pressed
	 * check and obs_key_to_virtual_key() treat it apart. */
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_META), 0);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_VK_LWIN), 0x5B);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_NONE), 0);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_F24), 0x87);
	assert_int_equal(obs_windows_keys_virtual_key(OBS_KEY_VK_OEM_CLEAR), 0xFE);
}

static void either_win(void **state)
{
	(void)state;

	for (int key = 0; key < OBS_KEY_LAST_VALUE; key++)
		assert_int_equal(obs_windows_keys_either_win((obs_key_t)key), key == OBS_KEY_META);
	assert_false(obs_windows_keys_either_win(OBS_KEY_VK_LWIN));
	assert_false(obs_windows_keys_either_win(OBS_KEY_LAST_VALUE));
}

static void name_route(void **state)
{
	(void)state;
	int number = -1;

	assert_int_equal(obs_windows_keys_name(OBS_KEY_NONE, &number), OBS_WINDOWS_KEYS_NAME_NONE);
	assert_int_equal(number, -1);

	assert_int_equal(obs_windows_keys_name(OBS_KEY_F13, &number), OBS_WINDOWS_KEYS_NAME_F);
	assert_int_equal(number, 13);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_F24, &number), OBS_WINDOWS_KEYS_NAME_F);
	assert_int_equal(number, 24);

	assert_int_equal(obs_windows_keys_name(OBS_KEY_MOUSE1, &number), OBS_WINDOWS_KEYS_NAME_MOUSE);
	assert_int_equal(number, 1);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_MOUSE29, &number), OBS_WINDOWS_KEYS_NAME_MOUSE);
	assert_int_equal(number, 29);

	/* Only F and Mouse write the number. */
	number = -1;
	assert_int_equal(obs_windows_keys_name(OBS_KEY_PAUSE, &number), OBS_WINDOWS_KEYS_NAME_PAUSE);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_META, &number), OBS_WINDOWS_KEYS_NAME_META);
	/* F1 to F12 and F25 on are named by the system. */
	assert_int_equal(obs_windows_keys_name(OBS_KEY_F12, &number), OBS_WINDOWS_KEYS_NAME_SYSTEM);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_F25, &number), OBS_WINDOWS_KEYS_NAME_SYSTEM);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_A, &number), OBS_WINDOWS_KEYS_NAME_SYSTEM);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_VK_LWIN, &number), OBS_WINDOWS_KEYS_NAME_SYSTEM);
	assert_int_equal(obs_windows_keys_name(OBS_KEY_VK_CANCEL, &number), OBS_WINDOWS_KEYS_NAME_SYSTEM);
	assert_int_equal(obs_windows_keys_name((obs_key_t)(OBS_KEY_MOUSE29 + 1), &number),
			 OBS_WINDOWS_KEYS_NAME_SYSTEM);
	assert_int_equal(number, -1);

	/* Every F13 to F24 and Mouse key, by number. */
	for (int key = OBS_KEY_F13; key <= OBS_KEY_F24; key++) {
		assert_int_equal(obs_windows_keys_name((obs_key_t)key, &number), OBS_WINDOWS_KEYS_NAME_F);
		assert_int_equal(number, key - OBS_KEY_F13 + 13);
	}
	for (int key = OBS_KEY_MOUSE1; key <= OBS_KEY_MOUSE29; key++) {
		assert_int_equal(obs_windows_keys_name((obs_key_t)key, &number), OBS_WINDOWS_KEYS_NAME_MOUSE);
		assert_int_equal(number, key - OBS_KEY_MOUSE1 + 1);
	}
}

/* VK_HOME, VK_END, VK_LEFT, VK_UP, VK_RIGHT, VK_DOWN, VK_PRIOR, VK_NEXT,
 * VK_INSERT, VK_DELETE and VK_NUMLOCK. */
static const int extended[] = {0x24, 0x23, 0x25, 0x26, 0x27, 0x28, 0x21, 0x22, 0x2D, 0x2E, 0x90};

static bool is_extended(int vk)
{
	for (size_t i = 0; i < sizeof(extended) / sizeof(extended[0]); i++) {
		if (extended[i] == vk)
			return true;
	}
	return false;
}

static void scan_code(void **state)
{
	(void)state;

	/* The scan code in bits 16 and up; Enter is not extended. */
	assert_int_equal(obs_windows_keys_scan_code(0x0D, 0x1C), 0x001C0000);
	assert_int_equal(obs_windows_keys_scan_code(0x0D, 0), 0);
	/* Home is: bit 24 is set, even without a scan code. */
	assert_int_equal(obs_windows_keys_scan_code(0x24, 0x47), 0x01470000);
	assert_int_equal(obs_windows_keys_scan_code(0x24, 0), 0x01000000);
	/* The scan code's high bits shift out; bit 24 overrides its own. */
	assert_int_equal(obs_windows_keys_scan_code(0x0D, 0xFFFF0001), 0x00010000);
	assert_int_equal(obs_windows_keys_scan_code(0x0D, 0x0100), 0x01000000);
	assert_int_equal(obs_windows_keys_scan_code(0x90, 0x0100), 0x01000000);

	/* Exactly the 11 extended keys, of every virtual key and a few beyond. */
	for (int vk = -2; vk < 0x200; vk++) {
		uint32_t expected = 0x00450000u | (is_extended(vk) ? 0x01000000u : 0);
		assert_int_equal(obs_windows_keys_scan_code(vk, 0x45), expected);
	}
}

static void system_name(void **state)
{
	(void)state;

	assert_true(obs_windows_keys_system_name(OBS_KEY_A, 0x001E0000));
	assert_false(obs_windows_keys_system_name(OBS_KEY_A, 0));
	assert_true(obs_windows_keys_system_name(OBS_KEY_A, 0x01000000));
	assert_true(obs_windows_keys_system_name(OBS_KEY_NONE, 1));

	/* The OBS_KEY_VK_* keys, OBS_KEY_VK_CANCEL to OBS_KEY_VK_OEM_CLEAR, never. */
	assert_true(obs_windows_keys_system_name((obs_key_t)(OBS_KEY_VK_CANCEL - 1), 1));
	assert_false(obs_windows_keys_system_name(OBS_KEY_VK_CANCEL, 1));
	assert_false(obs_windows_keys_system_name(OBS_KEY_VK_LWIN, 1));
	assert_false(obs_windows_keys_system_name(OBS_KEY_VK_OEM_CLEAR, 1));
	assert_true(obs_windows_keys_system_name((obs_key_t)(OBS_KEY_VK_OEM_CLEAR + 1), 1));
	assert_true(obs_windows_keys_system_name(OBS_KEY_LAST_VALUE, 1));
}

static void from_virtual_key(void **state)
{
	(void)state;

	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0x0D), OBS_KEY_RETURN);
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0x01), OBS_KEY_MOUSE1);
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0xFE), OBS_KEY_VK_OEM_CLEAR);
	/* Two keys share 'A': the first, the letter, wins. */
	assert_true(OBS_KEY_A < OBS_KEY_0x41);
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0x41), OBS_KEY_A);
	/* VK_LWIN is OBS_KEY_VK_LWIN, not Meta (characterized, not endorsed:
	 * obs_key_to_virtual_key(OBS_KEY_META) is VK_LWIN, so Meta doesn't
	 * round-trip). */
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0x5B), OBS_KEY_VK_LWIN);
	/* 0 is the first unmapped key, OBS_KEY_NONE itself. */
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0), OBS_KEY_NONE);
	/* No key has these. */
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0xFF), OBS_KEY_NONE);
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, -1), OBS_KEY_NONE);
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0x100), OBS_KEY_NONE);

	/* The last entry is searched too. */
	vk_codes[OBS_KEY_LAST_VALUE - 1] = 0x7777;
	assert_int_equal(obs_windows_keys_from_virtual_key(vk_codes, 0x7777), OBS_KEY_LAST_VALUE - 1);
}

static void to_virtual_key(void **state)
{
	(void)state;

	/* Meta is VK_LWIN, without asking for the table. */
	assert_int_equal(obs_windows_keys_to_virtual_key(OBS_KEY_META, table), 0x5B);
	assert_int_equal(table_asked, 0);

	/* Any other key is its entry, asked for once. */
	assert_int_equal(obs_windows_keys_to_virtual_key(OBS_KEY_RETURN, table), 0x0D);
	assert_int_equal(table_asked, 1);
	vk_codes[OBS_KEY_RETURN] = 0x1234;
	assert_int_equal(obs_windows_keys_to_virtual_key(OBS_KEY_RETURN, table), 0x1234);
	assert_int_equal(obs_windows_keys_to_virtual_key(OBS_KEY_NONE, table), 0);
	assert_int_equal(obs_windows_keys_to_virtual_key(OBS_KEY_VK_LWIN, table), 0x5B);
	assert_int_equal(obs_windows_keys_to_virtual_key((obs_key_t)(OBS_KEY_LAST_VALUE - 1), table),
			 vk_codes[OBS_KEY_LAST_VALUE - 1]);
	assert_int_equal(table_asked, 5);
}

static void combination(void **state)
{
	(void)state;
	obs_key_t keys[OBS_WINDOWS_KEYS_COMBINATION_MAX];

	assert_int_equal(obs_windows_keys_combination(0, OBS_KEY_NONE, keys), 0);

	assert_int_equal(obs_windows_keys_combination(0, OBS_KEY_A, keys), 1);
	assert_int_equal(keys[0], OBS_KEY_A);

	/* Control, Meta (Command), Alt, Shift, then the key. */
	uint32_t all = INTERACT_CONTROL_KEY | INTERACT_COMMAND_KEY | INTERACT_ALT_KEY | INTERACT_SHIFT_KEY;
	assert_int_equal(obs_windows_keys_combination(all, OBS_KEY_A, keys), 5);
	assert_int_equal(keys[0], OBS_KEY_CONTROL);
	assert_int_equal(keys[1], OBS_KEY_META);
	assert_int_equal(keys[2], OBS_KEY_ALT);
	assert_int_equal(keys[3], OBS_KEY_SHIFT);
	assert_int_equal(keys[4], OBS_KEY_A);

	/* Each of the 16 masks, with and without a key, and with the other
	 * bits (Caps Lock, Num Lock, the mouse buttons) set, which add nothing. */
	static const uint32_t bits[] = {INTERACT_CONTROL_KEY, INTERACT_COMMAND_KEY, INTERACT_ALT_KEY,
					INTERACT_SHIFT_KEY};
	static const obs_key_t order[] = {OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_ALT, OBS_KEY_SHIFT};
	uint32_t other = INTERACT_CAPS_KEY | INTERACT_NUMLOCK_KEY | INTERACT_MOUSE_LEFT | INTERACT_MOUSE_MIDDLE |
			 INTERACT_MOUSE_RIGHT | 0xFFFFFE00u;

	for (uint32_t mask = 0; mask < 16; mask++) {
		for (int with_key = 0; with_key < 2; with_key++) {
			for (int with_other = 0; with_other < 2; with_other++) {
				uint32_t modifiers = with_other ? other : 0;
				obs_key_t expected[OBS_WINDOWS_KEYS_COMBINATION_MAX];
				size_t num = 0;

				for (size_t b = 0; b < 4; b++) {
					if (mask & (1u << b)) {
						modifiers |= bits[b];
						expected[num++] = order[b];
					}
				}
				obs_key_t key = with_key ? OBS_KEY_F13 : OBS_KEY_NONE;
				if (with_key)
					expected[num++] = key;

				memset(keys, 0xFF, sizeof(keys));
				assert_int_equal(obs_windows_keys_combination(modifiers, key, keys), num);
				for (size_t i = 0; i < num; i++)
					assert_int_equal(keys[i], expected[i]);
			}
		}
	}

	/* A modifier key itself is the key, after the modifiers. */
	assert_int_equal(obs_windows_keys_combination(INTERACT_SHIFT_KEY, OBS_KEY_SHIFT, keys), 2);
	assert_int_equal(keys[0], OBS_KEY_SHIFT);
	assert_int_equal(keys[1], OBS_KEY_SHIFT);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(virtual_key_table),
		cmocka_unit_test(virtual_key_named),
		cmocka_unit_test(either_win),
		cmocka_unit_test(name_route),
		cmocka_unit_test(scan_code),
		cmocka_unit_test(system_name),
		cmocka_unit_test_setup(from_virtual_key, fill),
		cmocka_unit_test_setup(to_virtual_key, fill),
		cmocka_unit_test(combination),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
