/* Regression test for upstream obsproject/obs-studio#12213 (reported by
 * walker-WSH): video encoders keep a freed video_t after obs_reset_video().
 *
 * A video encoder stores the video_t it was given with
 * obs_encoder_set_video() (the frontend passes obs_get_video() before every
 * start). obs_reset_video() frees every video mix, including that video_t,
 * and creates new ones, but nothing moved the encoders to the new mixes:
 *
 * - An encoder that is not initialized keeps the stale pointer until the
 *   next obs_encoder_set_video(). obs_output_get_width/height() of its
 *   output reads it, so obs-websocket's GetOutputList crashes after a stream
 *   or a recording ran and SetVideoSettings changed the resolution.
 * - An encoder that an output initialized and then never started stays
 *   initialized. obs_encoder_set_video() refuses a new video_t while the
 *   encoder is initialized, so the next obs_encoder_update() (x264 reads its
 *   size) and the next start use the freed video_t. A recording into a folder
 *   that cannot be written does this: ffmpeg-mux initializes the encoders,
 *   then fails its write check and returns false from start(). So does a
 *   stream that fails to connect when the encoder shutdown of #236 has not
 *   run yet (walker-WSH's report).
 * - A stream that is still connecting has initialized its encoders, and
 *   obs_reset_video() allowed the reset under it because no encoder was
 *   active yet. When the connection succeeded, the output used the freed
 *   video_t (rtmp_output's metadata reads its size).
 *
 * The cases compare the encoder's video_t with the live one before anything
 * reads it, so a stale pointer fails the test without reading freed memory.
 * Without ASan the allocator may hand the freed video_t's memory straight back
 * to the new mix, which would hide the bug; as in
 * test_raw_output_video_reset.c, the test pins the freed memory while the
 * reset runs (obs_reset_video() logs "video settings reset" after it freed the
 * old mixes and before it creates the new ones).
 *
 * Core video needs a graphics module, and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <malloc.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define ENCODER_ID "test_reset_counting_encoder"
#define FAILING_START_OUTPUT_ID "test_reset_failing_start_output"
#define CONNECT_FAILING_OUTPUT_ID "test_reset_connect_failing_output"
#define GOOD_OUTPUT_ID "test_reset_running_output"
#define CONNECTING_OUTPUT_ID "test_reset_connecting_output"
#define TIMEOUT_MS 5000

static int obs_up;
static volatile long encoder_creates;
static volatile long encoder_destroys;
static volatile long encode_calls;
static uint32_t created_width;
static uint32_t created_height;
static uint32_t updated_width;

static long live_encoders(void)
{
	return os_atomic_load_long(&encoder_creates) - os_atomic_load_long(&encoder_destroys);
}

static long wait_for_live_encoders(long expected)
{
	for (int i = 0; i < TIMEOUT_MS / 10 && live_encoders() != expected; i++)
		os_sleep_ms(10);
	return live_encoders();
}

/* ------------------------------------------------------------------------- */
/* counting encoder: reads its size at create and update, as x264 does */

static const char *enc_name(void *type_data)
{
	(void)type_data;
	return "reset counting encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	created_width = obs_encoder_get_width(encoder);
	created_height = obs_encoder_get_height(encoder);
	os_atomic_inc_long(&encoder_creates);
	return encoder;
}

static void enc_destroy(void *data)
{
	(void)data;
	os_atomic_inc_long(&encoder_destroys);
}

static bool enc_update(void *data, obs_data_t *settings)
{
	(void)settings;
	updated_width = obs_encoder_get_width(data);
	return true;
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	os_atomic_inc_long(&encode_calls);
	*received_packet = false;
	return true;
}

static void register_encoder(void)
{
	struct obs_encoder_info info = {
		.id = ENCODER_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.update = enc_update,
		.encode = enc_encode,
	};
	obs_register_encoder(&info);
}

/* ------------------------------------------------------------------------- */
/* outputs */

