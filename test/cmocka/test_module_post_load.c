/* Characterization before the Rust swap (Refs #4673).
 * Call the actual shared-library entry point over stack-owned fake records.
 * No module is opened, no core is started, and no GPU is used. The fixture
 * restores the hidden core pointer in teardown, including assertion failures.
 * Calling with no core is outside the existing C domain (NULL dereference).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "module_post_load_fixture.h"

static struct obs_core core;
static struct obs_core *previous_core;
static struct obs_module modules[6];
static unsigned visits[16];
static size_t visit_count;

static void record(unsigned id)
{
	assert_true(visit_count < sizeof(visits) / sizeof(visits[0]));
	visits[visit_count++] = id;
}

static void first(void)
{
	record(1);
	/* Side effects must be visible to the following callback. */
	modules[0].loaded = false;
}

static void second(void)
{
	assert_false(modules[0].loaded);
	record(2);
}

static void third(void)
{
	record(3);
}

static void excluded(void)
{
	fail();
}

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	memset(modules, 0, sizeof(modules));
	memset(visits, 0, sizeof(visits));
	visit_count = 0;
	previous_core = obs_test_module_post_load_swap_core(&core);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_test_module_post_load_swap_core(previous_core);
	return 0;
}

static void empty_and_excluded(void **state)
{
	(void)state;
	/* Both an unloaded/detached record and an entire disabled list have
	 * callbacks which must never be reached, even with an empty loaded list. */
	modules[0].post_load = excluded;
	modules[0].loaded = false;
	modules[1].post_load = excluded;
	modules[1].load_state = OBS_MODULE_DISABLED;
	modules[1].next = &modules[2];
	modules[2].post_load = excluded;
	modules[2].load_state = OBS_MODULE_FAILED_TO_OPEN;
	core.first_disabled_module = &modules[1];
	struct obs_module before[6];
	memcpy(before, modules, sizeof(before));
	for (size_t i = 0; i < 2; ++i)
		obs_post_load_modules();
	assert_int_equal(visit_count, 0);
	assert_null(core.first_module);
	assert_ptr_equal(core.first_disabled_module, &modules[1]);
	assert_memory_equal(modules, before, sizeof(before));
}

static void null_callbacks(void **state)
{
	(void)state;
	core.first_module = &modules[0];
	modules[0].next = &modules[1];
	modules[1].next = &modules[2];
	struct obs_module before[6];
	memcpy(before, modules, sizeof(before));
	obs_post_load_modules();
	assert_int_equal(visit_count, 0);
	assert_memory_equal(modules, before, sizeof(before));
	assert_ptr_equal(core.first_module, &modules[0]);
}

static void traversal_order_and_repeat(void **state)
{
	(void)state;
	/* Deliberately use non-array order, with NULL callbacks at the head,
	 * middle and tail. All three real callbacks run on each invocation. */
	core.first_module = &modules[5];
	modules[5].next = &modules[2];
	modules[2].post_load = third;
	modules[2].next = &modules[3];
	modules[3].next = &modules[0];
	modules[0].post_load = first;
	modules[0].loaded = true;
	modules[0].next = &modules[1];
	modules[1].post_load = second;
	modules[1].next = &modules[4];
	for (size_t i = 0; i < 2; ++i) {
		obs_post_load_modules();
		assert_int_equal(visit_count, (i + 1) * 3);
		assert_int_equal(visits[i * 3], 3);
		assert_int_equal(visits[i * 3 + 1], 1);
		assert_int_equal(visits[i * 3 + 2], 2);
	}
	assert_ptr_equal(core.first_module, &modules[5]);
	assert_ptr_equal(modules[5].next, &modules[2]);
	assert_ptr_equal(modules[2].next, &modules[3]);
	assert_ptr_equal(modules[3].next, &modules[0]);
	assert_ptr_equal(modules[0].next, &modules[1]);
	assert_ptr_equal(modules[1].next, &modules[4]);
	assert_null(modules[4].next);
}

static void list_membership_controls_dispatch(void **state)
{
	(void)state;
	/* Characterized, not endorsed: first_module is the dispatch list.
	 * Neither loaded=false nor a disabled stored state filters its members.
	 * Conversely, a record on the disabled list is never called even if its
	 * loaded flag and stored state say enabled. */
	core.first_module = &modules[2];
	modules[2].post_load = third;
	modules[2].loaded = false;
	modules[2].load_state = OBS_MODULE_DISABLED;
	core.first_disabled_module = &modules[3];
	modules[3].post_load = excluded;
	modules[3].loaded = true;
	modules[3].load_state = OBS_MODULE_ENABLED;
	modules[4].post_load = excluded;
	struct obs_module before[6];
	memcpy(before, modules, sizeof(before));
	obs_post_load_modules();
	assert_int_equal(visit_count, 1);
	assert_int_equal(visits[0], 3);
	assert_memory_equal(modules, before, sizeof(before));
	assert_ptr_equal(core.first_module, &modules[2]);
	assert_ptr_equal(core.first_disabled_module, &modules[3]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(empty_and_excluded, setup, teardown),
		cmocka_unit_test_setup_teardown(null_callbacks, setup, teardown),
		cmocka_unit_test_setup_teardown(traversal_order_and_repeat, setup, teardown),
		cmocka_unit_test_setup_teardown(list_membership_controls_dispatch, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
