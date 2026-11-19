/* C-source characterization of the event wrapper and its per-binding adapter.
 * The registry seams record ordering without a desktop, GPU or threads. The
 * existing test_hotkey_press_state exercises the real shared-library state
 * machine. The Rust swap must relink this unchanged recorder to its replacement. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-hotkey-seam.h"

struct obs_core *inject_test_obs;
static struct obs_core core;
static obs_hotkey_binding_t bindings[3];
static bool locked, strict_at_lock, expected_pressed;
static obs_key_combination_t expected_key;
static size_t count, forwarded;
static unsigned locks, enumerations, unlocks;
static char order[16];
static size_t order_count;

static void record(char step)
{
	assert_true(order_count < sizeof(order));
	order[order_count++] = step;
}

bool obs_hotkey_seam_lock(void)
{
	assert_false(locked);
	locks++;
	record('L');
	if (!inject_test_obs)
		return false;
	locked = true;
	/* A read before locking would capture the opposite value. */
	core.hotkeys.strict_modifiers = strict_at_lock;
	return true;
}

void obs_hotkey_seam_unlock(void)
{
	assert_true(locked);
	assert_int_equal(forwarded, count);
	unlocks++;
	record('U');
	locked = false;
}

void obs_hotkey_binding_inject(obs_hotkey_binding_t *binding, obs_key_combination_t key, bool pressed,
			       bool strict_modifiers)
{
	assert_true(locked);
	assert_true(forwarded < count);
	assert_ptr_equal(binding, &bindings[forwarded]);
	assert_int_equal(key.key, expected_key.key);
	assert_int_equal(key.modifiers, expected_key.modifiers);
	assert_int_equal(pressed, expected_pressed);
	assert_int_equal(strict_modifiers, strict_at_lock);
	forwarded++;
	record('B');
	/* Every binding must still get the one captured event value. */
	core.hotkeys.strict_modifiers = !core.hotkeys.strict_modifiers;
}

void obs_hotkey_seam_enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	assert_true(locked);
	assert_non_null(func);
	assert_non_null(data);
	enumerations++;
	record('E');
	core.hotkeys.strict_modifiers = !strict_at_lock;
	for (size_t i = 0; i < count; i++)
		assert_true(func(data, i, &bindings[i]));
}

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	memset(bindings, 0, sizeof(bindings));
	memset(order, 0, sizeof(order));
	inject_test_obs = &core;
	locked = strict_at_lock = expected_pressed = false;
	count = forwarded = order_count = 0;
	locks = enumerations = unlocks = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_false(locked);
	inject_test_obs = NULL;
	return 0;
}

static void no_core(void **state)
{
	(void)state;
	inject_test_obs = NULL;
	obs_hotkey_inject_event((obs_key_combination_t){UINT32_MAX, OBS_KEY_MOUSE3}, false);
	assert_int_equal(locks, 1);
	assert_int_equal(enumerations, 0);
	assert_int_equal(unlocks, 0);
	assert_int_equal(forwarded, 0);
	assert_int_equal(order_count, 1);
	assert_memory_equal(order, "L", 1);
}

static void empty_registry(void **state)
{
	(void)state;
	strict_at_lock = true;
	obs_hotkey_inject_event((obs_key_combination_t){0, OBS_KEY_NONE}, true);
	assert_int_equal(locks, 1);
	assert_int_equal(enumerations, 1);
	assert_int_equal(unlocks, 1);
	assert_int_equal(order_count, 3);
	assert_memory_equal(order, "LEU", 3);
}

static void forwarding(void **state)
{
	(void)state;
	const obs_key_combination_t keys[] = {{0, OBS_KEY_NONE},
					      {UINT32_MAX, OBS_KEY_MOUSE3},
					      {INTERACT_CONTROL_KEY, OBS_KEY_A}};
	/* No invalid enum sentinel or host-dependent defaults. False events and
	 * otherwise unusual modifier bits reach the existing binding engine. */
	for (size_t k = 0; k < sizeof(keys) / sizeof(keys[0]); k++) {
		for (unsigned flags = 0; flags < 4; flags++) {
			expected_key = keys[k];
			expected_pressed = (flags & 1) != 0;
			strict_at_lock = (flags & 2) != 0;
			core.hotkeys.strict_modifiers = !strict_at_lock;
			count = 3;
			forwarded = order_count = 0;
			obs_hotkey_inject_event(expected_key, expected_pressed);
			assert_int_equal(forwarded, 3);
			assert_int_equal(order_count, 6);
			assert_memory_equal(order, "LEBBBU", 6);
		}
	}
	assert_int_equal(locks, 12);
	assert_int_equal(enumerations, 12);
	assert_int_equal(unlocks, 12);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_core, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_registry, setup, teardown),
		cmocka_unit_test_setup_teardown(forwarding, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
