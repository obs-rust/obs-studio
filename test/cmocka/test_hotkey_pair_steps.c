/* The hotkey pair press latch (libobs/obs-hotkey-pair.c), one event at a
 * time, in every state.
 *
 * This test compiles obs-hotkey.c and obs-hotkey-pair.c into itself with an
 * isolated core (obs is renamed hotkey_test_obs by CMake), registers a pair
 * through the public API, and calls the two half callbacks directly with the
 * pair's latch (pressed0, pressed1) set by hand. So it characterizes the C
 * only; test_hotkey_pair_latch covers the real libobs (and with it a Rust
 * swap) through the public API.
 *
 * It pins what the real libobs cannot reach or cannot show:
 * - that obs_hotkey_pair_fields() points at the pair's own pressed0 and
 *   pressed1, the latch the callbacks read and write through it, and that
 *   obs_hotkey_pair_call_func() calls the half's func with its data, the
 *   pair id and the hotkey, and returns its result;
 * - the latch itself after every event;
 * - every state, inputs and func result: 4 latch states (both latched
 *   included, which only a func that triggers its partner from inside its
 *   own call reaches), 2 halves, press or release, func true or false;
 * - that register_hotkey_pair_internal() registers the two callbacks with
 *   the pair as their data;
 * - that a half writes only its own latch, after its func returns.
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

/* The key state is never queried: no binding is loaded, and no hotkey thread
 * runs. */
bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	UNUSED_PARAMETER(key);
	return false;
}

#define MAX_CALLS 8

/* One func call: the half (from its data), and its arguments. */
struct call {
	int half;
	obs_hotkey_pair_id id;
	obs_hotkey_t *hotkey;
	bool pressed;
};

static struct call calls[MAX_CALLS];
static size_t num_calls;
static bool result;

/* Set by a case: a func that, before it returns, sets this latch of its pair
 * to true (0: pressed0, 1: pressed1; -1: none). */
static int func_sets_latch = -1;

static obs_hotkey_pair_id pair_id;
static obs_hotkey_pair_t *pair;
static obs_hotkey_t *half_hotkey[2];

static bool pair_func(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	int half = (int)(intptr_t)data;
	if (num_calls < MAX_CALLS)
		calls[num_calls] = (struct call){half, id, hotkey, pressed};
	num_calls++;
	if (func_sets_latch == 0)
		pair->pressed0 = true;
	else if (func_sets_latch == 1)
		pair->pressed1 = true;
	return result;
}

static bool find_halves(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(id);
	if (strcmp(key->name, "test.pair.first") == 0)
		half_hotkey[0] = key;
	else if (strcmp(key->name, "test.pair.second") == 0)
		half_hotkey[1] = key;
	return true;
}

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

	/* Another pair first, so the pair under test does not have id 0. */
	assert_int_not_equal(obs_hotkey_pair_register_frontend("test.other.first", "other first", "test.other.second",
							       "other second", pair_func, pair_func, NULL, NULL),
			     OBS_INVALID_HOTKEY_PAIR_ID);
	pair_id = obs_hotkey_pair_register_frontend("test.pair.first", "first", "test.pair.second", "second", pair_func,
						    pair_func, (void *)(intptr_t)0, (void *)(intptr_t)1);
	assert_int_not_equal(pair_id, OBS_INVALID_HOTKEY_PAIR_ID);
	assert_int_not_equal(pair_id, 0);

	half_hotkey[0] = half_hotkey[1] = NULL;
	obs_enum_hotkeys(find_halves, NULL);
	assert_non_null(half_hotkey[0]);
	assert_non_null(half_hotkey[1]);
	pair = half_hotkey[0]->data;
	assert_non_null(pair);

	num_calls = 0;
	result = false;
	func_sets_latch = -1;
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
	pair = NULL;
	return 0;
}

/* One event of one half, as the hotkey's callback gets it. The hotkey id it
 * passes is not the one the func gets (the func gets the pair id), so an
 * unrelated value is passed here. */
static void event(int half, bool pressed)
{
	obs_hotkey_func func = half == 0 ? obs_hotkey_pair_first_func : obs_hotkey_pair_second_func;
	func(pair, 12345, half_hotkey[half], pressed);
}

