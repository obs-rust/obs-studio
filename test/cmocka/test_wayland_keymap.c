/* A keymap the Wayland hotkey platform can't use leaves the last good one
 * in place (obs-rust/obs-studio#2975). The keymap handler dropped the old
 * keymap and state before checking the new one, so a keymap libxkbcommon
 * can't compile crashed OBS in xkb_state_new(NULL), at startup or on a
 * layout change. It also compiled the fd of a no_keymap event, and
 * compiled with no xkb context when libxkbcommon found no keyboard data.
 *
 * The real Wayland hotkey platform (obs-nix-wayland.c) is compiled into
 * this test, whose wl_keyboard.keymap handler gets the keymap through a
 * memfd as the compositor sends it. No display is needed. */

/* memfd_create */
#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <sys/mman.h>

#include "obs-nix-wayland.c.h"

struct obs_core *wayland_test_obs;

/* Internal to libobs (obs-hotkey.c); only key names use it. */
const char *obs_get_hotkey_translation(obs_key_t key, const char *def)
{
	(void)key;
	return def;
}

/* A complete keymap needs no keyboard data from the system. <AC01> is
 * keycode 38, evdev KEY_A. */
#define KEYMAP(sym)                                                      \
	"xkb_keymap {\n"                                                 \
	"  xkb_keycodes \"t\" { minimum = 8; maximum = 255; <AC01> = 38; };\n" \
	"  xkb_types \"t\" { };\n"                                       \
	"  xkb_compat \"t\" { };\n"                                      \
	"  xkb_symbols \"t\" { key <AC01> { [ " sym " ] }; };\n"         \
	"};\n"

#define AC01 38

static const char good_keymap[] = KEYMAP("a");
static const char other_keymap[] = KEYMAP("b");
static const char broken_keymap[] = "xkb_keymap { this is not a keymap";

/* Send a keymap as the compositor does: a memfd and its size,
 * including the terminating NUL. The handler owns the fd. */
static void send_keymap(obs_hotkeys_platform_t *plat, uint32_t format, const char *text)
{
	size_t size = strlen(text) + 1;
	int fd = memfd_create("keymap", MFD_CLOEXEC);
	assert_true(fd >= 0);
	assert_int_equal(write(fd, text, size), (ssize_t)size);
	keyboard_listener.keymap(plat, NULL, format, fd, (uint32_t)size);
}

static obs_hotkeys_platform_t *platform_new(void)
{
	obs_hotkeys_platform_t *plat = bzalloc(sizeof(*plat));
	plat->xkb_context = xkb_context_new(XKB_CONTEXT_NO_DEFAULT_INCLUDES | XKB_CONTEXT_NO_ENVIRONMENT_NAMES);
	assert_non_null(plat->xkb_context);
	return plat;
}

static void platform_free(obs_hotkeys_platform_t *plat)
{
	xkb_context_unref(plat->xkb_context);
	xkb_keymap_unref(plat->xkb_keymap);
	xkb_state_unref(plat->xkb_state);
	bfree(plat);
}

static void check_ac01(obs_hotkeys_platform_t *plat, xkb_keysym_t expected)
{
	xkb_keysym_t sym = plat->key_to_sym[0][AC01];
	if (sym != expected) {
		char name[64], want[64];
		xkb_keysym_get_name(sym, name, sizeof(name));
		xkb_keysym_get_name(expected, want, sizeof(want));
		fprintf(stderr, "<AC01> is %s, expected %s\n", name, want);
		fail();
	}
}

/* The handler takes a good keymap: the baseline the other cases rely on. */
static void good_keymap_loads(void **state)
{
	(void)state;
	obs_hotkeys_platform_t *plat = platform_new();
	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, good_keymap);
	assert_non_null(plat->xkb_keymap);
	assert_non_null(plat->xkb_state);
	check_ac01(plat, XKB_KEY_a);

	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, other_keymap);
	check_ac01(plat, XKB_KEY_b);
	platform_free(plat);
}

/* A layout change to a keymap that doesn't compile keeps the old one. */
static void broken_keymap_keeps_previous(void **state)
{
	(void)state;
	obs_hotkeys_platform_t *plat = platform_new();
	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, good_keymap);
	struct xkb_keymap *keymap = plat->xkb_keymap;
	struct xkb_state *xkb_state = plat->xkb_state;

	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, broken_keymap);
	assert_ptr_equal(plat->xkb_keymap, keymap);
	assert_ptr_equal(plat->xkb_state, xkb_state);
	check_ac01(plat, XKB_KEY_a);
	platform_free(plat);
}

/* A first keymap that doesn't compile leaves hotkeys without one. */
static void broken_first_keymap(void **state)
{
	(void)state;
	obs_hotkeys_platform_t *plat = platform_new();
	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, broken_keymap);
	assert_null(plat->xkb_keymap);
	assert_null(plat->xkb_state);
	check_ac01(plat, XKB_KEY_NoSymbol);

	/* The next good one is taken. */
	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, good_keymap);
	check_ac01(plat, XKB_KEY_a);
	platform_free(plat);
}

/* With no_keymap the fd holds nothing meant as a keymap, so it is not
 * compiled, whatever it contains. */
static void no_keymap_format_is_ignored(void **state)
{
	(void)state;
	obs_hotkeys_platform_t *plat = platform_new();
	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_NO_KEYMAP, broken_keymap);
	assert_null(plat->xkb_keymap);

	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, good_keymap);
	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_NO_KEYMAP, other_keymap);
	check_ac01(plat, XKB_KEY_a);
	platform_free(plat);
}

/* xkb_context_new() fails when libxkbcommon finds no keyboard data
 * (obs-rust/obs-studio#2437); no keymap can be compiled then. */
static void no_xkb_context(void **state)
{
	(void)state;
	obs_hotkeys_platform_t *plat = platform_new();
	xkb_context_unref(plat->xkb_context);
	plat->xkb_context = NULL;

	send_keymap(plat, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, good_keymap);
	assert_null(plat->xkb_keymap);
	assert_null(plat->xkb_state);
	platform_free(plat);
}

int main(void)
{
	wayland_test_obs = bzalloc(sizeof(*wayland_test_obs));

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(good_keymap_loads),   cmocka_unit_test(broken_keymap_keeps_previous),
		cmocka_unit_test(broken_first_keymap), cmocka_unit_test(no_keymap_format_is_ignored),
		cmocka_unit_test(no_xkb_context),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
