/******************************************************************************
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

#include "libobs/build/libobs-wayland.pch.h"

// X11 only supports 256 scancodes, most keyboards dont have 256 keys so this should be reasonable.
#define MAX_KEYCODES 256
// X11 keymaps only have 4 shift levels, im not sure xkbcommon supports a way to shift the state into a higher level anyway.
#define MAX_SHIFT_LEVELS 4

struct platform_seat {
	struct wl_seat *seat;
	uint32_t name;
	uint32_t capabilities;
};

struct obs_hotkeys_platform {
	struct wl_display *display;
	struct wl_registry *registry;
	/* Every seat the compositor announced, with its registry name and
	 * last reported capabilities; all of them are released on shutdown. */
	DARRAY(struct platform_seat) seats;
	/* The seat the keyboard below was taken from. */
	struct wl_seat *keyboard_seat;
	struct wl_keyboard *keyboard;
	struct xkb_context *xkb_context;
	struct xkb_keymap *xkb_keymap;
	struct xkb_state *xkb_state;
	xkb_keysym_t key_to_sym[MAX_SHIFT_LEVELS][MAX_KEYCODES];
	xkb_keysym_t obs_to_key[OBS_KEY_LAST_VALUE];
	uint32_t current_layout;

	/* What the hotkey thread may treat as held: keys pressed while OBS
	 * has keyboard focus, and active GlobalShortcuts portal shortcuts. */
	struct obs_nix_hotkey_state key_state;
	/* The key each keycode was pressed as while focused, cleared with
	 * key_state (obs_nix_wayland_keys_focused()). */
	obs_key_t pressed_as[MAX_KEYCODES];
#ifdef HAVE_GLOBAL_SHORTCUTS_PORTAL
	struct obs_nix_portal_shortcuts *portal;
#endif
};

/* The keysym tables and the keymap and focused-key rules are in
 * obs-nix-wayland-keys.h, which includes no xkbcommon library header: check
 * what it assumes about them here. */
_Static_assert(sizeof(xkb_keysym_t) == sizeof(uint32_t), "xkb_keysym_t is a uint32_t");
_Static_assert(sizeof(xkb_keycode_t) == sizeof(uint32_t), "xkb_keycode_t is a uint32_t");
_Static_assert(sizeof(xkb_level_index_t) == sizeof(uint32_t), "xkb_level_index_t is a uint32_t");
_Static_assert(MAX_KEYCODES == OBS_NIX_WAYLAND_KEYS_MAX_KEYCODES, "MAX_KEYCODES");
_Static_assert(MAX_SHIFT_LEVELS == OBS_NIX_WAYLAND_KEYS_MAX_SHIFT_LEVELS, "MAX_SHIFT_LEVELS");
_Static_assert(OBS_KEY_NONE == 0, "a cleared pressed_as entry is OBS_KEY_NONE");

static obs_key_t obs_nix_wayland_key_from_virtual_key(int sym)
{
	return obs_nix_wayland_keys_from_keysym(sym);
}

static int obs_nix_wayland_key_to_virtual_key(obs_key_t key)
{
	return obs_nix_wayland_keys_to_keysym(key);
}

struct keymap_row {
	struct xkb_keymap *keymap;
	xkb_keycode_t key;
	obs_hotkeys_platform_t *plat;
};

static int keymap_row_level_syms(void *data, uint32_t level, const uint32_t **syms)
{
	struct keymap_row *row = data;
	return xkb_keymap_key_get_syms_by_level(row->keymap, row->key, row->plat->current_layout, level, syms);
}

static void load_keymap_data(struct xkb_keymap *keymap, xkb_keycode_t key, void *data)
{
	obs_hotkeys_platform_t *plat = (obs_hotkeys_platform_t *)data;
	struct keymap_row row = {keymap, key, plat};
	obs_nix_wayland_keys_row(key, keymap_row_level_syms, &row, plat->obs_to_key, plat->key_to_sym);
}

static void rebuild_keymap_data(obs_hotkeys_platform_t *plat)
{
	memset(plat->key_to_sym, 0, sizeof(xkb_keysym_t) * MAX_SHIFT_LEVELS * MAX_KEYCODES);
	memset(plat->obs_to_key, 0, sizeof(xkb_keysym_t) * OBS_KEY_LAST_VALUE);
	xkb_keymap_key_for_each(plat->xkb_keymap, load_keymap_data, plat);
}

