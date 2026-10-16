/* Regression test for obsproject/obs-studio#12069 (reported by
 * Flopblopper): with the replay buffer running, stopping a recording takes
 * seconds and leaves a broken file, at frame rates below 30.
 *
 * The recording joins the video encoder the replay buffer already runs.
 * libobs sends an output nothing before a keyframe (send_first_video_packet
 * in obs-encoder.c), so the recording waits for the encoder's next scheduled
 * keyframe: with x264's default interval of 250 frames that is up to 12.5 s
 * at 20 fps. A recording stopped before it is empty (the muxer then waits
 * for that keyframe to finish stopping, and writes a file with nothing but
 * headers and one frame), and a longer one is missing its first seconds.
 *
 * libobs now asks a running encoder for a keyframe (request_keyframe in
 * obs_encoder_info) when another output starts on it. The test runs a video
 * encoder that makes a keyframe every 300 frames, or when asked, with a
 * "replay buffer" output on it, then starts a "recording" on the same
 * encoder and measures how long the recording waits for its first packet:
 *
 *   - joining a running encoder: a keyframe is requested and the first
 *     packet comes within a few frames;
 *   - controls: the output that starts the encoder makes no request (the
 *     encoder starts with a keyframe anyway); an encoder without
 *     request_keyframe still works, its joining output waiting for its next
 *     keyframe.
 *
 * The real_ cases run the same scenario with the encoders OBS ships on Linux,
 * loaded from the built obs-x264 and obs-ffmpeg modules, each set to a 10 s
 * keyframe interval: the joining output's first packet must be a keyframe
 * made from the frame the encoder got when the output joined (give or take a
 * few frames), not the next scheduled keyframe. obs-x264 implements
 * request_keyframe already; the obs-ffmpeg ones are VA-API H.264 and HEVC,
 * AOM AV1 and SVT-AV1. Each real_ case skips when its encoder is not
 * available (no VA-API device, FFmpeg built without the library).
 * real_x264_video_thread_behind runs the x264 case while the video thread
 * that feeds the encoders is slow (a slow encoder or a busy machine): it
 * then hands the encoder its frames late, behind the wall clock.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <media-io/video-io.h>
#include <util/platform.h>
#include <util/threading.h>

#define VENC_ID "test_join_keyframe_encoder"
#define VENC_NOREQ_ID "test_join_keyframe_encoder_noreq"
#define OUTPUT_ID "test_join_keyframe_output"
#define FPS 30
#define KEYINT 300   /* frames between scheduled keyframes: 10 s */
#define SHORT_KEYINT 15 /* the encoder without request_keyframe */
/* A joining output must get its first packet this fast. */
#define JOIN_MS 1000
/* The real encoders: the recording's first packet must be the frame the
 * encoder was given when the recording started, or one of the next few; a
 * scheduled keyframe is 300 frames away. The frame is reckoned from the
 * frames the video thread has handed out since the encoder started (its pts
 * counts the frames it was given), just before and just after the recording
 * starts, with a little slack either way for the frame in flight. Not from
 * the wall clock: it leaves out the encoder's own delay (x264's lookahead
 * holds ~20 frames), and an encoder the video thread feeds late (a slow
 * encoder or a busy machine) is given fewer frames than the time since it
 * started. */
#define JOIN_FRAMES 3

static bool obs_up;

/* ------------------------------------------------------------------------- */

struct venc {
	int keyint;
	int64_t frames;
	volatile bool keyframe_requested;
	volatile long requests;
};

static uint8_t packet_byte;

static const char *venc_name(void *type_data)
{
	(void)type_data;
	return "test encoder";
}

static void *venc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)encoder;
	struct venc *v = bzalloc(sizeof(*v));
	v->keyint = (int)obs_data_get_int(settings, "keyint");
	return v;
}

static void venc_destroy(void *data)
{
	bfree(data);
}

static bool venc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	struct venc *v = data;
	bool requested = os_atomic_exchange_bool(&v->keyframe_requested, false);

	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->type = OBS_ENCODER_VIDEO;
	packet->keyframe = requested || v->frames % v->keyint == 0;
	*received = true;
	v->frames++;
	return true;
}

static void venc_request_keyframe(void *data)
{
	struct venc *v = data;
	os_atomic_inc_long(&v->requests);
	os_atomic_set_bool(&v->keyframe_requested, true);
}

static bool venc_extra_data(void *data, uint8_t **extra_data, size_t *size)
{
	(void)data;
	*extra_data = &packet_byte;
	*size = 1;
	return true;
}