/* ------------------------------------------------------------------------- */

/* The pair registers its two halves with the two latch callbacks, the pair
 * as their data, and each other as partners. */
static void registered_callbacks(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(half_hotkey[0]->func == obs_hotkey_pair_first_func);
	assert_true(half_hotkey[1]->func == obs_hotkey_pair_second_func);
	assert_ptr_equal(half_hotkey[1]->data, pair);
	assert_true(pair->pair_id == pair_id);
	assert_true(pair->id[0] == half_hotkey[0]->id);
	assert_true(pair->id[1] == half_hotkey[1]->id);
	assert_true(half_hotkey[0]->pair_partner_id == half_hotkey[1]->id);
	assert_true(half_hotkey[1]->pair_partner_id == half_hotkey[0]->id);
	assert_false(pair->pressed0);
	assert_false(pair->pressed1);
}

/* Every state, half, event and func result. `called` is whether the half's
 * func runs; p0 and p1 are the latch after the event. */
struct step {
	bool p0, p1;
	int half;
	bool pressed;
	bool result;
	bool called;
	bool p0_after, p1_after;
};

/* clang-format off */
static const struct step steps[] = {
	/* p0 p1  half pressed result  called  p0' p1' */

	/* Neither half latched: the func runs; a true result latches a press. */
	{0, 0,  0, true,  false,  true,   0, 0},
	{0, 0,  0, true,  true,   true,   1, 0},
	{0, 0,  0, false, false,  true,   0, 0},
	{0, 0,  0, false, true,   true,   0, 0},
	{0, 0,  1, true,  false,  true,   0, 0},
	{0, 0,  1, true,  true,   true,   0, 1},
	{0, 0,  1, false, false,  true,   0, 0},
	{0, 0,  1, false, true,   true,   0, 0},

	/* The first half latched. Its release clears the latch, no call. Its
	 * press calls the func again, and the half stays latched whatever the
	 * func returns (characterized, not endorsed: a press acted on twice).
	 * Every event of the second half is dropped. */
	{1, 0,  0, true,  false,  true,   1, 0},
	{1, 0,  0, true,  true,   true,   1, 0},
	{1, 0,  0, false, false,  false,  0, 0},
	{1, 0,  0, false, true,   false,  0, 0},
	{1, 0,  1, true,  false,  false,  1, 0},
	{1, 0,  1, true,  true,   false,  1, 0},
	{1, 0,  1, false, false,  false,  1, 0},
	{1, 0,  1, false, true,   false,  1, 0},

	/* The second half latched: the mirror image. */
	{0, 1,  0, true,  false,  false,  0, 1},
	{0, 1,  0, true,  true,   false,  0, 1},
	{0, 1,  0, false, false,  false,  0, 1},
	{0, 1,  0, false, true,   false,  0, 1},
	{0, 1,  1, true,  false,  true,   0, 1},
	{0, 1,  1, true,  true,   true,   0, 1},
	{0, 1,  1, false, false,  false,  0, 0},
	{0, 1,  1, false, true,   false,  0, 0},

	/* Both latched (characterized, not endorsed): each half sees the
	 * other's latch first, so every event is dropped and the pair stays
	 * stuck. */
	{1, 1,  0, true,  false,  false,  1, 1},
	{1, 1,  0, true,  true,   false,  1, 1},
	{1, 1,  0, false, false,  false,  1, 1},
	{1, 1,  0, false, true,   false,  1, 1},
	{1, 1,  1, true,  false,  false,  1, 1},
	{1, 1,  1, true,  true,   false,  1, 1},
	{1, 1,  1, false, false,  false,  1, 1},
	{1, 1,  1, false, true,   false,  1, 1},
};
/* clang-format on */

