/* Characterize the optional extra-data callback gate before its Rust swap (Refs #5091). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-encoder-extra-data.c.h")
#error "encoder extra-data extraction seam is missing"
#endif

static log_handler_t saved_handler;
static void *saved_param;
static unsigned log_count;
static int log_level;
static char log_message[256];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
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
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(saved_handler, saved_param);
	return 0;
}

struct recorder {
	unsigned calls;
	uint8_t **expected_data;
	size_t *expected_size;
	bool result;
	bool mutate;
	uint8_t *data;
	size_t size;
};

static struct recorder *expected_context;

static bool record_extra_data(void *data, uint8_t **extra_data, size_t *size)
{
	assert_ptr_equal(data, expected_context);
	struct recorder *recorder = data;
	recorder->calls++;
	assert_ptr_equal(extra_data, recorder->expected_data);
	assert_ptr_equal(size, recorder->expected_size);
	/* Nullable outputs are safe only because this recorder tolerates them. */
	if (recorder->mutate) {
		if (extra_data)
			*extra_data = recorder->data;
		if (size)
			*size = recorder->size;
	}
	return recorder->result;
}

static bool wrong_dispatch(void *data, uint8_t **extra_data, size_t *size)
{
	(void)data;
	(void)extra_data;
	(void)size;
	fail();
	return false;
}

static void null_encoder(void **state)
{
	(void)state;
	uint8_t token = 0;
	uint8_t *data = &token;
	size_t size = SIZE_MAX;
	assert_false(obs_encoder_get_extra_data(NULL, &data, &size));
	assert_ptr_equal(data, &token);
	assert_true(size == SIZE_MAX);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_get_extra_data: Null 'encoder' parameter");
}

static void optional_gate(void **state)
{
	(void)state;
	struct obs_encoder encoder;
	struct recorder recorder = {0};
	expected_context = &recorder;
	uint8_t token = 0;
	for (unsigned callback = 0; callback < 2; callback++) {
		for (unsigned context = 0; context < 2; context++) {
			if (callback && context)
				continue;
			memset(&encoder, 0, sizeof(encoder));
			encoder.info.get_extra_data = callback ? record_extra_data : NULL;
			encoder.orig_info.get_extra_data = wrong_dispatch;
			encoder.context.data = context ? &recorder : NULL;
			struct obs_encoder before;
			memcpy(&before, &encoder, sizeof(before));
			uint8_t *data = &token;
			size_t size = SIZE_MAX;
			assert_false(obs_encoder_get_extra_data(&encoder, &data, &size));
			assert_ptr_equal(data, &token);
			assert_true(size == SIZE_MAX);
			assert_false(obs_encoder_get_extra_data(&encoder, NULL, NULL));
			assert_int_equal(recorder.calls, 0);
			assert_memory_equal(&encoder, &before, sizeof(before));
		}
	}
	assert_int_equal(log_count, 0);
}

static void dispatch_and_outputs(void **state)
{
	(void)state;
	const size_t sizes[] = {0, 1, SIZE_MAX / 2, SIZE_MAX / 2 + 1, SIZE_MAX - 1, SIZE_MAX};
	uint8_t sentinel = 0, payload = 1;
	struct obs_encoder encoder;
	memset(&encoder, 0, sizeof(encoder));
	encoder.info.get_extra_data = record_extra_data;
	encoder.orig_info.get_extra_data = wrong_dispatch;
	struct recorder recorder = {0};
	expected_context = &recorder;
	encoder.context.data = &recorder;
	struct obs_encoder before;
	memcpy(&before, &encoder, sizeof(before));
	for (unsigned result = 0; result < 2; result++) {
		for (unsigned mutate = 0; mutate < 2; mutate++) {
			for (unsigned null_data = 0; null_data < 2; null_data++) {
				for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
					uint8_t *data = &sentinel;
					size_t size = 17;
					recorder = (struct recorder){.expected_data = &data,
								     .expected_size = &size,
								     .result = result != 0,
								     .mutate = mutate != 0,
								     .data = null_data ? NULL : &payload,
								     .size = sizes[i]};
					/* False with mutated outputs is characterized, not endorsed. */
					assert_int_equal(obs_encoder_get_extra_data(&encoder, &data, &size), result);
					assert_int_equal(recorder.calls, 1);
					assert_ptr_equal(data, mutate ? recorder.data : &sentinel);
					assert_true(size == (mutate ? sizes[i] : 17));
					assert_memory_equal(&encoder, &before, sizeof(before));
				}
			}
		}
	}
	assert_int_equal(log_count, 0);
}

static void nullable_outputs(void **state)
{
	(void)state;
	uint8_t sentinel = 0, payload = 1;
	struct obs_encoder encoder;
	memset(&encoder, 0, sizeof(encoder));
	encoder.info.get_extra_data = record_extra_data;
	/* A missing original callback must not suppress the active callback. */
	struct recorder recorder = {0};
	expected_context = &recorder;
	encoder.context.data = &recorder;
	for (unsigned result = 0; result < 2; result++) {
		for (unsigned outputs = 0; outputs < 4; outputs++) {
			uint8_t *data = &sentinel;
			size_t size = 17;
			recorder = (struct recorder){.expected_data = outputs & 1 ? &data : NULL,
						     .expected_size = outputs & 2 ? &size : NULL,
						     .result = result != 0,
						     .mutate = true,
						     .data = &payload,
						     .size = SIZE_MAX};
			assert_int_equal(obs_encoder_get_extra_data(&encoder, recorder.expected_data,
								    recorder.expected_size),
					 result);
			assert_int_equal(recorder.calls, 1);
			assert_ptr_equal(data, outputs & 1 ? &payload : &sentinel);
			assert_true(size == (outputs & 2 ? SIZE_MAX : 17));
		}
	}
	assert_int_equal(log_count, 0);
}

int main(void)
{
	/* Stack-only fixtures: no startup, owned resources, destruction or threads. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_encoder, setup, teardown),
		cmocka_unit_test_setup_teardown(optional_gate, setup, teardown),
		cmocka_unit_test_setup_teardown(dispatch_and_outputs, setup, teardown),
		cmocka_unit_test_setup_teardown(nullable_outputs, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
