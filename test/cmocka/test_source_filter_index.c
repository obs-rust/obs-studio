/* Characterize the actual libobs filter-index lookup before its Rust port. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void invalid_arguments(void **state)
{
	(void)state;
	struct obs_source source = {0};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	int null_source = obs_source_filter_get_index(NULL, &source);
	int both_null = obs_source_filter_get_index(NULL, NULL);
	unsigned source_logs = log_count;
	int source_level = log_level;
	char source_message[128];
	memcpy(source_message, log_message, sizeof(source_message));
	/* No mutex is initialized: invalid arguments must return before locking. */
	int null_filter = obs_source_filter_get_index(&source, NULL);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(null_source, -1);
	assert_int_equal(both_null, -1);
	assert_int_equal(null_filter, -1);
	assert_int_equal(source_logs, 2);
	assert_int_equal(log_count, 3);
	assert_int_equal(source_level, LOG_DEBUG);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(source_message, "obs_source_filter_get_index: Null 'source' parameter");
	assert_string_equal(log_message, "obs_source_filter_get_index: Null 'filter' parameter");
}

static void ordered_lookup(void **state)
{
	(void)state;
	struct obs_source source = {0};
	struct obs_source filters[4] = {0};
	obs_source_t *items[] = {&filters[2], &filters[0], &filters[1], &filters[0]};
	assert_int_equal(pthread_mutex_init(&source.filter_mutex, NULL), 0);
	int empty = obs_source_filter_get_index(&source, &filters[0]);
	/* Borrow stack storage: this getter must neither resize nor own it. */
	source.filters.array = items;
	source.filters.num = source.filters.capacity = 4;
	int first = obs_source_filter_get_index(&source, &filters[2]);
	int middle = obs_source_filter_get_index(&source, &filters[1]);
	/* Duplicate entries are characterized, not endorsed: first match wins. */
	int duplicate = obs_source_filter_get_index(&source, &filters[0]);
	int absent = obs_source_filter_get_index(&source, &filters[3]);
	int self = obs_source_filter_get_index(&source, &source);
	int unlocked = pthread_mutex_trylock(&source.filter_mutex);
	if (unlocked == 0)
		pthread_mutex_unlock(&source.filter_mutex);
	int destroyed = pthread_mutex_destroy(&source.filter_mutex);
	assert_int_equal(empty, -1);
	assert_int_equal(first, 0);
	assert_int_equal(middle, 2);
	assert_int_equal(duplicate, 1);
	assert_int_equal(absent, -1);
	assert_int_equal(self, -1);
	assert_int_equal(unlocked, 0);
	assert_int_equal(destroyed, 0);
	assert_ptr_equal(source.filters.array, items);
	assert_int_equal(source.filters.num, 4);
	assert_int_equal(source.filters.capacity, 4);
	assert_ptr_equal(items[0], &filters[2]);
	assert_ptr_equal(items[1], &filters[0]);
	assert_ptr_equal(items[2], &filters[1]);
	assert_ptr_equal(items[3], &filters[0]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(invalid_arguments), cmocka_unit_test(ordered_lookup)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
