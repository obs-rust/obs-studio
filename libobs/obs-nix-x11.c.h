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

#include "libobs/build/libobs-nix.pch.h"

void obs_nix_x11_log_info(void)
{
	Display *dpy = obs_get_nix_platform_display();
	if (!dpy) {
		blog(LOG_INFO, "Unable to open X display");
		return;
	}

	int protocol_version = ProtocolVersion(dpy);
	int protocol_revision = ProtocolRevision(dpy);
	int vendor_release = VendorRelease(dpy);
	const char *vendor_name = ServerVendor(dpy);

	if (strstr(vendor_name, "X.Org")) {
		blog(LOG_INFO,
		     "Window System: X%d.%d, Vendor: %s, Version: %d"
		     ".%d.%d",
		     protocol_version, protocol_revision, vendor_name, vendor_release / 10000000,
		     (vendor_release / 100000) % 100, (vendor_release / 1000) % 100);
	} else {
		blog(LOG_INFO,
		     "Window System: X%d.%d - vendor string: %s - "
		     "vendor release: %d",
		     protocol_version, protocol_revision, vendor_name, vendor_release);
	}
}

/* So here's how linux works with key mapping:
 *
 * First, there's a global key symbol enum (xcb_keysym_t) which has unique
 * values for all possible symbols keys can have (e.g., '1' and '!' are
 * different values).
 *
 * Then there's a key code (xcb_keycode_t), which is basically an index to the
 * actual key itself on the keyboard (e.g., '1' and '!' will share the same
 * value).
 *
 * xcb_keysym_t values should be given to libobs, and libobs will translate it
 * to an obs_key_t, and although xcb_keysym_t can differ ('!' vs '1'), it will
 * get the obs_key_t value that represents the actual key pressed; in other
 * words it will be based on the key code rather than the key symbol.  The same
 * applies to checking key press states.
 */

/* struct keycode_list and the lookups on it are in obs-nix-x11-keys.h, which
 * includes no X header: check what it assumes about them here. */
_Static_assert(sizeof(xcb_keysym_t) == sizeof(uint32_t), "xcb_keysym_t is a uint32_t");
_Static_assert(sizeof(xcb_keycode_t) == sizeof(uint8_t), "xcb_keycode_t is a uint8_t");
_Static_assert(OBS_NIX_X11_KEYS_SHIFT_R == XK_Shift_R, "XK_Shift_R");
_Static_assert(OBS_NIX_X11_KEYS_CONTROL_R == XK_Control_R, "XK_Control_R");
_Static_assert(OBS_NIX_X11_KEYS_ALT_R == XK_Alt_R, "XK_Alt_R");
_Static_assert(OBS_NIX_X11_KEYS_SUPER_L == XK_Super_L, "XK_Super_L");
_Static_assert(OBS_NIX_X11_KEYS_SUPER_R == XK_Super_R, "XK_Super_R");
_Static_assert(sizeof(((xcb_query_keymap_reply_t *)0)->keys) == OBS_NIX_X11_KEYS_KEYMAP_LEN,
	       "a QueryKeymap reply has a 32-byte key bitmap");

/* The keysym table and the mouse button answers are in obs-nix-x11-input.h,
 * which includes no X header: check the pointer button masks it uses here. */
_Static_assert(OBS_NIX_X11_INPUT_BUTTON_MASK_1 == XCB_BUTTON_MASK_1, "XCB_BUTTON_MASK_1");
_Static_assert(OBS_NIX_X11_INPUT_BUTTON_MASK_2 == XCB_BUTTON_MASK_2, "XCB_BUTTON_MASK_2");
_Static_assert(OBS_NIX_X11_INPUT_BUTTON_MASK_3 == XCB_BUTTON_MASK_3, "XCB_BUTTON_MASK_3");

struct obs_hotkeys_platform {
	Display *display;
	xcb_keysym_t base_keysyms[OBS_KEY_LAST_VALUE];
	struct keycode_list keycodes[OBS_KEY_LAST_VALUE];
	xcb_keycode_t min_keycode;
	xcb_keycode_t super_l_code;
	xcb_keycode_t super_r_code;

