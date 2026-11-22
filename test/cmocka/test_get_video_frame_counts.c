/* C-only characterization before extracting obs_get_video_frame_counts
 * (Refs #5443).
 *
 * obs_get_video_frame_counts() sums the total and skipped frame counters of
 * the video_t of the main canvas's mix and of every encoder-only mix of the
 * same view (a GPU-rescaling encoder's mix, maybe_set_up_gpu_rescale() in
 * obs-encoder.c). Mixes of other views do not count. With no main mix
 * (before the first obs_reset_video()) both outputs are 0. Either output may
 * be NULL. It reads the mix list under mixes_mutex and changes nothing.
 *
 * The counters are hidden in video-io, so the frames are real: a slow raw
 * video callback on the main video makes the main video_t count total and
 * skipped frames, and a slow raw encoder does the same on an encoder-only
 * mix or an auxiliary canvas. Once the main mix's callback is gone its
 * counters stop (nothing posts its video thread any more), so the cases
 * compare against fixed main counts; a running encoder-only mix is
 * bracketed by reads of its counters before and after each call.
 *
 * obs_startup() and obs_reset_video() need a display and OpenGL (run under
 * xvfb-run); without them the cases skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include "obs-internal.h"

#define VENC_ID "test_get_video_frame_counts_venc"
#define OUTPUT_ID "test_get_video_frame_counts_output"
#define CANVAS_SIZE 64
#define SCALED_SIZE 32
#define FPS 30
#define SLOW_MS 100
#define TIMEOUT_MS 10000
/* Frame periods a stopped counter must stay unchanged to count as stopped. */
#define SETTLE_MS 500
#define REPEATS 16
#define SENTINEL 0xdeadbeefu

/* Set before stopping: a lagging consumer keeps the video thread busy. */
static volatile bool consumers_fast;

#define REQUIRE(cond, why)                                       \
	do {                                                     \
		if (!(cond)) {                                   \
			fprintf(stderr, "skipped: %s\n", (why)); \
			skip();                                  \
		}                                                \
	} while (false)

/* ------------------------------------------------------------------------ */
/* A raw video encoder that takes SLOW_MS per frame and a video-only output  */

static const char *test_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void enc_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(frame);
	UNUSED_PARAMETER(packet);
	if (!os_atomic_load_bool(&consumers_fast))
		os_sleep_ms(SLOW_MS);
	*received = false;
	return true;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	return output;
}

static void output_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool output_start(void *data)
{
	obs_output_t *output = data;

	return obs_output_can_begin_data_capture(output, 0) && obs_output_initialize_encoders(output, 0) &&
	       obs_output_begin_data_capture(output, 0);
}

static void output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);
	obs_output_end_data_capture(data);
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(packet);
}

static void register_types(void)
{
	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = test_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	obs_register_encoder(&venc);

	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.encoded_video_codecs = "h264",
		.get_name = test_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);
}

/* ------------------------------------------------------------------------ */
/* Core lifecycle                                                            */

static struct obs_video_info video_info(void)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = FPS,
		.fps_den = 1,
		.base_width = CANVAS_SIZE,
		.base_height = CANVAS_SIZE,
		.output_width = CANVAS_SIZE,
		.output_height = CANVAS_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return ovi;
}

static int startup_only(void **state)
{
	*state = (void *)(uintptr_t)obs_startup("en-US", NULL, NULL);
	return 0;
}

static int startup_with_video(void **state)
{
	*state = NULL;
	if (!obs_startup("en-US", NULL, NULL))
		return 0;
	struct obs_video_info ovi = video_info();
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}
	register_types();
	*state = (void *)(uintptr_t)1;
	return 0;
}

