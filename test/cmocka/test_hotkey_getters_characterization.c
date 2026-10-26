/* Characterization of the libobs/obs-hotkey.c.h getters, setters,
 * registration and enumeration of frontend hotkeys and hotkey pairs.
 *
 * Like test_hotkey_pair_steps, this test compiles the hotkey sources into
 * itself with an isolated core (obs is renamed hotkey_test_obs by CMake), so
 * no display, hotkey thread or obs_startup() is needed. It therefore
 * characterizes the C only; it cannot see a Rust swap of these functions
 * until they are reached through the real libobs.
 *
 * Cases marked "characterized, not endorsed" pin behaviour that looks odd;
 * the port keeps it, and a fix is a separate change. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs-internal.h>

struct obs_core *hotkey_test_obs;

/* The key state is never queried: no hotkey thread runs. */
bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	UNUSED_PARAMETER(key);
	return false;
}

static bool hotkey_func_called;

static void hotkey_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	UNUSED_PARAMETER(pressed);
	hotkey_func_called = true;
}

static bool pair_func(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	UNUSED_PARAMETER(pressed);
	return false;
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
	hotkey_func_called = false;
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

/* ------------------------------------------------------------------------- */
/* Helpers */

struct find {
	obs_hotkey_id id;
	obs_hotkey_t *key;
	size_t count;
};

static bool find_by_id(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct find *f = data;
	f->count++;
	if (id == f->id)
		f->key = key;
	return true;
}

static obs_hotkey_t *hotkey_for(obs_hotkey_id id)
{
	struct find f = {id, NULL, 0};
	obs_enum_hotkeys(find_by_id, &f);
	return f.key;
}

static size_t hotkey_count(void)
{
	struct find f = {OBS_INVALID_HOTKEY_ID, NULL, 0};
	obs_enum_hotkeys(find_by_id, &f);
	return f.count;
}

static bool stop_after_first(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(key);
	(*(size_t *)data)++;
	return false;
}

struct binding_seen {
	size_t count;
	obs_hotkey_id ids[8];
	obs_hotkey_t *hotkeys[8];
	obs_key_combination_t combos[8];
	size_t stop_at;
};

static bool collect_bindings(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	struct binding_seen *s = data;
	UNUSED_PARAMETER(idx);
	if (s->count < 8) {
		s->ids[s->count] = obs_hotkey_binding_get_hotkey_id(binding);
		s->hotkeys[s->count] = obs_hotkey_binding_get_hotkey(binding);
		s->combos[s->count] = obs_hotkey_binding_get_key_combination(binding);
	}
	s->count++;
	return s->count != s->stop_at;
}

/* ------------------------------------------------------------------------- */
/* Registration and getters */

static void frontend_hotkey_getters(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = obs_hotkey_register_frontend("test.a", "Test A", hotkey_func, NULL);
	obs_hotkey_id b = obs_hotkey_register_frontend("test.b", "Test B", hotkey_func, NULL);

	/* Ids are sequential from next_id, which starts at 0. */
	assert_int_equal(a, 0);
	assert_int_equal(b, 1);

	obs_hotkey_t *key = hotkey_for(a);
	assert_non_null(key);
	assert_int_equal(obs_hotkey_get_id(key), a);
	assert_string_equal(obs_hotkey_get_name(key), "test.a");
	assert_string_equal(obs_hotkey_get_description(key), "Test A");
	assert_int_equal(obs_hotkey_get_registerer_type(key), OBS_HOTKEY_REGISTERER_FRONTEND);
	assert_null(obs_hotkey_get_registerer(key));
	/* An unpaired hotkey's partner is OBS_INVALID_HOTKEY_PAIR_ID, a pair id
	 * stored in a hotkey id field; both are (size_t)-1. characterized, not
	 * endorsed */
	assert_true(obs_hotkey_get_pair_partner_id(key) == OBS_INVALID_HOTKEY_PAIR_ID);
	assert_true(obs_hotkey_get_pair_partner_id(key) == OBS_INVALID_HOTKEY_ID);
	assert_false(hotkey_func_called);

	/* The name and description are copies. */
	key = hotkey_for(b);
	assert_non_null(key);
	assert_string_equal(obs_hotkey_get_name(key), "test.b");
	assert_string_equal(obs_hotkey_get_description(key), "Test B");
}

static void frontend_hotkey_null_strings(void **state)
{
	UNUSED_PARAMETER(state);
	/* bstrdup(NULL) is NULL, so NULL name and description are stored as
	 * NULL. characterized, not endorsed */
	obs_hotkey_id id = obs_hotkey_register_frontend(NULL, NULL, hotkey_func, NULL);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);
	obs_hotkey_t *key = hotkey_for(id);
	assert_non_null(key);
	assert_null(obs_hotkey_get_name(key));
	assert_null(obs_hotkey_get_description(key));
}

