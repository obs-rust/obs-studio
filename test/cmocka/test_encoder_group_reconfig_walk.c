/* The encoder group reconfigure rules of obs-encoder.c through the real
 * libobs (#2913): an encoder group runs two or three video encoders of one
 * output, as the multitrack video output does. New settings for one active
 * encoder of the group (obs_encoder_update()) must reach every encoder of
 * the group on the same frame, so that their new keyframes line up:
 *
 *   - one update: each encoder applies it once, all on the same pts, after
 *     the update;
 *   - a second update while the first is still pending rides on it: one
 *     apply each;
 *   - an update from inside the first apply (the encoder applied, its group
 *     may not yet): held back (reconfigure_again) or a new request, and
 *     each encoder applies a second time, again all on the same pts.
 *
 * The test encoder records the pts of each frame encoded right after its
 * update callback ran. These cases run every encoder at the full frame rate
 * (frame rate divisor 1): a grouped encoder starts only on the frame whose
 * timestamp is the group's start_timestamp, and an encoder with a divisor
 * takes every n-th frame from the first one video-io hands it after it
 * connects, which used to miss that frame so it never started (seen with
 * divisors 2 and 3 on macOS with the C libobs, #2920).
 * divisor_encoders_start_on_the_group_frame covers that case: divisors 1, 2
 * and 3 must all start on the group's start frame. The steps test and the
 * Tier 3 parity tests cover the rounding to the divisors' LCM.
 *
 * Core video needs a graphics module and libobs needs an X display on
 * Linux, so the cases skip without one (run under xvfb-run).
 * test_encoder_group_reconfig_steps covers the rules on their own. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define VENC_ID "test_group_reconfig_encoder"
#define OUTPUT_ID "test_group_reconfig_output"
#define FPS 30
#define MAX_ENCODERS 3
#define MAX_APPLIED 4
#define WAIT_MS 5000

static bool obs_up;

/* ------------------------------------------------------------------------- */
/* a video encoder that notes on which pts its updates apply */

struct venc {
	obs_encoder_t *encoder;
	volatile long frames;
	volatile long last_pts;
	volatile long first_pts;
	volatile long num_applied;
	int64_t applied[MAX_APPLIED];
	bool pending;
	/* Update the encoder again from inside its next update callback. */
	volatile bool update_again;
	long long bitrate;
};

static uint8_t packet_byte;

static const char *venc_name(void *type_data)
{
	(void)type_data;
	return "test group encoder";
}

static void *venc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	struct venc *v = bzalloc(sizeof(*v));
	v->encoder = encoder;
	v->bitrate = obs_data_get_int(settings, "bitrate");
	return v;
}

static void venc_destroy(void *data)
{
	bfree(data);
}

static void update_bitrate(obs_encoder_t *encoder, long long bitrate)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "bitrate", bitrate);
	obs_encoder_update(encoder, settings);
	obs_data_release(settings);
}

/* do_encode() calls it on the video thread, just before encode(). */
static bool venc_update(void *data, obs_data_t *settings)
{
	struct venc *v = data;
	v->bitrate = obs_data_get_int(settings, "bitrate");
	v->pending = true;
	if (os_atomic_exchange_bool(&v->update_again, false))
		update_bitrate(v->encoder, v->bitrate + 1);
	return true;
}

static bool venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	struct venc *v = data;

	if (v->pending) {
		long n = os_atomic_load_long(&v->num_applied);
		if (n < MAX_APPLIED) {
			v->applied[n] = frame->pts;
			os_atomic_set_long(&v->num_applied, n + 1);
		}
		v->pending = false;
	}

	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = OBS_ENCODER_VIDEO;
	packet->keyframe = os_atomic_load_long(&v->frames) == 0;
	*received = true;
	if (os_atomic_load_long(&v->frames) == 0)
		os_atomic_set_long(&v->first_pts, (long)frame->pts);
	os_atomic_set_long(&v->last_pts, (long)frame->pts);
	os_atomic_inc_long(&v->frames);
	return true;
}

static bool venc_extra_data(void *data, uint8_t **extra_data, size_t *size)
{
	(void)data;
	*extra_data = &packet_byte;
	*size = 1;
	return true;
}

static void register_encoder(void)
{
	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = venc_name,
		.create = venc_create,
		.destroy = venc_destroy,
		.encode = venc_encode,
		.update = venc_update,
		.get_extra_data = venc_extra_data,
	};
	obs_register_encoder(&venc);
}

/* ------------------------------------------------------------------------- */
/* a video output with one video encoder per track */

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "test group output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void out_destroy(void *data)
{
	(void)data;
}

static bool out_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	if (!obs_output_initialize_encoders(output, 0))
		return false;
	return obs_output_begin_data_capture(output, 0);
}

