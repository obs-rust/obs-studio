/* The hotkey binding press count (libobs/obs-hotkey-binding-press.c), one
 * press or release at a time, in every state.
 *
 * This test compiles obs-hotkey.c and obs-hotkey-binding-press.c into itself
 * with an isolated core (obs is renamed hotkey_test_obs by CMake), registers
 * hotkeys and binds them through the public API, and calls
 * obs_hotkey_binding_press() and obs_hotkey_binding_release() directly with
 * the binding's pressed flag and its hotkey's pressed count set by hand. So
 * it characterizes the C only; test_hotkey_press_state covers the real
 * libobs (and with it a Rust swap) through the public API.
 *
 * It pins what the real libobs cannot reach or cannot show:
 * - the flag and the count after every press and release, for both flags
 *   and counts from -2 to 3 (libobs itself only reaches a count from 0 to
 *   the number of bindings, and presses only a released binding);
 * - which is called, in each of the three routing states (rerouting off:
 *   the hotkey's func; rerouting on with a router: the router; rerouting on
 *   without one: nothing), and with which arguments;
 * - that the flag and the count are written before the call, and that
 *   nothing is read or written after it: a callback can change the binding
 *   or its hotkey from inside its call;
 * - that remove_bindings() (rebinding, unregistering) releases each pressed
 *   binding it removes, and only those.
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

#include <obs-internal.h>

struct obs_core *hotkey_test_obs;

/* The key state is never queried: no hotkey thread runs, and no binding is
 * polled or injected. */
bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	UNUSED_PARAMETER(key);
	return false;
}

#define MAX_CALLS 8

/* One call: from the hotkey's func or from the router, its arguments, and
 * the binding's flag and the hotkey's count as the call saw them. */
struct call {
	bool routed;
	void *data;
	obs_hotkey_id id;
	obs_hotkey_t *hotkey;
	bool pressed;
	bool flag_seen;
	int count_seen;
};

static struct call calls[MAX_CALLS];
static size_t num_calls;

/* The binding the callbacks look at, and what a callback does to it and its
 * hotkey from inside its call (a case sets these). */
static obs_hotkey_binding_t *watched;
static obs_hotkey_t *watched_hotkey;
static bool callback_changes;
static bool callback_flag;
static int callback_count;

static void record(bool routed, void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	if (num_calls < MAX_CALLS)
		calls[num_calls] = (struct call){routed,
						 data,
						 id,
						 hotkey,
						 pressed,
						 watched ? watched->pressed : false,
						 watched_hotkey ? watched_hotkey->pressed : 0};
	num_calls++;
	if (callback_changes) {
		watched->pressed = callback_flag;
		watched_hotkey->pressed = callback_count;
	}
}

static void hotkey_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	record(false, data, id, hotkey, pressed);
}

static void router(void *data, obs_hotkey_id id, bool pressed)
{
	record(true, data, id, NULL, pressed);
}

#define FUNC_DATA ((void *)(intptr_t)0x1234)
#define ROUTER_DATA ((void *)(intptr_t)0x5678)

/* The three routing states. */
enum routing {
	DIRECT,
	ROUTED,
	ROUTED_NO_ROUTER,
};

static void set_routing(enum routing routing)
{
	obs_hotkey_enable_callback_rerouting(routing != DIRECT);
	obs_hotkey_set_callback_routing_func(routing == ROUTED ? router : NULL, routing == ROUTED ? ROUTER_DATA : NULL);
}

static obs_hotkey_id hotkey_id;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	hotkey_test_obs = bzalloc(sizeof(*hotkey_test_obs));
	assert_int_equal(pthread_mutex_init_recursive(&hotkey_test_obs->hotkeys.mutex), 0);
	hotkey_test_obs->hotkeys.strict_modifiers = true;
	hotkey_test_obs->hotkeys.signals = signal_handler_create();
	static const char *signals[] = {"void hotkey_register(ptr key)", "void hotkey_unregister(ptr key)",
					"void hotkey_bindings_changed(ptr key)", NULL};
	assert_true(signal_handler_add_array(hotkey_test_obs->hotkeys.signals, signals));

	/* Another hotkey first, so the one under test does not have id 0. */
	assert_int_not_equal(obs_hotkey_register_frontend("test.other", "other", hotkey_func, NULL),
			     OBS_INVALID_HOTKEY_ID);
	hotkey_id = obs_hotkey_register_frontend("test.press", "press", hotkey_func, FUNC_DATA);
	assert_int_not_equal(hotkey_id, OBS_INVALID_HOTKEY_ID);
	assert_int_not_equal(hotkey_id, 0);

	num_calls = 0;
	watched = NULL;
	watched_hotkey = NULL;
	callback_changes = false;
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkeys_free();
	signal_handler_destroy(hotkey_test_obs->hotkeys.signals);
	pthread_mutex_destroy(&hotkey_test_obs->hotkeys.mutex);
	bfree(hotkey_test_obs);
	hotkey_test_obs = NULL;
	return 0;
}

