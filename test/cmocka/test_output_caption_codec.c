/* Captions on a video track whose codec carries no captions (obs-rust/obs-studio#1777).
 *
 * add_caption() in obs-output.c inserts queued captions into h264, hevc and
 * av1 packets. For any other codec (ProRes, or a plugin's own codec) it still
 * takes the caption off the queue, so the queue drains, but it must not leave
 * anything allocated behind: it used to copy the whole packet into a new
 * buffer before the codec check and return without freeing it, one packet
 * sized leak per caption page or CEA-708 batch.
 *
 * The test drives a real AV output through the public API, as
 * test_output_interleave_restart does: one video and one audio encoder,
 * registered here, fed by hand. Captions are queued with
 * obs_output_output_caption_text2() or obs_output_caption(), and a capture
 * must leave bnum_allocs() where it found it. The same cases run with an
 * h264 encoder as a control: there the captions must reach the packets
 * (the delivered video packets grow), which shows that the capture reaches
 * add_caption(). It uses real core video, so it needs an X display on Linux
 * (xvfb-run, as in CI). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <media-io/video-frame.h>
#include <util/bmem.h>
#include <util/threading.h>

#include "obs-internal.h"

#define FPS 50
#define AUDIO_FRAME 960 /* 20 ms at 48 kHz: one video frame */
#define BLOCK_NS (1024ULL * 1000000000ULL / 48000)
#define FRAME_NS (1000000000ULL / FPS)
#define STEPS 12
#define TEXT_PAGES 3

static struct {
	obs_output_t *output;
	obs_encoder_t *video_encoder;
	obs_encoder_t *audio_encoder;
	video_t *video;
	audio_t *audio;
	os_event_t *audio_command;
	os_event_t *audio_done;
	os_event_t *video_done;
	os_event_t *deactivated;
	atomic_bool shutdown;
	atomic_size_t video_packets;
	atomic_size_t captioned_packets; /* video packets bigger than the payload */
	uint64_t clock;
	uint64_t audio_timestamp;
	int64_t audio_pts;
	bool audio_packet_pending;
} harness;

static const uint8_t payload[16];

static const char *test_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "caption codec test";
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void test_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static size_t audio_frame_size(void *data)
{
	UNUSED_PARAMETER(data);
	return AUDIO_FRAME;
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	packet->type = obs_encoder_get_type(data);
	*received = packet->type == OBS_ENCODER_VIDEO || harness.audio_packet_pending;
	if (!*received)
		return true;

	packet->data = (uint8_t *)payload;
	packet->size = sizeof(payload);
	packet->pts = packet->dts = packet->type == OBS_ENCODER_VIDEO ? frame->pts : harness.audio_pts;
	packet->keyframe = packet->type == OBS_ENCODER_VIDEO;
	if (packet->type == OBS_ENCODER_AUDIO)
		harness.audio_packet_pending = false;
	return true;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	return output;
}

static bool output_start(void *data)
{
	return obs_output_can_begin_data_capture(data, 0) && obs_output_initialize_encoders(data, 0) &&
	       obs_output_begin_data_capture(data, 0);
}

static void output_stop(void *data, uint64_t timestamp)
{
	UNUSED_PARAMETER(timestamp);
	obs_output_end_data_capture(data);
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	if (!packet || packet->type != OBS_ENCODER_VIDEO)
		return;
	atomic_fetch_add(&harness.video_packets, 1);
	if (packet->size > sizeof(payload))
		atomic_fetch_add(&harness.captioned_packets, 1);
}

static bool audio_input(void *data, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
			struct audio_output_data *mixes)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(start);
	UNUSED_PARAMETER(end);
	UNUSED_PARAMETER(mixes); /* audio_output_open supplies zeroed silence. */
	if (atomic_load(&harness.shutdown) || !(mixers & 1))
		return false;
	os_event_wait(harness.audio_command);
	if (atomic_load(&harness.shutdown))
		return false;
	*timestamp = harness.audio_timestamp;
	harness.audio_packet_pending = harness.audio_pts >= 0;
	return true;
}

/* Audio callbacks run in reverse registration order, video callbacks in
 * registration order. These observers acknowledge only after the encoder
 * and the output have finished processing the submitted media block. */
static void audio_done(void *data, size_t mix, struct audio_data *audio)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(mix);
	UNUSED_PARAMETER(audio);
	os_event_signal(harness.audio_done);
}

static void video_done(void *data, struct video_data *video)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(video);
	os_event_signal(harness.video_done);
}

static void on_deactivate(void *data, calldata_t *params)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(params);
	os_event_signal(harness.deactivated);
}

