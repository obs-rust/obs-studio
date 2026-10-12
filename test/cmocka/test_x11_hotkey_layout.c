/* X11 hotkeys with a keyboard layout that has "[" on a second key
 * (upstream obsproject/obs-studio#12941, reported by Blayung).
 *
 * The reporter's layout has a Latin group and a Cyrillic group. The Cyrillic
 * group includes xkb "latin", whose 8 key is [ 8, asterisk, bracketleft,
 * trademark ], so the X server's core keyboard map lists bracketleft on the
 * 8 key (keycode 17, group 2 level 3) as well as on the real [ key (keycode
 * 34, group 1 level 1). Binding a hotkey to "[" in Settings showed "8", and
 * outside OBS only the 8 key triggered it.
 *
 * The test loads that layout's core map rows for the 8, 9, [ and ] keys
 * (exactly what xkbcomp produces for the reporter's layout) into the X
 * server, starts libobs on X11, and checks what "[" and "]" map to, what the
 * Settings label shows, and which physical key fires the hotkey (XTest). The
 * original rows are restored afterwards.
 *
 * Needs an X display (xvfb-run is enough) with the XTEST extension; the cases
 * skip without them. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <X11/Xlib.h>
/* For XK_squareroot and XK_trademark. */
#define XK_TECHNICAL
#define XK_PUBLISHING
#include <X11/keysym.h>
#include <X11/extensions/XTest.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/dstr.h>
#include <util/platform.h>
#include <util/threading.h>

#define KEYCODE_8 17
#define KEYCODE_9 18
#define KEYCODE_BRACKETLEFT 34
#define KEYCODE_BRACKETRIGHT 35
#define LAYOUT_SYMS 8

/* Core map rows (G1L1, G1L2, G2L1, G2L2, G1L3, G1L4, G2L3, G2L4) of the
 * reporter's "Latin,Cyrillic" layout, as the X server reports them. */
static const struct layout_row {
	KeyCode code;
	KeySym syms[LAYOUT_SYMS];
} layout[] = {
	{KEYCODE_8,
	 {XK_8, XK_asterisk, XK_8, XK_asterisk, XK_squareroot, XK_threequarters, XK_bracketleft, XK_trademark}},
	{KEYCODE_9,
	 {XK_9, XK_parenleft, XK_9, XK_parenleft, XK_guillemotleft, XK_plusminus, XK_bracketright, XK_plusminus}},
	{KEYCODE_BRACKETLEFT,
	 {XK_bracketleft, XK_braceleft, XK_bracketleft, XK_braceleft, XK_dead_diaeresis, XK_dead_abovering,
	  XK_dead_diaeresis, XK_dead_abovering}},
	{KEYCODE_BRACKETRIGHT,
	 {XK_bracketright, XK_braceright, XK_bracketright, XK_braceright, XK_dead_tilde, XK_dead_macron, XK_dead_tilde,
	  XK_dead_macron}},
};
#define LAYOUT_ROWS (sizeof(layout) / sizeof(layout[0]))

/* The platform display handed to libobs, and a connection for changing the
 * keyboard map and faking key presses. Holding a connection open also keeps
 * the X server from resetting (and reloading its default keymap) while the
 * test runs. */
static Display *obs_display;
static Display *control;
static KeySym *saved_rows[LAYOUT_ROWS];
static int saved_syms_per_code;
static bool layout_loaded;
static bool obs_up;

struct hotkey_log {
	volatile long presses;
	volatile long pressed;
};

static void hotkey_callback(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	struct hotkey_log *log = data;
	os_atomic_set_long(&log->pressed, pressed);
	if (pressed)
		os_atomic_inc_long(&log->presses);
}

static bool load_layout(void)
{
	for (size_t i = 0; i < LAYOUT_ROWS; i++) {
		saved_rows[i] = XGetKeyboardMapping(control, layout[i].code, 1, &saved_syms_per_code);
		if (!saved_rows[i])
			return false;
	}
	for (size_t i = 0; i < LAYOUT_ROWS; i++)
		XChangeKeyboardMapping(control, layout[i].code, LAYOUT_SYMS, (KeySym *)layout[i].syms, 1);
	XSync(control, False);

	/* Check that the server took the rows as given. */
	for (size_t i = 0; i < LAYOUT_ROWS; i++) {
		int per = 0;
		KeySym *row = XGetKeyboardMapping(control, layout[i].code, 1, &per);
		bool same = row && per >= LAYOUT_SYMS && memcmp(row, layout[i].syms, sizeof(layout[i].syms)) == 0;
		if (row)
			XFree(row);
		if (!same)
			return false;
	}
	return true;
}