static void register_encoders(void)
{
	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = venc_name,
		.create = venc_create,
		.destroy = venc_destroy,
		.encode = venc_encode,
		.get_extra_data = venc_extra_data,
		.request_keyframe = venc_request_keyframe,
	};
	obs_register_encoder(&venc);

	venc.id = VENC_NOREQ_ID;
	venc.request_keyframe = NULL;
	obs_register_encoder(&venc);
}

/* ------------------------------------------------------------------------- */
/* a video-only output that notes when its first packet arrives */

struct out {
	obs_output_t *output;
	volatile long packets;
	volatile long first_is_keyframe;
	uint64_t started_ns;
	volatile long long first_packet_ns;
	volatile long long first_pts;
	volatile long long last_pts;
};

static const char *out_name(void *type_data)
{
	(void)type_data;
	return "test output";
}

static void *out_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	struct out *o = bzalloc(sizeof(*o));
	o->output = output;
	return o;
}

static void out_destroy(void *data)
{
	bfree(data);
}

static bool out_start(void *data)
{
	struct out *o = data;
	if (!obs_output_can_begin_data_capture(o->output, 0))
		return false;
	if (!obs_output_initialize_encoders(o->output, 0))
		return false;
	o->started_ns = os_gettime_ns();
	return obs_output_begin_data_capture(o->output, 0);
}

static void out_stop(void *data, uint64_t ts)
{
	struct out *o = data;
	(void)ts;
	obs_output_end_data_capture(o->output);
}

static void out_packet(void *data, struct encoder_packet *packet)
{
	struct out *o = data;
	if (!packet)
		return;
	if (packet->pts > o->last_pts || !o->packets)
		o->last_pts = packet->pts;
	if (os_atomic_inc_long(&o->packets) == 1) {
		o->first_packet_ns = (long long)os_gettime_ns();
		o->first_pts = packet->pts;
		os_atomic_set_long(&o->first_is_keyframe, packet->keyframe);
	}
}

static void register_outputs(void)
{
	struct obs_output_info out = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
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

struct fixture {
	obs_encoder_t *venc;
	obs_output_t *replay;
	obs_output_t *recording;
};

static bool wait_packets(obs_output_t *output, long n, int timeout_ms)
{
	struct out *o = obs_obj_get_data(output);
	for (int i = 0; i < timeout_ms / 10; i++) {
		if (os_atomic_load_long(&o->packets) >= n)
			return true;
		os_sleep_ms(10);
	}
	return os_atomic_load_long(&o->packets) >= n;
}

static double first_packet_ms(obs_output_t *output)
{
	struct out *o = obs_obj_get_data(output);
	if (!os_atomic_load_long(&o->packets))
		return -1.0;
	return (double)((uint64_t)o->first_packet_ns - o->started_ns) / 1e6;
}

static void setup_encoder(struct fixture *fx, const char *id, int keyint)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "keyint", keyint);
	fx->venc = obs_video_encoder_create(id, "venc", settings, NULL);
	obs_data_release(settings);
	assert_non_null(fx->venc);
	obs_encoder_set_video(fx->venc, obs_get_video());

	fx->replay = obs_output_create(OUTPUT_ID, "replay buffer", NULL, NULL);
	fx->recording = obs_output_create(OUTPUT_ID, "recording", NULL, NULL);
	assert_non_null(fx->replay);
	assert_non_null(fx->recording);
	obs_output_set_video_encoder(fx->replay, fx->venc);
	obs_output_set_video_encoder(fx->recording, fx->venc);
}

/* The encoder's data exists once an output has started it. */
static long keyframe_requests(struct fixture *fx)
{
	struct venc *v = obs_obj_get_data(fx->venc);
	return v ? os_atomic_load_long(&v->requests) : -1;
}

static void teardown_fixture(struct fixture *fx)
{
	obs_output_stop(fx->recording);
	obs_output_stop(fx->replay);
	for (int i = 0; i < 300 && (obs_output_active(fx->recording) || obs_output_active(fx->replay)); i++)
		os_sleep_ms(10);
	obs_output_release(fx->recording);
	obs_output_release(fx->replay);
	obs_encoder_release(fx->venc);
}

/* Start the "replay buffer", let the encoder run past its first keyframe,
 * then start the "recording" on the same encoder. */
