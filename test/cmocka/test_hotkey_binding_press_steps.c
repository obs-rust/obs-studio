/* The hotkey binding press count (libobs/obs-hotkey-binding-press.c), one
 * event at a time, in every state.
 *
 * This test compiles obs-hotkey.c and obs-hotkey-binding-press.c into itself
 * with an isolated core (obs is renamed hotkey_test_obs by CMake), registers a
 * hotkey through the public API, and calls the two wrapper functions with the
 * binding's pressed flag and its hotkey's pressed count set by hand. So it
 * characterizes the C only; test_hotkey_press_state covers the real libobs
 * (and with it a Rust swap) through the public API.
 *
 * It pins what the real libobs cannot reach or cannot show:
 * - that obs_hotkey_binding_count_fields() points at the binding's own pressed
 *   flag and its hotkey's count (and the hotkey), the fields the unit writes
 *   through it;
 * - the flag, the count and the call after every event: 2 flag states x 6
 *   counts (-2..3) x press or release, each in the three routing states;
 * - the arguments of each call, and that the flag and the count are written
 *   before it and nothing after it;
 * - two bindings of one hotkey (the count is per hotkey, not per binding);
 * - remove_bindings() releasing only the pressed bindings it removes;
 * - that obs_hotkey_dispatch() makes the rerouting choice and nothing else.
 *
 * Cases marked "characterized, not endorsed" pin behaviour that looks odd:
 * neither function checks the flag, so a press of a pressed binding counts
 * again and a release at count 0 makes it negative. libobs presses only a
 * released binding and releases only a pressed one, so this is unreachable
 * today. The port keeps it, and a fix is a separate change. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs-internal.h>

struct obs_core *hotkey_test_obs;

/* The key state is never queried: no key is held and no hotkey thread runs. */
bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	UNUSED_PARAMETER(key);
	return false;
}

#define MAX_CALLS 8

/* One call: which call (the hotkey's own func, 1, or the router, 2), the
 * arguments, and the flag and the count the call sees. */
struct call {
	int via;
	obs_hotkey_id id;
	obs_hotkey_t *hotkey;
	bool pressed;
	bool flag_at_call;
	int count_at_call;
};

static struct call calls[MAX_CALLS];
static size_t num_calls;

/* The binding the unit writes, so a call can read its flag. Set by setup. */
static obs_hotkey_binding_t *binding_under_test;

static void record(int via, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	if (num_calls < MAX_CALLS)
		calls[num_calls] = (struct call){
			.via = via,
			.id = id,
			.hotkey = hotkey,
			.pressed = pressed,
			.flag_at_call = binding_under_test ? binding_under_test->pressed : false,
			.count_at_call = hotkey ? hotkey->pressed : 0,
		};
	num_calls++;
}

static void hotkey_callback(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(data);
	record(1, id, hotkey, pressed);
}

static void router(void *data, obs_hotkey_id id, bool pressed)
{
	UNUSED_PARAMETER(data);
	record(2, id, binding_under_test ? binding_under_test->hotkey : NULL, pressed);
}

static obs_hotkey_id hotkey_id;
static obs_hotkey_t *hotkey_under_test;

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

	hotkey_id = obs_hotkey_register_frontend("test.press", "Press", hotkey_callback, NULL);
	assert_int_not_equal(hotkey_id, OBS_INVALID_HOTKEY_ID);

	obs_key_combination_t combo = {0, OBS_KEY_A};
	obs_hotkey_load_bindings(hotkey_id, &combo, 1);
	assert_int_equal(hotkey_test_obs->hotkeys.bindings.num, 1);

	binding_under_test = &hotkey_test_obs->hotkeys.bindings.array[0];
	hotkey_under_test = binding_under_test->hotkey;
	assert_non_null(hotkey_under_test);
	assert_ptr_equal(hotkey_under_test->func, hotkey_callback);

	num_calls = 0;
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
	binding_under_test = NULL;
	hotkey_under_test = NULL;
	return 0;
}

/* ------------------------------------------------------------------------- */

enum routing {
	ROUTING_OFF,
	ROUTING_ON_WITH_ROUTER,
	ROUTING_ON_WITHOUT_ROUTER,
};

static void set_routing(enum routing routing)
{
	hotkey_test_obs->hotkeys.reroute_hotkeys = routing != ROUTING_OFF;
	hotkey_test_obs->hotkeys.router_func = routing == ROUTING_ON_WITH_ROUTER ? router : NULL;
	hotkey_test_obs->hotkeys.router_func_data = NULL;
}

/* One event of the binding, with the flag and the count set by hand. */
static void event(bool flag, int count, bool press)
{
	binding_under_test->pressed = flag;
	hotkey_under_test->pressed = count;
	if (press)
		obs_hotkey_binding_press(binding_under_test);
	else
		obs_hotkey_binding_release(binding_under_test);
}

/* Every state: `flag` is the binding's flag before the event, `count` the
 * hotkey's count before it, `press` the event. */
