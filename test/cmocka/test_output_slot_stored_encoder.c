/* obs_output_set_video_encoder2() must act on the encoder it stored in the
 * slot, not on the encoder it was passed (#2491).
 *
 * The setter stores obs_encoder_get_ref(encoder), which is NULL when the
 * encoder is being destroyed, and then made two decisions from the
 * argument:
 * - a caption track was created whenever the argument was not NULL, so a
 *   slot left empty by a failed reference still got one;
 * - at index 0 with a preferred size, obs_encoder_set_scaled_size() was
 *   called on the stored slot even when it is NULL (the slot is cleared, or
 *   the reference failed), which logs a NULL parameter.
 * obs_output_set_preferred_size2() already checks the slot before the same
 * call.
 *
 * The encoder whose reference fails is a real one in the middle of its
 * destruction: its destroy callback, which libobs runs after the last
 * reference is gone, sets it on another output, as a plugin holding a
 * plain pointer can. The cases check the output's slots and caption tracks
 * and count the NULL parameter lines through base_set_log_handler().
 * Controls: a live encoder still gets its caption track and the preferred
 * size. The encoder must be initialized for libobs to run its destroy
 * callback, which needs core video, so the test only builds on Linux and
 * runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs-internal.h>
#include <util/threading.h>

#define TARGET_IDX_OTHER 2

static bool obs_up;

/* ------------------------------------------------------------------------
 * log capture */

static log_handler_t prev_handler;
static void *prev_param;
static volatile long null_scaled_size_lines;

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	char buf[512];
	va_list copy;
	va_copy(copy, args);
	vsnprintf(buf, sizeof(buf), msg, copy);
	va_end(copy);
	if (strstr(buf, "obs_encoder_set_scaled_size") && strstr(buf, "Null"))
		os_atomic_inc_long(&null_scaled_size_lines);
	if (prev_handler)
		prev_handler(lvl, msg, args, prev_param);
}

/* ------------------------------------------------------------------------
 * output and encoder types */

static const char *out_get_name(void *type_data)
{
	(void)type_data;
	return "Slot stored test output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return bzalloc(1);
}

static void out_destroy(void *data)
{
	bfree(data);
}

static bool out_start(void *data)
{
	(void)data;
	return false;
}

static void out_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void out_encoded_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static const struct obs_output_info test_output = {
	.id = "test_slot_stored_output",
	.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK_VIDEO,
	.encoded_video_codecs = "h264",
	.get_name = out_get_name,
	.create = out_create,
	.destroy = out_destroy,
	.start = out_start,
	.stop = out_stop,
	.encoded_packet = out_encoded_packet,
};

/* What the encoder's destroy callback does: set the encoder being destroyed
 * on this output, at this index. */
static struct {
	obs_output_t *target;
	size_t idx;
	bool ran;
} on_destroy;

struct test_encoder {
	obs_encoder_t *self;
};

static const char *enc_get_name(void *type_data)
{
	(void)type_data;
	return "Slot stored test encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	struct test_encoder *te = bzalloc(sizeof(*te));
	te->self = encoder;
	return te;
}

static void enc_destroy(void *data)
{
	struct test_encoder *te = data;
	if (on_destroy.target) {
		obs_output_set_video_encoder2(on_destroy.target, te->self, on_destroy.idx);
		on_destroy.ran = true;
	}
	bfree(te);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received_packet = false;
	return true;
}

static const struct obs_encoder_info test_encoder_info = {
	.id = "test_slot_stored_encoder",
	.type = OBS_ENCODER_VIDEO,
	.codec = "h264",
	.get_name = enc_get_name,
	.create = enc_create,
	.destroy = enc_destroy,
	.encode = enc_encode,
};

/* ------------------------------------------------------------------------
 * helpers */

static obs_output_t *new_output(const char *name)
{
	obs_output_t *output = obs_output_create(test_output.id, name, NULL, NULL);
	assert_non_null(output);
	return output;
}

static obs_encoder_t *new_encoder(const char *name)
{
	obs_encoder_t *encoder = obs_video_encoder_create(test_encoder_info.id, name, NULL, NULL);
	assert_non_null(encoder);
	obs_encoder_set_video(encoder, obs_get_video());
	return encoder;
}

/* Releases the last reference on an initialized encoder: libobs then runs
 * its destroy callback, which sets it on `target` at `idx` while its
 * reference can no longer be taken. */
static void destroy_onto(obs_output_t *target, size_t idx, const char *name)
{
	obs_output_t *holder = new_output("slot stored holder");
	obs_encoder_t *encoder = new_encoder(name);

	obs_output_set_video_encoder2(holder, encoder, 0);
	assert_true(obs_output_initialize_encoders(holder, 0));
	obs_output_set_video_encoder2(holder, NULL, 0);
	obs_output_release(holder);

	on_destroy.target = target;
	on_destroy.idx = idx;
	on_destroy.ran = false;
	obs_encoder_release(encoder);
	on_destroy.target = NULL;
	assert_true(on_destroy.ran);
}