static void out_stop(void *data, uint64_t ts)
{
	(void)ts;
	obs_output_end_data_capture(data);
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void register_output(void)
{
	struct obs_output_info out = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK_VIDEO,
		.get_name = out_name,
		.create = out_create,
		.destroy = out_destroy,
		.start = out_start,
		.stop = out_stop,
		.encoded_packet = out_packet,
	};
	obs_register_output(&out);
}

/* ------------------------------------------------------------------------- */

/* What an encoder recorded, copied before the output stops: the stop shuts
 * the encoders down, which frees their data. */
struct result {
	long frames;
	long num_applied;
	int64_t applied[MAX_APPLIED];
	long long bitrate;
};

struct fixture {
	size_t num;
	obs_encoder_group_t *group;
	obs_encoder_t *encoders[MAX_ENCODERS];
	obs_output_t *output;
	struct result results[MAX_ENCODERS];
};

static struct venc *venc_of(struct fixture *fx, size_t i)
{
	return obs_obj_get_data(fx->encoders[i]);
}

/* divisors, when not NULL, is per encoder and must be set before the encoder
 * starts (obs_encoder_set_frame_rate_divisor). */
static void start_group_divisors(struct fixture *fx, size_t num, const uint32_t *divisors)
{
	fx->num = num;
	fx->group = obs_encoder_group_create();
	assert_non_null(fx->group);
	fx->output = obs_output_create(OUTPUT_ID, "group output", NULL, NULL);
	assert_non_null(fx->output);

	for (size_t i = 0; i < num; i++) {
		char name[16];
		snprintf(name, sizeof(name), "venc%zu", i);
		obs_data_t *settings = obs_data_create();
		obs_data_set_int(settings, "bitrate", 1000);
		fx->encoders[i] = obs_video_encoder_create(VENC_ID, name, settings, NULL);
		obs_data_release(settings);
		assert_non_null(fx->encoders[i]);
		obs_encoder_set_video(fx->encoders[i], obs_get_video());
		if (divisors)
			assert_true(obs_encoder_set_frame_rate_divisor(fx->encoders[i], divisors[i]));
		assert_true(obs_encoder_set_group(fx->encoders[i], fx->group));
		obs_output_set_video_encoder2(fx->output, fx->encoders[i], i);
	}

	assert_true(obs_output_start(fx->output));
}

static void start_group(struct fixture *fx, size_t num)
{
	start_group_divisors(fx, num, NULL);
}

static void stop_group(struct fixture *fx)
{
	for (size_t i = 0; i < fx->num; i++) {
		struct venc *v = venc_of(fx, i);
		struct result *r = &fx->results[i];
		r->frames = os_atomic_load_long(&v->frames);
		r->num_applied = os_atomic_load_long(&v->num_applied);
		for (long k = 0; k < r->num_applied; k++)
			r->applied[k] = v->applied[k];
		r->bitrate = v->bitrate;
	}

	obs_output_stop(fx->output);
	for (int i = 0; i < 300 && obs_output_active(fx->output); i++)
		os_sleep_ms(10);
	obs_output_release(fx->output);
	for (size_t i = 0; i < fx->num; i++)
		obs_encoder_release(fx->encoders[i]);
	obs_encoder_group_destroy(fx->group);
}

