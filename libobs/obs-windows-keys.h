/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#pragma once

#include "obs-internal.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The Windows key map and key name route of obs-windows.c, on plain values
 * (obs-windows-keys.c):
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
 * obs-windows.c keeps the platform context (its vk_codes table, allocation
 * and free), GetAsyncKeyState(), MapVirtualKey(), GetKeyNameTextW(), the
 * hotkey translations, obs_key_to_name() and every dstr call. Nothing here
 * calls into Windows, allocates, logs or keeps state. Without EXPORT, so
 * hidden in libobs.
 *
 * A virtual key is an int, as in vk_codes, and a scan code a UINT (32 bits),
 * as MapVirtualKey() returns it. obs-windows.c checks UINT's size and every
 * virtual key below against winuser.h, which this file does not include. */

/* The VK_* virtual keys of winuser.h that the key map uses, by value. */
#define OBS_WINDOWS_KEYS_VK_LBUTTON 0x01
#define OBS_WINDOWS_KEYS_VK_RBUTTON 0x02
#define OBS_WINDOWS_KEYS_VK_CANCEL 0x03
#define OBS_WINDOWS_KEYS_VK_MBUTTON 0x04
#define OBS_WINDOWS_KEYS_VK_XBUTTON1 0x05
#define OBS_WINDOWS_KEYS_VK_XBUTTON2 0x06
#define OBS_WINDOWS_KEYS_VK_BACK 0x08
#define OBS_WINDOWS_KEYS_VK_TAB 0x09
#define OBS_WINDOWS_KEYS_VK_CLEAR 0x0C
#define OBS_WINDOWS_KEYS_VK_RETURN 0x0D
#define OBS_WINDOWS_KEYS_VK_SHIFT 0x10
#define OBS_WINDOWS_KEYS_VK_CONTROL 0x11
#define OBS_WINDOWS_KEYS_VK_MENU 0x12
#define OBS_WINDOWS_KEYS_VK_PAUSE 0x13
#define OBS_WINDOWS_KEYS_VK_CAPITAL 0x14
#define OBS_WINDOWS_KEYS_VK_HANGUL 0x15
#define OBS_WINDOWS_KEYS_VK_JUNJA 0x17
#define OBS_WINDOWS_KEYS_VK_FINAL 0x18
#define OBS_WINDOWS_KEYS_VK_KANJI 0x19
#define OBS_WINDOWS_KEYS_VK_ESCAPE 0x1B
#define OBS_WINDOWS_KEYS_VK_CONVERT 0x1C
#define OBS_WINDOWS_KEYS_VK_NONCONVERT 0x1D
#define OBS_WINDOWS_KEYS_VK_ACCEPT 0x1E
#define OBS_WINDOWS_KEYS_VK_MODECHANGE 0x1F
#define OBS_WINDOWS_KEYS_VK_SPACE 0x20
#define OBS_WINDOWS_KEYS_VK_PRIOR 0x21
#define OBS_WINDOWS_KEYS_VK_NEXT 0x22
#define OBS_WINDOWS_KEYS_VK_END 0x23
#define OBS_WINDOWS_KEYS_VK_HOME 0x24
#define OBS_WINDOWS_KEYS_VK_LEFT 0x25
#define OBS_WINDOWS_KEYS_VK_UP 0x26
#define OBS_WINDOWS_KEYS_VK_RIGHT 0x27
#define OBS_WINDOWS_KEYS_VK_DOWN 0x28
#define OBS_WINDOWS_KEYS_VK_SELECT 0x29
#define OBS_WINDOWS_KEYS_VK_PRINT 0x2A
#define OBS_WINDOWS_KEYS_VK_EXECUTE 0x2B
#define OBS_WINDOWS_KEYS_VK_SNAPSHOT 0x2C
#define OBS_WINDOWS_KEYS_VK_INSERT 0x2D
#define OBS_WINDOWS_KEYS_VK_DELETE 0x2E
#define OBS_WINDOWS_KEYS_VK_HELP 0x2F
#define OBS_WINDOWS_KEYS_VK_LWIN 0x5B
#define OBS_WINDOWS_KEYS_VK_RWIN 0x5C
#define OBS_WINDOWS_KEYS_VK_APPS 0x5D
#define OBS_WINDOWS_KEYS_VK_SLEEP 0x5F
#define OBS_WINDOWS_KEYS_VK_NUMPAD0 0x60
#define OBS_WINDOWS_KEYS_VK_NUMPAD1 0x61
#define OBS_WINDOWS_KEYS_VK_NUMPAD2 0x62
#define OBS_WINDOWS_KEYS_VK_NUMPAD3 0x63
#define OBS_WINDOWS_KEYS_VK_NUMPAD4 0x64
#define OBS_WINDOWS_KEYS_VK_NUMPAD5 0x65
#define OBS_WINDOWS_KEYS_VK_NUMPAD6 0x66
#define OBS_WINDOWS_KEYS_VK_NUMPAD7 0x67
#define OBS_WINDOWS_KEYS_VK_NUMPAD8 0x68
#define OBS_WINDOWS_KEYS_VK_NUMPAD9 0x69
#define OBS_WINDOWS_KEYS_VK_MULTIPLY 0x6A
#define OBS_WINDOWS_KEYS_VK_ADD 0x6B
#define OBS_WINDOWS_KEYS_VK_SEPARATOR 0x6C
#define OBS_WINDOWS_KEYS_VK_SUBTRACT 0x6D
#define OBS_WINDOWS_KEYS_VK_DECIMAL 0x6E
#define OBS_WINDOWS_KEYS_VK_DIVIDE 0x6F
#define OBS_WINDOWS_KEYS_VK_F1 0x70
#define OBS_WINDOWS_KEYS_VK_F2 0x71
#define OBS_WINDOWS_KEYS_VK_F3 0x72
#define OBS_WINDOWS_KEYS_VK_F4 0x73
#define OBS_WINDOWS_KEYS_VK_F5 0x74
#define OBS_WINDOWS_KEYS_VK_F6 0x75
#define OBS_WINDOWS_KEYS_VK_F7 0x76
#define OBS_WINDOWS_KEYS_VK_F8 0x77
#define OBS_WINDOWS_KEYS_VK_F9 0x78
#define OBS_WINDOWS_KEYS_VK_F10 0x79
#define OBS_WINDOWS_KEYS_VK_F11 0x7A
#define OBS_WINDOWS_KEYS_VK_F12 0x7B
#define OBS_WINDOWS_KEYS_VK_F13 0x7C
#define OBS_WINDOWS_KEYS_VK_F14 0x7D
#define OBS_WINDOWS_KEYS_VK_F15 0x7E
#define OBS_WINDOWS_KEYS_VK_F16 0x7F
#define OBS_WINDOWS_KEYS_VK_F17 0x80
#define OBS_WINDOWS_KEYS_VK_F18 0x81
#define OBS_WINDOWS_KEYS_VK_F19 0x82
#define OBS_WINDOWS_KEYS_VK_F20 0x83
#define OBS_WINDOWS_KEYS_VK_F21 0x84
#define OBS_WINDOWS_KEYS_VK_F22 0x85
#define OBS_WINDOWS_KEYS_VK_F23 0x86
#define OBS_WINDOWS_KEYS_VK_F24 0x87
#define OBS_WINDOWS_KEYS_VK_NUMLOCK 0x90
#define OBS_WINDOWS_KEYS_VK_SCROLL 0x91
#define OBS_WINDOWS_KEYS_VK_OEM_FJ_JISHO 0x92
#define OBS_WINDOWS_KEYS_VK_OEM_FJ_MASSHOU 0x93
#define OBS_WINDOWS_KEYS_VK_OEM_FJ_TOUROKU 0x94
#define OBS_WINDOWS_KEYS_VK_OEM_FJ_LOYA 0x95
#define OBS_WINDOWS_KEYS_VK_OEM_FJ_ROYA 0x96
#define OBS_WINDOWS_KEYS_VK_LSHIFT 0xA0
#define OBS_WINDOWS_KEYS_VK_RSHIFT 0xA1
#define OBS_WINDOWS_KEYS_VK_LCONTROL 0xA2
#define OBS_WINDOWS_KEYS_VK_RCONTROL 0xA3
#define OBS_WINDOWS_KEYS_VK_LMENU 0xA4
#define OBS_WINDOWS_KEYS_VK_RMENU 0xA5
#define OBS_WINDOWS_KEYS_VK_BROWSER_BACK 0xA6
#define OBS_WINDOWS_KEYS_VK_BROWSER_FORWARD 0xA7
#define OBS_WINDOWS_KEYS_VK_BROWSER_REFRESH 0xA8
#define OBS_WINDOWS_KEYS_VK_BROWSER_STOP 0xA9
#define OBS_WINDOWS_KEYS_VK_BROWSER_SEARCH 0xAA
#define OBS_WINDOWS_KEYS_VK_BROWSER_FAVORITES 0xAB
#define OBS_WINDOWS_KEYS_VK_BROWSER_HOME 0xAC
#define OBS_WINDOWS_KEYS_VK_VOLUME_MUTE 0xAD
#define OBS_WINDOWS_KEYS_VK_VOLUME_DOWN 0xAE
#define OBS_WINDOWS_KEYS_VK_VOLUME_UP 0xAF
#define OBS_WINDOWS_KEYS_VK_MEDIA_NEXT_TRACK 0xB0
#define OBS_WINDOWS_KEYS_VK_MEDIA_PREV_TRACK 0xB1
#define OBS_WINDOWS_KEYS_VK_MEDIA_STOP 0xB2
#define OBS_WINDOWS_KEYS_VK_MEDIA_PLAY_PAUSE 0xB3
#define OBS_WINDOWS_KEYS_VK_LAUNCH_MAIL 0xB4
#define OBS_WINDOWS_KEYS_VK_LAUNCH_MEDIA_SELECT 0xB5
#define OBS_WINDOWS_KEYS_VK_LAUNCH_APP1 0xB6
#define OBS_WINDOWS_KEYS_VK_LAUNCH_APP2 0xB7
#define OBS_WINDOWS_KEYS_VK_OEM_1 0xBA
#define OBS_WINDOWS_KEYS_VK_OEM_PLUS 0xBB
#define OBS_WINDOWS_KEYS_VK_OEM_COMMA 0xBC
#define OBS_WINDOWS_KEYS_VK_OEM_MINUS 0xBD
#define OBS_WINDOWS_KEYS_VK_OEM_PERIOD 0xBE
#define OBS_WINDOWS_KEYS_VK_OEM_2 0xBF
#define OBS_WINDOWS_KEYS_VK_OEM_3 0xC0
#define OBS_WINDOWS_KEYS_VK_OEM_4 0xDB
#define OBS_WINDOWS_KEYS_VK_OEM_5 0xDC
#define OBS_WINDOWS_KEYS_VK_OEM_6 0xDD
#define OBS_WINDOWS_KEYS_VK_OEM_7 0xDE
#define OBS_WINDOWS_KEYS_VK_OEM_8 0xDF
#define OBS_WINDOWS_KEYS_VK_OEM_AX 0xE1
#define OBS_WINDOWS_KEYS_VK_OEM_102 0xE2
#define OBS_WINDOWS_KEYS_VK_ICO_HELP 0xE3
#define OBS_WINDOWS_KEYS_VK_ICO_00 0xE4
#define OBS_WINDOWS_KEYS_VK_PROCESSKEY 0xE5
#define OBS_WINDOWS_KEYS_VK_ICO_CLEAR 0xE6
#define OBS_WINDOWS_KEYS_VK_PACKET 0xE7
#define OBS_WINDOWS_KEYS_VK_OEM_RESET 0xE9
#define OBS_WINDOWS_KEYS_VK_OEM_JUMP 0xEA
#define OBS_WINDOWS_KEYS_VK_OEM_PA1 0xEB
#define OBS_WINDOWS_KEYS_VK_OEM_PA2 0xEC
#define OBS_WINDOWS_KEYS_VK_OEM_PA3 0xED
#define OBS_WINDOWS_KEYS_VK_OEM_WSCTRL 0xEE
#define OBS_WINDOWS_KEYS_VK_OEM_CUSEL 0xEF
#define OBS_WINDOWS_KEYS_VK_OEM_ATTN 0xF0
#define OBS_WINDOWS_KEYS_VK_OEM_FINISH 0xF1
#define OBS_WINDOWS_KEYS_VK_OEM_COPY 0xF2
#define OBS_WINDOWS_KEYS_VK_OEM_AUTO 0xF3
#define OBS_WINDOWS_KEYS_VK_OEM_ENLW 0xF4
#define OBS_WINDOWS_KEYS_VK_OEM_BACKTAB 0xF5
#define OBS_WINDOWS_KEYS_VK_ATTN 0xF6
#define OBS_WINDOWS_KEYS_VK_CRSEL 0xF7
#define OBS_WINDOWS_KEYS_VK_EXSEL 0xF8
#define OBS_WINDOWS_KEYS_VK_EREOF 0xF9
#define OBS_WINDOWS_KEYS_VK_PLAY 0xFA
#define OBS_WINDOWS_KEYS_VK_ZOOM 0xFB
#define OBS_WINDOWS_KEYS_VK_NONAME 0xFC
#define OBS_WINDOWS_KEYS_VK_PA1 0xFD
#define OBS_WINDOWS_KEYS_VK_OEM_CLEAR 0xFE