static void platform_keyboard_keymap(void *data, struct wl_keyboard *keyboard, uint32_t format, int32_t fd,
				     uint32_t size)
{
	UNUSED_PARAMETER(keyboard);
	obs_hotkeys_platform_t *plat = (obs_hotkeys_platform_t *)data;

	/* With no_keymap the fd holds nothing to compile. Without an xkb
	 * context (no keyboard data found) nothing can be compiled. */
	if (format != WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 || !plat->xkb_context) {
		close(fd);
		return;
	}

	char *keymap_shm = mmap(NULL, size, PROT_READ, MAP_PRIVATE, fd, 0);
	if (keymap_shm == MAP_FAILED) {
		close(fd);
		return;
	}

	struct xkb_keymap *xkb_keymap = xkb_keymap_new_from_string(
		plat->xkb_context, keymap_shm, XKB_KEYMAP_FORMAT_TEXT_V1, XKB_KEYMAP_COMPILE_NO_FLAGS);
	munmap(keymap_shm, size);
	close(fd);

	/* Keep the last good keymap and state rather than dropping them for
	 * one that doesn't compile. */
	if (!xkb_keymap) {
		blog(LOG_WARNING, "[wayland] could not compile the compositor's keymap, keeping the previous one");
		return;
	}

	// cleanup old keymap and state
	xkb_keymap_unref(plat->xkb_keymap);
	xkb_state_unref(plat->xkb_state);

	plat->xkb_keymap = xkb_keymap;
	plat->xkb_state = xkb_state_new(xkb_keymap);
	rebuild_keymap_data(plat);
}

static void platform_keyboard_modifiers(void *data, struct wl_keyboard *keyboard, uint32_t serial,
					uint32_t mods_depressed, uint32_t mods_latched, uint32_t mods_locked,
					uint32_t group)
{
	UNUSED_PARAMETER(keyboard);
	UNUSED_PARAMETER(serial);
	obs_hotkeys_platform_t *plat = (obs_hotkeys_platform_t *)data;
	if (!plat->xkb_state)
		return;
	xkb_state_update_mask(plat->xkb_state, mods_depressed, mods_latched, mods_locked, 0, 0, group);

	static const struct {
		const char *name;
		uint32_t modifier;
	} modifiers[] = {
		{XKB_MOD_NAME_SHIFT, INTERACT_SHIFT_KEY},
		{XKB_MOD_NAME_CTRL, INTERACT_CONTROL_KEY},
		{XKB_MOD_NAME_ALT, INTERACT_ALT_KEY},
		{XKB_MOD_NAME_LOGO, INTERACT_COMMAND_KEY},
	};
	uint32_t held = 0;
	for (size_t i = 0; i < sizeof(modifiers) / sizeof(modifiers[0]); i++) {
		if (xkb_state_mod_name_is_active(plat->xkb_state, modifiers[i].name, XKB_STATE_MODS_DEPRESSED) > 0)
			held |= modifiers[i].modifier;
	}
	obs_nix_hotkey_state_set_modifiers(&plat->key_state, held);

	if (plat->current_layout != group) {
		plat->current_layout = group;
		rebuild_keymap_data(plat);
	}
}

/* Qt still injects hotkey presses; these events only tell the hotkey thread
 * which keys are held while OBS has keyboard focus, so it does not release
 * a binding that is still held (as it can on X11 by querying the server). */
static void set_focused_key(obs_hotkeys_platform_t *plat, uint32_t key, bool pressed)
{
	obs_key_t obs_key;
	if (obs_nix_wayland_keys_focused(key, pressed, plat->key_to_sym[0], plat->pressed_as, &obs_key))
		obs_nix_hotkey_state_set_key(&plat->key_state, obs_key, pressed);
}

static void platform_keyboard_key(void *data, struct wl_keyboard *keyboard, uint32_t serial, uint32_t time,
				  uint32_t key, uint32_t state)
{
	UNUSED_PARAMETER(keyboard);
	UNUSED_PARAMETER(serial);
	UNUSED_PARAMETER(time);
	set_focused_key(data, key, state == WL_KEYBOARD_KEY_STATE_PRESSED);
}

static void platform_keyboard_enter(void *data, struct wl_keyboard *keyboard, uint32_t serial,
				    struct wl_surface *surface, struct wl_array *keys)
{
	UNUSED_PARAMETER(keyboard);
	UNUSED_PARAMETER(serial);
	UNUSED_PARAMETER(surface);
	uint32_t *key;
	wl_array_for_each(key, keys)
	{
		set_focused_key(data, *key, true);
	}
}

static void platform_keyboard_leave(void *data, struct wl_keyboard *keyboard, uint32_t serial,
				    struct wl_surface *surface)
{
	UNUSED_PARAMETER(keyboard);
	UNUSED_PARAMETER(serial);
	UNUSED_PARAMETER(surface);
	obs_hotkeys_platform_t *plat = data;
	/* No release events follow for keys held when focus moves away. */
	obs_nix_hotkey_state_clear_keys(&plat->key_state);
	memset(plat->pressed_as, 0, sizeof(plat->pressed_as));
}