static const char *output_name(void *type_data)
{
	(void)type_data;
	return "reset test output";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void output_destroy(void *data)
{
	(void)data;
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

/* Fails like ffmpeg-mux recording into a folder it cannot write: the
 * encoders are initialized, then start() returns false. */
static bool failing_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	if (!obs_output_initialize_encoders(output, 0))
		return false;
	obs_output_set_last_error(output, "the test output cannot write its file");
	return false;
}

static void failing_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

/* Fails like rtmp_output on a refused connection: start() initializes the
 * encoders and returns true, the connect thread then calls
 * obs_output_signal_stop(). */
struct connect_failing {
	obs_output_t *output;
	pthread_t thread;
	bool thread_active;
};

static void *connect_failing_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	struct connect_failing *f = bzalloc(sizeof(*f));
	f->output = output;
	return f;
}

static void connect_failing_join(struct connect_failing *f)
{
	if (f->thread_active) {
		pthread_join(f->thread, NULL);
		f->thread_active = false;
	}
}

static void connect_failing_destroy(void *data)
{
	connect_failing_join(data);
	bfree(data);
}

static void *connect_thread(void *data)
{
	struct connect_failing *f = data;
	os_sleep_ms(20);
	obs_output_signal_stop(f->output, OBS_OUTPUT_CONNECT_FAILED);
	return NULL;
}

static bool connect_failing_start(void *data)
{
	struct connect_failing *f = data;
	connect_failing_join(f);
	if (!obs_output_can_begin_data_capture(f->output, 0))
		return false;
	if (!obs_output_initialize_encoders(f->output, 0))
		return false;
	f->thread_active = pthread_create(&f->thread, NULL, connect_thread, f) == 0;
	return f->thread_active;
}

static void connect_failing_stop(void *data, uint64_t ts)
{
	(void)ts;
	connect_failing_join(data);
}

static bool good_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	if (!obs_output_initialize_encoders(output, 0))
		return false;
	return obs_output_begin_data_capture(output, 0);
}

static void good_stop(void *data, uint64_t ts)
{
	(void)ts;
	obs_output_end_data_capture(data);
}

/* A stream still connecting: start() initializes the encoders and returns
 * true, the test decides when the connection succeeds or fails. */
static bool connecting_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	return obs_output_initialize_encoders(output, 0);
}

static void connecting_stop(void *data, uint64_t ts)
{
	(void)ts;
	obs_output_t *output = data;
	if (obs_output_active(output))
		obs_output_end_data_capture(output);
	else
		obs_output_signal_stop(output, OBS_OUTPUT_SUCCESS);
}

static void register_outputs(void)
{
	struct obs_output_info connecting = {
		.id = CONNECTING_OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = connecting_start,
		.stop = connecting_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&connecting);

	struct obs_output_info failing = {
		.id = FAILING_START_OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = failing_start,
		.stop = failing_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&failing);

	struct obs_output_info connect_failing = {
		.id = CONNECT_FAILING_OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = output_name,
		.create = connect_failing_create,
		.destroy = connect_failing_destroy,
		.start = connect_failing_start,
		.stop = connect_failing_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&connect_failing);

	struct obs_output_info good = {
		.id = GOOD_OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = good_start,
		.stop = good_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&good);
}

/* ------------------------------------------------------------------------- */
/* pinning freed video_t memory during obs_reset_video(), see
 * test_raw_output_video_reset.c */

#define MAX_HELD 32768
#define PIN_SMALL_STEP 16
#define PIN_SMALL_MAX 4096
#define PIN_SMALL_BLOCKS 16
#define PIN_LARGE_BLOCKS 256
#define PIN_MAX_SIZE (64 * 1024)

static log_handler_t default_log_handler;
static void *default_log_param;

static const void *pin_target;
static bool pin_armed;
static bool pinned;
static void *held[MAX_HELD];
static size_t num_held;

static bool block_contains(void *block, const void *target)
{
	uintptr_t start = (uintptr_t)block;
	uintptr_t t = (uintptr_t)target;
	return t >= start && t < start + malloc_usable_size(block);
}