/* Bind the hotkey under test to `count` combinations (A, B, C, ...). */
static void bind_keys(size_t count)
{
	obs_key_combination_t combos[4];
	assert_true(count <= sizeof(combos) / sizeof(combos[0]));
	for (size_t i = 0; i < count; i++)
		combos[i] = (obs_key_combination_t){0, (obs_key_t)(OBS_KEY_A + (int)i)};
	obs_hotkey_load_bindings(hotkey_id, combos, count);
	assert_int_equal(hotkey_test_obs->hotkeys.bindings.num, count);
}

static obs_hotkey_binding_t *binding_at(size_t idx)
{
	assert_true(idx < hotkey_test_obs->hotkeys.bindings.num);
	return &hotkey_test_obs->hotkeys.bindings.array[idx];
}

/* Check the one call a press or release made, in this routing state. */
static void expect_call(enum routing routing, obs_hotkey_t *hotkey, bool pressed)
{
	assert_int_equal(num_calls, routing == ROUTED_NO_ROUTER ? 0 : 1);
	if (routing == ROUTED_NO_ROUTER)
		return;
	assert_int_equal(calls[0].routed, routing == ROUTED);
	assert_ptr_equal(calls[0].data, routing == ROUTED ? ROUTER_DATA : FUNC_DATA);
	assert_true(calls[0].id == hotkey_id);
	assert_ptr_equal(calls[0].hotkey, routing == ROUTED ? NULL : hotkey);
	assert_int_equal(calls[0].pressed, pressed);
}

/* ------------------------------------------------------------------------- */

/* Every flag, count and event. `called` is whether the hotkey is called
 * (in a routing state that calls anything); flag and count are as after the
 * event. */
struct step {
	bool flag;
	int count;
	bool press;
	bool called;
	bool flag_after;
	int count_after;
};

/* clang-format off */
static const struct step steps[] = {
	/* flag count  press  called  flag' count' */

	/* A press sets the flag and counts up; it calls only from 0. The flag
	 * is not checked: a pressed binding pressed again counts again
	 * (characterized, not endorsed; libobs presses only a released
	 * binding). */
	{0, -2,  true,  false,  1, -1},
	{0, -1,  true,  false,  1,  0},
	{0,  0,  true,  true,   1,  1},
	{0,  1,  true,  false,  1,  2},
	{0,  2,  true,  false,  1,  3},
	{0,  3,  true,  false,  1,  4},
	{1, -2,  true,  false,  1, -1},
	{1, -1,  true,  false,  1,  0},
	{1,  0,  true,  true,   1,  1},
	{1,  1,  true,  false,  1,  2},
	{1,  2,  true,  false,  1,  3},
	{1,  3,  true,  false,  1,  4},

	/* A release clears the flag and counts down; it calls only to 0. The
	 * flag is not checked here either, and a count of 0 goes negative
	 * (characterized, not endorsed; libobs releases only a pressed
	 * binding). */
	{0, -2,  false, false,  0, -3},
	{0, -1,  false, false,  0, -2},
	{0,  0,  false, false,  0, -1},
	{0,  1,  false, true,   0,  0},
	{0,  2,  false, false,  0,  1},
	{0,  3,  false, false,  0,  2},
	{1, -2,  false, false,  0, -3},
	{1, -1,  false, false,  0, -2},
	{1,  0,  false, false,  0, -1},
	{1,  1,  false, true,   0,  0},
	{1,  2,  false, false,  0,  1},
	{1,  3,  false, false,  0,  2},
};
/* clang-format on */

static void every_step(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(sizeof(steps) / sizeof(steps[0]), 24);
	bind_keys(1);
	obs_hotkey_binding_t *binding = binding_at(0);
	obs_hotkey_t *hotkey = binding->hotkey;
	assert_true(hotkey->id == hotkey_id);

	for (int r = DIRECT; r <= ROUTED_NO_ROUTER; r++) {
		enum routing routing = (enum routing)r;
		set_routing(routing);
		for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++) {
			const struct step *s = &steps[i];
			binding->pressed = s->flag;
			hotkey->pressed = s->count;
			num_calls = 0;

			if (s->press)
				obs_hotkey_binding_press(binding);
			else
				obs_hotkey_binding_release(binding);

			size_t want = s->called && routing != ROUTED_NO_ROUTER ? 1 : 0;
			bool ok = num_calls == want && binding->pressed == s->flag_after &&
				  hotkey->pressed == s->count_after;
			if (!ok)
				fprintf(stderr,
					"routing %d, step %zu (flag %d count %d, %s): %zu calls, flag %d count %d; "
					"expected %zu calls, flag %d count %d\n",
					r, i, s->flag, s->count, s->press ? "press" : "release", num_calls,
					binding->pressed, hotkey->pressed, want, s->flag_after, s->count_after);
			assert_true(ok);
			if (s->called)
				expect_call(routing, hotkey, s->press);
		}
	}
}

