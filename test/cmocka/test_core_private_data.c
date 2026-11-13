/* Characterize the real libobs private-data wrappers before their Rust swap.
 * An initialized core is a precondition, including for NULL settings. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void replacement_keeps_storage_and_copies_settings(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	assert_non_null(held);
	obs_data_set_string(held, "old-only", "discard");
	obs_data_set_string(held, "shared", "old");

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "shared", 42);
	obs_data_set_bool(settings, "new-only", true);
	obs_set_private_data(settings);

	obs_data_t *read = obs_get_private_data();
	assert_ptr_equal(read, held);
	assert_ptr_not_equal(read, settings);
	assert_false(obs_data_has_user_value(read, "old-only"));
	assert_string_equal(obs_data_get_string(read, "old-only"), "");
	assert_int_equal(obs_data_get_int(read, "shared"), 42);
	assert_true(obs_data_get_bool(read, "new-only"));
	assert_int_equal(obs_data_get_int(settings, "shared"), 42);
	assert_true(obs_data_get_bool(settings, "new-only"));
	assert_false(obs_data_has_user_value(settings, "old-only"));

	obs_data_set_int(settings, "shared", 99);
	assert_int_equal(obs_data_get_int(held, "shared"), 42);
	obs_data_release(settings);
	assert_true(obs_data_get_bool(held, "new-only"));
	obs_data_set_string(read, "through-read", "shared storage");
	assert_string_equal(obs_data_get_string(held, "through-read"), "shared storage");
	obs_data_release(read);
	obs_data_release(held);
}

static void null_and_empty_settings_clear_user_values(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	obs_data_set_default_string(held, "fallback", "default");
	obs_data_set_string(held, "fallback", "user");
	obs_data_set_int(held, "old-only", 7);
	obs_set_private_data(NULL);
	assert_false(obs_data_has_user_value(held, "fallback"));
	assert_false(obs_data_has_user_value(held, "old-only"));
	/* Characterized, not endorsed: clearing preserves data defaults. */
	assert_true(obs_data_has_default_value(held, "fallback"));
	assert_string_equal(obs_data_get_string(held, "fallback"), "default");

	obs_data_set_int(held, "again", 13);
	obs_data_t *empty = obs_data_create();
	obs_set_private_data(empty);
	assert_false(obs_data_has_user_value(held, "again"));
	assert_string_equal(obs_data_get_string(held, "fallback"), "default");
	obs_data_release(empty);
	obs_set_private_data(NULL);
	obs_data_t *read = obs_get_private_data();
	assert_ptr_equal(read, held);
	assert_false(obs_data_has_user_value(read, "again"));
	obs_data_release(read);
	obs_data_release(held);
}

static void self_assignment_clears_before_applying(void **state)
{
	(void)state;
	obs_data_t *held = obs_get_private_data();
	obs_data_set_string(held, "value", "cleared before apply");
	/* Characterized, not endorsed: setting the returned object to itself clears
	 * its user values, because the clear happens before the apply. */
	obs_set_private_data(held);
	assert_false(obs_data_has_user_value(held, "value"));
	assert_string_equal(obs_data_get_string(held, "value"), "");
	obs_data_t *read = obs_get_private_data();
	assert_ptr_equal(read, held);
	obs_data_release(read);
	obs_data_release(held);
}

struct item_probe {
	obs_data_item_t *first_before;
	obs_data_item_t *first_after;
	obs_data_item_t *tail;
	obs_data_item_t *tail_guard;
};

static struct item_probe prepare_probe(obs_data_t *data)
{
	obs_data_set_int(data, "first", 1);
	obs_data_set_int(data, "tail", 2);
	struct item_probe probe = {
		obs_data_item_byname(data, "first"),
		obs_data_item_byname(data, "first"),
		obs_data_item_byname(data, "tail"),
		obs_data_item_byname(data, "tail"),
	};
	return probe;
}

static unsigned observe_destroyed_parent(struct item_probe *probe)
{
	/* Retain both items throughout: the original C keeps a detached item's
	 * next link, while the hybrid data API resolves it through its parent.
	 * Holding the tail makes both traversals safe after parent destruction.
	 * Removing the tail changes the first item's link only in a live parent. */
	unsigned before = obs_data_item_next(&probe->first_before) ? 1 : 0;
	obs_data_item_remove(&probe->tail);
	unsigned after = obs_data_item_next(&probe->first_after) ? 2 : 0;
	obs_data_item_release(&probe->first_before);
	obs_data_item_release(&probe->first_after);
	obs_data_item_release(&probe->tail_guard);
	return before | after;
}

static void every_getter_retains_exactly_once(void **state)
{
	(void)state;
	/* Obtain the selected data implementation's observable destruction
	 * behavior from an independent object with exactly one reference. */
	obs_data_t *control = obs_data_create();
	struct item_probe control_probe = prepare_probe(control);
	obs_data_release(control);
	unsigned destroyed = observe_destroyed_parent(&control_probe);
	/* A live parent has a successor before tail removal and none after. */
	assert_int_not_equal(destroyed, 1);

	obs_data_t *held[8];
	for (size_t i = 0; i < 8; ++i) {
		held[i] = obs_get_private_data();
		assert_non_null(held[i]);
		assert_ptr_equal(held[i], held[0]);
	}
	struct item_probe probe = prepare_probe(held[0]);
	/* The core releases its own reference; each getter's reference remains. */
	obs_shutdown();
	for (size_t i = 0; i < 8; ++i) {
		assert_int_equal(obs_data_get_int(held[i], "first"), 1);
		assert_int_equal(obs_data_get_int(held[i], "tail"), 2);
		obs_data_release(held[i]);
	}
	/* Extra retains leave a live parent; missing retains destroy it early. */
	assert_int_equal(observe_destroyed_parent(&probe), destroyed);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(replacement_keeps_storage_and_copies_settings, setup, teardown),
		cmocka_unit_test_setup_teardown(null_and_empty_settings_clear_user_values, setup, teardown),
		cmocka_unit_test_setup_teardown(self_assignment_clears_before_applying, setup, teardown),
		cmocka_unit_test_setup_teardown(every_getter_retains_exactly_once, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
