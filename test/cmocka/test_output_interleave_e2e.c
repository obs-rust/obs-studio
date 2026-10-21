/* The encoded packet interleaver end to end through the libobs that ships.
 * test_output_interleave compiles obs-output-interleave.c.h into itself, so
 * it characterizes the C source only; this test drives a real output with
 * one video and two audio tracks, registered in the test, and checks what
 * its encoded_packet callback receives: the start on a keyframe with every
 * track offset to 0, the dts order with video before audio at an equal dts,
 * the premature prune of a keyframe that audio starts more than one frame
 * after, and a stop and a restart of the same output. The same cases run
 * again with the video encoder in slot 1 and slot 0 empty, which the public
 * API allows for a multitrack video output.
 *
 * The media is fed by hand, as in test_output_interleave_restart: each step
 * submits one audio block (one packet from each audio encoder, with a dts the
 * test picks) and then one video frame (a keyframe every third frame), and
 * waits until the encoders and the output have handled it. Audio encoders
 * paired with the video start at the video's start time, so an audio packet's
 * dts sets its place relative to the first video frame. Nothing depends on
 * the wall clock. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <media-io/video-frame.h>
#include <util/threading.h>

#define AUDIO_TRACKS 2
#define FPS 50
#define FRAME_USEC (1000000 / FPS)  /* 20000 us */
#define AUDIO_FRAME 960             /* 20 ms at 48 kHz: one video frame */
#define BLOCK_NS (1024ULL * 1000000000ULL / 48000)
#define FRAME_NS (1000000000ULL / FPS)
#define KEYFRAME_INTERVAL 3
#define STEPS 12
#define MAX_RECORDS 256

struct record {
	enum obs_encoder_type type;
	size_t track;
	int64_t dts;
	int64_t dts_usec;
	bool keyframe;
	uint32_t index; /* the video frame number, from the payload */
};

static struct {
	obs_output_t *output;
	obs_encoder_t *video_encoder;
	obs_encoder_t *audio_encoders[AUDIO_TRACKS];
	video_t *video;
	audio_t *audio;
	os_event_t *audio_command;
	os_event_t *audio_done;
	os_event_t *video_done;
	os_event_t *deactivated;
	atomic_bool audio_enabled;
	atomic_bool shutdown;
	uint64_t audio_timestamp;
	int64_t audio_dts;
	bool audio_pending[AUDIO_TRACKS];
	uint32_t video_index;
	uint64_t clock;    /* next media timestamp, ns; never goes back */
	size_t video_slot; /* the output's video encoder slot */

	pthread_mutex_t records_mutex;
	struct record records[MAX_RECORDS];
	size_t num_records;
} harness;

static const char *test_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "interleave e2e test";
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
	static uint32_t video_payload;
	static uint32_t audio_payload;

	packet->type = obs_encoder_get_type(data);
	if (packet->type == OBS_ENCODER_VIDEO) {
		video_payload = harness.video_index;
		packet->data = (uint8_t *)&video_payload;
		packet->pts = packet->dts = frame->pts;
		packet->keyframe = harness.video_index % KEYFRAME_INTERVAL == 0;
		*received = true;
	} else {
		size_t track = data == harness.audio_encoders[1] ? 1 : 0;
		/* One packet per submitted block, from each audio encoder. */
		*received = harness.audio_pending[track];
		if (!*received)
			return true;
		harness.audio_pending[track] = false;
		audio_payload = UINT32_MAX;
		packet->data = (uint8_t *)&audio_payload;
		packet->pts = packet->dts = harness.audio_dts;
		packet->keyframe = true;
	}
	packet->size = sizeof(uint32_t);
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
	if (!packet)
		return;

	pthread_mutex_lock(&harness.records_mutex);
	if (harness.num_records < MAX_RECORDS) {
		struct record *r = &harness.records[harness.num_records++];
		r->type = packet->type;
		r->track = packet->track_idx;
		r->dts = packet->dts;
		r->dts_usec = packet->dts_usec;
		r->keyframe = packet->keyframe;
		r->index = UINT32_MAX;
		if (packet->size >= sizeof(uint32_t))
			memcpy(&r->index, packet->data + packet->size - sizeof(uint32_t), sizeof(uint32_t));
	}
	pthread_mutex_unlock(&harness.records_mutex);
}

