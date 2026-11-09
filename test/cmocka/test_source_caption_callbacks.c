/* Characterize the actual libobs source caption callback registration.
 * Stack fixtures isolate these operations without starting media or OBS. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[256];
static log_handler_t saved_handler;
static void *saved_param;

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

static int setup(void **state)
{
	(void)state;
	log_count = 0;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(saved_handler, saved_param);
	return 0;
}

static void caption_a(void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	(void)param;
	(void)source;
	(void)captions;
	fail(); /* Registration must not invoke callbacks. */
}

static void caption_b(void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	caption_a(param, source, captions);
}

static void null_sources(void **state)
{
	(void)state;
	obs_source_add_caption_callback(NULL, caption_a, NULL);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_add_caption_callback: Null 'source' parameter");
	log_count = 0;
	obs_source_remove_caption_callback(NULL, caption_a, NULL);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_remove_caption_callback: Null 'source' parameter");
}

static void expect_pair(struct obs_source *source, size_t index, obs_source_caption_t callback, void *param)
{
	assert_true(source->caption_cb_list.array[index].callback == callback);
	assert_ptr_equal(source->caption_cb_list.array[index].param, param);
}

static void callback_pairs(void **state)
{
	(void)state;
	struct obs_source source = {0};
	int first = 1, second = 2;
	assert_int_equal(pthread_mutex_init(&source.caption_cb_mutex, NULL), 0);
	obs_source_remove_caption_callback(&source, caption_a, &first);
	assert_int_equal(source.caption_cb_list.num, 0);
	obs_source_add_caption_callback(&source, caption_a, &first);
	obs_source_add_caption_callback(&source, caption_b, &first);
	obs_source_add_caption_callback(&source, caption_a, &second);
	obs_source_add_caption_callback(&source, caption_a, &first);
	assert_int_equal(source.caption_cb_list.num, 4);
	expect_pair(&source, 0, caption_a, &first);
	expect_pair(&source, 1, caption_b, &first);
	expect_pair(&source, 2, caption_a, &second);
	expect_pair(&source, 3, caption_a, &first);
	/* Duplicates are stored; erase removes the first exact pair only. */
	obs_source_remove_caption_callback(&source, caption_a, &first);
	assert_int_equal(source.caption_cb_list.num, 3);
	expect_pair(&source, 0, caption_b, &first);
	expect_pair(&source, 1, caption_a, &second);
	expect_pair(&source, 2, caption_a, &first);
	obs_source_remove_caption_callback(&source, caption_b, &second);
	assert_int_equal(source.caption_cb_list.num, 3);
	expect_pair(&source, 0, caption_b, &first);
	expect_pair(&source, 1, caption_a, &second);
	expect_pair(&source, 2, caption_a, &first);
	obs_source_remove_caption_callback(&source, caption_a, &second);
	assert_int_equal(source.caption_cb_list.num, 2);
	expect_pair(&source, 0, caption_b, &first);
	expect_pair(&source, 1, caption_a, &first);
	obs_source_remove_caption_callback(&source, caption_b, &first);
	obs_source_remove_caption_callback(&source, caption_a, &first);
	assert_int_equal(source.caption_cb_list.num, 0);
	/* NULL entries are stored too: characterized, not endorsed. */
	obs_source_add_caption_callback(&source, NULL, NULL);
	assert_int_equal(source.caption_cb_list.num, 1);
	expect_pair(&source, 0, NULL, NULL);
	obs_source_remove_caption_callback(&source, NULL, NULL);
	assert_int_equal(source.caption_cb_list.num, 0);
	da_free(source.caption_cb_list);
	assert_int_equal(pthread_mutex_destroy(&source.caption_cb_mutex), 0);
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(null_sources, setup, teardown),
					   cmocka_unit_test_setup_teardown(callback_pairs, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
