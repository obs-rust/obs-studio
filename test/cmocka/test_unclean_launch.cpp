#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

#include <util/c99defs.h>

#include "unclean-launch.hpp"

/* obs-rust/obs-studio#809 (upstream obsproject/obs-studio#12650): after an
 * unclean shutdown, an unattended start (systemd unit, headless X, kiosk)
 * blocked forever on the modal "Crash Detected" dialog, because
 * --disable-shutdown-check was removed in OBS 32.0. With the flag, the
 * prompt must be skipped and logged instead. */

static void unclean_with_flag_skips_prompt(void **state)
{
	UNUSED_PARAMETER(state);
	const UncleanLaunchDecision d = decideUncleanLaunch(true, true);

	assert_false(d.showPrompt);
	assert_true(d.logSkippedPrompt);
}

static void unclean_without_flag_prompts(void **state)
{
	UNUSED_PARAMETER(state);
	const UncleanLaunchDecision d = decideUncleanLaunch(true, false);

	assert_true(d.showPrompt);
	assert_false(d.logSkippedPrompt);
}

static void clean_without_flag_does_not_prompt(void **state)
{
	UNUSED_PARAMETER(state);
	const UncleanLaunchDecision d = decideUncleanLaunch(false, false);

	assert_false(d.showPrompt);
	assert_false(d.logSkippedPrompt);
}

static void clean_with_flag_does_not_prompt(void **state)
{
	UNUSED_PARAMETER(state);
	const UncleanLaunchDecision d = decideUncleanLaunch(false, true);

	assert_false(d.showPrompt);
	assert_false(d.logSkippedPrompt);
}

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(unclean_with_flag_skips_prompt),
		cmocka_unit_test(unclean_without_flag_prompts),
		cmocka_unit_test(clean_without_flag_does_not_prompt),
		cmocka_unit_test(clean_with_flag_does_not_prompt),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