static bool audio_input(void *data, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
			struct audio_output_data *mixes)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(start);
	UNUSED_PARAMETER(end);
	UNUSED_PARAMETER(mixes); /* audio_output_open supplies zeroed silence. */
	if (!atomic_load(&harness.audio_enabled) || atomic_load(&harness.shutdown) || !(mixers & 1))
		return false;
	os_event_wait(harness.audio_command);
	if (atomic_load(&harness.shutdown))
		return false;
	*timestamp = harness.audio_timestamp;
	for (size_t i = 0; i < AUDIO_TRACKS; i++)
		harness.audio_pending[i] = harness.audio_dts >= 0;
	return true;
}

/* Audio callbacks run in reverse registration order, video callbacks in
 * registration order, so these run after the encoders and the output. */
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

static void create_output(void)
{
	harness.output = obs_output_create("test_interleave_e2e_output", "output", NULL, NULL);
	assert_non_null(harness.output);
	obs_output_set_video_encoder2(harness.output, harness.video_encoder, harness.video_slot);
	for (size_t i = 0; i < AUDIO_TRACKS; i++)
		obs_output_set_audio_encoder(harness.output, harness.audio_encoders[i], i);
	signal_handler_connect(obs_output_get_signal_handler(harness.output), "deactivate", on_deactivate, NULL);
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

	struct obs_encoder_info encoder = {
		.id = "test_interleave_e2e_video",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = test_name,
		.create = encoder_create,
		.destroy = test_destroy,
		.encode = encode,
	};
	obs_register_encoder(&encoder);
	encoder.id = "test_interleave_e2e_audio";
	encoder.type = OBS_ENCODER_AUDIO;
	encoder.codec = "aac";
	encoder.get_frame_size = audio_frame_size;
	obs_register_encoder(&encoder);
	struct obs_output_info output = {
		.id = "test_interleave_e2e_output",
		.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK_AV,
		.get_name = test_name,
		.create = output_create,
		.destroy = test_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);

	assert_int_equal(pthread_mutex_init(&harness.records_mutex, NULL), 0);
	assert_int_equal(os_event_init(&harness.audio_command, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&harness.audio_done, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&harness.video_done, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&harness.deactivated, OS_EVENT_TYPE_AUTO), 0);
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

	harness.video_encoder = obs_video_encoder_create("test_interleave_e2e_video", "video", NULL, NULL);
	assert_non_null(harness.video_encoder);
	obs_encoder_set_video(harness.video_encoder, harness.video);
	for (size_t i = 0; i < AUDIO_TRACKS; i++) {
		/* Both tracks encode mix 0, so audio_done runs after both. */
		harness.audio_encoders[i] =
			obs_audio_encoder_create("test_interleave_e2e_audio", i ? "audio 2" : "audio 1", NULL, 0, NULL);
		assert_non_null(harness.audio_encoders[i]);
		obs_encoder_set_audio(harness.audio_encoders[i], harness.audio);
	}
	harness.clock = 1000000000ULL;
	return 0;
}

static void reset_output(size_t video_slot)
{
	/* A failed test leaves its capture running. */
	video_output_disconnect(harness.video, video_done, NULL);
	if (harness.output) {
		if (obs_output_active(harness.output))
			obs_output_force_stop(harness.output);
		obs_output_release(harness.output);
	}
	harness.video_slot = video_slot;
	create_output();
}

static int new_output(void **state)
{
	UNUSED_PARAMETER(state);
	reset_output(0);
	return 0;
}

/* The video encoder in slot 1 only, slot 0 empty, as
 * obs_output_set_video_encoder2() allows on a multitrack video output. */
