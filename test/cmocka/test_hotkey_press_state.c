/* Characterization of the hotkey press state machine in libobs before its
 * port to the obs-hotkey crate: when a binding is pressed and released, and
 * when its hotkey's callback runs.
 *
 * Two paths press a binding:
 * - the hotkey thread, which polls the platform key state every 25 ms and
 *   presses or releases every binding (query_hotkeys);
 * - obs_hotkey_inject_event(), which the frontend calls for key events while
 *   OBS has focus, and obs-websocket for TriggerHotkeyByKeySequence. It only
 *   ever presses; the next poll releases.
 *
 * test_hotkey_load and test_wayland_hotkeys compile the C into themselves, so
 * they cannot see a Rust swap. This test links the real libobs instead
 * (obs_startup on X11) and goes only through the public API. It holds keys
 * with XTEST, so the X11 platform's key state (the X server's keymap) drives
 * the hotkey thread. Run under xvfb-run: it gets a private X server, and no
 * key is ever held on a real display. Without a display or XTEST the cases
 * skip.
 *
 * libobs always uses strict modifiers (obs.c sets strict_modifiers and no API
 * changes it), so here a binding's modifiers must equal the held modifiers.
 * test_hotkey_press_steps covers the non-strict branch and the per-poll
 * steps, against the C compiled into it.
 *
 * Settling: settle() presses and releases a sentinel hotkey (F12, bound with
 * the modifiers held) twice and waits for each callback, then takes the
 * hotkey mutex (obs_enum_hotkey_bindings). So at least two whole polls ran
 * after the last key change, and none is running. No sleeps. While
 * background presses are off, settle_injected() injects the sentinel press
 * instead.
 *
 * Cases marked "characterized, not endorsed" pin behaviour that looks odd;
 * the port keeps it, and a fix is a separate change. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <X11/Xlib.h>
#include <X11/keysym.h>
#include <X11/extensions/XTest.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>
#include <util/threading.h>

#define WAIT_MS 5000
#define MAX_EVENTS 64

/* One callback: the hotkey's tag (or, for a routed one, -id) and pressed. */
struct event {
	long tag;
	bool pressed;
};

static Display *obs_display;
static Display *control;
static bool obs_up;

static pthread_mutex_t log_mutex;
static os_event_t *log_signal;
static struct event events[MAX_EVENTS];
static size_t num_events;
static long sentinel_presses;
static long sentinel_releases;

static obs_hotkey_id sentinel;
static uint32_t held_modifiers;
static KeySym held[8];
static size_t num_held;

/* ------------------------------------------------------------------------- */

static void log_event(long tag, bool pressed)
{
	pthread_mutex_lock(&log_mutex);
	if (num_events < MAX_EVENTS)
		events[num_events] = (struct event){tag, pressed};
	num_events++;
	pthread_mutex_unlock(&log_mutex);
	os_event_signal(log_signal);
}

static void hotkey_callback(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	log_event((long)(intptr_t)data, pressed);
}

static void sentinel_callback(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	pthread_mutex_lock(&log_mutex);
	if (pressed)
		sentinel_presses++;
	else
		sentinel_releases++;
	pthread_mutex_unlock(&log_mutex);
	os_event_signal(log_signal);
}

/* The frontend's router queues the call to the UI thread; this one logs it. */
static void router(void *data, obs_hotkey_id id, bool pressed)
{
	UNUSED_PARAMETER(data);
	if (id == sentinel)
		sentinel_callback(NULL, id, NULL, pressed);
	else
		log_event(-(long)id, pressed);
}

static long sentinel_count(bool pressed)
{
	pthread_mutex_lock(&log_mutex);
	long count = pressed ? sentinel_presses : sentinel_releases;
	pthread_mutex_unlock(&log_mutex);
	return count;
}

static void wait_sentinel(bool pressed, long count)
{
	uint64_t deadline = os_gettime_ns() + (uint64_t)WAIT_MS * 1000000;
	while (sentinel_count(pressed) < count) {
		uint64_t now = os_gettime_ns();
		if (now >= deadline) {
			fprintf(stderr, "the sentinel hotkey was not %s within %d ms\n",
				pressed ? "pressed" : "released", WAIT_MS);
			fail();
		}
		os_event_timedwait(log_signal, (unsigned long)((deadline - now) / 1000000) + 1);
	}
}

