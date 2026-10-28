/* The X11 key table names Volume Up and Down the right way round
 * (obs-rust/obs-studio#2635): it had OBS_KEY_VK_VOLUME_UP as
 * XF86AudioLowerVolume and OBS_KEY_VK_VOLUME_DOWN as XF86AudioRaiseVolume,
 * so pressing Volume Up in Settings -> Hotkeys bound "Volume Down". Windows
 * calls them VK_VOLUME_UP and VK_VOLUME_DOWN.
 *
 * Starts libobs on X11. Needs an X display (xvfb-run is enough); the cases
 * skip without one. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <X11/Xlib.h>
#include <X11/XF86keysym.h>

#include <obs.h>
#include <obs-nix-platform.h>

static Display *display;
static bool obs_up;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	display = XOpenDisplay(NULL);
	if (!display) {
		fprintf(stderr, "no X display: cases skip\n");
		return 0;
	}
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	obs_set_nix_platform_display(display);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed: cases skip\n");
		return 0;
	}
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	if (display)
		XCloseDisplay(display);
	return 0;
}

static void check_key(KeySym sym, obs_key_t expected)
{
	int back = obs_key_to_virtual_key(expected);
	if (back != (int)sym) {
		fprintf(stderr, "%s maps to %s, expected %s\n", obs_key_to_name(expected),
			XKeysymToString((KeySym)back), XKeysymToString(sym));
		fail();
	}

	/* The key the X server has the symbol on, as Settings reads it. */
	if (!XKeysymToKeycode(display, sym)) {
		fprintf(stderr, "the X keymap has no %s key: only its name is checked\n", XKeysymToString(sym));
		return;
	}
	obs_key_t key = obs_key_from_virtual_key((int)sym);
	if (key != expected) {
		fprintf(stderr, "%s maps to %s, expected %s\n", XKeysymToString(sym), obs_key_to_name(key),
			obs_key_to_name(expected));
		fail();
	}
}

static void volume_keys(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	check_key(XF86XK_AudioRaiseVolume, OBS_KEY_VK_VOLUME_UP);
	check_key(XF86XK_AudioLowerVolume, OBS_KEY_VK_VOLUME_DOWN);
	check_key(XF86XK_AudioMute, OBS_KEY_VK_VOLUME_MUTE);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(volume_keys),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