/* ------------------------------------------------------------------------
 * cases */

/* control: a live encoder gets its slot and its caption track */
static void live_encoder_gets_caption_track(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_output_t *output = new_output("slot stored live");
	obs_encoder_t *encoder = new_encoder("slot stored live encoder");

	obs_output_set_video_encoder2(output, encoder, TARGET_IDX_OTHER);
	assert_ptr_equal(output->video_encoders[TARGET_IDX_OTHER], encoder);
	assert_non_null(output->caption_tracks[TARGET_IDX_OTHER]);

	/* clearing it removes both */
	obs_output_set_video_encoder2(output, NULL, TARGET_IDX_OTHER);
	assert_null(output->video_encoders[TARGET_IDX_OTHER]);
	assert_null(output->caption_tracks[TARGET_IDX_OTHER]);

	obs_encoder_release(encoder);
	obs_output_release(output);
}

/* an encoder being destroyed leaves the slot empty, and an empty slot has
 * no caption track */
static void dying_encoder_gets_no_caption_track(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_output_t *output = new_output("slot stored dying");

	destroy_onto(output, TARGET_IDX_OTHER, "slot stored dying encoder");
	assert_null(output->video_encoders[TARGET_IDX_OTHER]);
	if (output->caption_tracks[TARGET_IDX_OTHER]) {
		fprintf(stderr, "slot %d is empty but has a caption track\n", TARGET_IDX_OTHER);
		fail();
	}

	obs_output_release(output);
}

/* control: at index 0 a live encoder takes the output's preferred size */
static void live_encoder_takes_preferred_size(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_output_t *output = new_output("slot stored size live");
	obs_encoder_t *encoder = new_encoder("slot stored size encoder");

	obs_output_set_preferred_size(output, 32, 32);
	os_atomic_set_long(&null_scaled_size_lines, 0);
	obs_output_set_video_encoder2(output, encoder, 0);
	assert_int_equal(encoder->scaled_width, 32);
	assert_int_equal(encoder->scaled_height, 32);
	assert_int_equal(os_atomic_load_long(&null_scaled_size_lines), 0);

	obs_output_set_video_encoder2(output, NULL, 0);
	obs_encoder_release(encoder);
	obs_output_release(output);
}

/* clearing index 0 on an output with a preferred size sets no size on the
 * empty slot */
static void clearing_slot0_sets_no_size_on_null(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_output_t *output = new_output("slot stored size clear");
	obs_encoder_t *encoder = new_encoder("slot stored clear encoder");

	obs_output_set_preferred_size(output, 32, 32);
	obs_output_set_video_encoder2(output, encoder, 0);
	os_atomic_set_long(&null_scaled_size_lines, 0);
	obs_output_set_video_encoder2(output, NULL, 0);
	assert_null(output->video_encoders[0]);
	assert_null(output->caption_tracks[0]);
	long lines = os_atomic_load_long(&null_scaled_size_lines);
	if (lines != 0) {
		fprintf(stderr, "clearing slot 0 logged %ld NULL obs_encoder_set_scaled_size line(s)\n", lines);
		fail();
	}

	obs_encoder_release(encoder);
	obs_output_release(output);
}

/* an encoder being destroyed at index 0, with a preferred size: no caption
 * track and no size on the empty slot */
static void dying_encoder_at_slot0_with_size(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_output_t *output = new_output("slot stored dying 0");

	obs_output_set_preferred_size(output, 32, 32);
	os_atomic_set_long(&null_scaled_size_lines, 0);
	destroy_onto(output, 0, "slot stored dying encoder 0");
	assert_null(output->video_encoders[0]);
	long lines = os_atomic_load_long(&null_scaled_size_lines);
	bool caption = output->caption_tracks[0] != NULL;
	if (caption || lines != 0) {
		fprintf(stderr, "empty slot 0: caption track %s, %ld NULL obs_encoder_set_scaled_size line(s)\n",
			caption ? "created" : "none", lines);
		fail();
	}

	obs_output_release(output);
}

/* ------------------------------------------------------------------------ */

static int group_setup(void **state)
{
	(void)state;
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, NULL);

	/* obs_startup() and core video need a display on Linux (run under
	 * xvfb-run); the cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_output(&test_output);
	obs_register_encoder(&test_encoder_info);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	base_set_log_handler(prev_handler, prev_param);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(live_encoder_gets_caption_track),
		cmocka_unit_test(dying_encoder_gets_no_caption_track),
		cmocka_unit_test(live_encoder_takes_preferred_size),
		cmocka_unit_test(clearing_slot0_sets_no_size_on_null),
		cmocka_unit_test(dying_encoder_at_slot0_with_size),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