static void every_step(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(sizeof(steps) / sizeof(steps[0]), 32);
	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++) {
		const struct step *s = &steps[i];
		pair->pressed0 = s->p0;
		pair->pressed1 = s->p1;
		result = s->result;
		num_calls = 0;

		event(s->half, s->pressed);

		bool ok = num_calls == (s->called ? 1u : 0u) && pair->pressed0 == s->p0_after &&
			  pair->pressed1 == s->p1_after;
		if (!ok)
			fprintf(stderr,
				"step %zu (p0 %d p1 %d, half %d %s, func %d): %zu calls, p0 %d p1 %d; "
				"expected %d calls, p0 %d p1 %d\n",
				i, s->p0, s->p1, s->half, s->pressed ? "press" : "release", s->result, num_calls,
				pair->pressed0, pair->pressed1, s->called, s->p0_after, s->p1_after);
		assert_true(ok);
		if (s->called) {
			assert_int_equal(calls[0].half, s->half);
			assert_true(calls[0].id == pair_id);
			assert_ptr_equal(calls[0].hotkey, half_hotkey[s->half]);
			assert_int_equal(calls[0].pressed, s->pressed);
		}
	}
}

/* A half writes only its own latch, and only after its func returned: a func
 * that latches the other half from inside its call (as a func triggering its
 * partner does) leaves both latched; one that latches its own half and
 * returns false keeps that. */
static void latch_written_after_func(void **state)
{
	UNUSED_PARAMETER(state);
	func_sets_latch = 1;
	result = true;
	event(0, true);
	assert_int_equal(num_calls, 1);
	assert_true(pair->pressed0);
	assert_true(pair->pressed1);

	pair->pressed0 = pair->pressed1 = false;
	func_sets_latch = 0;
	result = true;
	event(1, true);
	assert_true(pair->pressed0);
	assert_true(pair->pressed1);

	pair->pressed0 = pair->pressed1 = false;
	func_sets_latch = 0;
	result = false;
	event(0, true);
	assert_true(pair->pressed0);
	assert_false(pair->pressed1);

	/* A true result overwrites what the func set: a release latches
	 * false. */
	pair->pressed0 = pair->pressed1 = false;
	func_sets_latch = 0;
	result = true;
	event(0, false);
	assert_false(pair->pressed0);
	assert_false(pair->pressed1);
}

/* The accessor points at the pair's own latch, and the call wrapper calls
 * one half's func. */
static void pair_fields(void **state)
{
	UNUSED_PARAMETER(state);
	struct obs_hotkey_pair_fields fields = obs_hotkey_pair_fields(pair);
	assert_ptr_equal(fields.pressed0, &pair->pressed0);
	assert_ptr_equal(fields.pressed1, &pair->pressed1);

	/* A latch set through the accessor is the one the callbacks read. */
	*fields.pressed1 = true;
	event(0, true);
	assert_int_equal(num_calls, 0);
	event(1, false);
	assert_int_equal(num_calls, 0);
	assert_false(pair->pressed1);

	/* A latch set by a callback shows through it. */
	result = true;
	event(0, true);
	assert_int_equal(num_calls, 1);
	assert_true(*fields.pressed0);
	assert_false(*fields.pressed1);

	/* The call wrapper: each half's func, with its data, the pair id and
	 * the hotkey, and its result. It does not touch the latch. */
	pair->pressed0 = pair->pressed1 = false;
	num_calls = 0;
	for (int half = 0; half < 2; half++) {
		for (int r = 0; r < 2; r++) {
			result = r != 0;
			assert_int_equal(obs_hotkey_pair_call_func(pair, (size_t)half, half_hotkey[1 - half], r == 0),
					 r != 0);
		}
	}
	assert_int_equal(num_calls, 4);
	for (size_t i = 0; i < 4; i++) {
		assert_int_equal(calls[i].half, (int)(i / 2));
		assert_true(calls[i].id == pair_id);
		assert_ptr_equal(calls[i].hotkey, half_hotkey[1 - i / 2]);
		assert_int_equal(calls[i].pressed, i % 2 == 0);
	}
	assert_false(pair->pressed0);
	assert_false(pair->pressed1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(registered_callbacks, setup, teardown),
		cmocka_unit_test_setup_teardown(every_step, setup, teardown),
		cmocka_unit_test_setup_teardown(latch_written_after_func, setup, teardown),
		cmocka_unit_test_setup_teardown(pair_fields, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
