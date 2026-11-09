/* Direct characterization of the extracted background control, using a
 * real private core layout and an instrumented registry lock (no threads). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs-internal.h>

struct obs_core *control_test_obs;
static struct obs_core core;
static bool locked;
static unsigned locks, unlocks, wakes;

bool obs_hotkey_seam_lock(void)
{
	assert_false(locked);
	locks++;
	locked = control_test_obs != NULL;
	return locked;
}

void obs_hotkey_seam_unlock(void)
{
	assert_true(locked);
	unlocks++;
	locked = false;
}

static void wake(void *data)
{
	assert_true(locked);
	assert_ptr_equal(data, &core);
	wakes++;
}

static void control(void **state)
{
	UNUSED_PARAMETER(state);
	control_test_obs = NULL;
	obs_hotkeys_retry_background();
	obs_hotkeys_configure_background();
	obs_hotkeys_set_background_probe(true);
	assert_int_equal(locks, 3);
	assert_int_equal(unlocks, 0);
	assert_int_equal(wakes, 0);
	control_test_obs = &core;
	core.hotkeys.background.wake = wake;
	core.hotkeys.background.wake_data = &core;
	obs_hotkeys_retry_background();
	obs_hotkeys_retry_background();
	assert_true(core.hotkeys.background.retry);
	assert_int_equal(wakes, 2);
	core.hotkeys.background.flags = UINT32_MAX;
	obs_hotkeys_configure_background();
	obs_hotkeys_configure_background();
	assert_true(core.hotkeys.background.configure);
	assert_int_equal(core.hotkeys.background.flags, UINT32_MAX & ~(uint32_t)OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED);
	assert_int_equal(wakes, 4);
	/* A repeated false probe does not cancel configure: characterized, not endorsed. */
	obs_hotkeys_set_background_probe(false);
	assert_true(core.hotkeys.background.configure);
	assert_int_equal(wakes, 4);
	obs_hotkeys_set_background_probe(true);
	assert_true(core.hotkeys.background.probe);
	assert_true(core.hotkeys.background.configure);
	obs_hotkeys_set_background_probe(true);
	assert_int_equal(wakes, 5);
	obs_hotkeys_set_background_probe(false);
	assert_false(core.hotkeys.background.probe);
	assert_false(core.hotkeys.background.configure);
	assert_int_equal(wakes, 6);
	core.hotkeys.background.wake = NULL;
	obs_hotkeys_retry_background();
	obs_hotkeys_configure_background();
	obs_hotkeys_set_background_probe(true);
	assert_int_equal(wakes, 6);
	assert_false(locked);
	assert_int_equal(locks, 14);
	assert_int_equal(unlocks, 11);
	control_test_obs = NULL;
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(control)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