static void noop_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	UNUSED_PARAMETER(pressed);
}

static bool no_binding(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(idx);
	UNUSED_PARAMETER(binding);
	return false;
}

/* ------------------------------------------------------------------------- */

static uint32_t modifier_of(KeySym sym)
{
	switch (sym) {
	case XK_Shift_L:
		return INTERACT_SHIFT_KEY;
	case XK_Control_L:
		return INTERACT_CONTROL_KEY;
	case XK_Alt_L:
		return INTERACT_ALT_KEY;
	default:
		return 0;
	}
}

static void fake_key(KeySym sym, bool down)
{
	KeyCode code = XKeysymToKeycode(control, sym);
	assert_int_not_equal(code, 0);
	XTestFakeKeyEvent(control, code, down ? True : False, CurrentTime);
	XSync(control, False);
}

static void key_down(KeySym sym)
{
	fake_key(sym, true);
	held_modifiers |= modifier_of(sym);
	assert_true(num_held < sizeof(held) / sizeof(held[0]));
	held[num_held++] = sym;
}

static void key_up(KeySym sym)
{
	fake_key(sym, false);
	held_modifiers &= ~modifier_of(sym);
	for (size_t i = 0; i < num_held; i++) {
		if (held[i] == sym) {
			memmove(&held[i], &held[i + 1], (num_held - i - 1) * sizeof(held[0]));
			num_held--;
			break;
		}
	}
}

/* At least two whole polls of the hotkey thread ran after the last key
 * change, and none is running now. */
static void settle(void)
{
	obs_key_combination_t combo = {held_modifiers, OBS_KEY_F12};
	obs_hotkey_load_bindings(sentinel, &combo, 1);

	for (int i = 0; i < 2; i++) {
		long presses = sentinel_count(true);
		long releases = sentinel_count(false);
		fake_key(XK_F12, true);
		wait_sentinel(true, presses + 1);
		fake_key(XK_F12, false);
		wait_sentinel(false, releases + 1);
		obs_enum_hotkey_bindings(no_binding, NULL);
	}
}

/* settle() for while background presses are off, when the hotkey thread
 * presses nothing: the sentinel is pressed by injecting it (with the held
 * modifiers and F12, so only a binding with exactly those modifiers and no key
 * would be pressed too), and the next poll, which starts after the inject,
 * releases it. */
static void settle_injected(void)
{
	obs_key_combination_t combo = {held_modifiers, OBS_KEY_F12};
	obs_hotkey_load_bindings(sentinel, &combo, 1);

	for (int i = 0; i < 2; i++) {
		long releases = sentinel_count(false);
		obs_hotkey_inject_event(combo, true);
		wait_sentinel(false, releases + 1);
		obs_enum_hotkey_bindings(no_binding, NULL);
	}
}

/* The callbacks so far are exactly `expected`; the log is then cleared. */
static void expect_events(const struct event *expected, size_t count)
{
	pthread_mutex_lock(&log_mutex);
	bool same = num_events == count;
	for (size_t i = 0; same && i < count; i++)
		same = events[i].tag == expected[i].tag && events[i].pressed == expected[i].pressed;
	if (!same) {
		fprintf(stderr, "callbacks:");
		for (size_t i = 0; i < num_events && i < MAX_EVENTS; i++)
			fprintf(stderr, " %ld%s", events[i].tag, events[i].pressed ? "+" : "-");
		fprintf(stderr, "\nexpected: ");
		for (size_t i = 0; i < count; i++)
			fprintf(stderr, " %ld%s", expected[i].tag, expected[i].pressed ? "+" : "-");
		fprintf(stderr, "\n");
	}
	num_events = 0;
	pthread_mutex_unlock(&log_mutex);
	assert_true(same);
}

#define EXPECT(...)                                                                    \
	do {                                                                           \
		const struct event expected_[] = {__VA_ARGS__};                        \
		expect_events(expected_, sizeof(expected_) / sizeof(expected_[0]));    \
	} while (false)
#define EXPECT_NONE() expect_events(NULL, 0)

#define P(tag) {(tag), true}
#define R(tag) {(tag), false}

/* Hotkeys a case registered, unregistered by teardown() if the case failed
 * before it did. */
static obs_hotkey_id registered[8];
static size_t num_registered;
static obs_hotkey_pair_id registered_pair = OBS_INVALID_HOTKEY_PAIR_ID;

