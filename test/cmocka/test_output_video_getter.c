/* Characterize borrowed raw/encoded video routing before the Rust swap (Refs #5089). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-output-video-getter.c.h")
#error "output video getter extraction seam is missing"
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

static void expect_log(int level, const char *message)
{
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, level);
	assert_string_equal(log_message, message);
	log_count = 0;
}

struct fixture {
	struct obs_output output;
	struct obs_encoder encoders[2];
	struct obs_weak_object controls[3];
};

static void check_borrowed(struct fixture *fixture, video_t *expected)
{
	struct fixture before;
	memcpy(&before, fixture, sizeof(before));
	assert_ptr_equal(obs_output_video(&fixture->output), expected);
	assert_ptr_equal(obs_output_video(&fixture->output), expected);
	/* Includes output/encoder fields and their real strong/weak reference counters. */
	assert_memory_equal(fixture, &before, sizeof(before));
}

static void init_fixture(struct fixture *fixture)
{
	memset(fixture, 0, sizeof(*fixture));
	fixture->output.context.control = &fixture->controls[0];
	fixture->output.video_encoders[0] = &fixture->encoders[0];
	fixture->output.video_encoders[1] = &fixture->encoders[1];
	for (size_t i = 0; i < 2; i++) {
		fixture->encoders[i].context.control = &fixture->controls[i + 1];
		fixture->encoders[i].context.name = "video-%s";
		fixture->encoders[i].info.type = OBS_ENCODER_VIDEO;
	}
	for (size_t i = 0; i < 3; i++) {
		fixture->controls[i].ref.refs = 7;
		fixture->controls[i].ref.weak_refs = 11;
	}
}

static void null_output(void **state)
{
	(void)state;
	assert_null(obs_output_video(NULL));
	expect_log(LOG_DEBUG, "obs_output_video: Null 'output' parameter");
}

static void raw_video(void **state)
{
	(void)state;
	uint64_t raw_token = 0;
	const uint32_t flags[] = {0, OBS_OUTPUT_VIDEO, OBS_OUTPUT_AUDIO | OBS_OUTPUT_SERVICE,
				  UINT32_MAX ^ OBS_OUTPUT_ENCODED};
	struct fixture fixture;
	init_fixture(&fixture);
	/* Unreadable encoder slots prove that the raw branch never follows them. */
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++)
		fixture.output.video_encoders[i] = (obs_encoder_t *)(uintptr_t)1;
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		fixture.output.info.flags = flags[i];
		fixture.output.video = (video_t *)&raw_token;
		check_borrowed(&fixture, (video_t *)&raw_token);
		fixture.output.video = NULL;
		check_borrowed(&fixture, NULL);
	}
	assert_int_equal(log_count, 0);
}

static void encoded_video(void **state)
{
	(void)state;
	uint64_t raw_token = 0, first_token = 0, second_token = 0, override_token = 0;
	const uint32_t flags[] = {OBS_OUTPUT_ENCODED, OBS_OUTPUT_ENCODED | OBS_OUTPUT_AUDIO, UINT32_MAX};
	struct fixture fixture;
	init_fixture(&fixture);
	fixture.encoders[1].media = &second_token;
	for (size_t i = 0; i < sizeof(flags) / sizeof(flags[0]); i++) {
		fixture.output.info.flags = flags[i];
		for (unsigned raw = 0; raw < 2; raw++) {
			fixture.output.video = raw ? (video_t *)&raw_token : NULL;
			for (unsigned media = 0; media < 2; media++) {
				fixture.encoders[0].media = media ? &first_token : NULL;
				for (unsigned override = 0; override < 2; override++) {
					fixture.encoders[0].fps_override = override ? (video_t *)&override_token : NULL;
					check_borrowed(&fixture, override ? (video_t *)&override_token
									  : fixture.encoders[0].media);
				}
			}
		}
	}
	assert_int_equal(log_count, 0);
}

static void first_occupied_and_missing_encoder(void **state)
{
	(void)state;
	uint64_t raw_token = 0, second_token = 0;
	struct fixture fixture;
	init_fixture(&fixture);
	fixture.output.info.flags = OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO;
	fixture.output.video = (video_t *)&raw_token;
	fixture.encoders[1].media = &second_token;
	memset(fixture.output.video_encoders, 0, sizeof(fixture.output.video_encoders));
	/* "First" means first occupied slot, including gaps before the final slot. */
	for (size_t i = 1; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		fixture.output.video_encoders[i] = &fixture.encoders[1];
		check_borrowed(&fixture, (video_t *)&second_token);
		fixture.output.video_encoders[i] = NULL;
	}
	assert_int_equal(log_count, 0);
	/* No occupied encoder: preserve the downstream diagnostic, with no raw fallback. */
	struct fixture before;
	memcpy(&before, &fixture, sizeof(before));
	assert_null(obs_output_video(&fixture.output));
	expect_log(LOG_DEBUG, "obs_encoder_video: Null 'encoder' parameter");
	assert_memory_equal(&fixture, &before, sizeof(before));
}

static void wrong_encoder_type(void **state)
{
	(void)state;
	uint64_t raw_token = 0, encoder_token = 0;
	struct fixture fixture;
	init_fixture(&fixture);
	fixture.output.info.flags = OBS_OUTPUT_ENCODED;
	fixture.output.video = (video_t *)&raw_token;
	fixture.encoders[0].info.type = OBS_ENCODER_AUDIO;
	fixture.encoders[0].media = &encoder_token;
	struct fixture before;
	memcpy(&before, &fixture, sizeof(before));
	assert_null(obs_output_video(&fixture.output));
	/* Historical setter label comes from obs_encoder_video: characterized, not endorsed. */
	expect_log(LOG_WARNING, "obs_encoder_set_video: encoder 'video-%s' is not a video encoder");
	assert_memory_equal(&fixture, &before, sizeof(before));
}

int main(void)
{
	/* No startup, owned media, destruction, graphics, audio or threads. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_output, setup, teardown),
		cmocka_unit_test_setup_teardown(raw_video, setup, teardown),
		cmocka_unit_test_setup_teardown(encoded_video, setup, teardown),
		cmocka_unit_test_setup_teardown(first_occupied_and_missing_encoder, setup, teardown),
		cmocka_unit_test_setup_teardown(wrong_encoder_type, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