/* Every encoder of the group encoded at least frames frames. */
static bool wait_frames(struct fixture *fx, long frames)
{
	for (int t = 0; t < WAIT_MS / 10; t++) {
		bool all = true;
		for (size_t i = 0; i < fx->num; i++)
			all = all && os_atomic_load_long(&venc_of(fx, i)->frames) >= frames;
		if (all)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* Every encoder of the group encodes frames more frames. */
static void wait_more_frames(struct fixture *fx, long frames)
{
	long until[MAX_ENCODERS];
	for (size_t i = 0; i < fx->num; i++)
		until[i] = os_atomic_load_long(&venc_of(fx, i)->frames) + frames;
	for (int t = 0; t < WAIT_MS / 10; t++) {
		bool all = true;
		for (size_t i = 0; i < fx->num; i++)
			all = all && os_atomic_load_long(&venc_of(fx, i)->frames) >= until[i];
		if (all)
			return;
		os_sleep_ms(10);
	}
}

static bool wait_applied(struct fixture *fx, long applied)
{
	for (int t = 0; t < WAIT_MS / 10; t++) {
		bool all = true;
		for (size_t i = 0; i < fx->num; i++)
			all = all && os_atomic_load_long(&venc_of(fx, i)->num_applied) >= applied;
		if (all)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* After the output stopped: each encoder applied rounds times, round k on
 * the same pts for all, and the first after the pts each encoder had
 * reached at the update. */
static void check_applied(const struct fixture *fx, long rounds, const long long *pts_at_update)
{
	for (size_t i = 0; i < fx->num; i++) {
		const struct result *r = &fx->results[i];
		fprintf(stderr, "encoder %zu: %ld frames, applied %ld time(s):", i, r->frames, r->num_applied);
		for (long k = 0; k < r->num_applied; k++)
			fprintf(stderr, " %lld", (long long)r->applied[k]);
		fprintf(stderr, "\n");
	}
	for (size_t i = 0; i < fx->num; i++) {
		const struct result *r = &fx->results[i];
		assert_int_equal(r->num_applied, rounds);
		assert_true(r->applied[0] > pts_at_update[i]);
		for (long k = 0; k < rounds; k++)
			assert_int_equal(r->applied[k], fx->results[0].applied[k]);
	}
}

static void note_pts(struct fixture *fx, long long *pts)
{
	for (size_t i = 0; i < fx->num; i++)
		pts[i] = os_atomic_load_long(&venc_of(fx, i)->last_pts);
}

/* One update to one encoder of the group. */
static void run_one_update(size_t num, size_t updated)
{
	if (!obs_up)
		skip();

	struct fixture fx = {0};
	long long pts[MAX_ENCODERS];
	start_group(&fx, num);
	bool started = wait_frames(&fx, 10);
	note_pts(&fx, pts);
	update_bitrate(fx.encoders[updated], 2000);
	bool applied = wait_applied(&fx, 1);
	wait_more_frames(&fx, 6); /* frames after the apply: no second one */
	stop_group(&fx);

	assert_true(started);
	assert_true(applied);
	check_applied(&fx, 1, pts);
	assert_int_equal(fx.results[updated].bitrate, 2000);
}

static void one_update_two_encoders(void **state)
{
	(void)state;
	run_one_update(2, 0);
}

static void one_update_three_encoders(void **state)
{
	(void)state;
	run_one_update(3, 2);
}

/* Two updates in a row, before the first can apply (the next shared frame
 * is at least one frame away): the second rides on the first. */
static void second_update_while_pending(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture fx = {0};
	long long pts[MAX_ENCODERS];
	start_group(&fx, 2);
	bool started = wait_frames(&fx, 10);
	note_pts(&fx, pts);
	update_bitrate(fx.encoders[0], 2000);
	update_bitrate(fx.encoders[1], 3000);
	bool applied = wait_applied(&fx, 1);
	wait_more_frames(&fx, 6);
	stop_group(&fx);

	assert_true(started);
	assert_true(applied);
	check_applied(&fx, 1, pts);
	assert_int_equal(fx.results[0].bitrate, 2000);
	assert_int_equal(fx.results[1].bitrate, 3000);
}

/* Encoder 0 updates itself again from inside its first apply: whether the
 * others applied yet or not, every encoder applies a second time, on a
 * shared pts. */
static void update_from_inside_the_apply(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture fx = {0};
	long long pts[MAX_ENCODERS];
	start_group(&fx, 3);
	bool started = wait_frames(&fx, 10);
	note_pts(&fx, pts);
	os_atomic_set_bool(&venc_of(&fx, 0)->update_again, true);
	update_bitrate(fx.encoders[0], 2000);
	bool applied = wait_applied(&fx, 2);
	wait_more_frames(&fx, 6);
	stop_group(&fx);

	assert_true(started);
	assert_true(applied);
	check_applied(&fx, 2, pts);
	assert_true(fx.results[0].applied[1] > fx.results[0].applied[0]);
	assert_int_equal(fx.results[0].bitrate, 2001);
}

/* A group whose encoders run at frame rate divisors 1, 2 and 3 starts every
 * encoder on the group's start frame (#2920): video-io's per-input divisor
 * counter has to be aligned with start_timestamp, not left where the frames
 * queued before it moved it, or the divisors above 1 skip that frame and the
 * encoder never starts. */
static void divisor_encoders_start_on_the_group_frame(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	static const uint32_t divisors[] = {1, 2, 3};
	struct fixture fx = {0};
	start_group_divisors(&fx, 3, divisors);
	bool started = wait_frames(&fx, 3);
	long long first[MAX_ENCODERS];
	for (size_t i = 0; i < fx.num; i++)
		first[i] = os_atomic_load_long(&venc_of(&fx, i)->first_pts);
	stop_group(&fx);

	for (size_t i = 0; i < fx.num; i++)
		fprintf(stderr, "encoder %zu (divisor %u): %ld frames, first pts %lld\n", i, divisors[i],
			fx.results[i].frames, first[i]);
	assert_true(started);
	for (size_t i = 0; i < fx.num; i++) {
		assert_true(fx.results[i].frames > 0);
		assert_int_equal(first[i], first[0]);
	}
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = FPS,
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

	register_encoder();
	register_output();
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(one_update_two_encoders),
		cmocka_unit_test(one_update_three_encoders),
		cmocka_unit_test(second_update_while_pending),
		cmocka_unit_test(update_from_inside_the_apply),
		cmocka_unit_test(divisor_encoders_start_on_the_group_frame),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