static void start_both(struct fixture *fx)
{
	assert_true(obs_output_start(fx->replay));
	assert_true(wait_packets(fx->replay, 10, 3000));
	os_sleep_ms(500);
	assert_true(obs_output_start(fx->recording));
}

/* The report: a recording started while the replay buffer runs. */
static void joining_output_gets_a_keyframe_soon(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture fx = {0};
	setup_encoder(&fx, VENC_ID, KEYINT);
	start_both(&fx);

	bool got = wait_packets(fx.recording, 1, JOIN_MS * 3);
	double ms = first_packet_ms(fx.recording);
	long requests = keyframe_requests(&fx);
	fprintf(stderr, "joining a running encoder (keyframe every %d s): first packet after %.0f ms, %ld request(s)\n",
		KEYINT / FPS, ms, requests);

	teardown_fixture(&fx);
	assert_true(got);
	assert_true(ms >= 0.0 && ms < JOIN_MS);
	assert_int_equal(requests, 1);
}

/* Control: the output that starts the encoder makes no request. */
static void first_output_makes_no_request(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture fx = {0};
	setup_encoder(&fx, VENC_ID, KEYINT);
	assert_true(obs_output_start(fx.replay));
	bool got = wait_packets(fx.replay, 10, 3000);
	long requests = keyframe_requests(&fx);
	struct out *o = obs_obj_get_data(fx.replay);
	long keyframe = os_atomic_load_long(&o->first_is_keyframe);

	teardown_fixture(&fx);
	assert_true(got);
	assert_true(keyframe);
	assert_int_equal(requests, 0);
}

/* Control: an encoder without request_keyframe: the joining output waits
 * for its next scheduled keyframe, as before. */
static void encoder_without_request_still_works(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct fixture fx = {0};
	setup_encoder(&fx, VENC_NOREQ_ID, SHORT_KEYINT);
	start_both(&fx);

	bool got = wait_packets(fx.recording, 1, 3000);
	struct out *o = obs_obj_get_data(fx.recording);
	long keyframe = os_atomic_load_long(&o->first_is_keyframe);

	teardown_fixture(&fx);
	assert_true(got);
	assert_true(keyframe);
}

/* ------------------------------------------------------------------------- */
/* the encoders OBS ships on Linux */

static bool encoder_available(const char *id)
{
	const char *type;
	for (size_t i = 0; obs_enum_encoder_types(i, &type); i++) {
		if (strcmp(type, id) == 0)
			return true;
	}
	return false;
}

/* A raw video callback that keeps the video thread busy for lag_ms per
 * frame, as a slow encoder does. */
static volatile long video_lag_ms;

static void slow_video(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
	long ms = os_atomic_load_long(&video_lag_ms);
	if (ms > 0)
		os_sleep_ms((uint32_t)ms);
}

/* Replay buffer on a real encoder with a 10 s keyframe interval, then a
 * recording joins it. With lag_ms, the video thread spends that long on
 * every frame from just before the recording joins until it has its first
 * packet. */