static obs_hotkey_id register_tagged(long tag)
{
	char name[32];
	snprintf(name, sizeof(name), "test.press.%ld", tag);
	obs_hotkey_id id = obs_hotkey_register_frontend(name, name, hotkey_callback, (void *)(intptr_t)tag);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);
	assert_true(num_registered < sizeof(registered) / sizeof(registered[0]));
	registered[num_registered++] = id;
	return id;
}

static void bind(obs_hotkey_id id, uint32_t modifiers, obs_key_t key)
{
	obs_key_combination_t combo = {modifiers, key};
	obs_hotkey_load_bindings(id, &combo, 1);
}

static void bind2(obs_hotkey_id id, obs_key_combination_t first, obs_key_combination_t second)
{
	obs_key_combination_t combos[] = {first, second};
	obs_hotkey_load_bindings(id, combos, 2);
}

static void inject(uint32_t modifiers, obs_key_t key, bool pressed)
{
	obs_key_combination_t combo = {modifiers, key};
	obs_hotkey_inject_event(combo, pressed);
}

#define CTRL INTERACT_CONTROL_KEY
#define SHIFT INTERACT_SHIFT_KEY
#define ALT INTERACT_ALT_KEY

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	int event_base, error_base, major, minor;

	pthread_mutex_init(&log_mutex, NULL);
	os_event_init(&log_signal, OS_EVENT_TYPE_AUTO);

	/* The X11 hotkey platform's connection is used from the hotkey
	 * thread. */
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

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	if (obs_display)
		XCloseDisplay(obs_display);
	if (control)
		XCloseDisplay(control);
	os_event_destroy(log_signal);
	pthread_mutex_destroy(&log_mutex);
	return 0;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;
	sentinel = obs_hotkey_register_frontend("test.press.sentinel", "sentinel", sentinel_callback, NULL);
	assert_int_not_equal(sentinel, OBS_INVALID_HOTKEY_ID);
	settle();
	EXPECT_NONE();
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;
	while (num_held)
		key_up(held[num_held - 1]);
	while (num_registered)
		obs_hotkey_unregister(registered[--num_registered]);
	if (registered_pair != OBS_INVALID_HOTKEY_PAIR_ID)
		obs_hotkey_pair_unregister(registered_pair);
	registered_pair = OBS_INVALID_HOTKEY_PAIR_ID;
	obs_hotkey_enable_background_press(true);
	obs_hotkey_enable_callback_rerouting(false);
	obs_hotkey_set_callback_routing_func(NULL, NULL);
	obs_hotkey_unregister(sentinel);
	pthread_mutex_lock(&log_mutex);
	num_events = 0;
	pthread_mutex_unlock(&log_mutex);
	return 0;
}

#define REQUIRE_OBS()          \
	do {                   \
		if (!obs_up)   \
			skip(); \
	} while (false)

/* ------------------------------------------------------------------------- */
/* The hotkey thread */

/* A key press calls the callback once with true, its release once with
 * false. */
static void key_press_and_release(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(1));

	key_up(XK_a);
	settle();
	EXPECT(R(1));
	obs_hotkey_unregister(a);
}

/* Ctrl+A: pressed while Ctrl and A are held; releasing Ctrl releases it even
 * while A is still held, and pressing Ctrl again presses it again. A alone
 * does nothing. */
static void modifier_combination(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_A);
	settle();

	key_down(XK_a);
	settle();
	EXPECT_NONE();
	key_up(XK_a);

	key_down(XK_Control_L);
	settle();
	EXPECT_NONE();
	key_down(XK_a);
	settle();
	EXPECT(P(1));

	key_up(XK_Control_L);
	settle();
	EXPECT(R(1));

	key_down(XK_Control_L);
	settle();
	EXPECT(P(1));

	key_up(XK_a);
	settle();
	EXPECT(R(1));
	key_up(XK_Control_L);
	settle();
	EXPECT_NONE();
	obs_hotkey_unregister(a);
}

/* Strict modifiers: an extra held modifier stops a binding, also a binding
 * with no modifiers. Adding a modifier to a pressed binding releases it. */