static int new_output_video_slot_1(void **state)
{
	UNUSED_PARAMETER(state);
	reset_output(1);
	assert_null(obs_output_get_video_encoder2(harness.output, 0));
	assert_ptr_equal(obs_output_get_video_encoder2(harness.output, 1), harness.video_encoder);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	atomic_store(&harness.shutdown, true);
	if (harness.audio_command)
		os_event_signal(harness.audio_command);
	if (harness.output && obs_output_active(harness.output))
		obs_output_force_stop(harness.output);
	obs_output_release(harness.output);
	obs_encoder_release(harness.video_encoder);
	for (size_t i = 0; i < AUDIO_TRACKS; i++)
		obs_encoder_release(harness.audio_encoders[i]);
	audio_output_close(harness.audio);
	video_output_close(harness.video);
	os_event_destroy(harness.audio_command);
	os_event_destroy(harness.audio_done);
	os_event_destroy(harness.video_done);
	os_event_destroy(harness.deactivated);
	pthread_mutex_destroy(&harness.records_mutex);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* One audio block; with dts >= 0, one packet of that dts from each track. */
static void submit_audio(uint64_t timestamp, int64_t dts)
{
	harness.audio_timestamp = timestamp;
	harness.audio_dts = dts;
	os_event_signal(harness.audio_command);
	assert_int_equal(os_event_timedwait(harness.audio_done, 5000), 0);
}

static void submit_video(uint64_t timestamp, uint32_t index)
{
	struct video_frame frame;
	harness.video_index = index;
	assert_true(video_output_lock_frame(harness.video, &frame, 1, timestamp));
	memset(frame.data[0], 0, frame.linesize[0] * 64);
	video_output_unlock_frame(harness.video);
	assert_int_equal(os_event_timedwait(harness.video_done, 5000), 0);
}

static size_t num_records(void)
{
	pthread_mutex_lock(&harness.records_mutex);
	size_t num = harness.num_records;
	pthread_mutex_unlock(&harness.records_mutex);
	return num;
}

/* One capture. Video frame k (from 0) is at k frames; the first audio packet
 * is `late` frames after video frame 0, and the next ones follow one frame
 * apart, each submitted before the video frame of the same step, so at an
 * equal dts the audio is queued first. */
static void capture(int64_t late)
{
	pthread_mutex_lock(&harness.records_mutex);
	harness.num_records = 0;
	pthread_mutex_unlock(&harness.records_mutex);

	assert_true(obs_output_start(harness.output));
	assert_true(video_output_connect(harness.video, NULL, video_done, NULL));
	atomic_store(&harness.audio_enabled, true);

	const uint64_t t0 = harness.clock;
	/* Preroll: the paired video encoder starts once audio has arrived. */
	submit_audio(t0, -1);
	submit_video(t0, 0);
	submit_audio(t0 + BLOCK_NS, late * AUDIO_FRAME);
	for (uint32_t k = 1; k <= STEPS; k++) {
		submit_audio(t0 + (k + 1) * BLOCK_NS, (late + k) * AUDIO_FRAME);
		submit_video(t0 + k * FRAME_NS, k);
	}
	harness.clock = t0 + 10 * 1000000000ULL;
}

static void stop_capture(void)
{
	os_event_reset(harness.deactivated);
	obs_output_stop(harness.output);
	assert_int_equal(os_event_timedwait(harness.deactivated, 5000), 0);
	video_output_disconnect(harness.video, video_done, NULL);
}

/* What every capture must deliver: the first packet is the keyframe of video
 * frame `first_index` at dts 0, no earlier frame is sent, each audio track
 * starts after it, near 0, and the dts never decrease across the tracks,
 * with video before audio at an equal dts. */
static void check_delivery(uint32_t first_index)
{
	pthread_mutex_lock(&harness.records_mutex);
	size_t num = harness.num_records;
	pthread_mutex_unlock(&harness.records_mutex);

	for (size_t i = 0; i < num; i++) {
		const struct record *r = &harness.records[i];
		if (r->type == OBS_ENCODER_VIDEO)
			print_message("  %2zu: video frame %u dts %lld%s\n", i, r->index, (long long)r->dts_usec,
				      r->keyframe ? " keyframe" : "");
		else
			print_message("  %2zu: audio %zu dts %lld\n", i, r->track, (long long)r->dts_usec);
	}

	assert_true(num > 0);
	const struct record *first = &harness.records[0];
	assert_int_equal(first->type, OBS_ENCODER_VIDEO);
	assert_true(first->keyframe);
	assert_int_equal(first->index, first_index);
	assert_int_equal(first->dts, 0);
	assert_int_equal(first->dts_usec, 0);

	size_t video = 0;
	size_t audio[AUDIO_TRACKS] = {0};
	for (size_t i = 0; i < num; i++) {
		const struct record *r = &harness.records[i];
		if (r->type == OBS_ENCODER_VIDEO) {
			assert_int_equal(r->track, harness.video_slot);
			assert_true(r->index >= first_index);
			video++;
		} else {
			assert_true(r->track < AUDIO_TRACKS);
			if (!audio[r->track]++) {
				/* The track's first packet: offset to start near 0. */
				assert_true(r->dts >= 0);
				assert_true(r->dts_usec >= first->dts_usec);
				assert_true(r->dts_usec <= FRAME_USEC);
			}
		}
		if (!i)
			continue;
		const struct record *prev = &harness.records[i - 1];
		assert_true(r->dts_usec >= prev->dts_usec);
		if (r->dts_usec == prev->dts_usec)
			assert_false(prev->type == OBS_ENCODER_AUDIO && r->type == OBS_ENCODER_VIDEO);
	}
	assert_true(video >= (STEPS - first_index) / 2);
	for (size_t i = 0; i < AUDIO_TRACKS; i++)
		assert_true(audio[i] >= (STEPS - first_index) / 2);
}

static void starts_on_keyframe_and_interleaves(void **state)
{
	UNUSED_PARAMETER(state);
	/* Audio and video start together. */
	capture(0);
	check_delivery(0);
	stop_capture();
}

static void audio_one_frame_late_keeps_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	/* Exactly one frame later is not premature (strict >): the output
	 * starts at the first keyframe, the audio offset to 0 as well. */
	capture(1);
	check_delivery(0);
	stop_capture();
}

