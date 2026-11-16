/* Actual-library characterization of the borrowed core audio getter (#4807).
 * Only initialized-core calls are valid. No hardware input or frame waits. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void initialized_core_without_audio(void **state)
{
	(void)state;
	struct obs_audio_info info = {0};
	assert_false(obs_get_audio_info(&info));
	for (unsigned i = 0; i < 32; ++i)
		assert_null(obs_get_audio());
	assert_false(obs_get_audio_info(&info));
}

static void expect_current_output(uint32_t rate, enum speaker_layout speakers)
{
	audio_t *expected = obs_get_audio();
	assert_non_null(expected);
	assert_false(audio_output_active(expected));
	for (unsigned i = 0; i < 32; ++i) {
		audio_t *actual = obs_get_audio();
		assert_ptr_equal(actual, expected);
		assert_int_equal(audio_output_get_sample_rate(actual), rate);
		assert_int_equal(audio_output_get_info(actual)->speakers, speakers);
		assert_false(audio_output_active(actual));
	}
	struct obs_audio_info info = {0};
	assert_true(obs_get_audio_info(&info));
	assert_int_equal(info.samples_per_sec, rate);
	assert_int_equal(info.speakers, speakers);
}

static void borrowed_output_tracks_reset_and_disable(void **state)
{
	(void)state;
	const struct obs_audio_info stereo = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	const struct obs_audio_info mono = {.samples_per_sec = 44100, .speakers = SPEAKERS_MONO};
	assert_true(obs_reset_audio(&stereo));
	expect_current_output(48000, SPEAKERS_STEREO);
	assert_true(obs_reset_audio(&mono));
	/* Do not compare with the freed old pointer: the allocator may reuse it. */
	expect_current_output(44100, SPEAKERS_MONO);
	assert_true(obs_reset_audio2(NULL));
	for (unsigned i = 0; i < 32; ++i)
		assert_null(obs_get_audio());
	assert_true(obs_reset_audio(&stereo));
	expect_current_output(48000, SPEAKERS_STEREO);
	/* Borrowed outputs are closed by reset/shutdown, never by the reader. */
}

static void sink(void *param, size_t mix, struct audio_data *data)
{
	(void)param;
	(void)mix;
	(void)data;
}

static void active_output_is_unchanged(void **state)
{
	(void)state;
	const struct obs_audio_info info = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&info));
	audio_t *expected = obs_get_audio();
	assert_non_null(expected);
	obs_add_raw_audio_callback(0, NULL, sink, NULL);
	for (unsigned i = 0; i < 32; ++i) {
		assert_ptr_equal(obs_get_audio(), expected);
		assert_true(audio_output_active(expected));
	}
	/* Duplicate rejection proves the reader did not disconnect the sink. */
	assert_false(audio_output_connect(expected, 0, NULL, sink, NULL));
	obs_remove_raw_audio_callback(0, sink, NULL);
	assert_false(audio_output_active(expected));
	assert_ptr_equal(obs_get_audio(), expected);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(initialized_core_without_audio, setup, teardown),
		cmocka_unit_test_setup_teardown(borrowed_output_tracks_reset_and_disable, setup, teardown),
		cmocka_unit_test_setup_teardown(active_output_is_unchanged, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