static void register_video_encoder(const char *id, const char *codec)
{
	struct obs_encoder_info encoder = {
		.id = id,
		.type = OBS_ENCODER_VIDEO,
		.codec = codec,
		.get_name = test_name,
		.create = encoder_create,
		.destroy = test_destroy,
		.encode = encode,
	};
	obs_register_encoder(&encoder);
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = FPS,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_RGBA,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_SUCCESS);

	register_video_encoder("test_caption_prores", "prores");
	register_video_encoder("test_caption_h264", "h264");
	struct obs_encoder_info audio = {
		.id = "test_caption_audio",
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = test_name,
		.create = encoder_create,
		.destroy = test_destroy,
		.encode = encode,
		.get_frame_size = audio_frame_size,
	};
	obs_register_encoder(&audio);
	struct obs_output_info output = {
		.id = "test_caption_output",
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
		.get_name = test_name,
		.create = output_create,
		.destroy = test_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);

	assert_int_equal(os_event_init(&harness.audio_command, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&harness.audio_done, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&harness.video_done, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&harness.deactivated, OS_EVENT_TYPE_MANUAL), 0);
	struct video_output_info vi = {
		.name = "controlled video",
		.format = VIDEO_FORMAT_RGBA,
		.fps_num = FPS,
		.fps_den = 1,
		.width = 64,
		.height = 64,
		.cache_size = 2,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_FULL,
	};
	assert_int_equal(video_output_open(&harness.video, &vi), VIDEO_OUTPUT_SUCCESS);
	struct audio_output_info ai = {
		.name = "controlled audio",
		.samples_per_sec = 48000,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.speakers = SPEAKERS_STEREO,
		.input_callback = audio_input,
	};
	assert_int_equal(audio_output_open(&harness.audio, &ai), AUDIO_OUTPUT_SUCCESS);
	assert_true(audio_output_connect(harness.audio, 0, NULL, audio_done, NULL));

	harness.audio_encoder = obs_audio_encoder_create("test_caption_audio", "audio", NULL, 0, NULL);
	assert_non_null(harness.audio_encoder);
	obs_encoder_set_audio(harness.audio_encoder, harness.audio);
	harness.clock = 1000000000ULL;
	return 0;
}

static void release_output(void)
{
	if (harness.output) {
		if (obs_output_active(harness.output))
			obs_output_force_stop(harness.output);
		signal_handler_disconnect(obs_output_get_signal_handler(harness.output), "deactivate", on_deactivate,
					  NULL);
		obs_output_release(harness.output);
		harness.output = NULL;
	}
	obs_encoder_release(harness.video_encoder);
	harness.video_encoder = NULL;
}