static void late_audio_starts_at_next_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	/* Characterized, not endorsed: audio that starts more than one frame
	 * after the first keyframe drops it, and the output starts at the
	 * next keyframe (frame 3). */
	capture(2);
	check_delivery(KEYFRAME_INTERVAL);
	stop_capture();
}

static void stop_and_restart(void **state)
{
	UNUSED_PARAMETER(state);
	capture(0);
	check_delivery(0);
	stop_capture();

	/* Nothing reaches a stopped output. */
	size_t delivered = num_records();
	assert_true(video_output_connect(harness.video, NULL, video_done, NULL));
	submit_audio(harness.clock, 0);
	submit_video(harness.clock, 0);
	video_output_disconnect(harness.video, video_done, NULL);
	harness.clock += 10 * 1000000000ULL;
	assert_int_equal(num_records(), delivered);
	assert_false(obs_output_active(harness.output));

	/* The same output starts afresh: the start rules and the offsets apply
	 * to the new capture, not the old one. */
	capture(2);
	check_delivery(KEYFRAME_INTERVAL);
	stop_capture();
}

/* The same case with the video encoder in slot 1: the start rules follow the
 * output's first video encoder, not slot 0. */
#define VIDEO_SLOT_1(f) {.name = "video_slot_1_" #f, .test_func = f, .setup_func = new_output_video_slot_1}

int main(void)
{
	if (!getenv("DISPLAY"))
		return 77; /* Run with xvfb-run on headless Linux hosts. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(starts_on_keyframe_and_interleaves, new_output),
		cmocka_unit_test_setup(audio_one_frame_late_keeps_keyframe, new_output),
		cmocka_unit_test_setup(late_audio_starts_at_next_keyframe, new_output),
		cmocka_unit_test_setup(stop_and_restart, new_output),
		VIDEO_SLOT_1(starts_on_keyframe_and_interleaves),
		VIDEO_SLOT_1(audio_one_frame_late_keeps_keyframe),
		VIDEO_SLOT_1(late_audio_starts_at_next_keyframe),
		VIDEO_SLOT_1(stop_and_restart),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