static void pair_getters(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_pair_id p0 =
		obs_hotkey_pair_register_frontend("p0.a", "P0 A", "p0.b", "P0 B", pair_func, pair_func, NULL, NULL);
	obs_hotkey_pair_id p1 =
		obs_hotkey_pair_register_frontend("p1.a", "P1 A", "p1.b", "P1 B", pair_func, pair_func, NULL, NULL);
	/* Pair ids are their own sequence from 0. */
	assert_int_equal(p0, 0);
	assert_int_equal(p1, 1);

	/* Each pair registers two hotkeys with sequential ids. */
	assert_int_equal(hotkey_count(), 4);
	obs_hotkey_t *a0 = hotkey_for(0);
	obs_hotkey_t *b0 = hotkey_for(1);
	obs_hotkey_t *a1 = hotkey_for(2);
	obs_hotkey_t *b1 = hotkey_for(3);
	assert_non_null(a0);
	assert_non_null(b0);
	assert_non_null(a1);
	assert_non_null(b1);
	assert_string_equal(obs_hotkey_get_name(a0), "p0.a");
	assert_string_equal(obs_hotkey_get_description(a0), "P0 A");
	assert_string_equal(obs_hotkey_get_name(b0), "p0.b");
	assert_string_equal(obs_hotkey_get_description(b0), "P0 B");
	assert_string_equal(obs_hotkey_get_name(a1), "p1.a");
	assert_string_equal(obs_hotkey_get_name(b1), "p1.b");

	assert_int_equal(obs_hotkey_get_pair_partner_id(a0), 1);
	assert_int_equal(obs_hotkey_get_pair_partner_id(b0), 0);
	assert_int_equal(obs_hotkey_get_pair_partner_id(a1), 3);
	assert_int_equal(obs_hotkey_get_pair_partner_id(b1), 2);
	assert_int_equal(obs_hotkey_get_registerer_type(a0), OBS_HOTKEY_REGISTERER_FRONTEND);
	assert_null(obs_hotkey_get_registerer(a0));
}

/* ------------------------------------------------------------------------- */
/* Setters */

static void set_name_and_description(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id id = obs_hotkey_register_frontend("old.name", "Old", hotkey_func, NULL);
	obs_hotkey_set_name(id, "new.name");
	obs_hotkey_set_description(id, "New");
	obs_hotkey_t *key = hotkey_for(id);
	assert_string_equal(obs_hotkey_get_name(key), "new.name");
	assert_string_equal(obs_hotkey_get_description(key), "New");

	/* NULL replaces the string with NULL. characterized, not endorsed */
	obs_hotkey_set_name(id, NULL);
	obs_hotkey_set_description(id, NULL);
	assert_null(obs_hotkey_get_name(key));
	assert_null(obs_hotkey_get_description(key));

	/* An unknown id is ignored. */
	obs_hotkey_set_name(id + 100, "x");
	obs_hotkey_set_description(OBS_INVALID_HOTKEY_ID, "x");
	assert_null(obs_hotkey_get_name(key));
	assert_int_equal(hotkey_count(), 1);
}

