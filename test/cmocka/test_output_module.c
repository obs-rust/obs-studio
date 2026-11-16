/* Public API characterization before the obs-output Rust swap (Refs #4653).
 * Modules belong to the real core; shutdown frees their registered IDs.
 * Both queries require a live core and, once any ID is registered, non-NULL
 * input (strcmp has no NULL guard). Characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static obs_module_t *disabled(enum obs_module_load_state load_state)
{
	obs_module_t *module = NULL;
	assert_true(obs_create_disabled_module(&module, "output-module-fixture.so", "", load_state));
	assert_non_null(module);
	return module;
}

static obs_module_t *enabled(void)
{
	obs_module_t *module = NULL;
	assert_int_equal(obs_open_module(&module, CORE_STUB_PATH, ""), MODULE_SUCCESS);
	assert_non_null(module);
	return module;
}

static void expect_module(const char *id, obs_module_t *module, enum obs_module_load_state load_state)
{
	struct obs_module before;
	memcpy(&before, module, sizeof(before));
	/* Borrowed identity is stable and querying does not mutate the module. */
	for (size_t i = 0; i < 3; ++i) {
		assert_ptr_equal(obs_output_get_module(id), module);
		assert_int_equal(obs_output_load_state(id), load_state);
		assert_memory_equal(module, &before, sizeof(before));
	}
}

static void empty_registry(void **state)
{
	(void)state;
	assert_int_equal(obs_output_load_state(NULL), OBS_MODULE_MISSING);
	/* NULL getter only has a defined result while no strcmp is reached. */
	assert_null(obs_output_get_module(NULL));
	assert_null(obs_output_get_module("unknown-output-module"));
	assert_int_equal(obs_output_load_state("unknown-output-module"), OBS_MODULE_MISSING);
	assert_null(obs_output_get_module(""));
	assert_int_equal(obs_output_load_state(""), OBS_MODULE_MISSING);
	assert_null(obs_output_get_module("scene"));
	assert_null(obs_output_get_module("group"));
	assert_int_equal(obs_output_load_state("scene"), OBS_MODULE_MISSING);
	assert_int_equal(obs_output_load_state("group"), OBS_MODULE_MISSING);
	assert_int_equal(obs_output_load_state("Scene"), OBS_MODULE_MISSING);
	assert_int_equal(obs_output_load_state("groups"), OBS_MODULE_MISSING);
	/* Empty nodes on either list do not read query bytes either. */
	disabled(OBS_MODULE_DISABLED);
	enabled();
	assert_null(obs_output_get_module(NULL));
	assert_int_equal(obs_output_load_state(NULL), OBS_MODULE_MISSING);
	const char non_string = 'x';
	assert_null(obs_output_get_module(&non_string));
	assert_int_equal(obs_output_load_state(&non_string), OBS_MODULE_MISSING);
}

static void disabled_states(void **state)
{
	(void)state;
	const enum obs_module_load_state values[] = {
		OBS_MODULE_INVALID,
		OBS_MODULE_ENABLED,
		OBS_MODULE_MISSING,
		OBS_MODULE_DISABLED,
		OBS_MODULE_DISABLED_SAFE,
		OBS_MODULE_FAILED_TO_OPEN,
		OBS_MODULE_FAILED_TO_INITIALIZE,
	};
	const char *ids[] = {"module-invalid",       "module-enabled",     "module-missing",    "module-disabled",
			     "module-disabled-safe", "module-failed-open", "module-failed-init"};
	obs_module_t *modules[7];
	for (size_t i = 0; i < 7; ++i) {
		assert_int_equal(values[i], i);
		modules[i] = disabled(values[i]);
		obs_module_add_output(modules[i], "unrelated-first-id");
		obs_module_add_output(modules[i], ids[i]);
		obs_module_add_output(modules[i], "unrelated-last-id");
	}
	for (size_t i = 0; i < 7; ++i)
		expect_module(ids[i], modules[i], values[i]);
	/* Restore disabled-list classification before the real shutdown path. */
	for (size_t i = 0; i < 7; ++i)
		modules[i]->load_state = OBS_MODULE_DISABLED;
	assert_null(obs_output_get_module("not-in-any-module"));
	assert_int_equal(obs_output_load_state("not-in-any-module"), OBS_MODULE_MISSING);
}