/* The flag and the count are written before the call: the callback sees
 * them already changed. */
static void written_before_call(void **state)
{
	UNUSED_PARAMETER(state);
	bind_keys(1);
	watched = binding_at(0);
	watched_hotkey = watched->hotkey;

	for (int r = DIRECT; r <= ROUTED; r++) {
		set_routing((enum routing)r);
		num_calls = 0;
		obs_hotkey_binding_press(watched);
		assert_int_equal(num_calls, 1);
		assert_true(calls[0].flag_seen);
		assert_int_equal(calls[0].count_seen, 1);

		num_calls = 0;
		obs_hotkey_binding_release(watched);
		assert_int_equal(num_calls, 1);
		assert_false(calls[0].flag_seen);
		assert_int_equal(calls[0].count_seen, 0);
	}
}

/* Nothing is read or written after the call: what a callback leaves in the
 * binding and its hotkey stays. */
static void nothing_after_call(void **state)
{
	UNUSED_PARAMETER(state);
	bind_keys(1);
	watched = binding_at(0);
	watched_hotkey = watched->hotkey;
	callback_changes = true;

	for (int r = DIRECT; r <= ROUTED; r++) {
		set_routing((enum routing)r);
		watched->pressed = false;
		watched_hotkey->pressed = 0;
		callback_flag = false;
		callback_count = 7;
		num_calls = 0;
		obs_hotkey_binding_press(watched);
		assert_int_equal(num_calls, 1);
		assert_false(watched->pressed);
		assert_int_equal(watched_hotkey->pressed, 7);

		watched->pressed = true;
		watched_hotkey->pressed = 1;
		callback_flag = true;
		callback_count = -4;
		num_calls = 0;
		obs_hotkey_binding_release(watched);
		assert_int_equal(num_calls, 1);
		assert_true(watched->pressed);
		assert_int_equal(watched_hotkey->pressed, -4);
	}
}

/* Two bindings of one hotkey: one call on the first press and on the last
 * release, whichever binding they are. */
static void two_bindings(void **state)
{
	UNUSED_PARAMETER(state);
	bind_keys(2);
	obs_hotkey_binding_t *a = binding_at(0);
	obs_hotkey_binding_t *b = binding_at(1);
	obs_hotkey_t *hotkey = a->hotkey;
	assert_ptr_equal(b->hotkey, hotkey);

	for (int r = DIRECT; r <= ROUTED_NO_ROUTER; r++) {
		enum routing routing = (enum routing)r;
		set_routing(routing);

		num_calls = 0;
		obs_hotkey_binding_press(a);
		expect_call(routing, hotkey, true);
		num_calls = 0;
		obs_hotkey_binding_press(b);
		assert_int_equal(num_calls, 0);
		assert_int_equal(hotkey->pressed, 2);
		obs_hotkey_binding_release(a);
		assert_int_equal(num_calls, 0);
		assert_false(a->pressed);
		assert_true(b->pressed);
		assert_int_equal(hotkey->pressed, 1);
		obs_hotkey_binding_release(b);
		expect_call(routing, hotkey, false);
		assert_int_equal(hotkey->pressed, 0);
	}
}

/* Rebinding or unregistering releases each pressed binding it removes, and
 * only those: one release call for a hotkey held through two of its three
 * bindings, none for one whose bindings are all released. */
static void remove_releases_pressed(void **state)
{
	UNUSED_PARAMETER(state);
	for (int r = DIRECT; r <= ROUTED_NO_ROUTER; r++) {
		enum routing routing = (enum routing)r;
		set_routing(routing);

		bind_keys(3);
		obs_hotkey_t *hotkey = binding_at(0)->hotkey;
		obs_hotkey_binding_press(binding_at(0));
		obs_hotkey_binding_press(binding_at(2));
		assert_int_equal(hotkey->pressed, 2);
		num_calls = 0;
		bind_keys(1);
		expect_call(routing, hotkey, false);
		assert_int_equal(hotkey->pressed, 0);
		assert_false(binding_at(0)->pressed);

		num_calls = 0;
		bind_keys(2);
		assert_int_equal(num_calls, 0);
		assert_int_equal(hotkey->pressed, 0);
	}

	set_routing(DIRECT);
	obs_hotkey_binding_press(binding_at(1));
	num_calls = 0;
	obs_hotkey_unregister(hotkey_id);
	assert_int_equal(num_calls, 1);
	assert_false(calls[0].routed);
	assert_true(calls[0].id == hotkey_id);
	assert_false(calls[0].pressed);
	assert_int_equal(hotkey_test_obs->hotkeys.bindings.num, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(every_step, setup, teardown),
		cmocka_unit_test_setup_teardown(written_before_call, setup, teardown),
		cmocka_unit_test_setup_teardown(nothing_after_call, setup, teardown),
		cmocka_unit_test_setup_teardown(two_bindings, setup, teardown),
		cmocka_unit_test_setup_teardown(remove_releases_pressed, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