	/* stores a copy of the keysym map for keycodes */
	xcb_keysym_t *keysyms;
	int num_keysyms;
	int syms_per_code;

#if defined(XCB_XINPUT_FOUND)
	bool pressed[XINPUT_MOUSE_LEN];
	bool update[XINPUT_MOUSE_LEN];
	bool button_pressed[XINPUT_MOUSE_LEN];
#endif
};

static inline void fill_base_keysyms(struct obs_core_hotkeys *hotkeys)
{
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++)
		hotkeys->platform_context->base_keysyms[i] = (xcb_keysym_t)obs_nix_x11_input_keysym((obs_key_t)i);
}

static inline void add_key(obs_hotkeys_platform_t *context, obs_key_t key, int code)
{
	xcb_keycode_t kc = (xcb_keycode_t)code;
	da_push_back(context->keycodes[key].list, &kc);

	if (context->keycodes[key].list.num > 1) {
		blog(LOG_DEBUG,
		     "found alternate keycode %d for %s "
		     "which already has keycode %d",
		     code, obs_key_to_name(key), (int)context->keycodes[key].list.array[0]);
	}
}

static inline bool fill_keycodes(struct obs_core_hotkeys *hotkeys)
{
	obs_hotkeys_platform_t *context = hotkeys->platform_context;
	xcb_connection_t *connection = XGetXCBConnection(context->display);
	const struct xcb_setup_t *setup = xcb_get_setup(connection);
	xcb_get_keyboard_mapping_cookie_t cookie;
	xcb_get_keyboard_mapping_reply_t *reply;
	xcb_generic_error_t *error = NULL;
	int code;

	int mincode = setup->min_keycode;
	int maxcode = setup->max_keycode;

	context->min_keycode = setup->min_keycode;

	cookie = xcb_get_keyboard_mapping(connection, (xcb_keycode_t)mincode, (uint8_t)(maxcode - mincode + 1));

	reply = xcb_get_keyboard_mapping_reply(connection, cookie, &error);

	if (error || !reply) {
		blog(LOG_WARNING, "xcb_get_keyboard_mapping_reply failed");
		goto error1;
	}

	const xcb_keysym_t *keysyms = xcb_get_keyboard_mapping_keysyms(reply);
	int syms_per_code = (int)reply->keysyms_per_keycode;

	context->num_keysyms = (maxcode - mincode + 1) * syms_per_code;
	context->syms_per_code = syms_per_code;
	context->keysyms = bmemdup(keysyms, sizeof(xcb_keysym_t) * (size_t)context->num_keysyms);

	for (code = mincode; code <= maxcode; code++) {
		const xcb_keysym_t *sym;
		obs_key_t key;

		sym = &keysyms[(code - mincode) * syms_per_code];

		switch (obs_nix_x11_keys_row(sym, syms_per_code, context->base_keysyms, &key)) {
		case OBS_NIX_X11_KEYS_ROW_SUPER_L:
			context->super_l_code = (xcb_keycode_t)code;
			break;
		case OBS_NIX_X11_KEYS_ROW_SUPER_R:
			context->super_r_code = (xcb_keycode_t)code;
			break;
		case OBS_NIX_X11_KEYS_ROW_KEY:
			add_key(context, key, code);
			break;
		case OBS_NIX_X11_KEYS_ROW_NONE:
			break;
		}
	}

error1:
	free(reply);
	free(error);

	return error != NULL || reply == NULL;
}

static xcb_screen_t *default_screen(obs_hotkeys_platform_t *context, xcb_connection_t *connection)
{
	int def_screen_idx = XDefaultScreen(context->display);
	xcb_screen_iterator_t iter;

	iter = xcb_setup_roots_iterator(xcb_get_setup(connection));
	while (iter.rem) {
		if (def_screen_idx-- == 0)
			return iter.data;

		xcb_screen_next(&iter);
	}

	return NULL;
}

