/* C-only characterization before extracting obs_video_active (Refs #5434).
 *
 * obs_video_active() answers whether any video mix in the core's mix list has
 * a raw consumer (raw_active) or a GPU encoder (gpu_encoder_active) counted
 * above zero. It reads the counts under mixes_mutex and changes nothing.
 *
 * Activity comes from public consumers wherever one exists: a raw video
 * callback on the main mix, a raw (CPU) encoder on an auxiliary canvas's
 * video, and a texture encoder on the main mix (skipped when the mix has no
 * texture path). The mix fields are read white-box (obs-internal.h) to show
 * which count made the answer. Only the "above zero" boundary is set by hand,
 * with a negative raw count: the graphics thread also treats a count that is
 * not above zero as inactive, so that leaves it alone.
 *
 * obs_startup() and obs_reset_video() need a display and OpenGL (run under
 * xvfb-run); without them the cases skip. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include "obs-internal.h"

#define RAW_VENC_ID "test_video_active_raw_venc"
#define TEX_VENC_ID "test_video_active_tex_venc"
#define OUTPUT_ID "test_video_active_output"
#define CANVAS_SIZE 64
#define TIMEOUT_MS 10000

static volatile long raw_encodes;
static volatile long texture_encodes;
static uint8_t packet_byte;

#define REQUIRE(cond, why)                                       \
	do {                                                     \
		if (!(cond)) {                                   \
			fprintf(stderr, "skipped: %s\n", (why)); \
			skip();                                  \
		}                                                \
	} while (false)

/* ------------------------------------------------------------------------ */
/* Fixture encoders and a video-only encoded output                          */

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

static void fill_packet(struct encoder_packet *packet, int64_t pts)
{
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = pts;
	packet->dts = pts;
	packet->type = OBS_ENCODER_VIDEO;
	packet->keyframe = true;
}

static bool raw_venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	os_atomic_inc_long(&raw_encodes);
	fill_packet(packet, frame->pts);
	*received = true;
	return true;
}