static void strict_modifiers(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	obs_hotkey_id b = register_tagged(2);
	bind(a, CTRL, OBS_KEY_A);
	bind(b, 0, OBS_KEY_B);
	settle();

	key_down(XK_Control_L);
	key_down(XK_Shift_L);
	key_down(XK_a);
	settle();
	EXPECT_NONE();
	key_up(XK_a);
	key_up(XK_Shift_L);

	key_down(XK_b);
	settle();
	EXPECT_NONE();
	key_up(XK_Control_L);
	settle();
	EXPECT(P(2));

	key_down(XK_Alt_L);
	settle();
	EXPECT(R(2));
	key_up(XK_Alt_L);
	settle();
	EXPECT(P(2));
	key_up(XK_b);
	settle();
	EXPECT(R(2));
	obs_hotkey_unregister(a);
	obs_hotkey_unregister(b);
}

/* Ctrl+A also fires when A is pressed before Ctrl. The modifiers_match latch
 * only makes the binding wait one poll after its modifiers first match.
 * Characterized, not endorsed: the latch looks meant to require the
 * modifiers first, but the order is not enforced. */
static void key_before_modifier(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_A);
	settle();

	key_down(XK_a);
	settle();
	EXPECT_NONE();
	key_down(XK_Control_L);
	settle();
	EXPECT(P(1));

	key_up(XK_Control_L);
	key_up(XK_a);
	settle();
	EXPECT(R(1));
	obs_hotkey_unregister(a);
}

/* A modifier-only binding (Ctrl, no key) is pressed while exactly its
 * modifiers are held, whatever other keys are held, and released when they
 * change. */
static void modifier_only_binding(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_NONE);
	settle();

	key_down(XK_a);
	settle();
	EXPECT_NONE();
	key_down(XK_Control_L);
	settle();
	EXPECT(P(1));

	key_up(XK_a);
	settle();
	EXPECT_NONE();

	key_down(XK_Shift_L);
	settle();
	EXPECT(R(1));
	key_up(XK_Shift_L);
	settle();
	EXPECT(P(1));

	key_up(XK_Control_L);
	settle();
	EXPECT(R(1));
	obs_hotkey_unregister(a);
}

/* Two bindings of one hotkey: the callback runs on the first press and on
 * the last release only (the hotkey's pressed count). */
static void two_bindings_one_hotkey(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind2(a, (obs_key_combination_t){0, OBS_KEY_A}, (obs_key_combination_t){0, OBS_KEY_B});
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(1));
	key_down(XK_b);
	settle();
	EXPECT_NONE();
	key_up(XK_a);
	settle();
	EXPECT_NONE();
	key_down(XK_a);
	settle();
	EXPECT_NONE();
	key_up(XK_b);
	settle();
	EXPECT_NONE();
	key_up(XK_a);
	settle();
	EXPECT(R(1));
	obs_hotkey_unregister(a);
}

/* The same combination bound twice to one hotkey: one press and one release
 * callback. */
static void duplicate_bindings(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind2(a, (obs_key_combination_t){CTRL, OBS_KEY_A}, (obs_key_combination_t){CTRL, OBS_KEY_A});
	settle();

	key_down(XK_Control_L);
	key_down(XK_a);
	settle();
	EXPECT(P(1));
	key_up(XK_a);
	settle();
	EXPECT(R(1));
	obs_hotkey_unregister(a);
}

/* Two hotkeys on one key both fire, in the order of their bindings (the
 * order they were bound), on press and on release. */
static void two_hotkeys_one_key(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	obs_hotkey_id b = register_tagged(2);
	bind(b, 0, OBS_KEY_A);
	bind(a, 0, OBS_KEY_A);
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(2), P(1));
	key_up(XK_a);
	settle();
	EXPECT(R(2), R(1));
	obs_hotkey_unregister(a);
	obs_hotkey_unregister(b);
}

/* Rebinding or unregistering a pressed hotkey releases it at once. */
static void rebind_while_pressed(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(1));
	bind(a, 0, OBS_KEY_B);
	EXPECT(R(1));
	settle();
	EXPECT_NONE();
	key_down(XK_b);
	settle();
	EXPECT(P(1));
	obs_hotkey_unregister(a);
	EXPECT(R(1));
}

/* With callback rerouting on, the router gets the press and the release,
 * and the hotkey's own callback is not called. */
static void rerouted(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	obs_hotkey_set_callback_routing_func(router, NULL);
	obs_hotkey_enable_callback_rerouting(true);
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(-(long)a));
	key_up(XK_a);
	settle();
	EXPECT(R(-(long)a));
	obs_hotkey_unregister(a);
}

