/******************************************************************************
    Copyright (C) 2014-2015 by Ruwen Hahn <palana@stunned.de>

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

#include "libobs/build/libobs.pch.h"

/* The hotkey key-name translations (moved from obs-hotkey.c.h):
 * obs_hotkeys_set_translations_s() copies the frontend's translated names of
 * the named keys into the key name store, and expands its numpad and mouse
 * templates ("Numpad %1", "Mouse %1") into the names of NUM0-NUM9 and
 * MOUSE1-MOUSE29. A NULL, empty or padding-only template is skipped, so those
 * keys keep their names. The store, obs_hotkeys_store_key_translation(), and
 * its readers (obs_get_hotkey_translation() and the platform key_to_str
 * functions) stay in obs-hotkey.c.h and the platform files. */

void obs_hotkeys_set_translations_s(struct obs_hotkeys_translations *translations, size_t size)
{
#define ADD_TRANSLATION(key_name, var_name) \
	if (t.var_name)                     \
		obs_hotkeys_store_key_translation(key_name, t.var_name);

	struct obs_hotkeys_translations t = {0};
	struct dstr numpad = {0};
	struct dstr mouse = {0};
	struct dstr button = {0};

	if (!translations) {
		return;
	}

	memcpy(&t, translations, (size < sizeof(t)) ? size : sizeof(t));

	ADD_TRANSLATION(OBS_KEY_INSERT, insert);
	ADD_TRANSLATION(OBS_KEY_DELETE, del);
	ADD_TRANSLATION(OBS_KEY_HOME, home);
	ADD_TRANSLATION(OBS_KEY_END, end);
	ADD_TRANSLATION(OBS_KEY_PAGEUP, page_up);
	ADD_TRANSLATION(OBS_KEY_PAGEDOWN, page_down);
	ADD_TRANSLATION(OBS_KEY_NUMLOCK, num_lock);
	ADD_TRANSLATION(OBS_KEY_SCROLLLOCK, scroll_lock);
	ADD_TRANSLATION(OBS_KEY_CAPSLOCK, caps_lock);
	ADD_TRANSLATION(OBS_KEY_BACKSPACE, backspace);
	ADD_TRANSLATION(OBS_KEY_TAB, tab);
	ADD_TRANSLATION(OBS_KEY_PRINT, print);
	ADD_TRANSLATION(OBS_KEY_PAUSE, pause);
	ADD_TRANSLATION(OBS_KEY_LEFT, left);
	ADD_TRANSLATION(OBS_KEY_RIGHT, right);
	ADD_TRANSLATION(OBS_KEY_UP, up);
	ADD_TRANSLATION(OBS_KEY_DOWN, down);
	ADD_TRANSLATION(OBS_KEY_SHIFT, shift);
	ADD_TRANSLATION(OBS_KEY_ALT, alt);
	ADD_TRANSLATION(OBS_KEY_CONTROL, control);
	ADD_TRANSLATION(OBS_KEY_META, meta);
	ADD_TRANSLATION(OBS_KEY_MENU, menu);
	ADD_TRANSLATION(OBS_KEY_SPACE, space);
	ADD_TRANSLATION(OBS_KEY_ESCAPE, escape);
#ifdef __APPLE__
	const char *numpad_str = t.apple_keypad_num;
	ADD_TRANSLATION(OBS_KEY_NUMSLASH, apple_keypad_divide);
	ADD_TRANSLATION(OBS_KEY_NUMASTERISK, apple_keypad_multiply);
	ADD_TRANSLATION(OBS_KEY_NUMMINUS, apple_keypad_minus);
	ADD_TRANSLATION(OBS_KEY_NUMPLUS, apple_keypad_plus);
	ADD_TRANSLATION(OBS_KEY_NUMPERIOD, apple_keypad_decimal);
	ADD_TRANSLATION(OBS_KEY_NUMEQUAL, apple_keypad_equal);
#else
	const char *numpad_str = t.numpad_num;
	ADD_TRANSLATION(OBS_KEY_NUMSLASH, numpad_divide);
	ADD_TRANSLATION(OBS_KEY_NUMASTERISK, numpad_multiply);
	ADD_TRANSLATION(OBS_KEY_NUMMINUS, numpad_minus);
	ADD_TRANSLATION(OBS_KEY_NUMPLUS, numpad_plus);
	ADD_TRANSLATION(OBS_KEY_NUMPERIOD, numpad_decimal);
#endif

	/* dstr_copy() of "" leaves the array NULL, and dstr_depad() of a
	 * padding-only string frees it: skip those like a NULL template. */
	if (numpad_str) {
		dstr_copy(&numpad, numpad_str);
		dstr_depad(&numpad);
	}

	if (!dstr_is_empty(&numpad)) {
		if (dstr_find(&numpad, "%1") == NULL) {
			dstr_cat(&numpad, " %1");
		}

#define ADD_NUMPAD_NUM(idx)                \
	dstr_copy_dstr(&button, &numpad);  \
	dstr_replace(&button, "%1", #idx); \
	obs_hotkeys_store_key_translation(OBS_KEY_NUM##idx, button.array)

		ADD_NUMPAD_NUM(0);
		ADD_NUMPAD_NUM(1);
		ADD_NUMPAD_NUM(2);
		ADD_NUMPAD_NUM(3);
		ADD_NUMPAD_NUM(4);
		ADD_NUMPAD_NUM(5);
		ADD_NUMPAD_NUM(6);
		ADD_NUMPAD_NUM(7);
		ADD_NUMPAD_NUM(8);
		ADD_NUMPAD_NUM(9);
	}

	if (t.mouse_num) {
		dstr_copy(&mouse, t.mouse_num);
		dstr_depad(&mouse);
	}

	if (!dstr_is_empty(&mouse)) {
		if (dstr_find(&mouse, "%1") == NULL) {
			dstr_cat(&mouse, " %1");
		}

#define ADD_MOUSE_NUM(idx)                 \
	dstr_copy_dstr(&button, &mouse);   \
	dstr_replace(&button, "%1", #idx); \
	obs_hotkeys_store_key_translation(OBS_KEY_MOUSE##idx, button.array)

		ADD_MOUSE_NUM(1);
		ADD_MOUSE_NUM(2);
		ADD_MOUSE_NUM(3);
		ADD_MOUSE_NUM(4);
		ADD_MOUSE_NUM(5);
		ADD_MOUSE_NUM(6);
		ADD_MOUSE_NUM(7);
		ADD_MOUSE_NUM(8);
		ADD_MOUSE_NUM(9);
		ADD_MOUSE_NUM(10);
		ADD_MOUSE_NUM(11);
		ADD_MOUSE_NUM(12);
		ADD_MOUSE_NUM(13);
		ADD_MOUSE_NUM(14);
		ADD_MOUSE_NUM(15);
		ADD_MOUSE_NUM(16);
		ADD_MOUSE_NUM(17);
		ADD_MOUSE_NUM(18);
		ADD_MOUSE_NUM(19);
		ADD_MOUSE_NUM(20);
		ADD_MOUSE_NUM(21);
		ADD_MOUSE_NUM(22);
		ADD_MOUSE_NUM(23);
		ADD_MOUSE_NUM(24);
		ADD_MOUSE_NUM(25);
		ADD_MOUSE_NUM(26);
		ADD_MOUSE_NUM(27);
		ADD_MOUSE_NUM(28);
		ADD_MOUSE_NUM(29);
	}

	dstr_free(&numpad);
	dstr_free(&mouse);
	dstr_free(&button);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef ADD_TRANSLATION
#undef ADD_NUMPAD_NUM
#undef ADD_MOUSE_NUM
