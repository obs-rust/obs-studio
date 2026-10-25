/* obs_frontend_warn_fallback() without a frontend (obs-rust/obs-studio#2218).
 * A plugin that falls back to a less safe mode calls it; in a host with no
 * OBS frontend (no callbacks set) there is nobody to warn, so the API
 * library logs the warning at error level and accepts at once, inside the
 * call, without the "no callbacks" error every other call logs. The line is
 * the one rust/obs-fallback-warning logs in the frontend. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <obs-frontend-api.h>
#include <util/base.h>

static int log_count;
static int log_level;
static char log_line[512];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_line, sizeof(log_line), format, args);
}

struct answers {
	int count;
	bool accepted;
};

static void answer(void *param, bool accepted)
{
	struct answers *a = param;
	a->count++;
	a->accepted = accepted;
}

static int setup(void **state)
{
	(void)state;
	log_count = 0;
	log_line[0] = 0;
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(NULL, NULL);
	return 0;
}

static void logs_and_accepts_at_once(void **state)
{
	(void)state;
	struct answers a = {0};
	obs_frontend_warn_fallback("browser.sandbox", "Title", "The sandbox could not start.", true, answer, &a);

	assert_int_equal(a.count, 1);
	assert_true(a.accepted);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_ERROR);
	assert_string_equal(log_line, "[fallback] browser.sandbox: The sandbox could not start. "
				      "This may become an error in a future version of OBS.");
}

static void null_callback_and_strings(void **state)
{
	(void)state;
	obs_frontend_warn_fallback(NULL, NULL, NULL, false, NULL, NULL);

	assert_int_equal(log_count, 1);
	assert_string_equal(log_line, "[fallback] : This may become an error in a future version of OBS.");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(logs_and_accepts_at_once, setup, teardown),
		cmocka_unit_test_setup_teardown(null_callback_and_strings, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