static void inject_then_route(void *data)
{
	UNUSED_PARAMETER(data);
	inject(0, OBS_KEY_A, true);
	obs_hotkey_set_callback_routing_func(router, NULL);
}

/* With rerouting on and no router, a press calls nothing but still counts:
 * once a router is set, the release reaches it without a press.
 * Characterized, not endorsed. */
static void rerouted_without_router(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	obs_hotkey_set_callback_routing_func(NULL, NULL);
	obs_hotkey_enable_callback_rerouting(true);

	/* The sentinel cannot fire without a router: press by injecting,
	 * and set the router before the next poll. */
	obs_hotkey_update_atomic(inject_then_route, NULL);
	EXPECT_NONE();
	settle();
	EXPECT(R(-(long)a));
	obs_hotkey_unregister(a);
}

/* Background presses off (the frontend's "disable hotkeys when not
 * focused"): the hotkey thread presses nothing. A binding that is already
 * pressed is not released until its key is; one whose key is held when
 * presses come back on is pressed then. */
static void background_press_disabled(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	settle();

	obs_hotkey_enable_background_press(false);
	key_down(XK_a);
	settle_injected();
	EXPECT_NONE();
	key_up(XK_a);
	settle_injected();
	EXPECT_NONE();

	obs_hotkey_enable_background_press(true);
	key_down(XK_a);
	settle();
	EXPECT(P(1));
	obs_hotkey_enable_background_press(false);
	settle_injected();
	EXPECT_NONE();
	key_up(XK_a);
	settle_injected();
	EXPECT(R(1));

	key_down(XK_a);
	settle_injected();
	EXPECT_NONE();
	obs_hotkey_enable_background_press(true);
	settle();
	EXPECT(P(1));
	key_up(XK_a);
	settle();
	EXPECT(R(1));
	obs_hotkey_unregister(a);
}

/* A pair: the first hotkey's press calls func0; while it is active the
 * second hotkey calls nothing; the release of an active one is not passed
 * on. */
static bool pair_func(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	long which = (long)(intptr_t)data;
	log_event(10 + which, pressed);
	return true;
}

static bool find_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct {
		const char *name;
		obs_hotkey_id id;
	} *find = data;
	if (strcmp(obs_hotkey_get_name(key), find->name) != 0)
		return true;
	find->id = id;
	return false;
}

static obs_hotkey_id hotkey_named(const char *name)
{
	struct {
		const char *name;
		obs_hotkey_id id;
	} find = {name, OBS_INVALID_HOTKEY_ID};
	obs_enum_hotkeys(find_hotkey, &find);
	assert_int_not_equal(find.id, OBS_INVALID_HOTKEY_ID);
	return find.id;
}

static void pair(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_pair_id pair_id = obs_hotkey_pair_register_frontend("test.pair.0", "pair 0", "test.pair.1", "pair 1",
								       pair_func, pair_func, (void *)0, (void *)1);
	assert_int_not_equal(pair_id, OBS_INVALID_HOTKEY_PAIR_ID);
	registered_pair = pair_id;
	bind(hotkey_named("test.pair.0"), 0, OBS_KEY_A);
	bind(hotkey_named("test.pair.1"), 0, OBS_KEY_B);
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(10));
	key_down(XK_b);
	settle();
	EXPECT_NONE();
	key_up(XK_b);
	settle();
	EXPECT_NONE();
	key_up(XK_a);
	settle();
	EXPECT_NONE();

	key_down(XK_b);
	settle();
	EXPECT(P(11));
	key_up(XK_b);
	settle();
	EXPECT_NONE();
	obs_hotkey_pair_unregister(pair_id);
}

/* ------------------------------------------------------------------------- */
/* obs_hotkey_inject_event */

/* An injected press presses at once (on the calling thread), and once only.
 * The next poll releases it, as the key is not held. */
static void inject_twice(void *data)
{
	UNUSED_PARAMETER(data);
	inject(CTRL, OBS_KEY_A, true);
	inject(CTRL, OBS_KEY_A, true);
	inject(0, OBS_KEY_MOUSE3, true);
}

static void inject_press_then_poll_release(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	obs_hotkey_id m = register_tagged(2);
	bind(a, CTRL, OBS_KEY_A);
	bind(m, 0, OBS_KEY_MOUSE3);
	settle();

	obs_hotkey_update_atomic(inject_twice, NULL);
	settle();
	EXPECT(P(1), P(2), R(1), R(2));
	obs_hotkey_unregister(a);
	obs_hotkey_unregister(m);
}