static inline xcb_window_t root_window(obs_hotkeys_platform_t *context, xcb_connection_t *connection)
{
	xcb_screen_t *screen = default_screen(context, connection);
	if (screen)
		return screen->root;
	return 0;
}

#if defined(XCB_XINPUT_FOUND)
static inline void registerMouseEvents(struct obs_core_hotkeys *hotkeys)
{
	obs_hotkeys_platform_t *context = hotkeys->platform_context;
	xcb_connection_t *connection = XGetXCBConnection(context->display);
	xcb_window_t window = root_window(context, connection);

	struct {
		xcb_input_event_mask_t head;
		xcb_input_xi_event_mask_t mask;
	} mask;
	mask.head.deviceid = XCB_INPUT_DEVICE_ALL_MASTER;
	mask.head.mask_len = sizeof(mask.mask) / sizeof(uint32_t);
	mask.mask = XCB_INPUT_XI_EVENT_MASK_RAW_BUTTON_PRESS | XCB_INPUT_XI_EVENT_MASK_RAW_BUTTON_RELEASE;

	xcb_input_xi_select_events(connection, window, 1, &mask.head);
	xcb_flush(connection);
}
#endif

static bool obs_nix_x11_hotkeys_platform_init(struct obs_core_hotkeys *hotkeys)
{
	// Open a new X11 connection here, this avoids Qt masking events we care about.
	Display *display = XOpenDisplay(NULL);
	if (!display) {
		const char *display_name = getenv("DISPLAY");
		blog(LOG_ERROR, "XOpenDisplay failed during hotkey initialization (DISPLAY=%s)",
		     display_name ? display_name : "unset");
		return false;
	}

	hotkeys->platform_context = bzalloc(sizeof(obs_hotkeys_platform_t));
	hotkeys->platform_context->display = display;

#if defined(XCB_XINPUT_FOUND)
	registerMouseEvents(hotkeys);
#endif
	fill_base_keysyms(hotkeys);
	fill_keycodes(hotkeys);
	return true;
}

static void obs_nix_x11_hotkeys_platform_free(struct obs_core_hotkeys *hotkeys)
{
	obs_hotkeys_platform_t *context = hotkeys->platform_context;
	if (!context)
		return;

	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++)
		da_free(context->keycodes[i].list);

	bfree(context->keysyms);
	XCloseDisplay(context->display);
	bfree(context);

	hotkeys->platform_context = NULL;
}

static bool mouse_button_pressed(xcb_connection_t *connection, obs_hotkeys_platform_t *context, obs_key_t key)
{
	bool ret = false;

#if defined(XCB_XINPUT_FOUND)
	memset(context->pressed, 0, XINPUT_MOUSE_LEN);
	memset(context->update, 0, XINPUT_MOUSE_LEN);

	xcb_generic_event_t *ev;
	while ((ev = xcb_poll_for_event(connection))) {
		if ((ev->response_type & ~80) == XCB_GE_GENERIC) {
			switch (((xcb_ge_event_t *)ev)->event_type) {
			case XCB_INPUT_RAW_BUTTON_PRESS: {
				xcb_input_raw_button_press_event_t *mot;
				mot = (xcb_input_raw_button_press_event_t *)ev;
				if (mot->detail < XINPUT_MOUSE_LEN) {
					context->pressed[mot->detail - 1] = true;
					context->update[mot->detail - 1] = true;
				} else {
					blog(LOG_WARNING, "Unsupported button");
				}
				break;
			}
			case XCB_INPUT_RAW_BUTTON_RELEASE: {
				xcb_input_raw_button_release_event_t *mot;
				mot = (xcb_input_raw_button_release_event_t *)ev;
				if (mot->detail < XINPUT_MOUSE_LEN)
					context->update[mot->detail - 1] = true;
				else
					blog(LOG_WARNING, "Unsupported button");
				break;
			}
			default:
				break;
			}
		}
		free(ev);
	}

	ret = obs_nix_x11_input_mouse_pressed(key, context->pressed, context->button_pressed);

	obs_nix_x11_input_commit(context->update, context->pressed, context->button_pressed);
#else
	xcb_generic_error_t *error = NULL;
	xcb_query_pointer_cookie_t qpc;
	xcb_query_pointer_reply_t *reply;

	qpc = xcb_query_pointer(connection, root_window(context, connection));
	reply = xcb_query_pointer_reply(connection, qpc, &error);

	if (error) {
		blog(LOG_WARNING, "xcb_query_pointer_reply failed");
	} else {
		uint16_t buttons = reply->mask;
		ret = obs_nix_x11_input_mask_pressed(key, buttons);
	}

	free(reply);
	free(error);
#endif
	return ret;
}