static void restore_layout(void)
{
	for (size_t i = 0; i < LAYOUT_ROWS; i++) {
		if (!saved_rows[i])
			continue;
		XChangeKeyboardMapping(control, layout[i].code, saved_syms_per_code, saved_rows[i], 1);
		XFree(saved_rows[i]);
		saved_rows[i] = NULL;
	}
	XSync(control, False);
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	int event_base, error_base, major, minor;

	/* The X11 hotkey platform's connection is used from the hotkey thread
	 * and, for key labels, from this one. */
	XInitThreads();
	control = XOpenDisplay(NULL);
	if (!control) {
		fprintf(stderr, "no X display: cases skip\n");
		return 0;
	}
	if (!XTestQueryExtension(control, &event_base, &error_base, &major, &minor)) {
		fprintf(stderr, "the X server has no XTEST extension: cases skip\n");
		return 0;
	}
	if (!load_layout()) {
		fprintf(stderr, "the X server did not take the test keyboard map: cases skip\n");
		restore_layout();
		return 0;
	}
	layout_loaded = true;

	obs_display = XOpenDisplay(NULL);
	if (!obs_display) {
		fprintf(stderr, "cannot open a second X connection: cases skip\n");
		return 0;
	}
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	obs_set_nix_platform_display(obs_display);
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
	if (layout_loaded)
		restore_layout();
	if (obs_display)
		XCloseDisplay(obs_display);
	if (control)
		XCloseDisplay(control);
	return 0;
}

static void check_key(KeySym sym, obs_key_t expected)
{
	obs_key_t key = obs_key_from_virtual_key((int)sym);
	if (key != expected) {
		fprintf(stderr, "%s maps to %s, expected %s\n", XKeysymToString(sym), obs_key_to_name(key),
			obs_key_to_name(expected));
		fail();
	}
}

/* "[" and "]" are on their own keys in the first group. */
static void brackets_map_to_bracket_keys(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	check_key(XK_bracketleft, OBS_KEY_BRACKETLEFT);
	check_key(XK_bracketright, OBS_KEY_BRACKETRIGHT);
}

/* Symbols that only exist on a shifted or AltGr level still map to the key
 * that has them. */
static void shifted_symbols_map_to_their_key(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	check_key(XK_8, OBS_KEY_8);
	check_key(XK_asterisk, OBS_KEY_8);
	check_key(XK_squareroot, OBS_KEY_8);
	check_key(XK_trademark, OBS_KEY_8);
	check_key(XK_guillemotleft, OBS_KEY_9);
	check_key(XK_braceleft, OBS_KEY_BRACKETLEFT);
}

/* Settings -> Hotkeys shows the key that was pressed. */
static void bracket_hotkey_label(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	struct dstr str = {0};
	obs_key_to_str(obs_key_from_virtual_key(XK_bracketleft), &str);
	bool ok = str.array && strcmp(str.array, "[") == 0;
	if (!ok)
		fprintf(stderr, "a \"[\" hotkey is shown as \"%s\"\n", str.array ? str.array : "");
	dstr_free(&str);
	if (!ok)
		fail();
}

static long presses_after_key(struct hotkey_log *log, KeyCode code)
{
	long before = os_atomic_load_long(&log->presses);

	XTestFakeKeyEvent(control, code, True, CurrentTime);
	XSync(control, False);
	/* The hotkey thread polls every 25 ms. */
	for (int i = 0; i < 40 && os_atomic_load_long(&log->presses) == before; i++)
		os_sleep_ms(10);
	XTestFakeKeyEvent(control, code, False, CurrentTime);
	XSync(control, False);
	for (int i = 0; i < 40 && os_atomic_load_long(&log->pressed); i++)
		os_sleep_ms(10);

	return os_atomic_load_long(&log->presses) - before;
}

/* A "[" hotkey fires from the [ key, not from the 8 key, while OBS is not
 * focused (libobs polls the server's key state). */
static void bracket_hotkey_fires_from_bracket_key(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	struct hotkey_log log = {0};
	obs_hotkey_id id = obs_hotkey_register_frontend("test.bracket", "bracket", hotkey_callback, &log);
	obs_key_combination_t combo = {0, obs_key_from_virtual_key(XK_bracketleft)};
	obs_hotkey_load_bindings(id, &combo, 1);

	long from_bracket = presses_after_key(&log, KEYCODE_BRACKETLEFT);
	long from_8 = presses_after_key(&log, KEYCODE_8);

	obs_hotkey_unregister(id);

	if (from_bracket != 1 || from_8 != 0) {
		fprintf(stderr, "\"[\" hotkey: [ key fired %ld time(s), 8 key fired %ld time(s); expected 1 and 0\n",
			from_bracket, from_8);
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(brackets_map_to_bracket_keys),
		cmocka_unit_test(shifted_symbols_map_to_their_key),
		cmocka_unit_test(bracket_hotkey_label),
		cmocka_unit_test(bracket_hotkey_fires_from_bracket_key),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
