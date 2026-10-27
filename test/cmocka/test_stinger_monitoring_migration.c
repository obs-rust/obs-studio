#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "transition-stinger-monitoring.h"

/* A Stinger transition saved before 33.0 stored "audio_monitoring" as an int:
 * 0 (off), 1 (Monitor Only) or 2 (Monitor and Output). 33.0 replaced it with
 * the boolean "enable_monitoring" and reads nothing else, so a transition set
 * to Monitor Only lost its monitoring on the upgrade and started playing into
 * the stream and recordings, with no mixer entry to mute it back
 * (obs-rust/obs-studio#2638). stinger_monitoring_migration() converts the old
 * value once, while "enable_monitoring" has no user value; a transition 33.0
 * saved carries one and keeps the choice the user made. */

static void check(bool has_enable, bool enable, long long audio, bool enabled, bool mute)
{
	struct stinger_monitoring plan = stinger_monitoring_migration(has_enable, enable, audio);
	assert_int_equal(plan.enabled, enabled);
	assert_int_equal(plan.mute, mute);
}

/* A transition saved before 33.0: no "enable_monitoring" value, so the old
 * "audio_monitoring" decides. */

static void test_old_off(void **state)
{
	(void)state;
	check(false, false, STINGER_AUDIO_MONITORING_OFF, false, false);
}

static void test_old_monitor_only_is_muted(void **state)
{
	(void)state;
	/* Monitor Only loads with monitoring on and muted: 33.0 monitors and
	 * outputs at once, and the monitor plays a muted source. */
	check(false, false, STINGER_AUDIO_MONITORING_MONITOR_ONLY, true, true);
}

static void test_old_monitor_and_output(void **state)
{
	(void)state;
	check(false, false, STINGER_AUDIO_MONITORING_MONITOR_AND_OUTPUT, true, false);
}

/* An "audio_monitoring" that is not 0, 1 or 2 is read as off, as an unknown
 * value was. */
static void test_old_unknown_value_is_off(void **state)
{
	(void)state;
	check(false, false, 3, false, false);
	check(false, false, -1, false, false);
	check(false, false, 0, false, false);
}

/* A transition 33.0 saved has "enable_monitoring": its own choice is kept and
 * the old key, if still present, is ignored. */
static void test_user_value_is_kept(void **state)
{
	(void)state;
	check(true, true, STINGER_AUDIO_MONITORING_OFF, true, false);
	check(true, false, STINGER_AUDIO_MONITORING_MONITOR_ONLY, false, false);
	check(true, false, STINGER_AUDIO_MONITORING_MONITOR_AND_OUTPUT, false, false);
	check(true, true, STINGER_AUDIO_MONITORING_MONITOR_ONLY, true, false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_old_off),
		cmocka_unit_test(test_old_monitor_only_is_muted),
		cmocka_unit_test(test_old_monitor_and_output),
		cmocka_unit_test(test_old_unknown_value_is_off),
		cmocka_unit_test(test_user_value_is_kept),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
