/* The Wayland key table names Volume Up and Down the right way round
 * (obs-rust/obs-studio#2635): it had XF86AudioRaiseVolume as
 * OBS_KEY_VK_VOLUME_DOWN and XF86AudioLowerVolume as OBS_KEY_VK_VOLUME_UP,
 * both ways. Windows calls them VK_VOLUME_UP and VK_VOLUME_DOWN.
 *
 * The real Wayland hotkey platform (obs-nix-wayland.c) is compiled in; its
 * key table needs no display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>

#include <xkbcommon/xkbcommon.h>

#include <obs-internal.h>
#include "obs-nix-wayland.h"

struct obs_core *wayland_test_obs;

/* Internal to libobs (obs-hotkey.c); only key names use it. */
const char *obs_get_hotkey_translation(obs_key_t key, const char *def)
{
	(void)key;
	return def;
}

static void check_key(xkb_keysym_t sym, obs_key_t expected)
{
	const struct obs_nix_hotkeys_vtable *vtable = obs_nix_wayland_get_hotkeys_vtable();
	char name[64];
	xkb_keysym_get_name(sym, name, sizeof(name));

	obs_key_t key = vtable->key_from_virtual_key((int)sym);
	if (key != expected) {
		fprintf(stderr, "%s maps to %s, expected %s\n", name, obs_key_to_name(key), obs_key_to_name(expected));
		fail();
	}
	int back = vtable->key_to_virtual_key(expected);
	if (back != (int)sym) {
		char back_name[64];
		xkb_keysym_get_name((xkb_keysym_t)back, back_name, sizeof(back_name));
		fprintf(stderr, "%s maps to %s, expected %s\n", obs_key_to_name(expected), back_name, name);
		fail();
	}
}

static void volume_keys(void **state)
{
	(void)state;
	check_key(XKB_KEY_XF86AudioRaiseVolume, OBS_KEY_VK_VOLUME_UP);
	check_key(XKB_KEY_XF86AudioLowerVolume, OBS_KEY_VK_VOLUME_DOWN);
	check_key(XKB_KEY_XF86AudioMute, OBS_KEY_VK_VOLUME_MUTE);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(volume_keys),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
