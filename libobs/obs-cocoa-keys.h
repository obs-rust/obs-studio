/******************************************************************************
    Copyright (C) 2013 by Ruwen Hahn <palana@stunned.de>

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
#include <stdint.h>

/* The macOS key map and pressed check of obs-cocoa.m, on plain values
 * (obs-cocoa-keys.c):
 * - obs_cocoa_keys_modifier(): KeyboardEventProc()'s modifier flag map;
 * - obs_cocoa_keys_to_virtual_key(): obs_key_to_virtual_key();
 * - obs_cocoa_keys_from_virtual_key(): obs_key_from_virtual_key();
 * - obs_cocoa_keys_is_mouse(), obs_cocoa_keys_mouse_pressed() and
 *   obs_cocoa_keys_in_range(): obs_hotkeys_platform_is_pressed()'s mouse
 *   range, its button bit and the key range gate.
 *
 * obs-cocoa.m keeps the tables (virtual_keys, key_descriptions,
 * key_glyphs), the key names, the event tap and its callback, the input
 * source and every CGEvent, TIS, CF and NSEvent call, the key state and its
 * writes, the NULL platform check and every log. Nothing here calls into
 * macOS, allocates, logs or keeps state. Without EXPORT, so hidden in
 * libobs.
 *
 * An event's flags are a CGEventFlags (uint64_t) and the pressed mouse
 * buttons an NSUInteger (64 bits on macOS); obs-cocoa.m checks both, and the
 * flag bits and INVALID_KEY below, against the macOS headers, which this
 * file does not include. */

/* kCGEventFlagMaskShift, kCGEventFlagMaskControl, kCGEventFlagMaskAlternate
 * and kCGEventFlagMaskCommand (CoreGraphics/CGEventTypes.h). */
#define OBS_COCOA_KEYS_FLAG_SHIFT 0x00020000
#define OBS_COCOA_KEYS_FLAG_CONTROL 0x00040000
#define OBS_COCOA_KEYS_FLAG_ALTERNATE 0x00080000
#define OBS_COCOA_KEYS_FLAG_COMMAND 0x00100000

/* obs-cocoa.m's INVALID_KEY: the virtual key of a key with none. */
#define OBS_COCOA_KEYS_INVALID 0xFF

/* One entry of obs-cocoa.m's virtual_keys table: a key's macOS virtual key
 * code (kVK_*), if it has one. */
typedef struct obs_key_code {
	int code;
	bool is_valid;
} obs_key_code_t;

/* Whether the modifier key is down in an event's flags: OBS_KEY_SHIFT,
 * OBS_KEY_ALT, OBS_KEY_META and OBS_KEY_CONTROL read the Shift, Alternate
 * (Option), Command and Control bits; any other key is not down. */
bool obs_cocoa_keys_modifier(uint64_t flags, obs_key_t key);

/* The virtual key code of key in table (OBS_KEY_LAST_VALUE entries) when
 * its entry is valid, else OBS_COCOA_KEYS_INVALID. table[key] is read for
 * any key. */
int obs_cocoa_keys_to_virtual_key(const obs_key_code_t *table, obs_key_t key);

/* The first key whose valid entry in table (OBS_KEY_LAST_VALUE entries) has
 * code, or OBS_KEY_NONE. */
obs_key_t obs_cocoa_keys_from_virtual_key(const obs_key_code_t *table, int code);

/* Whether key is a mouse button, OBS_KEY_MOUSE1 to OBS_KEY_MOUSE29. */
bool obs_cocoa_keys_is_mouse(obs_key_t key);

/* Whether the mouse key's button is down in buttons, the pressed mouse
 * button mask (bit 0 the left button). */
bool obs_cocoa_keys_mouse_pressed(obs_key_t key, uint64_t buttons);

/* Whether key indexes the key state: below OBS_KEY_LAST_VALUE. */
bool obs_cocoa_keys_in_range(obs_key_t key);