static bool key_pressed(xcb_connection_t *connection, obs_hotkeys_platform_t *context, obs_key_t key)
{
	struct keycode_list *codes = &context->keycodes[key];
	xcb_generic_error_t *error = NULL;
	xcb_query_keymap_reply_t *reply;
	bool pressed = false;

	reply = xcb_query_keymap_reply(connection, xcb_query_keymap(connection), &error);
	if (error) {
		blog(LOG_WARNING, "xcb_query_keymap failed");

	} else {
		pressed = obs_nix_x11_keys_pressed(reply ? reply->keys : NULL, key, codes, &context->super_l_code,
						   &context->super_r_code);
	}

	free(reply);
	free(error);
	return pressed;
}

static bool obs_nix_x11_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	xcb_connection_t *conn = XGetXCBConnection(context->display);

	if (key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29) {
		return mouse_button_pressed(conn, context, key);
	} else {
		return key_pressed(conn, context, key);
	}
}

static bool get_key_translation(struct dstr *dstr, xcb_keycode_t keycode)
{
	xcb_connection_t *connection;
	char name[128];

	connection = XGetXCBConnection(obs->hotkeys.platform_context->display);

	XKeyEvent event = {0};
	event.type = KeyPress;
	event.display = obs->hotkeys.platform_context->display;
	event.keycode = keycode;
	event.root = root_window(obs->hotkeys.platform_context, connection);
	event.window = event.root;

	if (keycode) {
		int len = XLookupString(&event, name, 128, NULL, NULL);
		if (len) {
			dstr_ncopy(dstr, name, (size_t)len);
			dstr_to_upper(dstr);
			return true;
		}
	}

	return false;
}