static void platform_keyboard_repeat_info(void *data, struct wl_keyboard *keyboard, int32_t rate, int32_t delay)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(keyboard);
	UNUSED_PARAMETER(rate);
	UNUSED_PARAMETER(delay);
	// Nothing to do.
}

const struct wl_keyboard_listener keyboard_listener = {
	.keymap = platform_keyboard_keymap,
	.enter = platform_keyboard_enter,
	.leave = platform_keyboard_leave,
	.key = platform_keyboard_key,
	.modifiers = platform_keyboard_modifiers,
	.repeat_info = platform_keyboard_repeat_info,
};

static struct platform_seat *find_seat(obs_hotkeys_platform_t *plat, struct wl_seat *seat)
{
	for (size_t i = 0; i < plat->seats.num; i++) {
		if (plat->seats.array[i].seat == seat)
			return &plat->seats.array[i];
	}
	return NULL;
}

static void release_keyboard(obs_hotkeys_platform_t *plat)
{
	wl_keyboard_release(plat->keyboard);
	plat->keyboard = NULL;
	plat->keyboard_seat = NULL;
	/* No leave event follows, so nothing would release held keys. */
	obs_nix_hotkey_state_clear_keys(&plat->key_state);
	memset(plat->pressed_as, 0, sizeof(plat->pressed_as));
}

/* A compositor may announce several seats. Take the keyboard from a seat
 * that reports one: asking any other seat for it is a fatal
 * missing_capability protocol error. */
static void take_keyboard(obs_hotkeys_platform_t *plat)
{
	if (plat->keyboard)
		return;
	for (size_t i = 0; i < plat->seats.num; i++) {
		struct platform_seat *seat = &plat->seats.array[i];
		if (seat->capabilities & WL_SEAT_CAPABILITY_KEYBOARD) {
			plat->keyboard = wl_seat_get_keyboard(seat->seat);
			plat->keyboard_seat = seat->seat;
			wl_keyboard_add_listener(plat->keyboard, &keyboard_listener, plat);
			return;
		}
	}
}

static void platform_seat_capabilities(void *data, struct wl_seat *seat, uint32_t capabilities)
{
	obs_hotkeys_platform_t *plat = (obs_hotkeys_platform_t *)data;
	struct platform_seat *entry = find_seat(plat, seat);
	if (!entry)
		return;

	entry->capabilities = capabilities;
	/* Only the seat the keyboard came from losing its keyboard releases
	 * it; another seat with a keyboard then takes over. */
	if (!(capabilities & WL_SEAT_CAPABILITY_KEYBOARD) && seat == plat->keyboard_seat)
		release_keyboard(plat);
	take_keyboard(plat);
}
static void platform_seat_name(void *data, struct wl_seat *seat, const char *name)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(seat);
	UNUSED_PARAMETER(name);
	// Nothing to do.
}

const struct wl_seat_listener seat_listener = {
	.capabilities = platform_seat_capabilities,
	.name = platform_seat_name,
};

static void platform_registry_handler(void *data, struct wl_registry *registry, uint32_t id, const char *interface,
				      uint32_t version)
{
	obs_hotkeys_platform_t *plat = (obs_hotkeys_platform_t *)data;

	if (strcmp(interface, wl_seat_interface.name) == 0) {
		if (version < 4) {
			blog(LOG_WARNING, "[wayland] hotkeys disabled, compositor is too old");
			return;
		}
		// Only negotiate up to version 7, the current wl_seat at time of writing.
		struct platform_seat seat = {
			.seat = wl_registry_bind(registry, id, &wl_seat_interface, version <= 7 ? version : 7),
			.name = id,
		};
		da_push_back(plat->seats, &seat);
		wl_seat_add_listener(seat.seat, &seat_listener, plat);
	}
}

static void release_seat(struct wl_seat *seat)
{
	if (wl_seat_get_version(seat) >= WL_SEAT_RELEASE_SINCE_VERSION)
		wl_seat_release(seat);
	else
		wl_seat_destroy(seat);
}

static void platform_registry_remover(void *data, struct wl_registry *registry, uint32_t id)
{
	UNUSED_PARAMETER(registry);
	obs_hotkeys_platform_t *plat = (obs_hotkeys_platform_t *)data;

	for (size_t i = 0; i < plat->seats.num; i++) {
		struct wl_seat *seat = plat->seats.array[i].seat;
		if (plat->seats.array[i].name != id)
			continue;

		/* The seat is gone (a seat removed at runtime): drop it, and
		 * its keyboard in favour of another seat's if it had ours. */
		if (seat == plat->keyboard_seat)
			release_keyboard(plat);
		release_seat(seat);
		da_erase(plat->seats, i);
		take_keyboard(plat);
		return;
	}
}