/* The extended-key flag of a GetKeyNameTextW() key code (bit 24 of the
 * lParam of a keyboard message). */
#define OBS_WINDOWS_KEYS_EXTENDED 0x01000000

/* The most keys obs_windows_keys_combination() gives: four modifiers and
 * the key. */
#define OBS_WINDOWS_KEYS_COMBINATION_MAX 5

/* The virtual key of key: the mouse buttons and the keys of a US keyboard
 * their VK_*, the OBS_KEY_VK_* keys theirs, any other key 0. */
int obs_windows_keys_virtual_key(obs_key_t key);

/* Whether key is pressed when either Windows key is down (OBS_KEY_META),
 * rather than when its own virtual key is. */
bool obs_windows_keys_either_win(obs_key_t key);

/* How obs_key_to_str() names a key. */
enum obs_windows_keys_name {
	/* OBS_KEY_NONE: no name. */
	OBS_WINDOWS_KEYS_NAME_NONE,
	/* OBS_KEY_F13 to OBS_KEY_F24: "F<number>". */
	OBS_WINDOWS_KEYS_NAME_F,
	/* OBS_KEY_MOUSE1 to OBS_KEY_MOUSE29: its translation if it has one, else
	 * "Mouse <number>". */
	OBS_WINDOWS_KEYS_NAME_MOUSE,
	/* OBS_KEY_PAUSE: the "Pause" translation. */
	OBS_WINDOWS_KEYS_NAME_PAUSE,
	/* OBS_KEY_META: the "Windows" translation. */
	OBS_WINDOWS_KEYS_NAME_META,
	/* Any other key: the system's name of its scan code, or else its
	 * obs_key_to_name() name. */
	OBS_WINDOWS_KEYS_NAME_SYSTEM,
};