static void pin_freed_target(void)
{
	for (size_t size = PIN_SMALL_STEP; size <= PIN_MAX_SIZE && !pinned;
	     size = size < PIN_SMALL_MAX ? size + PIN_SMALL_STEP : size * 2) {
		size_t blocks = size < PIN_SMALL_MAX ? PIN_SMALL_BLOCKS : PIN_LARGE_BLOCKS;
		for (size_t n = 0; n < blocks && !pinned; n++) {
			if (num_held == MAX_HELD)
				return;
			void *block = malloc(size);
			if (!block)
				return;
			held[num_held++] = block;
			pinned = block_contains(block, pin_target);
		}
	}
}

static void pin_log_handler(int lvl, const char *msg, va_list args, void *param)
{
	va_list copy;
	va_copy(copy, args);
	char text[64];
	vsnprintf(text, sizeof(text), msg, copy);
	va_end(copy);

	if (pin_armed && strstr(text, "video settings reset")) {
		pin_armed = false;
		pin_freed_target();
	}

	if (default_log_handler)
		default_log_handler(lvl, msg, args, param);
}

static void release_pins(void)
{
	pin_armed = false;
	for (size_t i = 0; i < num_held; i++)
		free(held[i]);
	num_held = 0;
	if (pin_target && !pinned)
		fprintf(stderr, "note: the freed video_t address was not pinned\n");
	pin_target = NULL;
	pinned = false;
}

/* ------------------------------------------------------------------------- */
/* helpers */

static int reset_video(uint32_t width, uint32_t height)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = width,
		.base_height = height,
		.output_width = width,
		.output_height = height,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	return obs_reset_video(&ovi);
}

/* Resets the main video while the old video_t address stays pinned; the
 * caller releases the pin after its checks. */
static void reset_to(uint32_t width, uint32_t height)
{
	pin_target = obs_get_video();
	pinned = false;
	pin_armed = true;
	int ret = reset_video(width, height);
	if (ret != OBS_VIDEO_SUCCESS) {
		release_pins();
		fprintf(stderr, "obs_reset_video(%ux%u) failed: %d\n", width, height, ret);
		fail();
	}
}

/* Fails without reading the video_t when the encoder still holds a freed
 * one. */
static void check_encoder_video_is_live(obs_encoder_t *encoder, const char *what)
{
	video_t *video = obs_encoder_parent_video(encoder);
	if (video != obs_get_video()) {
		fprintf(stderr, "%s: the encoder holds video_t %p, the live one is %p (stale pointer to a freed mix)\n",
			what, (void *)video, (void *)obs_get_video());
		fail();
	}
}

static void check_output_size(obs_output_t *output, uint32_t width, uint32_t height, const char *what)
{
	uint32_t cx = obs_output_get_width(output);
	uint32_t cy = obs_output_get_height(output);
	if (cx != width || cy != height) {
		fprintf(stderr, "%s: the output reports %ux%u, expected %ux%u\n", what, cx, cy, width, height);
		fail();
	}
}

static obs_output_t *create_output(const char *id, const char *name, obs_encoder_t *encoder)
{
	obs_output_t *output = obs_output_create(id, name, NULL, NULL);
	if (!output) {
		fprintf(stderr, "could not create output %s\n", id);
		fail();
	}
	obs_output_set_video_encoder(output, encoder);
	return output;
}

static void wait_inactive(obs_output_t *output)
{
	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	while ((obs_output_active(output) || obs_video_active()) && os_gettime_ns() < deadline)
		os_sleep_ms(5);
}

/* What the frontend does for every start: hand the encoder the live video,
 * apply its settings, start a working output. The encoder must be created
 * for the size of the live video and receive frames. */
