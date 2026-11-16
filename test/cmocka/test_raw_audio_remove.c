/* Characterizes obs_remove_raw_audio_callback against the actual libobs.
 * Registration probes use connect's synchronous duplicate rejection under its
 * input mutex. No callback counts, frame waits or source destruction races
 * (#4680): the callbacks deliberately touch no shared state on the audio thread.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include <obs.h>

static int parameter_a;
static int parameter_b;
static int unknown_parameter;
static audio_t *independent_audio;

static void callback_a(void *param, size_t mix, struct audio_data *data)
{
	(void)param;
	(void)mix;
	(void)data;
}

static void callback_b(void *param, size_t mix, struct audio_data *data)
{
	/* Distinct local volatile operations keep MSVC /OPT:ICF from folding
	 * the callback addresses together. There is no shared counter to race. */
	volatile unsigned marker = 1;
	(void)marker;
	(void)param;
	(void)mix;
	(void)data;
}

static void callback_unknown(void *param, size_t mix, struct audio_data *data)
{
	volatile unsigned marker = 2;
	(void)marker;
	(void)param;
	(void)mix;
	(void)data;
}

static bool no_input(void *param, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
		     struct audio_output_data *data)
{
	(void)param;
	(void)start;
	(void)end;
	(void)timestamp;
	(void)mixers;
	(void)data;
	return false;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	independent_audio = NULL;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	audio_output_close(independent_audio);
	independent_audio = NULL;
	obs_shutdown();
	return 0;
}

static audio_t *open_main_audio(void)
{
	const struct obs_audio_info info = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&info));
	audio_t *audio = obs_get_audio();
	assert_non_null(audio);
	assert_false(audio_output_active(audio));
	return audio;
}

static void present(audio_t *audio, size_t mix, audio_output_callback_t callback, void *param)
{
	/* A duplicate is refused without changing the registration. */
	assert_false(audio_output_connect(audio, mix, NULL, callback, param));
}

static void absent(audio_t *audio, size_t mix, audio_output_callback_t callback, void *param)
{
	/* A missing tuple can be reinserted; remove that probe through the direct
	 * API so the wrapper cannot conceal its own failure in the cleanup. */
	assert_true(audio_output_connect(audio, mix, NULL, callback, param));
	audio_output_disconnect(audio, mix, callback, param);
}

static void initialized_core_without_audio(void **state)
{
	(void)state;
	assert_null(obs_get_audio());
	obs_remove_raw_audio_callback(0, callback_a, &parameter_a);
	obs_remove_raw_audio_callback(MAX_AUDIO_MIXES, callback_b, NULL);
	obs_remove_raw_audio_callback(SIZE_MAX, NULL, NULL);
	assert_null(obs_get_audio());
}

static void matching_mix_and_other_mixes(void **state)
{
	(void)state;
	audio_t *audio = open_main_audio();
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		assert_true(audio_output_connect(audio, mix, NULL, callback_a, &parameter_a));

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		obs_remove_raw_audio_callback(mix, callback_a, &parameter_a);
		absent(audio, mix, callback_a, &parameter_a);
		/* Repeated removal is a no-op; the other mixes retain this tuple. */
		obs_remove_raw_audio_callback(mix, callback_a, &parameter_a);
		for (size_t remaining = mix + 1; remaining < MAX_AUDIO_MIXES; remaining++)
			present(audio, remaining, callback_a, &parameter_a);
		assert_int_equal(audio_output_active(audio), mix + 1 < MAX_AUDIO_MIXES);
	}
}

