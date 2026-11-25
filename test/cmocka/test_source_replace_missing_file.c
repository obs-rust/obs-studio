/* Characterize obs_source_replace_missing_file() before its Rust swap (Refs #5635). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

#if !__has_include("obs-source-replace-missing-file.c.h")
#error "source replace-missing-file extraction seam is missing"
#endif

static unsigned log_count;
static int log_level;
static char message[256];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(message, sizeof(message), format, args);
}

static unsigned calls;
static void *got_src;
static const char *got_path;
static void *got_data;

static void on_replace(void *src, const char *new_path, void *data)
{
	calls++;
	got_src = src;
	got_path = new_path;
	got_data = data;
}

static void reset(void)
{
	log_count = 0;
	log_level = 0;
	message[0] = '\0';
	calls = 0;
	got_src = NULL;
	got_path = NULL;
	got_data = NULL;
}

static void replace(obs_source_t *source, const char *new_path, void *data)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	reset();
	base_set_log_handler(capture, NULL);
	obs_source_replace_missing_file(on_replace, source, new_path, data);
	base_set_log_handler(previous, param);
}

/* The function reads only context.data; every other byte is noise. */
static struct obs_source *noisy_source(void *data)
{
	struct obs_source *source = malloc(sizeof(*source));
	assert_non_null(source);
	memset(source, 0xa5, sizeof(*source));
	source->context.data = data;
	return source;
}

static void null_source(void **unused)
{
	(void)unused;
	int marker = 0;
	replace(NULL, "new.png", &marker);
	assert_int_equal(calls, 0);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(message, "obs_source_replace_missing_file: Null 'source' parameter");
}

static void null_data_skips_callback(void **unused)
{
	(void)unused;
	int marker = 0;
	struct obs_source *source = noisy_source(NULL);
	struct obs_source *before = malloc(sizeof(*before));
	assert_non_null(before);
	memcpy(before, source, sizeof(*source));
	replace(source, "new.png", &marker);
	assert_int_equal(calls, 0);
	assert_int_equal(log_count, 0);
	assert_memory_equal(source, before, sizeof(*source));
	free(before);
	free(source);
}

static void calls_back_with_private_data(void **unused)
{
	(void)unused;
	int private_data = 0;
	int marker = 0;
	const char *path = "fixed.png";
	struct obs_source *source = noisy_source(&private_data);
	struct obs_source *before = malloc(sizeof(*before));
	assert_non_null(before);
	memcpy(before, source, sizeof(*source));
	replace(source, path, &marker);
	assert_int_equal(calls, 1);
	assert_ptr_equal(got_src, &private_data);
	assert_ptr_equal(got_path, path);
	assert_ptr_equal(got_data, &marker);
	assert_int_equal(log_count, 0);
	assert_memory_equal(source, before, sizeof(*source));
	free(before);
	free(source);
}

static void passes_null_path_and_data(void **unused)
{
	(void)unused;
	int private_data = 0;
	struct obs_source *source = noisy_source(&private_data);
	got_path = "unset";
	replace(source, NULL, NULL);
	assert_int_equal(calls, 1);
	assert_ptr_equal(got_src, &private_data);
	assert_null(got_path);
	assert_null(got_data);
	assert_int_equal(log_count, 0);
	free(source);
}

static void each_call_calls_back_once(void **unused)
{
	(void)unused;
	int private_data = 0;
	int marker = 0;
	struct obs_source *source = noisy_source(&private_data);
	unsigned total = 0;
	for (int i = 0; i < 3; i++) {
		replace(source, "again.png", &marker);
		total += calls;
	}
	assert_int_equal(total, 3);
	free(source);
}

int main(void)
{
	/* No OBS startup, graphics, audio, threads, or source destruction is needed. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(null_data_skips_callback),
		cmocka_unit_test(calls_back_with_private_data),
		cmocka_unit_test(passes_null_path_and_data),
		cmocka_unit_test(each_call_calls_back_once),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