struct step {
	bool flag;
	int count;
	bool press;
};

/* 2 flag states x 6 counts x press or release. */
static const struct step steps[] = {
	/* clang-format off */
	{0, -2, true}, {0, -2, false},
	{0, -1, true}, {0, -1, false},
	{0, 0, true}, {0, 0, false},
	{0, 1, true}, {0, 1, false},
	{0, 2, true}, {0, 2, false},
	{0, 3, true}, {0, 3, false},
	{1, -2, true}, {1, -2, false},
	{1, -1, true}, {1, -1, false},
	{1, 0, true}, {1, 0, false},
	{1, 1, true}, {1, 1, false},
	{1, 2, true}, {1, 2, false},
	{1, 3, true}, {1, 3, false},
	/* clang-format on */
};

/* The count after the event, and whether it calls. Neither function checks the
 * flag, so the count alone decides (characterized, not endorsed). */
static void every_step(void **state)
{
	UNUSED_PARAMETER(state);
	const size_t num_steps = sizeof(steps) / sizeof(steps[0]);
	assert_int_equal(num_steps, 24);

	for (int r = 0; r < 3; r++) {
		set_routing((enum routing)r);
		for (size_t i = 0; i < num_steps; i++) {
			const struct step *s = &steps[i];
			const int count_after = s->press ? s->count + 1 : s->count - 1;
			/* The post-increment tests the count before the press; the
			 * pre-decrement tests the count after the release. */
			const bool calls_now = s->press ? s->count == 0 : s->count == 1;
			const int via = r == ROUTING_OFF ? 1 : (r == ROUTING_ON_WITH_ROUTER ? 2 : 0);
			const bool expects_call = calls_now && via != 0;

			num_calls = 0;
			event(s->flag, s->count, s->press);

			bool ok = binding_under_test->pressed == s->press &&
				  hotkey_under_test->pressed == count_after && num_calls == (expects_call ? 1u : 0u);
			if (!ok)
				fprintf(stderr,
					"routing %d step %zu (flag %d count %d %s): flag %d count %d, %zu calls; "
					"expected flag %d count %d, %d calls\n",
					r, i, s->flag, s->count, s->press ? "press" : "release",
					binding_under_test->pressed, hotkey_under_test->pressed, num_calls, s->press,
					count_after, expects_call ? 1 : 0);
			assert_true(ok);

			if (expects_call) {
				assert_int_equal(calls[0].via, via);
				assert_true(calls[0].id == hotkey_id);
				assert_ptr_equal(calls[0].hotkey, hotkey_under_test);
				assert_int_equal(calls[0].pressed, s->press);
				/* Written before the call, nothing after it. */
				assert_int_equal(calls[0].flag_at_call, s->press);
				assert_int_equal(calls[0].count_at_call, count_after);
			}
		}
	}

	/* Nothing is written over after the call: both fields already hold their
	 * new values, so a caller reading them right after still sees them. */
	set_routing(ROUTING_OFF);
	binding_under_test->pressed = false;
	hotkey_under_test->pressed = 0;
	obs_hotkey_binding_press(binding_under_test);
	assert_int_equal(num_calls, 1);
	assert_true(binding_under_test->pressed);
	assert_int_equal(hotkey_under_test->pressed, 1);
}

/* The count is per hotkey, not per binding: two bindings of one hotkey fire
 * once, and the last release is the one that calls. */
static void two_bindings(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id id = obs_hotkey_register_frontend("test.press.two", "Two", hotkey_callback, NULL);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);
	obs_key_combination_t combos[2] = {{0, OBS_KEY_B}, {0, OBS_KEY_C}};
	obs_hotkey_load_bindings(id, combos, 2);

	assert_int_equal(hotkey_test_obs->hotkeys.bindings.num, 3);
	obs_hotkey_binding_t *first = &hotkey_test_obs->hotkeys.bindings.array[1];
	obs_hotkey_binding_t *second = &hotkey_test_obs->hotkeys.bindings.array[2];
	assert_ptr_equal(first->hotkey, second->hotkey);
	obs_hotkey_t *hotkey = first->hotkey;
	assert_int_equal(hotkey->pressed, 0);

	set_routing(ROUTING_OFF);
	binding_under_test = first;
	obs_hotkey_binding_press(first);
	assert_int_equal(num_calls, 1);
	assert_int_equal(calls[0].pressed, true);
	assert_int_equal(hotkey->pressed, 1);
	assert_true(first->pressed);
	assert_false(second->pressed);

	binding_under_test = second;
	obs_hotkey_binding_press(second);
	assert_int_equal(num_calls, 1);
	assert_int_equal(hotkey->pressed, 2);
	assert_true(second->pressed);

	binding_under_test = first;
	obs_hotkey_binding_release(first);
	assert_int_equal(num_calls, 1);
	assert_false(first->pressed);
	assert_int_equal(hotkey->pressed, 1);

	binding_under_test = second;
	obs_hotkey_binding_release(second);
	assert_int_equal(num_calls, 2);
	assert_int_equal(calls[1].pressed, false);
	assert_int_equal(calls[1].count_at_call, 0);
	assert_false(second->pressed);
	assert_int_equal(hotkey->pressed, 0);
}