/* An injected release does nothing: only the poll releases. obs-websocket
 * injects false, true, false; that is one press, released by the next
 * poll. */
static void inject_release_ignored(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	settle();

	key_down(XK_a);
	settle();
	EXPECT(P(1));
	inject(0, OBS_KEY_A, false);
	settle();
	EXPECT_NONE();
	key_up(XK_a);
	settle();
	EXPECT(R(1));

	inject(0, OBS_KEY_A, false);
	inject(0, OBS_KEY_A, true);
	inject(0, OBS_KEY_A, false);
	settle();
	EXPECT(P(1), R(1));
	obs_hotkey_unregister(a);
}

/* Injected events use strict modifiers too, and press whatever the
 * background press setting is. */
static void inject_strict_and_background(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	obs_hotkey_id b = register_tagged(2);
	bind(a, 0, OBS_KEY_A);
	bind(b, CTRL, OBS_KEY_B);
	settle();

	inject(CTRL, OBS_KEY_A, true);
	inject(0, OBS_KEY_B, true);
	inject(CTRL | SHIFT, OBS_KEY_B, true);
	inject(CTRL, OBS_KEY_C, true);
	settle();
	EXPECT_NONE();

	obs_hotkey_enable_background_press(false);
	inject(0, OBS_KEY_A, true);
	settle_injected();
	EXPECT(P(1), R(1));
	obs_hotkey_unregister(a);
	obs_hotkey_unregister(b);
}

/* A modifier-only binding is pressed by any injected event with its
 * modifiers, whatever the key, and by a release as well as a press.
 * Characterized, not endorsed. */
static void inject_modifier_only(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_NONE);
	settle();

	inject(0, OBS_KEY_B, true);
	inject(SHIFT, OBS_KEY_NONE, true);
	settle();
	EXPECT_NONE();

	inject(CTRL, OBS_KEY_B, false);
	settle();
	EXPECT(P(1), R(1));
	obs_hotkey_unregister(a);
}

/* An empty binding (no modifiers, no key; obs_hotkey_load_bindings accepts
 * it, a saved profile cannot hold it) is never pressed by the hotkey thread,
 * but any injected event without modifiers presses it, until the next poll.
 * Characterized, not endorsed. */
static void inject_empty_binding(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_NONE);
	settle();

	key_down(XK_a);
	settle();
	EXPECT_NONE();
	key_up(XK_a);

	inject(0, OBS_KEY_Q, false);
	settle();
	EXPECT(P(1), R(1));
	inject(CTRL, OBS_KEY_Q, true);
	settle();
	EXPECT_NONE();
	obs_hotkey_unregister(a);
}

/* ------------------------------------------------------------------------- */

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(key_press_and_release, setup, teardown),
		cmocka_unit_test_setup_teardown(modifier_combination, setup, teardown),
		cmocka_unit_test_setup_teardown(strict_modifiers, setup, teardown),
		cmocka_unit_test_setup_teardown(key_before_modifier, setup, teardown),
		cmocka_unit_test_setup_teardown(modifier_only_binding, setup, teardown),
		cmocka_unit_test_setup_teardown(two_bindings_one_hotkey, setup, teardown),
		cmocka_unit_test_setup_teardown(duplicate_bindings, setup, teardown),
		cmocka_unit_test_setup_teardown(two_hotkeys_one_key, setup, teardown),
		cmocka_unit_test_setup_teardown(rebind_while_pressed, setup, teardown),
		cmocka_unit_test_setup_teardown(rerouted, setup, teardown),
		cmocka_unit_test_setup_teardown(rerouted_without_router, setup, teardown),
		cmocka_unit_test_setup_teardown(background_press_disabled, setup, teardown),
		cmocka_unit_test_setup_teardown(pair, setup, teardown),
		cmocka_unit_test_setup_teardown(inject_press_then_poll_release, setup, teardown),
		cmocka_unit_test_setup_teardown(inject_release_ignored, setup, teardown),
		cmocka_unit_test_setup_teardown(inject_strict_and_background, setup, teardown),
		cmocka_unit_test_setup_teardown(inject_modifier_only, setup, teardown),
		cmocka_unit_test_setup_teardown(inject_empty_binding, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
