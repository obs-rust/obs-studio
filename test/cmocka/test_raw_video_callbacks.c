/* Characterizes obs_add_raw_video_callback, obs_add_raw_video_callback2 and
 * obs_remove_raw_video_callback against the actual libobs before their Rust
 * swap (#5507). The video counterpart of test_raw_audio_add.
 *
 * An initialized core is a precondition. The main canvas mix is swapped for a
 * zeroed stand-in whose video is an output this test opens, so no graphics or
 * video reset is needed. Registration is observed synchronously through the
 * output's own bookkeeping: a duplicate video_output_connect is refused and a
 * video_output_disconnect2 of a missing pair reports false, neither changing
 * anything. No frame is ever pushed, so no callback runs (#4680).
 *
 * The stand-in mix is not in the core's mix list, so its raw_active count
 * stays at zero (start_raw_video counts only listed mixes). The count on a
 * real mix is pinned by test_video_active, and the calls before the first
 * obs_reset_video by test_main_mix_before_video; both need a display. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#define WIDTH 32
#define HEIGHT 16

static int parameter_a;
static int parameter_b;
static video_t *main_video;
static video_t *other_video;
static struct obs_core_video_mix stand_in;

static void callback_a(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
}

static void callback_b(void *param, struct video_data *frame)
{
	/* Distinct local volatile operations keep MSVC /OPT:ICF from folding
	 * the callback addresses together. There is no shared counter to race. */
	volatile unsigned marker = 1;
	(void)marker;
	(void)param;
	(void)frame;
}

/* The core keeps its own reference, so the borrowed pointer stays valid. */
static obs_canvas_t *main_canvas(void)
{
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	obs_canvas_release(canvas);
	return canvas;
}

static video_t *open_video(const char *name)
{
	struct video_output_info info = {
		.name = name,
		.format = VIDEO_FORMAT_I420,
		.fps_num = 30,
		.fps_den = 1,
		.width = WIDTH,
		.height = HEIGHT,
		.cache_size = 4,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
	};
	video_t *video = NULL;
	assert_int_equal(video_output_open(&video, &info), VIDEO_OUTPUT_SUCCESS);
	assert_non_null(video);
	assert_false(video_output_active(video));
	return video;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	main_video = open_video("raw-video-callbacks-main");
	other_video = open_video("raw-video-callbacks-other");
	obs_canvas_t *canvas = main_canvas();
	assert_null(canvas->mix);
	memset(&stand_in, 0, sizeof(stand_in));
	stand_in.video = main_video;
	canvas->mix = &stand_in;
	assert_ptr_equal(obs_get_video(), main_video);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized()) {
		/* The stand-in is not the core's to free. */
		main_canvas()->mix = NULL;
		obs_shutdown();
	}
	video_output_close(main_video);
	video_output_close(other_video);
	main_video = NULL;
	other_video = NULL;
	return 0;
}

static void present(video_t *video, void (*callback)(void *, struct video_data *), void *param)
{
	/* A duplicate is refused without changing the registration. */
	assert_false(video_output_connect(video, NULL, callback, param));
}

static void absent(video_t *video, void (*callback)(void *, struct video_data *), void *param)
{
	/* Removing a missing pair reports false and changes nothing. */
	assert_false(video_output_disconnect2(video, callback, param));
}

static void assert_uncounted(void)
{
	assert_int_equal(os_atomic_load_long(&stand_in.raw_active), 0);
	assert_false(obs_video_active());
}

static void add_registers_on_main_mix_video(void **state)
{
	(void)state;
	absent(main_video, callback_a, &parameter_a);
	obs_add_raw_video_callback(NULL, callback_a, &parameter_a);
	present(main_video, callback_a, &parameter_a);
	assert_true(video_output_active(main_video));
	/* An output the core does not use is left alone. */
	absent(other_video, callback_a, &parameter_a);
	assert_false(video_output_active(other_video));
	assert_uncounted();

	obs_remove_raw_video_callback(callback_a, &parameter_a);
	absent(main_video, callback_a, &parameter_a);
	assert_false(video_output_active(main_video));
	assert_uncounted();
}

static void add2_registers_with_any_nonzero_divisor(void **state)
{
	(void)state;
	obs_add_raw_video_callback2(NULL, 1, callback_a, &parameter_a);
	obs_add_raw_video_callback2(NULL, 2, callback_a, &parameter_b);
	obs_add_raw_video_callback2(NULL, UINT32_MAX, callback_b, &parameter_a);
	present(main_video, callback_a, &parameter_a);
	present(main_video, callback_a, &parameter_b);
	present(main_video, callback_b, &parameter_a);
	assert_uncounted();

	obs_remove_raw_video_callback(callback_a, &parameter_a);
	obs_remove_raw_video_callback(callback_a, &parameter_b);
	obs_remove_raw_video_callback(callback_b, &parameter_a);
	absent(main_video, callback_a, &parameter_a);
	absent(main_video, callback_a, &parameter_b);
	absent(main_video, callback_b, &parameter_a);
	assert_false(video_output_active(main_video));
}

static void zero_divisor_and_null_callback_register_nothing(void **state)
{
	(void)state;
	obs_add_raw_video_callback2(NULL, 0, callback_a, &parameter_a);
	absent(main_video, callback_a, &parameter_a);
	obs_add_raw_video_callback(NULL, NULL, &parameter_a);
	obs_add_raw_video_callback2(NULL, 1, NULL, NULL);
	absent(main_video, NULL, &parameter_a);
	assert_false(video_output_active(main_video));
	/* Removing a NULL callback is harmless too. */
	obs_remove_raw_video_callback(NULL, NULL);
	assert_false(video_output_active(main_video));
	assert_uncounted();
}