static void start_like_frontend(obs_encoder_t *encoder, uint32_t width, uint32_t height, const char *what)
{
	obs_encoder_set_video(encoder, obs_get_video());
	check_encoder_video_is_live(encoder, what);

	obs_data_t *settings = obs_data_create();
	obs_encoder_update(encoder, settings);
	obs_data_release(settings);

	obs_output_t *good = create_output(GOOD_OUTPUT_ID, "reset running output", encoder);
	created_width = created_height = 0;
	long calls = os_atomic_load_long(&encode_calls);
	if (!obs_output_start(good)) {
		fprintf(stderr, "%s: the working output did not start: %s\n", what, obs_output_get_last_error(good));
		obs_output_release(good);
		fail();
	}
	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	while (os_atomic_load_long(&encode_calls) < calls + 3 && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	long got = os_atomic_load_long(&encode_calls) - calls;
	obs_output_stop(good);
	wait_inactive(good);
	obs_output_release(good);

	if (created_width != width || created_height != height) {
		fprintf(stderr, "%s: the encoder was created for %ux%u, the video is %ux%u\n", what, created_width,
			created_height, width, height);
		fail();
	}
	if (got < 3) {
		fprintf(stderr, "%s: the encoder encoded %ld frames in %d ms\n", what, got, TIMEOUT_MS);
		fail();
	}
}

/* ------------------------------------------------------------------------- */
/* cases */

static int fixture_setup(void **state)
{
	if (!obs_up) {
		*state = NULL;
		return 0;
	}
	int ret = reset_video(64, 64);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video(64x64) failed: %d\n", ret);
		return -1;
	}
	obs_encoder_t *encoder = obs_video_encoder_create(ENCODER_ID, "reset venc", NULL, NULL);
	if (!encoder)
		return -1;
	obs_encoder_set_video(encoder, obs_get_video());
	*state = encoder;
	return 0;
}

static int fixture_teardown(void **state)
{
	release_pins();
	obs_encoder_release(*state);
	if (obs_up && wait_for_live_encoders(0) != 0)
		fprintf(stderr, "note: %ld encoder instances left after the case\n", live_encoders());
	return 0;
}

/* The websocket case: a stream ran and stopped, the resolution changes, then
 * GetOutputList reads the stream output's size. */
static void stopped_encoder_follows_reset(void **state)
{
	if (!obs_up)
		skip();
	obs_encoder_t *encoder = *state;

	start_like_frontend(encoder, 64, 64, "before the reset");
	obs_output_t *output = create_output(GOOD_OUTPUT_ID, "reset idle output", encoder);
	check_output_size(output, 64, 64, "before the reset");

	reset_to(128, 72);
	check_encoder_video_is_live(encoder, "after resetting to 128x72");
	release_pins();
	check_output_size(output, 128, 72, "after resetting to 128x72");

	obs_output_release(output);
	start_like_frontend(encoder, 128, 72, "after resetting to 128x72");
}

/* A recording into a folder that cannot be written: the encoder is
 * initialized and the output never starts. After the resolution changes,
 * the next start must create the encoder for the new size. */
static void failed_start_encoder_follows_reset(void **state)
{
	if (!obs_up)
		skip();
	obs_encoder_t *encoder = *state;

	obs_output_t *output = create_output(FAILING_START_OUTPUT_ID, "reset failing output", encoder);
	if (obs_output_start(output)) {
		fprintf(stderr, "the failing output started\n");
		fail();
	}
	assert_int_equal(live_encoders(), 1);

	reset_to(128, 72);
	check_encoder_video_is_live(encoder, "after resetting to 128x72");
	release_pins();
	if (live_encoders() != 0) {
		fprintf(stderr, "after resetting to 128x72: the encoder is still initialized for the freed video\n");
		fail();
	}
	check_output_size(output, 128, 72, "after resetting to 128x72");

	obs_output_release(output);
	start_like_frontend(encoder, 128, 72, "after resetting to 128x72");
}

/* walker-WSH's steps: a stream fails to connect, the resolution changes,
 * the stream starts again. */