/* remove_bindings() releases only the pressed bindings it removes, through the
 * wrapper, so unregistering a hotkey held through two bindings calls it once. */
static void remove_bindings_releases(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id id = obs_hotkey_register_frontend("test.press.remove", "Remove", hotkey_callback, NULL);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);
	obs_key_combination_t combos[2] = {{0, OBS_KEY_D}, {0, OBS_KEY_E}};
	obs_hotkey_load_bindings(id, combos, 2);

	obs_hotkey_binding_t *first = &hotkey_test_obs->hotkeys.bindings.array[1];
	obs_hotkey_binding_t *second = &hotkey_test_obs->hotkeys.bindings.array[2];
	obs_hotkey_t *hotkey = first->hotkey;

	set_routing(ROUTING_OFF);
	binding_under_test = first;
	obs_hotkey_binding_press(first);
	binding_under_test = second;
	obs_hotkey_binding_press(second);
	assert_int_equal(num_calls, 1);
	assert_int_equal(hotkey->pressed, 2);

	/* Both bindings are pressed, so both are released: 2 -> 1 -> 0, and
	 * only the last release calls. */
	obs_hotkey_unregister(id);
	assert_int_equal(num_calls, 2);
	assert_int_equal(calls[1].pressed, false);
	assert_int_equal(calls[1].count_at_call, 0);

	/* Only this hotkey's bindings are gone; the setup hotkey's remains. */
	assert_int_equal(hotkey_test_obs->hotkeys.bindings.num, 1);
}

/* A release of a binding that is not pressed still counts down
 * (characterized, not endorsed: libobs does not reach it). */
static void release_without_press(void **state)
{
	UNUSED_PARAMETER(state);
	set_routing(ROUTING_OFF);
	num_calls = 0;
	event(false, 0, false);
	assert_int_equal(hotkey_under_test->pressed, -1);
	assert_int_equal(num_calls, 0);
	assert_false(binding_under_test->pressed);

	/* And a press of a pressed binding counts again. */
	event(true, 1, true);
	assert_int_equal(hotkey_under_test->pressed, 2);
	assert_int_equal(num_calls, 0);
}

/* The accessor points at the binding's own flag and its hotkey's count, and at
 * the hotkey; the dispatch makes the rerouting choice and nothing else. */
static void fields_and_dispatch(void **state)
{
	UNUSED_PARAMETER(state);
	struct obs_hotkey_binding_count_fields fields = obs_hotkey_binding_count_fields(binding_under_test);
	assert_ptr_equal(fields.pressed, &binding_under_test->pressed);
	assert_ptr_equal(fields.count, &hotkey_under_test->pressed);
	assert_ptr_equal(fields.hotkey, hotkey_under_test);

	/* A value written through the accessor is the one the unit writes. */
	*fields.pressed = true;
	*fields.count = 5;
	assert_true(binding_under_test->pressed);
	assert_int_equal(hotkey_under_test->pressed, 5);

	/* The dispatch: the hotkey's own func when rerouting is off, the router
	 * when it is on and set, nothing when it is on and unset. It does not
	 * touch the flag or the count. */
	num_calls = 0;
	binding_under_test->pressed = false;
	hotkey_under_test->pressed = 0;

	set_routing(ROUTING_OFF);
	obs_hotkey_dispatch(hotkey_under_test, true);
	assert_int_equal(num_calls, 1);
	assert_int_equal(calls[0].via, 1);
	assert_true(calls[0].id == hotkey_id);
	assert_ptr_equal(calls[0].hotkey, hotkey_under_test);
	assert_int_equal(calls[0].pressed, true);

	set_routing(ROUTING_ON_WITH_ROUTER);
	obs_hotkey_dispatch(hotkey_under_test, false);
	assert_int_equal(num_calls, 2);
	assert_int_equal(calls[1].via, 2);
	assert_true(calls[1].id == hotkey_id);
	assert_int_equal(calls[1].pressed, false);

	set_routing(ROUTING_ON_WITHOUT_ROUTER);
	obs_hotkey_dispatch(hotkey_under_test, true);
	assert_int_equal(num_calls, 2);

	assert_false(binding_under_test->pressed);
	assert_int_equal(hotkey_under_test->pressed, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(every_step, setup, teardown),
		cmocka_unit_test_setup_teardown(two_bindings, setup, teardown),
		cmocka_unit_test_setup_teardown(remove_bindings_releases, setup, teardown),
		cmocka_unit_test_setup_teardown(release_without_press, setup, teardown),
		cmocka_unit_test_setup_teardown(fields_and_dispatch, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