static void obs_nix_x11_key_to_str(obs_key_t key, struct dstr *dstr)
{
	if (key >= OBS_KEY_MOUSE1 && key <= OBS_KEY_MOUSE29) {
		if (obs->hotkeys.translations[key]) {
			dstr_copy(dstr, obs->hotkeys.translations[key]);
		} else {
			dstr_printf(dstr, "Mouse %d", (int)(key - OBS_KEY_MOUSE1 + 1));
		}
		return;
	}

	if (key >= OBS_KEY_NUM0 && key <= OBS_KEY_NUM9) {
		if (obs->hotkeys.translations[key]) {
			dstr_copy(dstr, obs->hotkeys.translations[key]);
		} else {
			dstr_printf(dstr, "Numpad %d", (int)(key - OBS_KEY_NUM0));
		}
		return;
	}

#define translate_key(key, def) dstr_copy(dstr, obs_get_hotkey_translation(key, def))

	switch (key) {
	case OBS_KEY_INSERT:
		translate_key(key, "Insert");
		return;
	case OBS_KEY_DELETE:
		translate_key(key, "Delete");
		return;
	case OBS_KEY_HOME:
		translate_key(key, "Home");
		return;
	case OBS_KEY_END:
		translate_key(key, "End");
		return;
	case OBS_KEY_PAGEUP:
		translate_key(key, "Page Up");
		return;
	case OBS_KEY_PAGEDOWN:
		translate_key(key, "Page Down");
		return;
	case OBS_KEY_NUMLOCK:
		translate_key(key, "Num Lock");
		return;
	case OBS_KEY_SCROLLLOCK:
		translate_key(key, "Scroll Lock");
		return;
	case OBS_KEY_CAPSLOCK:
		translate_key(key, "Caps Lock");
		return;
	case OBS_KEY_BACKSPACE:
		translate_key(key, "Backspace");
		return;
	case OBS_KEY_TAB:
		translate_key(key, "Tab");
		return;
	case OBS_KEY_PRINT:
		translate_key(key, "Print");
		return;
	case OBS_KEY_PAUSE:
		translate_key(key, "Pause");
		return;
	case OBS_KEY_LEFT:
		translate_key(key, "Left");
		return;
	case OBS_KEY_RIGHT:
		translate_key(key, "Right");
		return;
	case OBS_KEY_UP:
		translate_key(key, "Up");
		return;
	case OBS_KEY_DOWN:
		translate_key(key, "Down");
		return;
	case OBS_KEY_SHIFT:
		translate_key(key, "Shift");
		return;
	case OBS_KEY_ALT:
		translate_key(key, "Alt");
		return;
	case OBS_KEY_CONTROL:
		translate_key(key, "Control");
		return;
	case OBS_KEY_META:
		translate_key(key, "Super");
		return;
	case OBS_KEY_MENU:
		translate_key(key, "Menu");
		return;
	case OBS_KEY_NUMASTERISK:
		translate_key(key, "Numpad *");
		return;
	case OBS_KEY_NUMPLUS:
		translate_key(key, "Numpad +");
		return;
	case OBS_KEY_NUMMINUS:
		translate_key(key, "Numpad -");
		return;
	case OBS_KEY_NUMCOMMA:
		translate_key(key, "Numpad ,");
		return;
	case OBS_KEY_NUMPERIOD:
		translate_key(key, "Numpad .");
		return;
	case OBS_KEY_NUMSLASH:
		translate_key(key, "Numpad /");
		return;
	case OBS_KEY_SPACE:
		translate_key(key, "Space");
		return;
	case OBS_KEY_ESCAPE:
		translate_key(key, "Escape");
		return;
	default:;
	}

	if (key >= OBS_KEY_F1 && key <= OBS_KEY_F35) {
		dstr_printf(dstr, "F%d", (int)(key - OBS_KEY_F1 + 1));
		return;
	}

	obs_hotkeys_platform_t *context = obs->hotkeys.platform_context;
	struct keycode_list *keycodes = &context->keycodes[key];

	for (size_t i = 0; i < keycodes->list.num; i++) {
		if (get_key_translation(dstr, keycodes->list.array[i])) {
			break;
		}
	}

	if (key != OBS_KEY_NONE && dstr_is_empty(dstr)) {
		dstr_copy(dstr, obs_key_to_name(key));
	}
}

static obs_key_t obs_nix_x11_key_from_virtual_key(int sym)
{
	obs_hotkeys_platform_t *context = obs->hotkeys.platform_context;

	return obs_nix_x11_keys_from_virtual_key(sym, context->keysyms, context->syms_per_code, &context->num_keysyms,
						 &context->min_keycode, context->keycodes);
}

static const uint32_t *x11_base_keysyms(void)
{
	return obs->hotkeys.platform_context->base_keysyms;
}

static int obs_nix_x11_key_to_virtual_key(obs_key_t key)
{
	return obs_nix_x11_keys_to_virtual_key(key, x11_base_keysyms);
}

static const struct obs_nix_hotkeys_vtable x11_hotkeys_vtable = {
	.init = obs_nix_x11_hotkeys_platform_init,
	.free = obs_nix_x11_hotkeys_platform_free,
	.is_pressed = obs_nix_x11_hotkeys_platform_is_pressed,
	.key_to_str = obs_nix_x11_key_to_str,
	.key_from_virtual_key = obs_nix_x11_key_from_virtual_key,
	.key_to_virtual_key = obs_nix_x11_key_to_virtual_key,
};

const struct obs_nix_hotkeys_vtable *obs_nix_x11_get_hotkeys_vtable(void)
{
	return &x11_hotkeys_vtable;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef translate_key
