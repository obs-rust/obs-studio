/* Actual-library characterization of obs_encoder_get_name() before its body
 * moves out of obs-encoder.c.h (obs-rust/obs-studio#4239). It pins the NULL
 * path with its exact debug log and NULL result, and that a valid encoder
 * returns its own context name pointer without logging: the given name, the
 * name generated for an empty one, and the new name after
 * obs_encoder_set_name. test_context_names keeps its own naming cases.
 *
 * libobs needs an X display on Linux, so the cases skip without one (run
 * under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <obs-internal.h>

#define VENC_ID "test_get_name_encoder"

static unsigned log_count;
static int log_level;
static char log_message[256];
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

static void start_capture(void)
{
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
	base_set_log_handler(capture_log, NULL);
}

static void stop_capture(void)
{
	base_set_log_handler(NULL, NULL);
}

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "get name encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bzalloc(1);
}

static void enc_destroy(void *data)
{
	bfree(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static bool obs_up;

static int setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_encoder_info info = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	obs_register_encoder(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

static void null_encoder_logs_and_returns_null(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	start_capture();
	const char *name = obs_encoder_get_name(NULL);
	stop_capture();

	assert_null(name);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_get_name: Null 'encoder' parameter");
}

static void returns_the_context_name_pointer_silently(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_encoder_t *encoder = obs_video_encoder_create(VENC_ID, "get name", NULL, NULL);
	assert_non_null(encoder);

	start_capture();
	const char *name = obs_encoder_get_name(encoder);
	const char *again = obs_encoder_get_name(encoder);
	stop_capture();

	assert_int_equal(log_count, 0);
	assert_ptr_equal(name, encoder->context.name);
	assert_ptr_equal(again, name);
	assert_string_equal(name, "get name");

	obs_encoder_release(encoder);
}

static void unnamed_encoder_returns_its_generated_name(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_encoder_t *encoder = obs_video_encoder_create(VENC_ID, "", NULL, NULL);
	assert_non_null(encoder);

	start_capture();
	const char *name = obs_encoder_get_name(encoder);
	stop_capture();

	assert_int_equal(log_count, 0);
	assert_non_null(name);
	assert_ptr_equal(name, encoder->context.name);
	assert_int_equal(strncmp(name, "__unnamed", 9), 0);

	obs_encoder_release(encoder);
}

static void follows_a_rename(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_encoder_t *encoder = obs_video_encoder_create(VENC_ID, "before", NULL, NULL);
	assert_non_null(encoder);
	assert_string_equal(obs_encoder_get_name(encoder), "before");

	obs_encoder_set_name(encoder, "after");

	start_capture();
	const char *name = obs_encoder_get_name(encoder);
	stop_capture();

	assert_int_equal(log_count, 0);
	assert_ptr_equal(name, encoder->context.name);
	assert_string_equal(name, "after");

	obs_encoder_release(encoder);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_encoder_logs_and_returns_null),
		cmocka_unit_test(returns_the_context_name_pointer_silently),
		cmocka_unit_test(unnamed_encoder_returns_its_generated_name),
		cmocka_unit_test(follows_a_rename),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