static void pair_set_names_and_descriptions(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_pair_id p = obs_hotkey_pair_register_frontend("a", "A", "b", "B", pair_func, pair_func, NULL, NULL);
	obs_hotkey_pair_set_names(p, "a2", "b2");
	obs_hotkey_pair_set_descriptions(p, "A2", "B2");
	obs_hotkey_t *a = hotkey_for(0);
	obs_hotkey_t *b = hotkey_for(1);
	assert_string_equal(obs_hotkey_get_name(a), "a2");
	assert_string_equal(obs_hotkey_get_name(b), "b2");
	assert_string_equal(obs_hotkey_get_description(a), "A2");
	assert_string_equal(obs_hotkey_get_description(b), "B2");

	/* An unknown pair id is ignored. */
	obs_hotkey_pair_set_names(p + 1, "x", "y");
	obs_hotkey_pair_set_descriptions(OBS_INVALID_HOTKEY_PAIR_ID, "x", "y");
	assert_string_equal(obs_hotkey_get_name(a), "a2");
	assert_string_equal(obs_hotkey_get_description(b), "B2");
}

/* ------------------------------------------------------------------------- */
/* Enumeration */

static void enum_hotkeys_stops_on_false(void **state)
{
	UNUSED_PARAMETER(state);
	size_t calls = 0;
	obs_enum_hotkeys(stop_after_first, &calls);
	assert_int_equal(calls, 0);

	obs_hotkey_register_frontend("x", "X", hotkey_func, NULL);
	obs_hotkey_register_frontend("y", "Y", hotkey_func, NULL);
	obs_hotkey_register_frontend("z", "Z", hotkey_func, NULL);
	assert_int_equal(hotkey_count(), 3);
	calls = 0;
	obs_enum_hotkeys(stop_after_first, &calls);
	assert_int_equal(calls, 1);
}

static void enum_bindings_and_binding_getters(void **state)
{
	UNUSED_PARAMETER(state);
	struct binding_seen seen = {0};
	obs_enum_hotkey_bindings(collect_bindings, &seen);
	assert_int_equal(seen.count, 0);

	obs_hotkey_id a = obs_hotkey_register_frontend("a", "A", hotkey_func, NULL);
	obs_hotkey_id b = obs_hotkey_register_frontend("b", "B", hotkey_func, NULL);
	obs_key_combination_t ca[2] = {{INTERACT_SHIFT_KEY, OBS_KEY_A}, {0, OBS_KEY_B}};
	obs_key_combination_t cb[1] = {{INTERACT_CONTROL_KEY, OBS_KEY_C}};
	obs_hotkey_load_bindings(a, ca, 2);
	obs_hotkey_load_bindings(b, cb, 1);

	/* Bindings are walked in the order they were created. */
	memset(&seen, 0, sizeof(seen));
	obs_enum_hotkey_bindings(collect_bindings, &seen);
	assert_int_equal(seen.count, 3);
	assert_int_equal(seen.ids[0], a);
	assert_int_equal(seen.ids[1], a);
	assert_int_equal(seen.ids[2], b);
	assert_ptr_equal(seen.hotkeys[0], hotkey_for(a));
	assert_ptr_equal(seen.hotkeys[2], hotkey_for(b));
	assert_int_equal(seen.combos[0].modifiers, INTERACT_SHIFT_KEY);
	assert_int_equal(seen.combos[0].key, OBS_KEY_A);
	assert_int_equal(seen.combos[1].modifiers, 0);
	assert_int_equal(seen.combos[1].key, OBS_KEY_B);
	assert_int_equal(seen.combos[2].modifiers, INTERACT_CONTROL_KEY);
	assert_int_equal(seen.combos[2].key, OBS_KEY_C);

	/* A false return stops the walk. */
	memset(&seen, 0, sizeof(seen));
	seen.stop_at = 1;
	obs_enum_hotkey_bindings(collect_bindings, &seen);
	assert_int_equal(seen.count, 1);

	/* Unregistering a hotkey removes its bindings. */
	obs_hotkey_unregister(a);
	memset(&seen, 0, sizeof(seen));
	obs_enum_hotkey_bindings(collect_bindings, &seen);
	assert_int_equal(seen.count, 1);
	assert_int_equal(seen.ids[0], b);
}

