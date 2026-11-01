#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-module-load-rules.h"

/* Characterization of load_all_callback()'s skip ladder, moved from
 * libobs/obs-module.c to libobs/obs-module-load-rules-skip.c. The test
 * calls only the moved function, which is hidden in libobs, so this
 * characterizes that C source. It pins the CURRENT behaviour; cases marked
 * "characterized, not endorsed" are odd but real. */

#define SKIP(x) OBS_MODULE_LOAD_RULES_SKIP_##x

/* The undefined type, core, an unnamed one, legacy, one past it and the
 * largest word. */
static const uint32_t types[] = {
	0, OBS_MODULE_LOAD_RULES_TYPE_CORE, 2, OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN, 4, UINT32_MAX};
#define NUM_TYPES (sizeof(types) / sizeof(types[0]))

/* The reads load_all_callback() makes, in its order, each answered from
 * the bits of m (one per step, at the step's value); *asked gets one bit
 * per step reached. */
static enum obs_module_load_rules_skip walk(uint32_t type, unsigned m, unsigned *asked)
{
	enum obs_module_load_rules_skip step = SKIP(START);
	bool value = false;
	*asked = 0;
	for (int i = 0; i < 16; i++) {
		step = obs_module_load_rules_skip_step(step, type, value);
		if (step < SKIP(PLUGIN) || step > SKIP(LOADED))
			return step;
		assert_false(*asked & (1u << step));
		*asked |= 1u << step;
		value = (m >> step) & 1;
	}
	fail();
	return step;
}

static void test_constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_MODULE_LOAD_RULES_TYPE_CORE, 1);
	assert_int_equal(OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN, 3);

	assert_int_equal(SKIP(START), 0);
	assert_int_equal(SKIP(PLUGIN), 1);
	assert_int_equal(SKIP(SAFE), 2);
	assert_int_equal(SKIP(DISABLED), 3);
	assert_int_equal(SKIP(CORE_NAME), 4);
	assert_int_equal(SKIP(LOADED), 5);
	assert_int_equal(SKIP(NOT_PLUGIN), 6);
	assert_int_equal(SKIP(DISABLE_SAFE), 7);
	assert_int_equal(SKIP(DISABLE), 8);
	assert_int_equal(SKIP(LEGACY_CORE), 9);
	assert_int_equal(SKIP(LOADED_CORE), 10);
	assert_int_equal(SKIP(LOADED_LEGACY), 11);
	assert_int_equal(SKIP(LOADED_PLUGIN), 12);
	assert_int_equal(SKIP(LOAD), 13);
}

/* Each step on its own, for every type and both values. */
static void test_each_step(void **state)
{
	(void)state;
	for (size_t i = 0; i < NUM_TYPES; i++) {
		uint32_t t = types[i];
		bool core = t == OBS_MODULE_LOAD_RULES_TYPE_CORE;
		bool legacy = t == OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN;

		for (int v = 0; v < 2; v++)
			assert_int_equal(obs_module_load_rules_skip_step(SKIP(START), t, v), SKIP(PLUGIN));

		assert_int_equal(obs_module_load_rules_skip_step(SKIP(PLUGIN), t, false), SKIP(NOT_PLUGIN));
		assert_int_equal(obs_module_load_rules_skip_step(SKIP(PLUGIN), t, true), SKIP(SAFE));

		/* Characterized, not endorsed: the safe list does not look at
		 * the type, so a core module left out of a safe-mode list is
		 * disabled, although the disabled list never disables one. */
		assert_int_equal(obs_module_load_rules_skip_step(SKIP(SAFE), t, false), SKIP(DISABLE_SAFE));
		/* A core module never reads the disabled list, and is never
		 * legacy, so it goes on to the loaded check. */
		assert_int_equal(obs_module_load_rules_skip_step(SKIP(SAFE), t, true),
				 core ? SKIP(LOADED) : SKIP(DISABLED));

		assert_int_equal(obs_module_load_rules_skip_step(SKIP(DISABLED), t, true), SKIP(DISABLE));
		assert_int_equal(obs_module_load_rules_skip_step(SKIP(DISABLED), t, false),
				 legacy ? SKIP(CORE_NAME) : SKIP(LOADED));

		assert_int_equal(obs_module_load_rules_skip_step(SKIP(CORE_NAME), t, true), SKIP(LEGACY_CORE));
		assert_int_equal(obs_module_load_rules_skip_step(SKIP(CORE_NAME), t, false), SKIP(LOADED));

		assert_int_equal(obs_module_load_rules_skip_step(SKIP(LOADED), t, false), SKIP(LOAD));
		assert_int_equal(obs_module_load_rules_skip_step(SKIP(LOADED), t, true), core ? SKIP(LOADED_CORE)
											 : legacy
												 ? SKIP(LOADED_LEGACY)
												 : SKIP(LOADED_PLUGIN));

		/* A route comes back unchanged. */
		for (int r = SKIP(NOT_PLUGIN); r <= SKIP(LOAD); r++)
			for (int v = 0; v < 2; v++)
				assert_int_equal(
					obs_module_load_rules_skip_step((enum obs_module_load_rules_skip)r, t, v), r);
	}
}