static void new_output(const char *video_encoder_id)
{
	release_output();
	harness.video_encoder = obs_video_encoder_create(video_encoder_id, "video", NULL, NULL);
	harness.output = obs_output_create("test_caption_output", "output", NULL, NULL);
	assert_non_null(harness.video_encoder);
	assert_non_null(harness.output);
	obs_encoder_set_video(harness.video_encoder, harness.video);
	obs_output_set_video_encoder(harness.output, harness.video_encoder);
	obs_output_set_audio_encoder(harness.output, harness.audio_encoder, 0);
	signal_handler_connect(obs_output_get_signal_handler(harness.output), "deactivate", on_deactivate, NULL);
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	atomic_store(&harness.shutdown, true);
	if (harness.audio_command)
		os_event_signal(harness.audio_command);
	release_output();
	obs_encoder_release(harness.audio_encoder);
	audio_output_close(harness.audio);
	video_output_close(harness.video);
	os_event_destroy(harness.audio_command);
	os_event_destroy(harness.audio_done);
	os_event_destroy(harness.video_done);
	os_event_destroy(harness.deactivated);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

static void submit_audio(uint64_t timestamp, int64_t pts)
{
	harness.audio_timestamp = timestamp;
	harness.audio_pts = pts;
	os_event_signal(harness.audio_command);
	assert_int_equal(os_event_timedwait(harness.audio_done, 5000), 0);
}

static void submit_video(uint64_t timestamp)
{
	struct video_frame frame;
	assert_true(video_output_lock_frame(harness.video, &frame, 1, timestamp));
	memset(frame.data[0], 0, frame.linesize[0] * 64);
	video_output_unlock_frame(harness.video);
	assert_int_equal(os_event_timedwait(harness.video_done, 5000), 0);
}

enum caption_kind {
	CAPTION_NONE,
	CAPTION_TEXT,
	CAPTION_CEA708,
};

static void queue_captions(enum caption_kind kind)
{
	if (kind == CAPTION_TEXT) {
		/* A short duration, so that on h264 the next page is due on the
		 * next frame, as it is on a codec that cannot carry them. */
		for (int i = 0; i < TEXT_PAGES; i++)
			obs_output_output_caption_text2(harness.output, "caption", 0.001);
	} else if (kind == CAPTION_CEA708) {
		/* Two field 1 CEA-608 byte pairs with odd parity: "AB", "CD". */
		static const uint8_t cc[] = {0x04, 0xc1, 0xc2, 0x04, 0x43, 0xc4};
		struct obs_source_cea_708 captions = {
			.data = cc,
			.packets = sizeof(cc) / 3,
		};
		obs_output_caption(harness.output, &captions);
	}
}

static bool captions_queued(void)
{
	struct caption_track_data *ctrack = harness.output->caption_tracks[0];
	pthread_mutex_lock(&ctrack->caption_mutex);
	bool queued = ctrack->caption_head || ctrack->caption_data.size;
	pthread_mutex_unlock(&ctrack->caption_mutex);
	return queued;
}

/* bnum_allocs() counts every thread's allocations, and core video and the
 * output's stop keep running briefly after a capture. Wait until the count
 * is steady (unchanged for 200 ms, within 3 s). */
static long settled_allocs(void)
{
	long count = bnum_allocs();
	int steady = 0;
	for (int i = 0; i < 300 && steady < 20; i++) {
		os_sleep_ms(10);
		long now = bnum_allocs();
		steady = now == count ? steady + 1 : 0;
		count = now;
	}
	return count;
}

/* After a capture, wait up to 3 s for the count to come back to `before`:
 * what a capture leaks never comes back. Returns how far it stayed off. */
static long allocs_since(long before)
{
	long delta = bnum_allocs() - before;
	for (int i = 0; i < 300 && delta != 0; i++) {
		os_sleep_ms(10);
		delta = bnum_allocs() - before;
	}
	return delta;
}

/* One capture: start, queue the captions, run STEPS frames with audio
 * alongside, then stop. Returns how far bnum_allocs() moved. */
static long capture(enum caption_kind kind)
{
	atomic_store(&harness.video_packets, 0);
	atomic_store(&harness.captioned_packets, 0);
	long before = settled_allocs();

	assert_true(obs_output_start(harness.output));
	assert_true(video_output_connect(harness.video, NULL, video_done, NULL));
	queue_captions(kind);

	const uint64_t t0 = harness.clock;
	/* Preroll: the paired video encoder starts once audio has arrived. */
	submit_audio(t0, -1);
	submit_video(t0);
	submit_audio(t0 + BLOCK_NS, 0);
	for (uint32_t k = 1; k <= STEPS; k++) {
		submit_audio(t0 + (k + 1) * BLOCK_NS, (int64_t)k * AUDIO_FRAME);
		submit_video(t0 + k * FRAME_NS);
	}
	harness.clock = t0 + 10 * 1000000000ULL;

	/* Every caption was taken off the queue, so add_caption() ran. */
	assert_false(captions_queued());
	assert_true(atomic_load(&harness.video_packets) > TEXT_PAGES);

	os_event_reset(harness.deactivated);
	obs_output_stop(harness.output);
	assert_int_equal(os_event_timedwait(harness.deactivated, 5000), 0);
	video_output_disconnect(harness.video, video_done, NULL);

	long delta = allocs_since(before);
	print_message("captions %d: %zu video packets, %zu with captions, allocations %+ld\n", (int)kind,
		      atomic_load(&harness.video_packets), atomic_load(&harness.captioned_packets), delta);
	return delta;
}

/* A warm-up capture of the same kind first, so that what a capture keeps
 * until the next start (the CEA-708 queue) and one-time allocations do not
 * count. */
static void check_no_leak(const char *video_encoder_id, enum caption_kind kind, bool carries_captions)
{
	new_output(video_encoder_id);
	capture(kind);
	long delta = capture(kind);
	if (carries_captions)
		assert_true(atomic_load(&harness.captioned_packets) > 0);
	else
		assert_int_equal(atomic_load(&harness.captioned_packets), 0);
	assert_int_equal(delta, 0);
}

static void no_captions_allocates_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	check_no_leak("test_caption_prores", CAPTION_NONE, false);
}

static void text_captions_on_prores_free_the_copy(void **state)
{
	UNUSED_PARAMETER(state);
	check_no_leak("test_caption_prores", CAPTION_TEXT, false);
}

static void cea708_captions_on_prores_free_the_copy(void **state)
{
	UNUSED_PARAMETER(state);
	check_no_leak("test_caption_prores", CAPTION_CEA708, false);
}

static void text_captions_on_h264_are_inserted(void **state)
{
	UNUSED_PARAMETER(state);
	check_no_leak("test_caption_h264", CAPTION_TEXT, true);
}

static void cea708_captions_on_h264_are_inserted(void **state)
{
	UNUSED_PARAMETER(state);
	check_no_leak("test_caption_h264", CAPTION_CEA708, true);
}

int main(void)
{
	if (!getenv("DISPLAY"))
		return 77; /* Run with xvfb-run on headless Linux hosts. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(no_captions_allocates_nothing),
		cmocka_unit_test(text_captions_on_prores_free_the_copy),
		cmocka_unit_test(cea708_captions_on_prores_free_the_copy),
		cmocka_unit_test(text_captions_on_h264_are_inserted),
		cmocka_unit_test(cea708_captions_on_h264_are_inserted),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