static void connect_failed_encoder_follows_reset(void **state)
{
	if (!obs_up)
		skip();
	obs_encoder_t *encoder = *state;

	obs_output_t *output = create_output(CONNECT_FAILING_OUTPUT_ID, "reset connect failing output", encoder);
	if (!obs_output_start(output)) {
		fprintf(stderr, "the connect failing output did not start\n");
		fail();
	}
	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	while (os_gettime_ns() < deadline && live_encoders() != 0)
		os_sleep_ms(5);
	wait_inactive(output);

	reset_to(128, 72);
	check_encoder_video_is_live(encoder, "after resetting to 128x72");
	release_pins();
	check_output_size(output, 128, 72, "after resetting to 128x72");

	obs_output_release(output);
	start_like_frontend(encoder, 128, 72, "after resetting to 128x72");
}

/* An encoder with a frame rate divisor encodes from a video_t derived from
 * the main one; it must be derived from the new main video. */
static void divisor_encoder_follows_reset(void **state)
{
	if (!obs_up)
		skip();
	obs_encoder_t *encoder = *state;

	assert_true(obs_encoder_set_frame_rate_divisor(encoder, 2));
	obs_encoder_set_video(encoder, obs_get_video());
	assert_ptr_not_equal(obs_encoder_video(encoder), obs_get_video());

	reset_to(128, 72);
	check_encoder_video_is_live(encoder, "after resetting to 128x72");
	release_pins();
	video_t *derived = obs_encoder_video(encoder);
	if (!derived || derived == obs_get_video()) {
		fprintf(stderr, "after resetting to 128x72: the encoder has no frame rate divided video (%p)\n",
			(void *)derived);
		fail();
	}
	const struct video_output_info *voi = video_output_get_info(derived);
	assert_int_equal(voi->width, 128);
	assert_int_equal(voi->fps_den, 2);

	start_like_frontend(encoder, 128, 72, "after resetting to 128x72");
}

/* A stream that is still connecting has initialized its encoders for the
 * current video and starts using it as soon as it connects, so the video
 * cannot be reset under it (as while it streams). */
static void reset_refused_while_starting(void **state)
{
	if (!obs_up)
		skip();
	obs_encoder_t *encoder = *state;

	obs_output_t *output = create_output(CONNECTING_OUTPUT_ID, "reset connecting output", encoder);
	if (!obs_output_start(output)) {
		fprintf(stderr, "the connecting output did not start\n");
		fail();
	}
	video_t *before = obs_get_video();
	int ret = reset_video(128, 72);
	if (ret != OBS_VIDEO_CURRENTLY_ACTIVE) {
		fprintf(stderr, "obs_reset_video() while an output is connecting returned %d, expected %d (active)\n",
			ret, OBS_VIDEO_CURRENTLY_ACTIVE);
		obs_output_signal_stop(output, OBS_OUTPUT_CONNECT_FAILED);
		fail();
	}
	assert_ptr_equal(obs_get_video(), before);

	/* the connection succeeds: the encoder runs on the video it was set up for */
	long calls = os_atomic_load_long(&encode_calls);
	assert_true(obs_output_begin_data_capture(output, 0));
	uint64_t deadline = os_gettime_ns() + (uint64_t)TIMEOUT_MS * 1000000ULL;
	while (os_atomic_load_long(&encode_calls) < calls + 3 && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	long got = os_atomic_load_long(&encode_calls) - calls;
	obs_output_stop(output);
	wait_inactive(output);
	obs_output_release(output);
	if (got < 3) {
		fprintf(stderr, "the connected output encoded %ld frames in %d ms\n", got, TIMEOUT_MS);
		fail();
	}

	/* once it stopped, the reset goes through */
	reset_to(128, 72);
	check_encoder_video_is_live(encoder, "after the output stopped and the video was reset");
	release_pins();
}

/* ------------------------------------------------------------------------- */

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up)
		return 0;
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}
	register_encoder();
	register_outputs();
	base_get_log_handler(&default_log_handler, &default_log_param);
	base_set_log_handler(pin_log_handler, default_log_param);
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
		cmocka_unit_test_setup_teardown(stopped_encoder_follows_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(failed_start_encoder_follows_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(connect_failed_encoder_follows_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(divisor_encoder_follows_reset, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(reset_refused_while_starting, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
