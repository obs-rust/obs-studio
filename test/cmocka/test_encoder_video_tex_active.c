/* C-source characterization of obs_encoder_video_tex_active (#5212).
 * Compile the current production body with a recording mix lookup and real
 * private layouts. This does not characterize GPU support or replace the
 * unchanged shared-library checks in test_encoder_tex_active_null.c. A later
 * Rust swap must relink this fixture through real private field adapters. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "obs-internal.h"

static struct obs_encoder encoder;
static struct obs_core_video_mix mix;
static unsigned char video_token;
static video_t *expected_media;
static struct obs_core_video_mix *lookup_result;
static unsigned lookup_calls;
static bool nv12_after_lookup, p010_after_lookup;

struct obs_core_video_mix *get_mix_for_video(video_t *video)
{
	assert_ptr_equal(video, expected_media);
	lookup_calls++;
	/* The query must read flags after the lookup returns its selected mix. */
	mix.using_nv12_tex = nv12_after_lookup;
	mix.using_p010_tex = p010_after_lookup;
	return lookup_result;
}

static void check_query(enum video_format format, bool expected)
{
	unsigned char encoder_before[sizeof(encoder)], mix_before[sizeof(mix)];
	mix.using_nv12_tex = nv12_after_lookup;
	mix.using_p010_tex = p010_after_lookup;
	memcpy(encoder_before, &encoder, sizeof(encoder));
	memcpy(mix_before, &mix, sizeof(mix));
	lookup_calls = 0;
	assert_int_equal(obs_encoder_video_tex_active(&encoder, format), expected);
	assert_int_equal(lookup_calls, 1);
	assert_memory_equal(&encoder, encoder_before, sizeof(encoder));
	assert_memory_equal(&mix, mix_before, sizeof(mix));
}

static int setup(void **state)
{
	(void)state;
	memset(&encoder, 0, sizeof(encoder));
	memset(&mix, 0, sizeof(mix));
	expected_media = (video_t *)&video_token;
	encoder.media = expected_media;
	lookup_result = &mix;
	nv12_after_lookup = p010_after_lookup = false;
	lookup_calls = 0;
	return 0;
}

static void missing_mix(void **state)
{
	(void)state;
	lookup_result = NULL;
	nv12_after_lookup = p010_after_lookup = true;
	check_query(VIDEO_FORMAT_NV12, false);
	check_query(VIDEO_FORMAT_P010, false);
	check_query(VIDEO_FORMAT_RGBA, false);
	/* A live encoder with no video still performs the exact NULL lookup. */
	encoder.media = expected_media = NULL;
	check_query(VIDEO_FORMAT_NV12, false);
	check_query(VIDEO_FORMAT_P010, false);
	check_query(VIDEO_FORMAT_NONE, false);
}

static void format_and_flag_matrix(void **state)
{
	(void)state;
	for (unsigned flags = 0; flags < 4; flags++) {
		nv12_after_lookup = (flags & 1) != 0;
		p010_after_lookup = (flags & 2) != 0;
		/* The real enum is contiguous, NONE through R10L; no host defaults. */
		for (enum video_format format = VIDEO_FORMAT_NONE; format <= VIDEO_FORMAT_R10L; format++) {
			bool expected = (format == VIDEO_FORMAT_NV12 && nv12_after_lookup) ||
					(format == VIDEO_FORMAT_P010 && p010_after_lookup);
			check_query(format, expected);
		}
	}
}

static void reads_selected_mix_after_lookup(void **state)
{
	(void)state;
	for (unsigned flags = 0; flags < 4; flags++) {
		nv12_after_lookup = (flags & 1) != 0;
		p010_after_lookup = (flags & 2) != 0;
		mix.using_nv12_tex = !nv12_after_lookup;
		mix.using_p010_tex = !p010_after_lookup;
		lookup_calls = 0;
		assert_int_equal(obs_encoder_video_tex_active(&encoder, VIDEO_FORMAT_NV12), nv12_after_lookup);
		assert_int_equal(lookup_calls, 1);
		mix.using_nv12_tex = !nv12_after_lookup;
		mix.using_p010_tex = !p010_after_lookup;
		lookup_calls = 0;
		assert_int_equal(obs_encoder_video_tex_active(&encoder, VIDEO_FORMAT_P010), p010_after_lookup);
		assert_int_equal(lookup_calls, 1);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup(missing_mix, setup),
					   cmocka_unit_test_setup(format_and_flag_matrix, setup),
					   cmocka_unit_test_setup(reads_selected_mix_after_lookup, setup)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