static void repeated_add_is_one_registration(void **state)
{
	(void)state;
	obs_add_raw_video_callback(NULL, callback_a, &parameter_a);
	obs_add_raw_video_callback(NULL, callback_a, &parameter_a);
	/* A later add2 with another divisor is still the same pair. */
	obs_add_raw_video_callback2(NULL, 3, callback_a, &parameter_a);
	present(main_video, callback_a, &parameter_a);

	/* One removal takes it out completely: the repeats kept no copy. */
	obs_remove_raw_video_callback(callback_a, &parameter_a);
	absent(main_video, callback_a, &parameter_a);
	assert_false(video_output_active(main_video));
	assert_uncounted();
}

static void callback_and_parameter_identity(void **state)
{
	(void)state;
	obs_add_raw_video_callback(NULL, callback_a, &parameter_a);
	obs_add_raw_video_callback(NULL, callback_a, &parameter_b);
	obs_add_raw_video_callback(NULL, callback_b, &parameter_a);
	obs_add_raw_video_callback(NULL, callback_b, NULL);
	present(main_video, callback_a, &parameter_a);
	present(main_video, callback_a, &parameter_b);
	present(main_video, callback_b, &parameter_a);
	present(main_video, callback_b, NULL);
	absent(main_video, callback_b, &parameter_b);
	absent(main_video, callback_a, NULL);

	/* A removal takes out only its own pair. */
	obs_remove_raw_video_callback(callback_a, &parameter_b);
	absent(main_video, callback_a, &parameter_b);
	present(main_video, callback_a, &parameter_a);
	present(main_video, callback_b, &parameter_a);
	present(main_video, callback_b, NULL);

	/* Removing a pair that was never added changes nothing. */
	obs_remove_raw_video_callback(callback_b, &parameter_b);
	obs_remove_raw_video_callback(callback_a, NULL);
	present(main_video, callback_a, &parameter_a);
	present(main_video, callback_b, &parameter_a);
	present(main_video, callback_b, NULL);

	obs_remove_raw_video_callback(callback_a, &parameter_a);
	obs_remove_raw_video_callback(callback_b, &parameter_a);
	assert_true(video_output_active(main_video));
	obs_remove_raw_video_callback(callback_b, NULL);
	assert_false(video_output_active(main_video));
	assert_uncounted();
}

static void conversion_is_accepted(void **state)
{
	(void)state;
	/* Zero sizes take the output's own, so no scaler is needed. */
	const struct video_scale_info conversion = {
		.format = VIDEO_FORMAT_I420,
		.range = VIDEO_RANGE_PARTIAL,
		.colorspace = VIDEO_CS_709,
	};
	obs_add_raw_video_callback(&conversion, callback_b, &parameter_b);
	present(main_video, callback_b, &parameter_b);
	obs_add_raw_video_callback2(&conversion, 2, callback_a, &parameter_b);
	present(main_video, callback_a, &parameter_b);
	obs_remove_raw_video_callback(callback_b, &parameter_b);
	obs_remove_raw_video_callback(callback_a, &parameter_b);
	absent(main_video, callback_b, &parameter_b);
	absent(main_video, callback_a, &parameter_b);
	assert_false(video_output_active(main_video));
}

static void follows_the_current_main_mix(void **state)
{
	(void)state;
	obs_canvas_t *canvas = main_canvas();
	obs_add_raw_video_callback(NULL, callback_a, &parameter_a);

	/* After the main mix changes, adds and removals go to the new video. */
	struct obs_core_video_mix replacement;
	memset(&replacement, 0, sizeof(replacement));
	replacement.video = other_video;
	canvas->mix = &replacement;
	obs_add_raw_video_callback(NULL, callback_b, &parameter_b);
	present(other_video, callback_b, &parameter_b);
	absent(main_video, callback_b, &parameter_b);
	/* The removal looks only at the current video. */
	obs_remove_raw_video_callback(callback_a, &parameter_a);
	present(main_video, callback_a, &parameter_a);
	obs_remove_raw_video_callback(callback_b, &parameter_b);
	absent(other_video, callback_b, &parameter_b);
	assert_int_equal(os_atomic_load_long(&replacement.raw_active), 0);
	/* A live mix always has a video: the C video-io walks a NULL video's
	 * parent before its NULL check, so that case is not exercised. */

	canvas->mix = &stand_in;
	obs_remove_raw_video_callback(callback_a, &parameter_a);
	absent(main_video, callback_a, &parameter_a);
	assert_false(video_output_active(main_video));
	assert_false(video_output_active(other_video));
	assert_uncounted();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(add_registers_on_main_mix_video, setup, teardown),
		cmocka_unit_test_setup_teardown(add2_registers_with_any_nonzero_divisor, setup, teardown),
		cmocka_unit_test_setup_teardown(zero_divisor_and_null_callback_register_nothing, setup, teardown),
		cmocka_unit_test_setup_teardown(repeated_add_is_one_registration, setup, teardown),
		cmocka_unit_test_setup_teardown(callback_and_parameter_identity, setup, teardown),
		cmocka_unit_test_setup_teardown(conversion_is_accepted, setup, teardown),
		cmocka_unit_test_setup_teardown(follows_the_current_main_mix, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