const struct wl_registry_listener registry_listener = {
	.global = platform_registry_handler,
	.global_remove = platform_registry_remover,
};

void obs_nix_wayland_log_info(void)
{
	struct wl_display *display = obs_get_nix_platform_display();
	if (display == NULL) {
		blog(LOG_INFO, "Unable to connect to Wayland server");
		return;
	}
	//TODO: query some information about the wayland server if possible
	blog(LOG_INFO, "Connected to Wayland server");
}

#ifdef HAVE_GLOBAL_SHORTCUTS_PORTAL
/* XKB keysym name for a portal preferred trigger ("F13", "a"). */
static bool wayland_key_name(obs_key_t key, struct dstr *name)
{
	xkb_keysym_t sym = (xkb_keysym_t)obs_nix_wayland_key_to_virtual_key(key);
	char buf[64];

	if (!sym)
		return false;
	sym = xkb_keysym_to_lower(sym);
	if (xkb_keysym_get_name(sym, buf, sizeof(buf)) <= 0)
		return false;
	dstr_copy(name, buf);
	return true;
}
#endif

static bool obs_nix_wayland_hotkeys_platform_init(struct obs_core_hotkeys *hotkeys)
{
	struct wl_display *display = obs_get_nix_platform_display();
	hotkeys->platform_context = bzalloc(sizeof(obs_hotkeys_platform_t));
	hotkeys->platform_context->display = display;
	hotkeys->platform_context->xkb_context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);

	struct wl_registry *registry = wl_display_get_registry(display);
	hotkeys->platform_context->registry = registry;
	wl_registry_add_listener(registry, &registry_listener, hotkeys->platform_context);
	wl_display_roundtrip(display);

#ifdef HAVE_GLOBAL_SHORTCUTS_PORTAL
	hotkeys->platform_context->portal =
		obs_nix_portal_shortcuts_start(&hotkeys->platform_context->key_state, wayland_key_name);
#endif
	return true;
}

static void obs_nix_wayland_hotkeys_platform_free(struct obs_core_hotkeys *hotkeys)
{
	obs_hotkeys_platform_t *plat = hotkeys->platform_context;
#ifdef HAVE_GLOBAL_SHORTCUTS_PORTAL
	obs_nix_portal_shortcuts_stop(plat->portal);
#endif
	/* The listeners point at plat; no events may arrive after this. */
	if (plat->keyboard)
		wl_keyboard_release(plat->keyboard);
	for (size_t i = 0; i < plat->seats.num; i++)
		release_seat(plat->seats.array[i].seat);
	da_free(plat->seats);
	if (plat->registry)
		wl_registry_destroy(plat->registry);
	obs_nix_hotkey_state_free(&plat->key_state);
	xkb_context_unref(plat->xkb_context);
	xkb_keymap_unref(plat->xkb_keymap);
	xkb_state_unref(plat->xkb_state);
	bfree(plat);
}

static bool obs_nix_wayland_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	// Wayland never delivers key events while OBS is out of focus; out of
	// focus presses come from the GlobalShortcuts portal, if there is one.
	return obs_nix_hotkey_state_is_pressed(&context->key_state, key);
}

static void obs_nix_wayland_key_to_str(obs_key_t key, struct dstr *dstr)
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

	obs_hotkeys_platform_t *plat = obs->hotkeys.platform_context;
	// Translate the obs key back down to shift level 1 and then back to obs key.
	xkb_keycode_t keycode = plat->obs_to_key[key];
	xkb_keysym_t base_sym = plat->key_to_sym[0][keycode];
	if (base_sym != 0) {
		char buf[16] = {0};
		if (xkb_keysym_to_utf8(base_sym, buf, 15)) {
			// Normally obs uses capital letters but we are shift level 1 (lower case).
			dstr_copy(dstr, buf);
		}
	}

	if (key != OBS_KEY_NONE && dstr_is_empty(dstr)) {
		dstr_copy(dstr, obs_key_to_name(key));
	}
}

static const struct obs_nix_hotkeys_vtable wayland_hotkeys_vtable = {
	.init = obs_nix_wayland_hotkeys_platform_init,
	.free = obs_nix_wayland_hotkeys_platform_free,
	.is_pressed = obs_nix_wayland_hotkeys_platform_is_pressed,
	.key_to_str = obs_nix_wayland_key_to_str,
	.key_from_virtual_key = obs_nix_wayland_key_from_virtual_key,
	.key_to_virtual_key = obs_nix_wayland_key_to_virtual_key,

};

const struct obs_nix_hotkeys_vtable *obs_nix_wayland_get_hotkeys_vtable(void)
{
	return &wayland_hotkeys_vtable;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef MAX_KEYCODES
#undef MAX_SHIFT_LEVELS
#undef translate_key