/* The route of key. Sets *number only for OBS_WINDOWS_KEYS_NAME_F (13 to 24)
 * and OBS_WINDOWS_KEYS_NAME_MOUSE (1 to 29). */
enum obs_windows_keys_name obs_windows_keys_name(obs_key_t key, int *number);

/* The GetKeyNameTextW() key code of virtual key vk, whose MapVirtualKey()
 * scan code is mapped: mapped in bits 16 and up, with
 * OBS_WINDOWS_KEYS_EXTENDED for the 11 extended keys (Home, End, the arrows,
 * Page Up and Down, Insert, Delete and Num Lock). */
uint32_t obs_windows_keys_scan_code(int vk, uint32_t mapped);

/* Whether key's name is asked of the system (GetKeyNameTextW()): not an
 * OBS_KEY_VK_* key, and a non-zero scan_code. */
bool obs_windows_keys_system_name(obs_key_t key, uint32_t scan_code);

/* The first key whose entry in vk_codes (OBS_KEY_LAST_VALUE entries) is
 * code, or OBS_KEY_NONE. */
obs_key_t obs_windows_keys_from_virtual_key(const int *vk_codes, int code);

/* The vk_codes table, as obs_windows_keys_to_virtual_key() asks for it. */
typedef const int *(*obs_windows_keys_table_fn)(void);

/* VK_LWIN for OBS_KEY_META; any other key's entry in the table vk_codes()
 * returns, which is asked for only then. vk_codes()[key] is read for any
 * key. */
int obs_windows_keys_to_virtual_key(obs_key_t key, obs_windows_keys_table_fn vk_codes);

/* The keys obs_key_combination_to_str() names for modifiers (INTERACT_*
 * bits) and key, in order: Control, Meta, Alt, Shift for their bits, then
 * key unless it is OBS_KEY_NONE. Writes them to keys
 * (OBS_WINDOWS_KEYS_COMBINATION_MAX entries) and returns how many. */
size_t obs_windows_keys_combination(uint32_t modifiers, obs_key_t key, obs_key_t *keys);
