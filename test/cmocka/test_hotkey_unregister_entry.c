/* C-source characterization of the public removal wrappers and the real pair
 * helper. Only mutex calls and the already-ported single-key remover record;
 * there is no desktop, input thread, synthetic wait or duplicated pair logic.
 * The Rust swap must relink this unchanged test to its replacement wrappers. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"

/* Supply the real C owner-release helper, as the other isolated C fixtures do.
 * Its single-key removal entry has a separate name so the recorder below owns
 * dispatch observation; no placeholder cleanup or Rust archive fallback. */
#undef obs_hotkey_unregister_one
#define obs_hotkey_unregister_one unregister_entry_c_remove_one
#include "libobs/obs-hotkey-unregister.c.h"
#undef obs_hotkey_unregister_one
#define obs_hotkey_unregister_one unregister_entry_remove_one

struct obs_core *hotkey_test_obs;
static struct obs_core core;
static bool locked, removing_pair;
static long pair_allocations;
static char order[16];
static size_t order_count, removed_count;
static obs_hotkey_id removed[4];
static obs_hotkey_pair_t *expected_pair;

static void record(char step)
{
	assert_true(order_count < sizeof(order));
	order[order_count++] = step;
}

int pthread_mutex_lock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &core.hotkeys.mutex);
	assert_false(locked);
	locked = true;
	record('L');
	return 0;
}

int pthread_mutex_unlock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &core.hotkeys.mutex);
	assert_true(locked);
	if (removing_pair) {
		assert_null(core.hotkeys.hotkey_pairs);
		assert_int_equal(bnum_allocs(), pair_allocations);
	}
	locked = false;
	record('U');
	return 0;
}

void obs_hotkey_unregister_one(obs_hotkey_id id)
{
	assert_true(locked);
	assert_true(removed_count < sizeof(removed) / sizeof(removed[0]));
	if (removing_pair) {
		assert_true(removed_count < 2);
		/* The pair stays registered during both member removals. */
		assert_ptr_equal(core.hotkeys.hotkey_pairs, expected_pair);
		assert_int_equal(id, expected_pair->id[removed_count]);
	}
	removed[removed_count++] = id;
	record('R');
}

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	(void)context;
	(void)key;
	return false;
}

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	memset(order, 0, sizeof(order));
	memset(removed, 0, sizeof(removed));
	hotkey_test_obs = &core;
	locked = removing_pair = false;
	order_count = removed_count = 0;
	expected_pair = NULL;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_false(locked);
	assert_null(core.hotkeys.hotkey_pairs);
	hotkey_test_obs = NULL;
	return 0;
}

static void no_core(void **state)
{
	(void)state;
	hotkey_test_obs = NULL;
	obs_hotkey_unregister(OBS_INVALID_HOTKEY_ID);
	obs_hotkey_pair_unregister(OBS_INVALID_HOTKEY_PAIR_ID);
	assert_int_equal(order_count, 0);
	assert_int_equal(removed_count, 0);
}

static void single_forwarding(void **state)
{
	(void)state;
	const obs_hotkey_id ids[] = {0, (size_t)UINT32_MAX + (size_t)1, OBS_INVALID_HOTKEY_ID, 0};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		obs_hotkey_unregister(ids[i]);
		assert_int_equal(removed[i], ids[i]);
	}
	assert_int_equal(removed_count, 4);
	assert_int_equal(order_count, 12);
	assert_memory_equal(order, "LRULRULRULRU", 12);
}

static void missing_pair(void **state)
{
	(void)state;
	core.hotkeys.next_pair_id = 4;
	const obs_hotkey_pair_id ids[] = {0, 3, 4, OBS_INVALID_HOTKEY_PAIR_ID};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++)
		obs_hotkey_pair_unregister(ids[i]);
	assert_int_equal(removed_count, 0);
	assert_int_equal(order_count, 8);
	assert_memory_equal(order, "LULULULU", 8);
}

static void pair_member_order(void **state)
{
	(void)state;
	pair_allocations = bnum_allocs();
	obs_hotkey_pair_t *pair = bzalloc(sizeof(*pair));
	const obs_hotkey_pair_id id = (size_t)UINT32_MAX + (size_t)1;
	pair->pair_id = id;
	pair->id[0] = OBS_INVALID_HOTKEY_ID;
	pair->id[1] = (size_t)UINT32_MAX + (size_t)1;
	core.hotkeys.next_pair_id = id + 1;
	/* The production registry's size_t hash rule, independent of host width. */
	HASH_ADD_KEYPTR_BYHASHVALUE(hh, core.hotkeys.hotkey_pairs, &pair->pair_id, sizeof(size_t),
				    (unsigned int)(pair->pair_id % UINT_MAX), pair);
	expected_pair = pair;
	removing_pair = true;
	obs_hotkey_pair_unregister(pair->pair_id);
	assert_int_equal(removed_count, 2);
	assert_int_equal(order_count, 4);
	assert_memory_equal(order, "LRRU", 4);
	assert_int_equal(bnum_allocs(), pair_allocations);
	expected_pair = NULL;
	removing_pair = false;
	/* A repeated removal finds no pair and still balances the lock. */
	obs_hotkey_pair_unregister(id);
	assert_int_equal(removed_count, 2);
	assert_int_equal(order_count, 6);
	assert_memory_equal(order, "LRRULU", 6);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_core, setup, teardown),
		cmocka_unit_test_setup_teardown(single_forwarding, setup, teardown),
		cmocka_unit_test_setup_teardown(missing_pair, setup, teardown),
		cmocka_unit_test_setup_teardown(pair_member_order, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}

#undef obs_hotkey_unregister_one