/* The whole ladder: which steps each type reads, and where it ends, for
 * every answer of the five reads. */
static void test_routes(void **state)
{
	(void)state;
	const unsigned plugin = 1u << SKIP(PLUGIN), safe = 1u << SKIP(SAFE), disabled = 1u << SKIP(DISABLED),
		       core_name = 1u << SKIP(CORE_NAME), loaded = 1u << SKIP(LOADED);

	for (size_t i = 0; i < NUM_TYPES; i++) {
		uint32_t t = types[i];
		bool core = t == OBS_MODULE_LOAD_RULES_TYPE_CORE;
		bool legacy = t == OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN;

		for (unsigned m = 0; m < 64; m += 2) {
			unsigned asked;
			int got = walk(t, m, &asked);
			int want = -1;
			unsigned want_asked = plugin;

			if (!(m & plugin))
				want = SKIP(NOT_PLUGIN);
			if (want < 0) {
				want_asked |= safe;
				if (!(m & safe))
					want = SKIP(DISABLE_SAFE);
			}
			if (want < 0 && !core) {
				want_asked |= disabled;
				if (m & disabled)
					want = SKIP(DISABLE);
			}
			if (want < 0 && legacy) {
				want_asked |= core_name;
				if (m & core_name)
					want = SKIP(LEGACY_CORE);
			}
			if (want < 0) {
				want_asked |= loaded;
				if (!(m & loaded))
					want = SKIP(LOAD);
				else
					want = core     ? SKIP(LOADED_CORE)
					       : legacy ? SKIP(LOADED_LEGACY)
							: SKIP(LOADED_PLUGIN);
			}

			assert_int_equal(got, want);
			assert_int_equal(asked, want_asked);
		}
	}
}

/* Which steps each type passes through when every check lets the module
 * by: a core type never reads the disabled list, and only a legacy type
 * reads the core module list. */
static void test_steps_per_type(void **state)
{
	(void)state;
	/* A plugin on the safe list, not disabled, no core module's name and
	 * not loaded yet. */
	const unsigned load = (1u << SKIP(PLUGIN)) | (1u << SKIP(SAFE));
	unsigned asked;

	for (size_t i = 0; i < NUM_TYPES; i++) {
		uint32_t t = types[i];
		unsigned want = load | (1u << SKIP(LOADED));
		if (t != OBS_MODULE_LOAD_RULES_TYPE_CORE)
			want |= 1u << SKIP(DISABLED);
		if (t == OBS_MODULE_LOAD_RULES_TYPE_LEGACY_PLUGIN)
			want |= 1u << SKIP(CORE_NAME);
		assert_int_equal(walk(t, load, &asked), SKIP(LOAD));
		assert_int_equal(asked, want);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_constants),
		cmocka_unit_test(test_each_step),
		cmocka_unit_test(test_routes),
		cmocka_unit_test(test_steps_per_type),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
