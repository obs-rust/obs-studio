/* Characterization of the public UI handler contract. No capture hardware or
 * timing waits: the handler records tasks and the test chooses when to run them. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdio.h>
#include <cmocka.h>
#include <obs.h>
#include <util/base.h>

struct dispatch {
	unsigned handler;
	obs_task_t task;
	void *param;
	bool wait;
};

static struct dispatch calls[16];
static unsigned call_count;
static unsigned task_count;
static bool reenter;
static unsigned log_count;
static int log_level;
static char log_message[128];
static log_handler_t old_log_handler;
static void *old_log_param;

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void count_task(void *param)
{
	assert_ptr_equal(param, &task_count);
	task_count++;
}

static void record_dispatch(unsigned handler, obs_task_t task, void *param, bool wait)
{
	assert_true(call_count < sizeof(calls) / sizeof(calls[0]));
	calls[call_count++] = (struct dispatch){handler, task, param, wait};
	if (reenter) {
		reenter = false;
		/* Public dispatch is reentrant while the setter calls the handler. */
		obs_queue_task(OBS_TASK_UI, count_task, &task_count, true);
	}
}

static void handler_a(obs_task_t task, void *param, bool wait)
{
	record_dispatch(1, task, param, wait);
}

static void handler_b(obs_task_t task, void *param, bool wait)
{
	record_dispatch(2, task, param, wait);
}

static void expect_bootstrap(unsigned index, unsigned handler)
{
	assert_int_equal(calls[index].handler, handler);
	assert_non_null(calls[index].task);
	assert_null(calls[index].param);
	assert_false(calls[index].wait);
}

static void expect_no_handler_log(void)
{
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_ERROR);
	assert_string_equal(log_message, "UI task could not be queued, there's no UI task handler!");
	log_count = 0;
}

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&old_log_handler, &old_log_param);
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(old_log_handler, old_log_param);
	obs_shutdown();
	return 0;
}

static void install_replace_clear_and_dispatch(void **state)
{
	(void)state;
	assert_false(obs_in_task_thread(OBS_TASK_UI));
	reenter = true;
	obs_set_ui_task_handler(handler_a);
	/* The newly stored handler receives the bootstrap synchronously, with
	 * NULL data and wait=false. It can queue another UI task without a lock
	 * blocking it, and neither recorded callback has executed yet. */
	assert_int_equal(call_count, 2);
	expect_bootstrap(0, 1);
	assert_int_equal(calls[1].handler, 1);
	assert_true(calls[1].task == count_task);
	assert_ptr_equal(calls[1].param, &task_count);
	assert_true(calls[1].wait);
	assert_int_equal(task_count, 0);
	assert_false(obs_in_task_thread(OBS_TASK_UI));

	calls[0].task(calls[0].param);
	assert_true(obs_in_task_thread(OBS_TASK_UI));
	calls[1].task(calls[1].param);
	assert_int_equal(task_count, 1);

	obs_set_ui_task_handler(handler_b);
	assert_int_equal(call_count, 3);
	expect_bootstrap(2, 2);
	assert_true(calls[2].task == calls[0].task);
	/* Even on the marked UI thread, public UI tasks go through the current
	 * handler; wait and the original task/data are forwarded unchanged. */
	obs_queue_task(OBS_TASK_UI, count_task, &task_count, false);
	assert_int_equal(call_count, 4);
	assert_int_equal(calls[3].handler, 2);
	assert_true(calls[3].task == count_task);
	assert_ptr_equal(calls[3].param, &task_count);
	assert_false(calls[3].wait);
	assert_int_equal(task_count, 1);
	calls[2].task(calls[2].param);
	calls[3].task(calls[3].param);
	assert_int_equal(task_count, 2);

	base_set_log_handler(capture_log, NULL);
	obs_set_ui_task_handler(NULL);
	expect_no_handler_log();
	assert_int_equal(call_count, 4);
	/* Clearing the handler does not clear the calling thread's UI marker.
	 * Missing-handler tasks are logged and dropped for either wait value. */
	assert_true(obs_in_task_thread(OBS_TASK_UI));
	obs_queue_task(OBS_TASK_UI, count_task, &task_count, false);
	expect_no_handler_log();
	obs_queue_task(OBS_TASK_UI, count_task, &task_count, true);
	expect_no_handler_log();
	assert_int_equal(call_count, 4);
	assert_int_equal(task_count, 2);

	obs_set_ui_task_handler(handler_a);
	assert_int_equal(call_count, 5);
	expect_bootstrap(4, 1);
	assert_true(calls[4].task == calls[0].task);
	calls[4].task(calls[4].param);
	assert_true(obs_in_task_thread(OBS_TASK_UI));
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(install_replace_clear_and_dispatch, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