static void traversal_and_identity(void **state)
{
	(void)state;
	obs_module_t *older_disabled = disabled(OBS_MODULE_DISABLED_SAFE);
	obs_module_add_output(older_disabled, "duplicate");
	obs_module_add_output(older_disabled, "disabled-tail");
	obs_module_t *newer_disabled = disabled(OBS_MODULE_DISABLED);
	obs_module_add_output(newer_disabled, "duplicate");
	obs_module_add_output(newer_disabled, "duplicate");
	disabled(OBS_MODULE_FAILED_TO_OPEN); /* Empty head is skipped. */
	expect_module("duplicate", newer_disabled, OBS_MODULE_DISABLED);
	expect_module("disabled-tail", older_disabled, OBS_MODULE_DISABLED_SAFE);

	obs_module_t *older_enabled = enabled();
	obs_module_add_output(older_enabled, "duplicate");
	obs_module_add_output(older_enabled, "enabled-tail");
	obs_module_t *newer_enabled = enabled();
	assert_ptr_not_equal(older_enabled, newer_enabled);
	obs_module_add_output(newer_enabled, "other");
	obs_module_add_output(newer_enabled, "duplicate");
	obs_module_add_output(newer_enabled, "duplicate");
	enabled(); /* Empty head is skipped. */
	/* A newer disabled node never outranks an enabled-list match. */
	obs_module_t *latest_disabled = disabled(OBS_MODULE_FAILED_TO_INITIALIZE);
	obs_module_add_output(latest_disabled, "duplicate");
	expect_module("duplicate", newer_enabled, OBS_MODULE_ENABLED);
	expect_module("enabled-tail", older_enabled, OBS_MODULE_ENABLED);
	expect_module("disabled-tail", older_disabled, OBS_MODULE_DISABLED_SAFE);
	/* List membership does not normalize the stored state. Restore it before shutdown. */
	newer_enabled->load_state = OBS_MODULE_FAILED_TO_OPEN;
	expect_module("duplicate", newer_enabled, OBS_MODULE_FAILED_TO_OPEN);
	newer_enabled->load_state = OBS_MODULE_ENABLED;
	assert_null(obs_output_get_module("Duplicate"));
	assert_null(obs_output_get_module("duplicat"));
	assert_int_equal(obs_output_load_state("Duplicate"), OBS_MODULE_MISSING);
}

static void exact_ids_and_registration_order(void **state)
{
	(void)state;
	obs_module_t *module = disabled(OBS_MODULE_DISABLED_SAFE);
	char bytes[] = {'x', (char)0xff, 0};
	obs_module_add_output(module, "");
	obs_module_add_output(module, bytes);
	obs_module_add_output(module, "scene");
	obs_module_add_output(module, "group");
	expect_module("", module, OBS_MODULE_DISABLED_SAFE);
	expect_module(bytes, module, OBS_MODULE_DISABLED_SAFE);
	char terminated[] = {'x', (char)0xff, 0, 'z', 0};
	expect_module(terminated, module, OBS_MODULE_DISABLED_SAFE);
	/* Output IDs have no source-style built-in state override. */
	expect_module("scene", module, OBS_MODULE_DISABLED_SAFE);
	expect_module("group", module, OBS_MODULE_DISABLED_SAFE);
	assert_int_equal(module->outputs.num, 4);
	assert_string_equal(module->outputs.array[0], "");
	assert_memory_equal(module->outputs.array[1], bytes, sizeof(bytes));
	assert_string_equal(module->outputs.array[2], "scene");
	assert_string_equal(module->outputs.array[3], "group");
	/* Other registration categories cannot satisfy an output query. */
	obs_module_add_source(module, "source-only");
	obs_module_add_encoder(module, "encoder-only");
	obs_module_add_service(module, "service-only");
	const char *other_ids[] = {"source-only", "encoder-only", "service-only"};
	for (size_t i = 0; i < 3; ++i) {
		assert_null(obs_output_get_module(other_ids[i]));
		assert_int_equal(obs_output_load_state(other_ids[i]), OBS_MODULE_MISSING);
	}
	assert_int_equal(module->load_state, OBS_MODULE_DISABLED_SAFE);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(empty_registry, setup, teardown),
		cmocka_unit_test_setup_teardown(disabled_states, setup, teardown),
		cmocka_unit_test_setup_teardown(traversal_and_identity, setup, teardown),
		cmocka_unit_test_setup_teardown(exact_ids_and_registration_order, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