/* ------------------------------------------------------------------------- */
/* Unregistration */

static void unregister_frontend(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = obs_hotkey_register_frontend("a", "A", hotkey_func, NULL);
	obs_hotkey_id b = obs_hotkey_register_frontend("b", "B", hotkey_func, NULL);
	obs_hotkey_unregister(a);
	assert_null(hotkey_for(a));
	assert_non_null(hotkey_for(b));
	assert_int_equal(hotkey_count(), 1);

	/* Unknown, already removed and out-of-range ids are ignored. */
	obs_hotkey_unregister(a);
	obs_hotkey_unregister(b + 1);
	obs_hotkey_unregister(OBS_INVALID_HOTKEY_ID);
	assert_int_equal(hotkey_count(), 1);

	/* Ids are not reused after unregistration. */
	obs_hotkey_id c = obs_hotkey_register_frontend("c", "C", hotkey_func, NULL);
	assert_int_equal(c, 2);
}

static void unregister_pair(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_pair_id p0 = obs_hotkey_pair_register_frontend("a", "A", "b", "B", pair_func, pair_func, NULL, NULL);
	obs_hotkey_pair_id p1 = obs_hotkey_pair_register_frontend("c", "C", "d", "D", pair_func, pair_func, NULL, NULL);
	obs_hotkey_pair_unregister(p0);
	assert_int_equal(hotkey_count(), 2);
	assert_null(hotkey_for(0));
	assert_null(hotkey_for(1));
	assert_non_null(hotkey_for(2));
	assert_non_null(hotkey_for(3));

	/* Unknown and out-of-range pair ids are ignored. */
	obs_hotkey_pair_unregister(p0);
	obs_hotkey_pair_unregister(p1 + 1);
	obs_hotkey_pair_unregister(OBS_INVALID_HOTKEY_PAIR_ID);
	assert_int_equal(hotkey_count(), 2);

	/* Set on a removed pair is ignored. */
	obs_hotkey_pair_set_names(p0, "x", "y");
	assert_string_equal(obs_hotkey_get_name(hotkey_for(2)), "c");

	/* Unregistering one half of a pair alone leaves the partner, whose
	 * partner id still names the removed hotkey. characterized, not
	 * endorsed */
	obs_hotkey_unregister(2);
	obs_hotkey_t *d = hotkey_for(3);
	assert_non_null(d);
	assert_int_equal(obs_hotkey_get_pair_partner_id(d), 2);
	assert_int_equal(hotkey_count(), 1);

	/* The pair still exists, so pair_unregister removes the other half. */
	obs_hotkey_pair_unregister(p1);
	assert_int_equal(hotkey_count(), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(frontend_hotkey_getters, setup, teardown),
		cmocka_unit_test_setup_teardown(frontend_hotkey_null_strings, setup, teardown),
		cmocka_unit_test_setup_teardown(pair_getters, setup, teardown),
		cmocka_unit_test_setup_teardown(set_name_and_description, setup, teardown),
		cmocka_unit_test_setup_teardown(pair_set_names_and_descriptions, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_hotkeys_stops_on_false, setup, teardown),
		cmocka_unit_test_setup_teardown(enum_bindings_and_binding_getters, setup, teardown),
		cmocka_unit_test_setup_teardown(unregister_frontend, setup, teardown),
		cmocka_unit_test_setup_teardown(unregister_pair, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