static bool tex_venc_encode(void *data, struct encoder_texture *texture, int64_t pts, uint64_t lock_key,
			    uint64_t *next_key, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(texture);
	UNUSED_PARAMETER(lock_key);
	UNUSED_PARAMETER(next_key);
	os_atomic_inc_long(&texture_encodes);
	fill_packet(packet, pts);
	*received = true;
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
	struct obs_encoder_info raw = {
		.id = RAW_VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = test_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = raw_venc_encode,
	};
	obs_register_encoder(&raw);

	struct obs_encoder_info tex = {
		.id = TEX_VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = test_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode_texture2 = tex_venc_encode,
		.caps = OBS_ENCODER_CAP_PASS_TEXTURE,
	};
	obs_register_encoder(&tex);

	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
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
		.fps_num = 30,
		.fps_den = 1,
		.base_width = CANVAS_SIZE,
		.base_height = CANVAS_SIZE,
		.output_width = CANVAS_SIZE,
		.output_height = CANVAS_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
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

struct counts {
	long raw;
	long gpu;
};

static struct counts read_counts(struct obs_core_video_mix *mix)
{
	struct counts c = {
		.raw = os_atomic_load_long(&mix->raw_active),
		.gpu = os_atomic_load_long(&mix->gpu_encoder_active),
	};
	return c;
}

/* Asks repeatedly and checks the answer changed neither count. */
static void assert_active_is(bool expected, struct obs_core_video_mix *a, struct obs_core_video_mix *b)
{
	struct counts before_a = read_counts(a);
	struct counts before_b = b ? read_counts(b) : before_a;
	for (unsigned i = 0; i < 16; ++i)
		assert_int_equal(obs_video_active(), expected);
	struct counts after_a = read_counts(a);
	assert_int_equal(after_a.raw, before_a.raw);
	assert_int_equal(after_a.gpu, before_a.gpu);
	if (b) {
		struct counts after_b = read_counts(b);
		assert_int_equal(after_b.raw, before_b.raw);
		assert_int_equal(after_b.gpu, before_b.gpu);
	}
}

static bool wait_counter(volatile long *counter)
{
	for (int i = 0; i < TIMEOUT_MS / 10; i++) {
		if (os_atomic_load_long(counter) > 0)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

static bool wait_inactive(obs_output_t *output)
{
	for (int i = 0; i < TIMEOUT_MS / 10; i++) {
		if (!obs_output_active(output) && !obs_video_active())
			return true;
		os_sleep_ms(10);
	}
	return false;
}

static void raw_cb(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(frame);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

static void no_mixes_before_video_reset(void **state)
{
	REQUIRE(*state, "obs_startup failed (no display?)");
	obs_canvas_t *canvas = main_canvas();
	assert_null(canvas->mix);
	for (unsigned i = 0; i < 16; ++i)
		assert_false(obs_video_active());
	assert_null(canvas->mix);
}

static void idle_mix_is_inactive(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct obs_core_video_mix *mix = main_canvas()->mix;
	assert_non_null(mix);
	struct counts c = read_counts(mix);
	assert_int_equal(c.raw, 0);
	assert_int_equal(c.gpu, 0);
	assert_active_is(false, mix, NULL);
}

static void raw_callback_on_main_mix(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct obs_core_video_mix *mix = main_canvas()->mix;

	obs_add_raw_video_callback(NULL, raw_cb, NULL);
	assert_int_equal(read_counts(mix).raw, 1);
	assert_int_equal(read_counts(mix).gpu, 0);
	assert_active_is(true, mix, NULL);

	/* A second consumer keeps it active until both are gone. */
	obs_add_raw_video_callback(NULL, raw_cb, &packet_byte);
	assert_int_equal(read_counts(mix).raw, 2);
	assert_active_is(true, mix, NULL);
	obs_remove_raw_video_callback(raw_cb, NULL);
	assert_int_equal(read_counts(mix).raw, 1);
	assert_active_is(true, mix, NULL);
	obs_remove_raw_video_callback(raw_cb, &packet_byte);
	assert_int_equal(read_counts(mix).raw, 0);
	assert_active_is(false, mix, NULL);
}

static void auxiliary_canvas_mix_counts(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct obs_core_video_mix *main_mix = main_canvas()->mix;
	struct obs_video_info ovi = video_info();
	obs_canvas_t *aux = obs_canvas_create_private("video active aux", &ovi, 0);
	assert_non_null(aux);
	struct obs_core_video_mix *aux_mix = aux->mix;
	assert_non_null(aux_mix);
	assert_ptr_not_equal(aux_mix, main_mix);

	/* An auxiliary mix that exists but has no consumer is inactive. */
	assert_active_is(false, main_mix, aux_mix);

	/* A raw encoder on the auxiliary canvas's video counts on its mix only. */
	obs_encoder_t *venc = obs_video_encoder_create(RAW_VENC_ID, "video active raw", NULL, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "video active aux output", NULL, NULL);
	assert_non_null(venc);
	assert_non_null(output);
	obs_encoder_set_video(venc, obs_canvas_get_video(aux));
	obs_output_set_video_encoder(output, venc);

	os_atomic_set_long(&raw_encodes, 0);
	assert_true(obs_output_start(output));
	assert_true(wait_counter(&raw_encodes));
	assert_true(read_counts(aux_mix).raw > 0);
	assert_int_equal(read_counts(main_mix).raw, 0);
	assert_int_equal(read_counts(main_mix).gpu, 0);
	assert_active_is(true, main_mix, aux_mix);

	obs_output_stop(output);
	assert_true(wait_inactive(output));
	assert_int_equal(read_counts(aux_mix).raw, 0);
	assert_active_is(false, main_mix, aux_mix);

	obs_output_release(output);
	obs_encoder_release(venc);
	obs_canvas_release(aux);
	obs_wait_for_destroy_queue();
}

static void gpu_encoder_alone_is_active(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct obs_core_video_mix *mix = main_canvas()->mix;

	obs_encoder_t *venc = obs_video_encoder_create(TEX_VENC_ID, "video active tex", NULL, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "video active tex output", NULL, NULL);
	assert_non_null(venc);
	assert_non_null(output);
	obs_encoder_set_video(venc, obs_get_video());
	obs_output_set_video_encoder(output, venc);

	os_atomic_set_long(&texture_encodes, 0);
	bool started = obs_output_start(output);
	bool textures = started && wait_counter(&texture_encodes);
	struct counts during = read_counts(mix);
	bool active_during = textures && obs_video_active();

	if (started) {
		obs_output_stop(output);
		assert_true(wait_inactive(output));
	}
	struct counts after = read_counts(mix);
	obs_output_release(output);
	obs_encoder_release(venc);

	REQUIRE(textures, "the core video mix has no texture path");
	/* The GPU encoder count alone makes it true. */
	assert_int_equal(during.raw, 0);
	assert_true(during.gpu > 0);
	assert_true(active_during);
	assert_int_equal(after.raw, 0);
	assert_int_equal(after.gpu, 0);
	assert_active_is(false, mix, NULL);
}

static void only_counts_above_zero_are_active(void **state)
{
	REQUIRE(*state, "obs_startup or obs_reset_video failed");
	struct obs_core_video_mix *mix = main_canvas()->mix;

	/* Characterized, not endorsed: an unbalanced count can go negative,
	 * and the check is "above zero", not "nonzero". The graphics thread
	 * uses the same test, so it keeps the mix inactive meanwhile. */
	os_atomic_set_long(&mix->raw_active, -1);
	assert_active_is(false, mix, NULL);

	/* A negative count does not hide another consumer. */
	obs_add_raw_video_callback(NULL, raw_cb, NULL);
	assert_int_equal(read_counts(mix).raw, 0);
	assert_active_is(false, mix, NULL);
	obs_add_raw_video_callback(NULL, raw_cb, &packet_byte);
	assert_int_equal(read_counts(mix).raw, 1);
	assert_active_is(true, mix, NULL);
	obs_remove_raw_video_callback(raw_cb, &packet_byte);
	obs_remove_raw_video_callback(raw_cb, NULL);
	assert_int_equal(read_counts(mix).raw, -1);
	assert_active_is(false, mix, NULL);

	os_atomic_set_long(&mix->raw_active, 0);
	assert_active_is(false, mix, NULL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_mixes_before_video_reset, startup_only, shutdown_core),
		cmocka_unit_test_setup_teardown(idle_mix_is_inactive, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(raw_callback_on_main_mix, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(auxiliary_canvas_mix_counts, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(gpu_encoder_alone_is_active, startup_with_video, shutdown_core),
		cmocka_unit_test_setup_teardown(only_counts_above_zero_are_active, startup_with_video, shutdown_core),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