static int shutdown_core(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* The core keeps its own reference, so the borrowed pointer stays valid. */
static obs_canvas_t *main_canvas(void)
{
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	obs_canvas_release(canvas);
	return canvas;
}

/* ------------------------------------------------------------------------ */
/* Counters                                                                  */

struct counts {
	uint32_t total;
	uint32_t skipped;
};

static struct counts video_counts(const video_t *video)
{
	struct counts c = {
		.total = video_output_get_total_frames(video),
		.skipped = video_output_get_skipped_frames(video),
	};
	return c;
}

static struct counts add_counts(struct counts a, struct counts b)
{
	struct counts c = {a.total + b.total, a.skipped + b.skipped};
	return c;
}

static struct counts frame_counts(void)
{
	struct counts c = {SENTINEL, SENTINEL};
	obs_get_video_frame_counts(&c.total, &c.skipped);
	return c;
}

static void assert_counts_equal(struct counts a, struct counts b)
{
	assert_int_equal(a.total, b.total);
	assert_int_equal(a.skipped, b.skipped);
}

/* Waits until a video_t has counted both total and skipped frames. */
static bool wait_skipping(const video_t *video)
{
	for (int i = 0; i < TIMEOUT_MS / 10; i++) {
		struct counts c = video_counts(video);
		if (c.total > 0 && c.skipped > 0)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* Waits until a video_t's counters have stopped (no consumer left). */
static struct counts wait_stopped(const video_t *video)
{
	struct counts last = video_counts(video);
	int still_ms = 0;
	for (int i = 0; i < TIMEOUT_MS / 10 && still_ms < SETTLE_MS; i++) {
		os_sleep_ms(10);
		struct counts now = video_counts(video);
		if (now.total == last.total && now.skipped == last.skipped) {
			still_ms += 10;
		} else {
			still_ms = 0;
			last = now;
		}
	}
	assert_true(still_ms >= SETTLE_MS);
	return last;
}

static void slow_raw_cb(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(frame);
	if (!os_atomic_load_bool(&consumers_fast))
		os_sleep_ms(SLOW_MS);
}

/* Makes the main mix's video_t count total and skipped frames, then removes
 * its consumer and returns the counts once they have stopped. */
static struct counts drive_main_mix(void)
{
	video_t *main_video = obs_get_video();
	assert_non_null(main_video);

	os_atomic_set_bool(&consumers_fast, false);
	obs_add_raw_video_callback(NULL, slow_raw_cb, NULL);
	bool skipping = wait_skipping(main_video);
	os_atomic_set_bool(&consumers_fast, true);
	obs_remove_raw_video_callback(slow_raw_cb, NULL);
	assert_true(skipping);

	struct counts main = wait_stopped(main_video);
	assert_true(main.total > 0);
	assert_true(main.skipped > 0);
	return main;
}

/* Asks repeatedly while nothing counts: every answer is the same and the
 * counters of the given video_t (and the main mix) do not move. */
static void assert_frame_counts_are(struct counts expected, const video_t *other)
{
	obs_canvas_t *canvas = main_canvas();
	struct obs_core_video_mix *mix = canvas->mix;
	struct counts main_before = video_counts(obs_get_video());
	struct counts other_before = other ? video_counts(other) : main_before;

	for (unsigned i = 0; i < REPEATS; ++i)
		assert_counts_equal(frame_counts(), expected);

	assert_ptr_equal(canvas->mix, mix);
	assert_counts_equal(video_counts(obs_get_video()), main_before);
	if (other)
		assert_counts_equal(video_counts(other), other_before);
}

struct run {
	obs_encoder_t *encoder;
	obs_output_t *output;
};

static void run_start(struct run *run, video_t *video, bool rescale, const char *name)
{
	os_atomic_set_bool(&consumers_fast, false);
	run->encoder = obs_video_encoder_create(VENC_ID, name, NULL, NULL);
	assert_non_null(run->encoder);
	if (rescale) {
		obs_encoder_set_gpu_scale_type(run->encoder, OBS_SCALE_BICUBIC);
		obs_encoder_set_scaled_size(run->encoder, SCALED_SIZE, SCALED_SIZE);
	}
	obs_encoder_set_video(run->encoder, video);

	run->output = obs_output_create(OUTPUT_ID, name, NULL, NULL);
	assert_non_null(run->output);
	obs_output_set_video_encoder(run->output, run->encoder);
	if (!obs_output_start(run->output)) {
		fprintf(stderr, "the test output did not start: %s\n", obs_output_get_last_error(run->output));
		fail();
	}
}

static void run_stop(struct run *run)
{
	os_atomic_set_bool(&consumers_fast, true);
	obs_output_force_stop(run->output);
	obs_output_release(run->output);
	obs_encoder_release(run->encoder);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

/* No main mix yet: both outputs are written as 0, either may be NULL. */
static void zero_before_video_reset(void **state)
{
	REQUIRE(*state, "obs_startup failed (no display?)");
	obs_canvas_t *canvas = main_canvas();
	assert_null(canvas->mix);

	for (unsigned i = 0; i < REPEATS; ++i)
		assert_counts_equal(frame_counts(), (struct counts){0, 0});

	uint32_t total = SENTINEL;
	uint32_t skipped = SENTINEL;
	obs_get_video_frame_counts(&total, NULL);
	assert_int_equal(total, 0);
	obs_get_video_frame_counts(NULL, &skipped);
	assert_int_equal(skipped, 0);
	obs_get_video_frame_counts(NULL, NULL);
	assert_null(canvas->mix);
}

/* After a reset the main mix's video_t counts are reported. */
static void main_mix_counts(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	assert_non_null(main_canvas()->mix);

	/* An idle main mix has counted nothing. */
	assert_frame_counts_are((struct counts){0, 0}, NULL);

	struct counts main = drive_main_mix();
	assert_counts_equal(video_counts(obs_get_video()), main);
	assert_frame_counts_are(main, NULL);
}

/* A NULL output is skipped and the other one is still written. */
static void null_output_accepted(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct counts main = drive_main_mix();

	for (unsigned i = 0; i < REPEATS; ++i) {
		uint32_t total = SENTINEL;
		uint32_t skipped = SENTINEL;
		obs_get_video_frame_counts(&total, NULL);
		assert_int_equal(total, main.total);
		obs_get_video_frame_counts(NULL, &skipped);
		assert_int_equal(skipped, main.skipped);
		obs_get_video_frame_counts(NULL, NULL);
	}
	assert_frame_counts_are(main, NULL);
}

/* A GPU-rescaling encoder on the main video gets an encoder-only mix of the
 * main view; its video_t's counts are added to the main mix's. */
static void encoder_only_mix_of_main_view_adds(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct obs_core_video_mix *main_mix = main_canvas()->mix;
	struct counts main = drive_main_mix();

	struct run run;
	run_start(&run, obs_get_video(), true, "rescaled");
	video_t *rescaled = obs_encoder_video(run.encoder);
	assert_non_null(rescaled);
	assert_ptr_not_equal(rescaled, obs_get_video());
	bool skipping = wait_skipping(rescaled);

	/* The rescaled mix keeps counting: bracket each answer by its
	 * counters read before and after the call. */
	bool bracketed = true;
	for (unsigned i = 0; skipping && i < REPEATS; ++i) {
		struct counts lo = add_counts(main, video_counts(rescaled));
		struct counts got = frame_counts();
		struct counts hi = add_counts(main, video_counts(rescaled));
		if (got.total < lo.total || got.total > hi.total || got.skipped < lo.skipped ||
		    got.skipped > hi.skipped) {
			fprintf(stderr, "got %u/%u, expected %u..%u total and %u..%u skipped\n", got.total, got.skipped,
				lo.total, hi.total, lo.skipped, hi.skipped);
			bracketed = false;
		}
	}
	struct counts main_during = video_counts(obs_get_video());
	struct obs_core_video_mix *main_mix_during = main_canvas()->mix;
	run_stop(&run);

	assert_true(skipping);
	assert_true(bracketed);
	assert_counts_equal(main_during, main);
	assert_ptr_equal(main_mix_during, main_mix);

	/* Its last encoder stopped, the encoder-only mix is gone: only the
	 * main mix counts again. */
	struct counts after = {0, 0};
	for (int i = 0; i < TIMEOUT_MS / 10; i++) {
		after = frame_counts();
		if (after.total == main.total && after.skipped == main.skipped)
			break;
		os_sleep_ms(10);
	}
	assert_counts_equal(after, main);
	assert_frame_counts_are(main, NULL);
}

/* An auxiliary canvas's mix, and an encoder-only mix of its view, do not
 * count, while they count frames of their own. */
static void other_view_does_not_count(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct counts main = drive_main_mix();

	struct obs_video_info ovi = video_info();
	obs_canvas_t *aux = obs_canvas_create_private("frame counts aux", &ovi, 0);
	assert_non_null(aux);
	assert_non_null(aux->mix);
	assert_ptr_not_equal(aux->mix, main_canvas()->mix);
	video_t *aux_video = obs_canvas_get_video(aux);
	assert_non_null(aux_video);

	struct run plain, rescaled;
	run_start(&plain, aux_video, false, "aux plain");
	run_start(&rescaled, aux_video, true, "aux rescaled");
	video_t *aux_rescaled = obs_encoder_video(rescaled.encoder);
	assert_non_null(aux_rescaled);
	bool skipping = wait_skipping(aux_video) && wait_skipping(aux_rescaled);

	struct counts got[REPEATS];
	for (unsigned i = 0; i < REPEATS; ++i)
		got[i] = frame_counts();
	struct counts aux_counts = video_counts(aux_video);
	struct counts aux_rescaled_counts = video_counts(aux_rescaled);

	run_stop(&rescaled);
	run_stop(&plain);
	obs_canvas_release(aux);
	obs_wait_for_destroy_queue();

	assert_ptr_not_equal(aux_rescaled, aux_video);
	assert_ptr_not_equal(aux_rescaled, obs_get_video());
	assert_true(skipping);
	assert_true(aux_counts.total > 0);
	assert_true(aux_rescaled_counts.total > 0);
	for (unsigned i = 0; i < REPEATS; ++i)
		assert_counts_equal(got[i], main);
	assert_frame_counts_are(main, NULL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(zero_before_video_reset, startup_only, shutdown_core),
		cmocka_unit_test_setup_teardown(main_mix_counts, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(null_output_accepted, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(encoder_only_mix_of_main_view_adds, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(other_view_does_not_count, startup_with_video, shutdown_core),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