static void callback_and_parameter_identity(void **state)
{
	(void)state;
	audio_t *audio = open_main_audio();
	assert_true(audio_output_connect(audio, 0, NULL, callback_a, &parameter_a));
	assert_true(audio_output_connect(audio, 0, NULL, callback_a, &parameter_b));
	assert_true(audio_output_connect(audio, 0, NULL, callback_b, &parameter_a));
	assert_true(audio_output_connect(audio, 0, NULL, callback_b, NULL));

	obs_remove_raw_audio_callback(0, callback_b, &parameter_b);
	obs_remove_raw_audio_callback(0, callback_a, &unknown_parameter);
	obs_remove_raw_audio_callback(0, callback_unknown, &parameter_a);
	obs_remove_raw_audio_callback(0, NULL, &parameter_a);
	obs_remove_raw_audio_callback(1, callback_a, &parameter_a);
	present(audio, 0, callback_a, &parameter_a);
	present(audio, 0, callback_a, &parameter_b);
	present(audio, 0, callback_b, &parameter_a);
	present(audio, 0, callback_b, NULL);

	/* Remove a middle entry, then the first, then the last; compaction must
	 * preserve the other callback/parameter combinations, including NULL. */
	obs_remove_raw_audio_callback(0, callback_a, &parameter_b);
	absent(audio, 0, callback_a, &parameter_b);
	present(audio, 0, callback_a, &parameter_a);
	present(audio, 0, callback_b, &parameter_a);
	present(audio, 0, callback_b, NULL);
	obs_remove_raw_audio_callback(0, callback_a, &parameter_a);
	absent(audio, 0, callback_a, &parameter_a);
	present(audio, 0, callback_b, &parameter_a);
	present(audio, 0, callback_b, NULL);
	obs_remove_raw_audio_callback(0, callback_b, NULL);
	absent(audio, 0, callback_b, NULL);
	present(audio, 0, callback_b, &parameter_a);
	assert_true(audio_output_active(audio));
	obs_remove_raw_audio_callback(0, callback_b, &parameter_a);
	absent(audio, 0, callback_b, &parameter_a);
	assert_false(audio_output_active(audio));
}

static void invalid_full_size_mix(void **state)
{
	(void)state;
	audio_t *audio = open_main_audio();
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		assert_true(audio_output_connect(audio, mix, NULL, callback_a, &parameter_a));

	obs_remove_raw_audio_callback(MAX_AUDIO_MIXES, callback_a, &parameter_a);
	obs_remove_raw_audio_callback(MAX_AUDIO_MIXES + 1, callback_a, &parameter_a);
	obs_remove_raw_audio_callback(SIZE_MAX, callback_a, &parameter_a);
	/* Increment a size_t variable rather than truncating a constant cast
	 * (MSVC C4310). On wide hosts these indices would alias mixes 0 and 1
	 * if the wrapper narrowed its argument to uint32_t. */
	if (SIZE_MAX > UINT32_MAX) {
		size_t wide_mix = UINT32_MAX;
		wide_mix++;
		obs_remove_raw_audio_callback(wide_mix, callback_a, &parameter_a);
		wide_mix++;
		obs_remove_raw_audio_callback(wide_mix, callback_a, &parameter_a);
	}
	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
		present(audio, mix, callback_a, &parameter_a);
}

static void delegates_to_main_audio(void **state)
{
	(void)state;
	audio_t *audio = open_main_audio();
	struct audio_output_info info = {
		.name = "independent-raw-audio-remove",
		.samples_per_sec = 48000,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.speakers = SPEAKERS_STEREO,
		.input_callback = no_input,
	};
	assert_int_equal(audio_output_open(&independent_audio, &info), AUDIO_OUTPUT_SUCCESS);
	assert_ptr_not_equal(audio, independent_audio);
	assert_true(audio_output_connect(independent_audio, 0, NULL, callback_a, &parameter_a));

	/* Legacy registration and direct main-audio registration share exactly
	 * the path the removal wrapper targets; an independent output is intact. */
	obs_add_raw_audio_callback(0, NULL, callback_a, &parameter_a);
	present(audio, 0, callback_a, &parameter_a);
	obs_remove_raw_audio_callback(0, callback_a, &parameter_a);
	absent(audio, 0, callback_a, &parameter_a);
	assert_false(audio_output_active(audio));
	present(independent_audio, 0, callback_a, &parameter_a);
	assert_true(audio_output_active(independent_audio));

	assert_true(audio_output_connect(audio, MAX_AUDIO_MIXES - 1, NULL, callback_b, &parameter_b));
	obs_remove_raw_audio_callback(MAX_AUDIO_MIXES - 1, callback_b, &parameter_b);
	absent(audio, MAX_AUDIO_MIXES - 1, callback_b, &parameter_b);
	assert_false(audio_output_active(audio));
	present(independent_audio, 0, callback_a, &parameter_a);
	assert_ptr_equal(obs_get_audio(), audio);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(initialized_core_without_audio, setup, teardown),
		cmocka_unit_test_setup_teardown(matching_mix_and_other_mixes, setup, teardown),
		cmocka_unit_test_setup_teardown(callback_and_parameter_identity, setup, teardown),
		cmocka_unit_test_setup_teardown(invalid_full_size_mix, setup, teardown),
		cmocka_unit_test_setup_teardown(delegates_to_main_audio, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