static void join_real_encoder(const char *id, long lag_ms)
{
	if (!obs_up)
		skip();
	if (!encoder_available(id)) {
		fprintf(stderr, "%s: skipped, encoder not available here\n", id);
		skip();
	}

	struct fixture fx = {0};
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "keyint_sec", KEYINT / FPS);
	obs_data_set_int(settings, "bitrate", 500);
	if (strcmp(id, "ffmpeg_svt_av1") == 0)
		obs_data_set_int(settings, "preset", 12);
	else if (strcmp(id, "ffmpeg_aom_av1") == 0)
		obs_data_set_int(settings, "preset", 10);
	fx.venc = obs_video_encoder_create(id, "venc", settings, NULL);
	obs_data_release(settings);
	assert_non_null(fx.venc);
	obs_encoder_set_video(fx.venc, obs_get_video());
	fx.replay = obs_output_create(OUTPUT_ID, "replay buffer", NULL, NULL);
	fx.recording = obs_output_create(OUTPUT_ID, "recording", NULL, NULL);
	obs_output_set_video_encoder(fx.replay, fx.venc);
	obs_output_set_video_encoder(fx.recording, fx.venc);

	if (!obs_output_start(fx.replay)) {
		fprintf(stderr, "%s: skipped, the encoder did not start: %s\n", id,
			obs_output_get_last_error(fx.replay) ? obs_output_get_last_error(fx.replay) : "");
		teardown_fixture(&fx);
		skip();
	}
	/* the encoder's first frame is the next one the video thread hands out */
	uint32_t first_frame = video_output_get_total_frames(obs_get_video());
	bool replay_ok = wait_packets(fx.replay, 10, 10000);
	if (lag_ms) {
		os_atomic_set_long(&video_lag_ms, lag_ms);
		obs_add_raw_video_callback(NULL, slow_video, NULL);
	}
	os_sleep_ms(500);

	struct out *replay = obs_obj_get_data(fx.replay);
	struct out *rec = obs_obj_get_data(fx.recording);
	long long joined_after = replay->last_pts;
	long long joined_at = (uint32_t)(video_output_get_total_frames(obs_get_video()) - first_frame);
	bool started = replay_ok && obs_output_start(fx.recording);
	long long joined_by = (uint32_t)(video_output_get_total_frames(obs_get_video()) - first_frame);
	bool got = started && wait_packets(fx.recording, 1, JOIN_MS * 3);
	double ms = first_packet_ms(fx.recording);
	long keyframe = os_atomic_load_long(&rec->first_is_keyframe);
	/* how far the first packet is from the frames given while it joined */
	long long frames = -1000;
	if (got)
		frames = rec->first_pts < joined_at   ? rec->first_pts - joined_at
			 : rec->first_pts > joined_by ? rec->first_pts - joined_by
						      : 0;
	fprintf(stderr,
		"%s, joining at frame %lld-%lld (keyframe every %d s): first packet after %.0f ms, pts %lld (%+lld frames;"
		" replay buffer's latest was %lld), keyframe %ld\n",
		id, joined_at, joined_by, KEYINT / FPS, ms, got ? rec->first_pts : -1, frames, joined_after, keyframe);

	if (lag_ms) {
		/* let the video thread catch up first: it holds the input mutex
		 * that removing the callback waits for */
		os_atomic_set_long(&video_lag_ms, 0);
		obs_remove_raw_video_callback(slow_video, NULL);
	}
	teardown_fixture(&fx);
	assert_true(replay_ok);
	assert_true(started);
	assert_true(got);
	assert_true(keyframe);
	assert_true(frames >= -JOIN_FRAMES && frames <= JOIN_FRAMES);
}

static void real_x264(void **state)
{
	(void)state;
	join_real_encoder("obs_x264", 0);
}

/* The video thread takes two frame intervals per frame for the 500 ms
 * before the recording joins, so the encoder is given about seven frames
 * fewer than the wall clock counts. */
static void real_x264_video_thread_behind(void **state)
{
	(void)state;
	join_real_encoder("obs_x264", 2000 / FPS);
}

static void real_vaapi_h264(void **state)
{
	(void)state;
	join_real_encoder("ffmpeg_vaapi", 0);
}

static void real_vaapi_hevc(void **state)
{
	(void)state;
	join_real_encoder("hevc_ffmpeg_vaapi", 0);
}

static void real_aom_av1(void **state)
{
	(void)state;
	join_real_encoder("ffmpeg_aom_av1", 0);
}

static void real_svt_av1(void **state)
{
	(void)state;
	join_real_encoder("ffmpeg_svt_av1", 0);
}

static void load_module(const char *path, const char *data_path)
{
	obs_module_t *module = NULL;
	if (obs_open_module(&module, path, data_path) != MODULE_SUCCESS || !obs_init_module(module))
		fprintf(stderr, "could not load %s: its real_ cases skip\n", path);
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
		/* large enough for the hardware encoders' minimum size */
		.base_width = 320,
		.base_height = 240,
		.output_width = 320,
		.output_height = 240,
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

	register_encoders();
	register_outputs();
#ifdef OBS_X264_MODULE_PATH
	load_module(OBS_X264_MODULE_PATH, OBS_X264_DATA_PATH);
#endif
#ifdef OBS_FFMPEG_MODULE_PATH
	load_module(OBS_FFMPEG_MODULE_PATH, OBS_FFMPEG_DATA_PATH);
#endif
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
		cmocka_unit_test(joining_output_gets_a_keyframe_soon),
		cmocka_unit_test(first_output_makes_no_request),
		cmocka_unit_test(encoder_without_request_still_works),
		cmocka_unit_test(real_x264),
		cmocka_unit_test(real_x264_video_thread_behind),
		cmocka_unit_test(real_vaapi_h264),
		cmocka_unit_test(real_vaapi_hevc),
		cmocka_unit_test(real_aom_av1),
		cmocka_unit_test(real_svt_av1),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
