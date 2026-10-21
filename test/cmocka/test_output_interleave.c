#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

/*
 * Characterization tests for the encoded packet interleaver in
 * libobs/obs-output-interleave.c (#1594): interleave_packets(), the encoded
 * packet callback of an output with both video and audio, and the functions
 * it calls: the encoder index and received flags, the start index and the
 * premature prune, the keyframe wait, the dts offsets, insertion order and
 * resort, the discard of unused audio, sending, the encoder group keyframe
 * check, and free_packets(). They pin the current C behavior for the Rust
 * port; cases marked "characterized, not endorsed" are odd but real.
 *
 * All of these are static, so the file is compiled into this test, which
 * therefore characterizes the C source only. The output is a zeroed struct
 * obs_output with only the fields the file reads set up; no libobs core is
 * started. The four obs-output.c functions the file calls are defined below:
 * active() and add_caption() are recording stubs, the two "first encoder
 * index" helpers are copies. The output's encoded_packet() and one packet
 * callback record what is sent. obs_encoder_packet_create_instance() is not
 * exported, so the test has a copy of it; packets are released with
 * libobs's obs_encoder_packet_release(), and teardown checks bnum_allocs(),
 * so a packet the interleaver fails to release fails the test.
 *
 * Time base of the cases: video 1/30 (one frame is 33333 us), audio 1/48000
 * with 1024-sample frames (21333 us). Encoder dts_usec values are system
 * times around T (1 s); after the start, the interleaver recomputes them from
 * the offset dts.
 */

#include "libobs/obs-output-interleave.c.h"

#define T 1000000LL
#define FRAME_US 33333LL
#define NUM_VIDEO 2
#define NUM_AUDIO 2

/* ------------------------------------------------------------------------- */
/* recorded calls */

struct sent {
	enum obs_encoder_type type;
	size_t track_idx;
	int64_t dts;
	int64_t pts;
	int64_t dts_usec;
	bool keyframe;
	uint8_t tag;
	/* interleaved_mutex was held when encoded_packet() ran */
	bool locked;
};

struct callback_call {
	uint8_t tag;
	bool time_found;
	struct encoder_packet_time time;
	bool callbacks_locked;
};

struct caption_call {
	size_t track_idx;
	bool locked;
};

#define MAX_RECORDS 64

static struct obs_output *out;
static struct obs_encoder *venc[NUM_VIDEO];
static struct obs_encoder *aenc[NUM_AUDIO];
static struct obs_encoder_group *group;

static struct sent sent[MAX_RECORDS];
static size_t num_sent;
static struct callback_call callback_calls[MAX_RECORDS];
static size_t num_callback_calls;
static struct caption_call caption_calls[MAX_RECORDS];
static size_t num_caption_calls;

static bool output_active;
static bool add_caption_result;
static int output_data_marker;

static bool mutex_held(pthread_mutex_t *mutex)
{
	if (pthread_mutex_trylock(mutex) != 0)
		return true;
	pthread_mutex_unlock(mutex);
	return false;
}

/* ------------------------------------------------------------------------- */
/* obs-output.c functions the file calls */

static inline bool active(const struct obs_output *output)
{
	assert_ptr_equal(output, out);
	return output_active;
}

/* Copies of obs-output.c's helpers: the first slot with an encoder. */
static bool get_first_audio_encoder_index(const struct obs_output *output, size_t *index)
{
	if (!index)
		return false;

	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		if (output->audio_encoders[i]) {
			*index = i;
			return true;
		}
	}

	return false;
}

static bool get_first_video_encoder_index(const struct obs_output *output, size_t *index)
{
	if (!index)
		return false;

	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i]) {
			*index = i;
			return true;
		}
	}

	return false;
}

/* The caption path stays in obs-output.c; send_interleaved() only decides
 * when to call it. */
static bool add_caption(struct obs_output *output, struct encoder_packet *pkt)
{
	assert_ptr_equal(output, out);
	assert_true(num_caption_calls < MAX_RECORDS);
	struct caption_call *call = &caption_calls[num_caption_calls++];
	call->track_idx = pkt->track_idx;
	call->locked = mutex_held(&output->caption_tracks[pkt->track_idx]->caption_mutex);
	return add_caption_result;
}

/* Not exported from libobs: the same reference-counted copy as in
 * obs-encoder.c, so libobs's obs_encoder_packet_release() frees it. */
void obs_encoder_packet_create_instance(struct encoder_packet *dst, const struct encoder_packet *src)
{
	long *p_refs;

	*dst = *src;
	p_refs = bmalloc(src->size + sizeof(long));
	dst->data = (void *)(p_refs + 1);
	*p_refs = 1;
	memcpy(dst->data, src->data, src->size);
}

/* The output's encoded_packet(): the output does not own the packet, the
 * interleaver releases it afterwards. */
static void encoded_packet(void *data, struct encoder_packet *packet)
{
	assert_ptr_equal(data, &output_data_marker);
	assert_true(num_sent < MAX_RECORDS);
	struct sent *s = &sent[num_sent++];
	s->type = packet->type;
	s->track_idx = packet->track_idx;
	s->dts = packet->dts;
	s->pts = packet->pts;
	s->dts_usec = packet->dts_usec;
	s->keyframe = packet->keyframe;
	s->tag = packet->size ? packet->data[0] : 0;
	s->locked = mutex_held(&out->interleaved_mutex);
}

static void packet_callback(obs_output_t *output, struct encoder_packet *pkt, struct encoder_packet_time *pkt_time,
			    void *param)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(output, out);
	assert_true(num_callback_calls < MAX_RECORDS);
	struct callback_call *call = &callback_calls[num_callback_calls++];
	call->tag = pkt->size ? pkt->data[0] : 0;
	call->time_found = pkt_time != NULL;
	if (pkt_time)
		call->time = *pkt_time;
	call->callbacks_locked = mutex_held(&output->pkt_callbacks_mutex);
}

/* ------------------------------------------------------------------------- */
/* log capture */

static char last_log[512];
static int last_log_level;
static size_t num_logs;
static size_t num_warnings;

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	UNUSED_PARAMETER(p);
	vsnprintf(last_log, sizeof(last_log), msg, args);
	last_log_level = lvl;
	num_logs++;
	if (lvl == LOG_WARNING)
		num_warnings++;
}

/* ------------------------------------------------------------------------- */
/* fixture */

static long allocs_before_setup;

static struct obs_encoder *make_encoder(const char *name, enum obs_encoder_type type)
{
	struct obs_encoder *encoder = bzalloc(sizeof(*encoder));
	encoder->context.name = (char *)name;
	encoder->info.type = type;
	encoder->timebase_num = 1;
	if (type == OBS_ENCODER_VIDEO) {
		encoder->timebase_den = 30;
	} else {
		encoder->timebase_den = 48000;
		encoder->framesize = 1024;
	}
	return encoder;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	allocs_before_setup = bnum_allocs();

	out = bzalloc(sizeof(*out));
	out->context.name = "interleaved output";
	out->context.data = &output_data_marker;
	out->info.encoded_packet = encoded_packet;
	assert_int_equal(pthread_mutex_init(&out->interleaved_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&out->pkt_callbacks_mutex, NULL), 0);

	static const char *video_names[NUM_VIDEO] = {"video 0", "video 1"};
	static const char *audio_names[NUM_AUDIO] = {"audio 0", "audio 1"};
	for (size_t i = 0; i < NUM_VIDEO; i++) {
		venc[i] = make_encoder(video_names[i], OBS_ENCODER_VIDEO);
		out->caption_tracks[i] = bzalloc(sizeof(struct caption_track_data));
		assert_int_equal(pthread_mutex_init(&out->caption_tracks[i]->caption_mutex, NULL), 0);
		deque_init(&out->caption_tracks[i]->caption_data);
	}
	for (size_t i = 0; i < NUM_AUDIO; i++)
		aenc[i] = make_encoder(audio_names[i], OBS_ENCODER_AUDIO);

	/* One video and one audio encoder unless a case adds more. */
	out->video_encoders[0] = venc[0];
	out->audio_encoders[0] = aenc[0];

	/* What reset_packet_data() in obs-output.c leaves at each start. */
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++)
		out->highest_video_ts[i] = INT64_MIN;
	out->interleaver_max_batch_size = 8;

	struct packet_callback cb = {packet_callback, NULL};
	da_push_back(out->pkt_callbacks, &cb);

	group = NULL;
	output_active = true;
	add_caption_result = true;
	num_sent = 0;
	num_callback_calls = 0;
	num_caption_calls = 0;

	last_log[0] = 0;
	last_log_level = -1;
	num_logs = 0;
	num_warnings = 0;
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	base_set_log_handler(NULL, NULL);

	free_packets(out);
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++)
		da_free(out->encoder_packet_times[i]);
	da_free(out->keyframe_group_tracking);
	da_free(out->pkt_callbacks);

	for (size_t i = 0; i < NUM_VIDEO; i++) {
		struct caption_track_data *ctrack = out->caption_tracks[i];
		while (ctrack->caption_head) {
			struct caption_text *next = ctrack->caption_head->next;
			bfree(ctrack->caption_head);
			ctrack->caption_head = next;
		}
		deque_free(&ctrack->caption_data);
		pthread_mutex_destroy(&ctrack->caption_mutex);
		bfree(ctrack);
		bfree(venc[i]);
	}
	for (size_t i = 0; i < NUM_AUDIO; i++)
		bfree(aenc[i]);
	if (group) {
		pthread_mutex_destroy(&group->mutex);
		bfree(group);
		group = NULL;
	}

	pthread_mutex_destroy(&out->pkt_callbacks_mutex);
	pthread_mutex_destroy(&out->interleaved_mutex);
	bfree(out);
	out = NULL;

	/* every packet the interleaver copied has been released */
	assert_int_equal(bnum_allocs(), allocs_before_setup);
	return 0;
}

static void use_tracks(size_t num_video, size_t num_audio)
{
	for (size_t i = 0; i < NUM_VIDEO; i++)
		out->video_encoders[i] = i < num_video ? venc[i] : NULL;
	for (size_t i = 0; i < NUM_AUDIO; i++)
		out->audio_encoders[i] = i < num_audio ? aenc[i] : NULL;
}

/* Packet data is one byte, a tag the case picks, so sent packets can be told
 * apart. The encoder owns it; the interleaver copies it. */
static uint8_t tags[256];

static struct encoder_packet video_packet(size_t track, int64_t dts, int64_t pts, int64_t dts_usec, bool keyframe,
					  uint8_t tag)
{
	tags[tag] = tag;
	return (struct encoder_packet){
		.data = &tags[tag],
		.size = 1,
		.pts = pts,
		.dts = dts,
		.timebase_num = 1,
		.timebase_den = 30,
		.type = OBS_ENCODER_VIDEO,
		.keyframe = keyframe,
		.dts_usec = dts_usec,
		.encoder = venc[track],
		.track_idx = 99,
	};
}

static struct encoder_packet audio_packet(size_t track, int64_t dts, int64_t dts_usec, uint8_t tag)
{
	tags[tag] = tag;
	return (struct encoder_packet){
		.data = &tags[tag],
		.size = 1,
		.pts = dts,
		.dts = dts,
		.timebase_num = 1,
		.timebase_den = 48000,
		.type = OBS_ENCODER_AUDIO,
		.dts_usec = dts_usec,
		.encoder = aenc[track],
		.track_idx = 99,
	};
}

/* An encoder's video packet comes with its packet time; cts carries the tag. */
static void feed_video(size_t track, int64_t dts, int64_t pts, int64_t dts_usec, bool keyframe, uint8_t tag)
{
	struct encoder_packet pkt = video_packet(track, dts, pts, dts_usec, keyframe, tag);
	struct encoder_packet_time ept = {.pts = pts, .cts = tag};
	interleave_packets(out, &pkt, &ept);
	/* the caller's packet is copied, not taken */
	assert_ptr_equal(pkt.data, &tags[tag]);
	assert_int_equal(pkt.track_idx, track);
}

static void feed_audio(size_t track, int64_t dts, int64_t dts_usec, uint8_t tag)
{
	struct encoder_packet pkt = audio_packet(track, dts, dts_usec, tag);
	interleave_packets(out, &pkt, NULL);
	assert_ptr_equal(pkt.data, &tags[tag]);
	assert_int_equal(pkt.track_idx, track);
}

/* Queue a copy directly, as the interleaver would have. */
static void queue_packet(struct encoder_packet pkt, size_t track)
{
	struct encoder_packet copy;
	pkt.track_idx = track;
	obs_encoder_packet_create_instance(&copy, &pkt);
	da_push_back(out->interleaved_packets, &copy);
}

static void push_packet_time(size_t track, int64_t pts, uint64_t cts)
{
	struct encoder_packet_time ept = {.pts = pts, .cts = cts};
	da_push_back(out->encoder_packet_times[track], &ept);
}

static size_t queued(void)
{
	return out->interleaved_packets.num;
}

static struct encoder_packet *queued_at(size_t idx)
{
	assert_true(idx < out->interleaved_packets.num);
	return &out->interleaved_packets.array[idx];
}

static uint8_t tag_at(size_t idx)
{
	return queued_at(idx)->data[0];
}

static void assert_queue_tags(const uint8_t *expected, size_t n)
{
	assert_int_equal(queued(), n);
	for (size_t i = 0; i < n; i++)
		assert_int_equal(tag_at(i), expected[i]);
}

/* ------------------------------------------------------------------------- */
/* encoder index and received flags */

static void encoder_index_is_the_encoder_slot(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 2);

	struct encoder_packet v1 = video_packet(1, 0, 0, T, true, 1);
	struct encoder_packet v0 = video_packet(0, 0, 0, T, true, 2);
	struct encoder_packet a1 = audio_packet(1, 0, T, 3);
	struct encoder_packet a0 = audio_packet(0, 0, T, 4);
	assert_int_equal(get_encoder_index(out, &v1), 1);
	assert_int_equal(get_encoder_index(out, &v0), 0);
	assert_int_equal(get_encoder_index(out, &a1), 1);
	assert_int_equal(get_encoder_index(out, &a0), 0);

	/* The type picks the array: an audio encoder's packet marked video is
	 * not found. That is an assert(false), then index 0 in a build without
	 * asserts. */
#ifdef NDEBUG
	struct encoder_packet stray = audio_packet(1, 0, T, 5);
	stray.type = OBS_ENCODER_VIDEO;
	assert_int_equal(get_encoder_index(out, &stray), 0);
#endif

	/* interleave_packets() writes the index into the caller's packet */
	feed_audio(1, 0, T, 6);
	assert_int_equal(queued_at(0)->track_idx, 1);
}

static void received_flags_per_video_track_one_for_audio(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 2);

	struct encoder_packet v1 = video_packet(1, 0, 0, T, true, 1);
	v1.track_idx = 1;
	check_received(out, &v1);
	assert_false(out->received_video[0]);
	assert_true(out->received_video[1]);
	assert_false(out->received_audio);

	/* any audio track sets the one audio flag */
	struct encoder_packet a1 = audio_packet(1, 0, T, 2);
	a1.track_idx = 1;
	check_received(out, &a1);
	assert_true(out->received_audio);
	assert_false(out->received_video[0]);

	/* Through interleave_packets(): each packet sets its flag before the
	 * start, and nothing is sent while one video track has nothing. */
	memset(out->received_video, 0, sizeof(out->received_video));
	out->received_audio = false;
	feed_audio(0, 48000, T + 1000, 3);
	assert_true(out->received_audio);
	feed_video(1, 30, 30, T, true, 4);
	assert_true(out->received_video[1]);
	assert_false(out->received_video[0]);
	assert_int_equal(num_sent, 0);
	assert_int_equal(queued(), 2);
}

static void inactive_output_ignores_the_packet(void **state)
{
	UNUSED_PARAMETER(state);
	output_active = false;

	struct encoder_packet pkt = video_packet(0, 30, 30, T, true, 1);
	interleave_packets(out, &pkt, NULL);
	/* not even the index is set */
	assert_int_equal(pkt.track_idx, 99);
	assert_int_equal(queued(), 0);
	assert_false(out->received_video[0]);

	/* Characterized, not endorsed: with a delay running, the delay hands the
	 * callback a copy that the callback owns (process_delay()), and this
	 * early return does not release it. The test releases it. */
	out->active_delay_ns = 1;
	struct encoder_packet held;
	obs_encoder_packet_create_instance(&held, &pkt);
	long allocs = bnum_allocs();
	interleave_packets(out, &held, NULL);
	assert_int_equal(bnum_allocs(), allocs);
	obs_encoder_packet_release(&held);
}

/* ------------------------------------------------------------------------- */
/* keyframe wait */

static void video_waits_for_a_keyframe(void **state)
{
	UNUSED_PARAMETER(state);

	feed_audio(0, 48000, T - 20000, 1);
	feed_audio(0, 49024, T + 1333, 2);
	assert_true(out->received_audio);

	/* A first video packet that is not a keyframe is dropped, and so is
	 * every queued packet with a lower dts_usec (here the first audio).
	 * The packet at or after it stays. */
	feed_video(0, 30, 30, T, false, 3);
	assert_false(out->received_video[0]);
	assert_queue_tags((uint8_t[]){2}, 1);
	assert_int_equal(out->encoder_packet_times[0].num, 0);
	assert_true(out->received_audio);

	/* The keyframe is queued and sets the flag. */
	feed_video(0, 31, 31, T + FRAME_US, true, 4);
	assert_true(out->received_video[0]);
	assert_int_equal(num_sent, 0);
	assert_queue_tags((uint8_t[]){2, 4}, 2);
	/* its packet time is kept */
	assert_int_equal(out->encoder_packet_times[0].num, 1);

	/* The start failed (no audio past the keyframe yet), but once a track
	 * has its keyframe, later non-keyframes are queued. */
	assert_false(out->received_audio);
	feed_video(0, 32, 32, T + 2 * FRAME_US, false, 5);
	assert_queue_tags((uint8_t[]){2, 4, 5}, 3);
}

static void keyframe_wait_discards_other_tracks(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);

	/* Track 1 has its keyframe, track 0 has not. */
	feed_video(1, 30, 30, T, true, 1);
	feed_audio(0, 48000, T + 1000, 2);
	assert_true(out->received_video[1]);
	assert_int_equal(out->encoder_packet_times[1].num, 1);

	/* Characterized, not endorsed: track 0's non-keyframe discards every
	 * packet below its dts_usec, track 1's keyframe and its packet time
	 * included, but track 1's received flag stays set. */
	feed_video(0, 31, 31, T + 2000, false, 3);
	assert_int_equal(queued(), 0);
	assert_int_equal(out->encoder_packet_times[1].num, 0);
	assert_true(out->received_video[1]);
	assert_false(out->received_video[0]);
	assert_int_equal(num_sent, 0);
}

static void delayed_packets_are_taken_not_copied(void **state)
{
	UNUSED_PARAMETER(state);
	/* With a delay running, the packet is the delay's copy and the callback
	 * owns it (process_delay()). */
	out->active_delay_ns = 1;

	struct encoder_packet src = audio_packet(0, 48000, T, 1);
	struct encoder_packet held;
	obs_encoder_packet_create_instance(&held, &src);
	interleave_packets(out, &held, NULL);
	/* queued as it is, no second copy */
	assert_ptr_equal(queued_at(0)->data, held.data);

	/* a non-keyframe that waits is released */
	src = video_packet(0, 30, 30, T + 1000, false, 2);
	obs_encoder_packet_create_instance(&held, &src);
	interleave_packets(out, &held, NULL);
	/* it discarded the queued audio below it too */
	assert_int_equal(queued(), 0);
	/* teardown's bnum_allocs() check shows both were released */
}

/* ------------------------------------------------------------------------- */
/* start index */

static void start_index_is_the_audio_closest_to_video(void **state)
{
	UNUSED_PARAMETER(state);

	queue_packet(audio_packet(0, 100, T - 100000, 1), 0);
	queue_packet(audio_packet(0, 200, T - 50000, 2), 0);
	queue_packet(audio_packet(0, 300, T - 10000, 3), 0);
	queue_packet(video_packet(0, 30, 30, T, true, 4), 0);
	queue_packet(audio_packet(0, 400, T + 10000, 5), 0);

	/* closest by |dts_usec - first video|; on a tie the earlier one */
	assert_int_equal(get_interleaved_start_idx(out), 2);

	/* the video packet when it comes first */
	free_packets(out);
	queue_packet(video_packet(0, 30, 30, T, true, 6), 0);
	queue_packet(audio_packet(0, 100, T + 100000, 7), 0);
	assert_int_equal(get_interleaved_start_idx(out), 0);
}

static void start_index_keeps_priming_audio(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(1, 2);

	/* Early AAC/Opus packets prime the encoder and have pts <= 0. If the
	 * first audio at the start index has pts <= 0, the start moves back to
	 * the first audio packet of every track. */
	queue_packet(audio_packet(0, 1024, T - 100000, 1), 0);
	queue_packet(audio_packet(1, -1024, T - 50000, 2), 1);
	queue_packet(audio_packet(1, 0, T - 10000, 3), 1);
	queue_packet(video_packet(0, 30, 30, T, true, 4), 0);
	queue_packet(audio_packet(0, 2048, T + 10000, 5), 0);

	/* Characterized, not endorsed: track 1's priming packet also keeps
	 * track 0's packet from 100 ms before the video, whose pts is > 0. */
	assert_int_equal(get_interleaved_start_idx(out), 0);

	/* without priming, the closest audio (index 2) */
	queued_at(2)->pts = 1;
	assert_int_equal(get_interleaved_start_idx(out), 2);
}

static void start_index_follows_video_track_0_only(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);

	queue_packet(video_packet(1, 30, 30, T - 10000, true, 1), 1);
	queue_packet(audio_packet(0, 100, T - 5000, 2), 0);
	queue_packet(video_packet(0, 30, 30, T, true, 3), 0);
	queue_packet(audio_packet(0, 200, T + 5000, 4), 0);

	/* Characterized, not endorsed: only track 0's first video counts, so
	 * track 1's keyframe before the start index would be discarded. */
	assert_int_equal(get_interleaved_start_idx(out), 1);
}

/* ------------------------------------------------------------------------- */
/* premature prune */

static void premature_audio_prunes_through_it(void **state)
{
	UNUSED_PARAMETER(state);

	/* Audio that starts more than one video frame after the first video
	 * packet: prune through the later of the two (index + 1). */
	queue_packet(video_packet(0, 30, 30, T, true, 1), 0);
	queue_packet(audio_packet(0, 48000, T + 40000, 2), 0);
	assert_int_equal(prune_premature_packets(out), 2);

	/* exactly one frame (33333 us) is not pruned: strict > */
	queued_at(1)->dts_usec = T + FRAME_US;
	assert_int_equal(prune_premature_packets(out), 0);

	/* audio before the video is not pruned either */
	queued_at(1)->dts_usec = T - 40000;
	assert_int_equal(prune_premature_packets(out), 0);
}

static void premature_prune_needs_every_track(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(1, 2);

	/* no video on track 0: -1, flags untouched */
	queue_packet(audio_packet(0, 48000, T, 1), 0);
	out->received_audio = true;
	assert_int_equal(prune_premature_packets(out), -1);
	assert_true(out->received_audio);

	/* an audio track with nothing queued: -1 and received_audio cleared */
	queue_packet(video_packet(0, 30, 30, T, true, 2), 0);
	assert_int_equal(prune_premature_packets(out), -1);
	assert_false(out->received_audio);

	/* prune_interleaved_packets() then does nothing */
	out->received_audio = true;
	assert_false(prune_interleaved_packets(out));
	assert_int_equal(queued(), 2);
}

static void premature_prune_uses_the_last_audio_track(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(1, 2);

	queue_packet(video_packet(0, 30, 30, T, true, 1), 0);
	queue_packet(audio_packet(1, 48000, T + 1000, 2), 1);
	queue_packet(audio_packet(0, 48000, T + 40000, 3), 0);

	/* Characterized, not endorsed: the decision uses the difference of the
	 * last audio track checked (track 1, 1 ms), not the largest one (track
	 * 0, 40 ms, computed as max_diff but not used), so this is not pruned. */
	assert_int_equal(prune_premature_packets(out), 0);

	/* with the late audio on track 1 it is, through the latest first audio */
	queued_at(1)->dts_usec = T + 40000;
	queued_at(2)->dts_usec = T + 1000;
	assert_int_equal(prune_premature_packets(out), 3);
}

static void premature_prune_widens_to_the_audio_frame(void **state)
{
	UNUSED_PARAMETER(state);

	/* 60 fps video: one frame is 16666 us. */
	struct encoder_packet v = video_packet(0, 60, 60, T, true, 1);
	v.timebase_den = 60;
	queue_packet(v, 0);
	queue_packet(audio_packet(0, 48000, T + 18000, 2), 0);

	/* one audio track: 18 ms > 16.7 ms, pruned */
	assert_int_equal(prune_premature_packets(out), 2);

	/* With more than one audio track, the limit widens to the longest audio
	 * frame, get_encoder_duration(). Characterized, not endorsed: that
	 * truncates 1e6 / 48000 to 20 before multiplying by the 1024-sample
	 * frame, so the limit is 20480 us, not 21333 us. */
	use_tracks(1, 2);
	queue_packet(audio_packet(1, 48000, T + 18000, 3), 1);
	assert_int_equal(get_encoder_duration(aenc[1]), 20480);
	assert_int_equal(prune_premature_packets(out), 0);
	queued_at(2)->dts_usec = T + 21000;
	assert_int_equal(prune_premature_packets(out), 3);
	queued_at(2)->dts_usec = T + 20480;
	assert_int_equal(prune_premature_packets(out), 0);
}

static void late_audio_drops_the_first_keyframe(void **state)
{
	UNUSED_PARAMETER(state);

	/* The case a delayed stream start hits: the first audio packet comes
	 * more than one frame after the keyframe. */
	feed_video(0, 30, 30, T, true, 1);
	feed_audio(0, 48000, T + 40000, 2);

	/* prune_start is 2: the keyframe, its packet time and the audio go.
	 * There is then no video, so the start fails and track 0 waits for a
	 * keyframe again. Nothing is sent. */
	assert_int_equal(queued(), 0);
	assert_int_equal(out->encoder_packet_times[0].num, 0);
	assert_false(out->received_video[0]);
	assert_true(out->received_audio);
	assert_int_equal(num_sent, 0);

	/* The next frames are not keyframes: dropped, and each takes the audio
	 * queued below it. */
	feed_video(0, 31, 31, T + FRAME_US, false, 3);
	feed_audio(0, 49024, T + 61333, 4);
	assert_queue_tags((uint8_t[]){4}, 1);
	feed_video(0, 32, 32, T + 2 * FRAME_US, false, 5);
	assert_int_equal(queued(), 0);
	assert_int_equal(num_sent, 0);

	/* Characterized, not endorsed: the output starts at the next keyframe,
	 * here one second later; everything before it is lost. With no audio
	 * queued at that keyframe, the start waits for audio. */
	feed_video(0, 60, 60, T + 1000000, true, 6);
	assert_false(out->received_audio);
	assert_int_equal(num_sent, 0);
	feed_audio(0, 96000, T + 1010000, 7);
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 6);
	assert_true(sent[0].keyframe);
	assert_int_equal(sent[0].dts, 0);
}

static void premature_prune_can_start_on_a_non_keyframe(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);

	/* Track 1 has no keyframe yet, so packets queue up: track 0's keyframe
	 * and the frames after it, and audio from 40 ms after the keyframe. */
	feed_video(0, 30, 30, T, true, 1);
	feed_audio(0, 48000, T + 40000, 2);
	feed_video(0, 31, 31, T + FRAME_US, false, 3);
	feed_audio(0, 49024, T + 50000, 4);
	feed_video(0, 32, 32, T + 2 * FRAME_US, false, 5);
	feed_audio(0, 50048, T + 70000, 6);
	assert_queue_tags((uint8_t[]){1, 3, 2, 4, 5, 6}, 6);
	assert_int_equal(num_sent, 0);

	/* Track 1's keyframe starts it. The premature prune drops through the
	 * first audio (the keyframe with it); the start index then drops the
	 * audio before the one closest to the first remaining video. */
	feed_video(1, 30, 30, T + 2 * FRAME_US, true, 7);

	/* Characterized, not endorsed: track 0 starts on a frame that is not a
	 * keyframe. */
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 5);
	assert_false(sent[0].keyframe);
	assert_int_equal(sent[0].dts, 0);
	assert_int_equal(out->video_offsets[0], 32);
	assert_int_equal(out->video_offsets[1], 30);
	assert_int_equal(out->audio_offsets[0], 50048);
	/* the dropped frames took their packet times; the sent one had its own */
	assert_true(callback_calls[0].time_found);
	assert_int_equal(callback_calls[0].time.cts, 5);
	assert_int_equal(out->encoder_packet_times[0].num, 0);
	assert_queue_tags((uint8_t[]){7, 6}, 2);
}

/* ------------------------------------------------------------------------- */
/* start: offsets and resort */

static void start_waits_for_every_track(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 2);

	feed_video(0, 30, 30, T, true, 10);
	feed_video(1, 30, 30, T, true, 11);
	feed_audio(0, 48000, T + 1000, 20);

	/* Every flag is set, but audio track 1 has nothing: the premature prune
	 * says -1 and clears received_audio. Nothing is sent or dropped. */
	assert_false(out->received_audio);
	assert_int_equal(num_sent, 0);
	assert_queue_tags((uint8_t[]){10, 11, 20}, 3);

	feed_audio(1, 48000, T + 2000, 21);

	/* Started: the first packet is sent at once. Each track gets its own
	 * offset, so both audio tracks start at dts 0 although they were 1 ms
	 * apart. */
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 10);
	assert_int_equal(sent[0].dts, 0);
	assert_true(sent[0].locked);
	assert_int_equal(out->video_offsets[0], 30);
	assert_int_equal(out->video_offsets[1], 30);
	assert_int_equal(out->audio_offsets[0], 48000);
	assert_int_equal(out->audio_offsets[1], 48000);
	assert_queue_tags((uint8_t[]){11, 20, 21}, 3);
	for (size_t i = 0; i < 3; i++) {
		assert_int_equal(queued_at(i)->dts, 0);
		assert_int_equal(queued_at(i)->dts_usec, 0);
	}
	assert_int_equal(out->highest_video_ts[0], 0);
	assert_int_equal(out->highest_video_ts[1], 0);
	assert_int_equal(out->highest_audio_ts, 0);
	assert_int_equal(out->total_frames, 1);
}

static void start_offsets_and_resort(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(1, 2);

	feed_audio(0, 48000, T - 1000, 20);
	feed_audio(1, 96000, T - 3000, 30);
	/* a B-frame stream: the keyframe's dts is below its pts */
	feed_video(0, 29, 30, T, true, 10);

	/* Characterized, not endorsed: the start index is track 0's audio,
	 * closest to the video, so track 1's only packet is discarded; then
	 * track 1 has no audio, and the start waits. */
	assert_false(out->received_audio);
	assert_queue_tags((uint8_t[]){20, 10}, 2);
	assert_int_equal(num_sent, 0);

	feed_audio(0, 49024, T + 20333, 21);
	assert_false(out->received_audio); /* track 1 still has nothing */
	feed_audio(1, 97024, T + 18333, 31);

	/* Started. The video offset is the first video pts, so the keyframe's
	 * dts is -1 (-33333 us). Each audio offset is that track's first dts. */
	assert_int_equal(out->video_offsets[0], 30);
	assert_int_equal(out->audio_offsets[0], 48000);
	assert_int_equal(out->audio_offsets[1], 97024);

	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 10);
	assert_int_equal(sent[0].dts, -1);
	assert_int_equal(sent[0].pts, 0);
	assert_int_equal(sent[0].dts_usec, -FRAME_US);
	/* its packet time, queued before the start, got the same offset */
	assert_int_equal(num_callback_calls, 1);
	assert_true(callback_calls[0].time_found);
	assert_int_equal(callback_calls[0].time.pts, 0);
	assert_int_equal(callback_calls[0].time.cts, 10);
	assert_int_equal(out->encoder_packet_times[0].num, 0);

	/* the rest, re-sorted by the offset dts_usec */
	assert_queue_tags((uint8_t[]){20, 31, 21}, 3);
	assert_int_equal(queued_at(0)->dts_usec, 0);
	assert_int_equal(queued_at(1)->dts_usec, 0);
	assert_int_equal(queued_at(2)->dts, 1024);
	assert_int_equal(queued_at(2)->dts_usec, 21333);
	assert_int_equal(out->highest_video_ts[0], -FRAME_US);
	assert_int_equal(out->highest_audio_ts, 21333);

	/* After the start, offsets apply as packets come: a video packet at the
	 * same dts_usec as queued audio goes before it, and is sent at once,
	 * with its packet time offset too. */
	feed_video(0, 30, 31, T + FRAME_US, false, 11);
	assert_int_equal(num_sent, 2);
	assert_int_equal(sent[1].tag, 11);
	assert_int_equal(sent[1].dts, 0);
	assert_int_equal(sent[1].pts, 1);
	assert_int_equal(callback_calls[1].time.pts, 1);
	assert_int_equal(out->highest_video_ts[0], 0);
	assert_queue_tags((uint8_t[]){20, 31, 21}, 3);
}

static void start_leaves_priming_audio_unshifted(void **state)
{
	UNUSED_PARAMETER(state);

	feed_audio(0, -1024, T - 21333, 20);
	feed_video(0, 30, 30, T, true, 10);
	feed_audio(0, 0, T, 21);

	/* The priming packet (pts <= 0) keeps the start at the first audio.
	 * Characterized, not endorsed: an audio offset is taken only from a
	 * dts > 0, so the track keeps offset 0 and its own timestamps. */
	assert_int_equal(out->audio_offsets[0], 0);
	assert_int_equal(out->video_offsets[0], 30);
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 20);
	assert_int_equal(sent[0].dts, -1024);
	assert_int_equal(sent[0].dts_usec, -21333);
	assert_queue_tags((uint8_t[]){10, 21}, 2);
	assert_int_equal(queued_at(0)->dts_usec, 0);
	assert_int_equal(queued_at(1)->dts_usec, 0);
	assert_int_equal(out->highest_audio_ts, 0);
}

static void start_with_only_priming_audio(void **state)
{
	UNUSED_PARAMETER(state);

	feed_audio(0, -2048, T - 10000, 20);
	feed_video(0, 30, 30, T, true, 10);
	feed_audio(0, -1024, T + 11333, 21);

	/* Every audio packet still primes the encoder (dts < 0, offset 0), so
	 * after the start all audio is below the video. highest_audio_ts is 0
	 * minus the first audio's system dts_usec, raised by the resort only to
	 * the highest (negative) offset audio. */
	assert_int_equal(out->audio_offsets[0], 0);
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 20);
	assert_int_equal(sent[0].dts_usec, -42666);
	assert_queue_tags((uint8_t[]){21, 10}, 2);
	assert_int_equal(queued_at(0)->dts_usec, -21333);
	assert_int_equal(out->highest_audio_ts, -21333);
	assert_int_equal(out->highest_video_ts[0], 0);
}

static void start_needs_audio_past_the_first_video(void **state)
{
	UNUSED_PARAMETER(state);

	feed_audio(0, 48000, T - 10000, 20);
	feed_video(0, 30, 30, T, true, 10);

	/* The last audio is before the video: received_audio is cleared, and
	 * nothing is dropped or sent. */
	assert_false(out->received_audio);
	assert_queue_tags((uint8_t[]){20, 10}, 2);
	assert_int_equal(num_sent, 0);

	feed_audio(0, 49024, T + 11333, 21);
	/* The audio 10 ms before the video starts with it at dts 0, and the
	 * video goes first. */
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 10);
	assert_queue_tags((uint8_t[]){20, 21}, 2);
	assert_int_equal(queued_at(0)->dts, 0);
	assert_int_equal(queued_at(1)->dts_usec, 21333);
	/* highest_audio_ts: 0 minus the first audio's dts_usec, then raised by
	 * the resort to the highest offset audio */
	assert_int_equal(out->highest_audio_ts, 21333);
}

static void offset_arithmetic(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 2);
	out->video_offsets[1] = 100;
	out->audio_offsets[1] = 48000;

	struct encoder_packet v = video_packet(1, 105, 107, 123, false, 1);
	v.track_idx = 1;
	struct encoder_packet_time ept = {.pts = 107};
	apply_interleaved_packet_offset(out, &v, &ept);
	assert_int_equal(v.dts, 5);
	assert_int_equal(v.pts, 7);
	assert_int_equal(ept.pts, 7);
	/* dts_usec is recomputed from the offset dts: dts * 1e6 / den */
	assert_int_equal(v.dts_usec, 166666);

	struct encoder_packet a = audio_packet(1, 49024, 123, 2);
	a.track_idx = 1;
	apply_interleaved_packet_offset(out, &a, NULL);
	assert_int_equal(a.dts, 1024);
	assert_int_equal(a.pts, 1024);
	assert_int_equal(a.dts_usec, 21333);

	/* the timebase numerator is not used: pts units are 1/den */
	struct encoder_packet ntsc = video_packet(0, 1001, 1001, 0, false, 3);
	ntsc.timebase_num = 1001;
	ntsc.timebase_den = 30000;
	ntsc.track_idx = 0;
	apply_interleaved_packet_offset(out, &ntsc, NULL);
	assert_int_equal(ntsc.dts_usec, 33366);

	/* queued packet times get the video offset of their track */
	push_packet_time(1, 110, 0);
	push_packet_time(0, 110, 0);
	apply_ept_offsets(out);
	assert_int_equal(out->encoder_packet_times[1].array[0].pts, 10);
	assert_int_equal(out->encoder_packet_times[0].array[0].pts, 110);
}

/* ------------------------------------------------------------------------- */
/* insertion order and resort */

static void insertion_order(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 2);

	struct encoder_packet p;

	p = audio_packet(0, 0, 100, 1);
	p.track_idx = 0;
	insert_interleaved_packet(out, &p);
	/* video goes before audio with the same dts_usec */
	p = video_packet(1, 0, 0, 100, false, 2);
	p.track_idx = 1;
	insert_interleaved_packet(out, &p);
	/* and video of a lower track before a higher one */
	p = video_packet(0, 0, 0, 100, false, 3);
	p.track_idx = 0;
	insert_interleaved_packet(out, &p);
	/* a second track 1 video goes before the first one */
	p = video_packet(1, 0, 0, 100, false, 4);
	p.track_idx = 1;
	insert_interleaved_packet(out, &p);
	/* audio goes after everything with the same dts_usec */
	p = audio_packet(1, 0, 100, 5);
	p.track_idx = 1;
	insert_interleaved_packet(out, &p);
	/* otherwise by dts_usec */
	p = audio_packet(0, 0, 50, 6);
	p.track_idx = 0;
	insert_interleaved_packet(out, &p);
	p = video_packet(0, 0, 0, 150, false, 7);
	p.track_idx = 0;
	insert_interleaved_packet(out, &p);

	/* The queue does not own these (the test's data): empty it by hand. */
	assert_queue_tags((uint8_t[]){6, 3, 4, 2, 1, 5, 7}, 7);
	da_free(out->interleaved_packets);
}

static void resort_reinserts_and_raises_highest(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);
	out->highest_audio_ts = 500;

	queue_packet(audio_packet(0, 0, 300, 1), 0);
	queue_packet(video_packet(1, 0, 0, 200, false, 2), 1);
	queue_packet(audio_packet(0, 0, 100, 3), 0);
	queue_packet(video_packet(0, 0, 0, 200, false, 4), 0);

	struct encoder_packet *before = out->interleaved_packets.array;
	resort_interleaved_packets(out);
	assert_queue_tags((uint8_t[]){3, 4, 2, 1}, 4);
	/* a new array */
	assert_ptr_not_equal(out->interleaved_packets.array, before);
	/* the highest timestamps are only raised */
	assert_int_equal(out->highest_audio_ts, 500);
	assert_int_equal(out->highest_video_ts[0], 200);
	assert_int_equal(out->highest_video_ts[1], 200);

	struct encoder_packet p = audio_packet(0, 0, 900, 5);
	p.track_idx = 0;
	set_higher_ts(out, &p);
	assert_int_equal(out->highest_audio_ts, 900);
	p.dts_usec = 800;
	set_higher_ts(out, &p);
	assert_int_equal(out->highest_audio_ts, 900);

	/* per video track, also only raised */
	p = video_packet(1, 0, 0, 150, false, 6);
	p.track_idx = 1;
	set_higher_ts(out, &p);
	assert_int_equal(out->highest_video_ts[1], 200);
	assert_int_equal(out->highest_video_ts[0], 200);
	p.dts_usec = 250;
	set_higher_ts(out, &p);
	assert_int_equal(out->highest_video_ts[1], 250);
	assert_int_equal(out->highest_video_ts[0], 200);
}

/* ------------------------------------------------------------------------- */
/* sending after the start */

static void sends_when_the_opposing_types_are_ahead(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);
	out->highest_video_ts[0] = 200;
	out->highest_video_ts[1] = 100;
	out->highest_audio_ts = 200;

	/* audio needs every video track above it; video needs the other video
	 * tracks and the audio above it. Equal is not above. */
	struct encoder_packet a = audio_packet(0, 0, 99, 1);
	assert_true(has_higher_opposing_ts(out, &a));
	a.dts_usec = 100;
	assert_false(has_higher_opposing_ts(out, &a));

	struct encoder_packet v0 = video_packet(0, 0, 0, 99, false, 2);
	v0.track_idx = 0;
	assert_true(has_higher_opposing_ts(out, &v0));
	v0.dts_usec = 100;
	assert_false(has_higher_opposing_ts(out, &v0));
	/* its own track does not count */
	struct encoder_packet v1 = video_packet(1, 0, 0, 150, false, 3);
	v1.track_idx = 1;
	assert_true(has_higher_opposing_ts(out, &v1));
	v1.dts_usec = 200;
	assert_false(has_higher_opposing_ts(out, &v1));

	/* tracks without an encoder are skipped */
	out->video_encoders[1] = NULL;
	a.dts_usec = 150;
	assert_true(has_higher_opposing_ts(out, &a));
}

static void started_state(void)
{
	out->received_audio = true;
	out->received_video[0] = true;
	queue_packet(audio_packet(0, 0, 0, 1), 0);
	queue_packet(audio_packet(0, 1024, 21333, 2), 0);
	queue_packet(audio_packet(0, 2048, 42666, 3), 0);
	out->highest_audio_ts = 42666;
}

static void one_packet_per_call_then_catch_up(void **state)
{
	UNUSED_PARAMETER(state);

	/* Three packets are streamable after this video packet (two audio
	 * below it, then itself, below the highest audio). One is sent, and a
	 * second one only when more than interleaver_max_batch_size are left. */
	started_state();
	out->interleaver_max_batch_size = 2;
	feed_video(0, 1, 1, 0, false, 4);
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 1);
	assert_queue_tags((uint8_t[]){2, 4, 3}, 3);
	assert_int_equal(out->highest_video_ts[0], FRAME_US);

	/* After the next video packet (66666 us) three are streamable again,
	 * and two left is more than a limit of 1: two are sent. */
	out->interleaver_max_batch_size = 1;
	feed_video(0, 2, 2, 0, false, 5);
	assert_int_equal(num_sent, 3);
	assert_int_equal(sent[1].tag, 2);
	assert_int_equal(sent[2].tag, 4);
	assert_queue_tags((uint8_t[]){3, 5}, 2);

	/* Audio alone sends nothing while no video is above it. */
	free_packets(out);
	num_sent = 0;
	out->highest_video_ts[0] = INT64_MIN;
	started_state();
	feed_audio(0, 3072, 0, 7);
	assert_int_equal(out->highest_audio_ts, 64000);
	assert_int_equal(num_sent, 0);

	/* Never more than two per call: four are streamable here. */
	out->interleaver_max_batch_size = 0;
	feed_video(0, 3, 3, 0, false, 8);
	assert_int_equal(num_sent, 2);
	assert_queue_tags((uint8_t[]){3, 7, 8}, 3);
}

static void send_interleaved_times_callbacks_and_counts(void **state)
{
	UNUSED_PARAMETER(state);

	queue_packet(video_packet(0, 5, 5, 0, false, 1), 0);
	queue_packet(audio_packet(0, 0, 0, 2), 0);
	queue_packet(video_packet(0, 7, 7, 0, false, 3), 0);
	queue_packet(video_packet(0, 8, 8, 0, false, 4), 0);
	push_packet_time(0, 4, 40);
	push_packet_time(0, 5, 50);
	push_packet_time(0, 6, 60);

	/* video: the packet time with the same pts is handed to the packet
	 * callback, with pir set, and taken out of the array; the others stay */
	send_interleaved(out);
	assert_int_equal(num_sent, 1);
	assert_int_equal(sent[0].tag, 1);
	assert_int_equal(out->total_frames, 1);
	assert_int_equal(num_callback_calls, 1);
	assert_int_equal(callback_calls[0].tag, 1);
	assert_true(callback_calls[0].time_found);
	assert_int_equal(callback_calls[0].time.cts, 50);
	assert_true(callback_calls[0].time.pir > 0);
	assert_true(callback_calls[0].callbacks_locked);
	assert_int_equal(out->encoder_packet_times[0].num, 2);
	assert_int_equal(out->encoder_packet_times[0].array[0].pts, 4);
	assert_int_equal(out->encoder_packet_times[0].array[1].pts, 6);

	/* audio: no packet time, not counted as a frame */
	send_interleaved(out);
	assert_int_equal(sent[1].tag, 2);
	assert_false(callback_calls[1].time_found);
	assert_int_equal(out->total_frames, 1);

	/* video without a matching time: NULL, and a debug line */
	num_logs = 0;
	send_interleaved(out);
	assert_false(callback_calls[2].time_found);
	assert_int_equal(out->total_frames, 2);
	assert_int_equal(out->encoder_packet_times[0].num, 2);
	assert_int_equal(num_logs, 1);
	assert_int_equal(last_log_level, LOG_DEBUG);
	assert_non_null(strstr(last_log, "encoder packet timing for PTS7 not found"));

	/* empty array: another debug line */
	out->encoder_packet_times[0].num = 0;
	send_interleaved(out);
	assert_false(callback_calls[3].time_found);
	assert_non_null(strstr(last_log, "encoder packet timing array empty"));

	assert_int_equal(queued(), 0);
	assert_int_equal(num_caption_calls, 0);
}

static void send_interleaved_caption_decision(void **state)
{
	UNUSED_PARAMETER(state);
	struct caption_track_data *ctrack = out->caption_tracks[0];

	struct caption_text *text = bzalloc(sizeof(*text) + 2);
	text->display_duration = 2.0;
	text->text[0] = 'x';
	ctrack->caption_head = text;
	ctrack->caption_tail = text;
	ctrack->caption_timestamp = 1.0;

	/* pts 15 at 1/30 is 0.5 s: before the caption's time, not added */
	queue_packet(video_packet(0, 15, 15, 0, false, 1), 0);
	send_interleaved(out);
	assert_int_equal(num_caption_calls, 0);

	/* pts 30 is 1.0 s: added (under the track's caption mutex), and the
	 * next caption waits for its display duration */
	queue_packet(video_packet(0, 30, 30, 0, false, 2), 0);
	send_interleaved(out);
	assert_int_equal(num_caption_calls, 1);
	assert_true(caption_calls[0].locked);
	assert_true(ctrack->caption_timestamp == 3.0);

	/* add_caption() failing leaves the time alone */
	add_caption_result = false;
	queue_packet(video_packet(0, 90, 90, 0, false, 3), 0);
	send_interleaved(out);
	assert_int_equal(num_caption_calls, 2);
	assert_true(ctrack->caption_timestamp == 3.0);

	/* raw CEA-608 data: added once per later frame time */
	bfree(ctrack->caption_head);
	ctrack->caption_head = NULL;
	ctrack->caption_tail = NULL;
	uint8_t cc[3] = {0xfc, 0x80, 0x80};
	deque_push_back(&ctrack->caption_data, cc, sizeof(cc));
	queue_packet(video_packet(0, 120, 120, 0, false, 4), 0);
	send_interleaved(out);
	assert_int_equal(num_caption_calls, 3);
	assert_true(ctrack->last_caption_timestamp == 4.0);
	queue_packet(video_packet(0, 120, 120, 0, false, 5), 0);
	send_interleaved(out);
	assert_int_equal(num_caption_calls, 3);

	/* audio never touches captions */
	queue_packet(audio_packet(0, 0, 0, 6), 0);
	send_interleaved(out);
	assert_int_equal(num_caption_calls, 3);
	assert_int_equal(num_sent, 6);
}

/* ------------------------------------------------------------------------- */
/* discard of unused audio */

static void discard_below_a_dts(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);

	queue_packet(audio_packet(0, 0, 10, 1), 0);
	queue_packet(video_packet(1, 0, 3, 20, true, 2), 1);
	queue_packet(audio_packet(0, 0, 30, 3), 0);
	queue_packet(video_packet(1, 1, 4, 35, false, 4), 1);
	push_packet_time(1, 3, 0);
	push_packet_time(1, 4, 0);

	/* everything below the dts_usec goes, whatever its type; a video
	 * packet takes the oldest packet time of its track with it */
	discard_unused_audio_packets(out, 30);
	assert_queue_tags((uint8_t[]){3, 4}, 2);
	assert_int_equal(out->encoder_packet_times[1].num, 1);
	assert_int_equal(out->encoder_packet_times[1].array[0].pts, 4);

	discard_unused_audio_packets(out, 0);
	assert_int_equal(queued(), 2);

	/* the first one at or above it stops the discard */
	queued_at(0)->dts_usec = 100;
	discard_unused_audio_packets(out, 50);
	assert_int_equal(queued(), 2);
}

/* ------------------------------------------------------------------------- */
/* encoder group keyframe check */

static struct encoder_packet group_keyframe(size_t track, int64_t pts)
{
	struct encoder_packet p = video_packet(track, pts, pts, 0, true, 1);
	p.track_idx = track;
	return p;
}

static void group_keyframes_are_tracked_until_every_track_has_one(void **state)
{
	UNUSED_PARAMETER(state);
	use_tracks(2, 1);
	group = bzalloc(sizeof(*group));
	assert_int_equal(pthread_mutex_init(&group->mutex, NULL), 0);
	group->num_encoders_started = 2;
	venc[0]->encoder_group = group;
	venc[1]->encoder_group = group;

	struct encoder_packet p = group_keyframe(0, 10);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 1);
	struct keyframe_group_data *d = &out->keyframe_group_tracking.array[0];
	assert_int_equal(d->group_id, (uintptr_t)group);
	assert_int_equal(d->pts, 10);
	assert_int_equal(d->required_tracks, 2);
	assert_int_equal(d->seen_on_track[0], KEYFRAME_TRACK_STATUS_SEEN);
	assert_int_equal(d->seen_on_track[1], KEYFRAME_TRACK_STATUS_NOT_SEEN);

	/* the other track's keyframe at the same pts completes it */
	p = group_keyframe(1, 10);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 0);
	assert_int_equal(num_warnings, 0);

	/* non-keyframes, audio and encoders without a group are not tracked */
	p = group_keyframe(0, 20);
	p.keyframe = false;
	check_encoder_group_keyframe_alignment(out, &p);
	struct encoder_packet a = audio_packet(0, 0, 0, 2);
	a.keyframe = true;
	check_encoder_group_keyframe_alignment(out, &a);
	venc[1]->encoder_group = NULL;
	p = group_keyframe(1, 20);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 0);
	venc[1]->encoder_group = group;

	/* Track 0 has keyframes at 20 and 30, track 1 only at 30. Characterized,
	 * not endorsed: track 0's own keyframe at 30 marks its entry at 20
	 * "skipped" (over "seen"), without a warning. */
	p = group_keyframe(0, 20);
	check_encoder_group_keyframe_alignment(out, &p);
	p = group_keyframe(0, 30);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 2);
	assert_int_equal(out->keyframe_group_tracking.array[0].seen_on_track[0], KEYFRAME_TRACK_STATUS_SKIPPED);
	assert_int_equal(num_warnings, 0);

	/* track 1 missed the keyframe at 20: a warning, and both entries close */
	p = group_keyframe(1, 30);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 0);
	assert_int_equal(num_warnings, 1);
	assert_string_equal(
		last_log,
		"obs-output 'interleaved output': Missing keyframe with pts 20 for encoder 'video 1' (track: 1)");

	/* entries of other groups and later pts are left alone; a new one is
	 * inserted in pts order */
	p = group_keyframe(0, 50);
	check_encoder_group_keyframe_alignment(out, &p);
	p = group_keyframe(0, 40);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 2);
	assert_int_equal(out->keyframe_group_tracking.array[0].pts, 40);
	assert_int_equal(out->keyframe_group_tracking.array[1].pts, 50);
	/* the same keyframe again marks track 0 seen and adds nothing */
	p = group_keyframe(0, 40);
	check_encoder_group_keyframe_alignment(out, &p);
	assert_int_equal(out->keyframe_group_tracking.num, 2);

	venc[0]->encoder_group = NULL;
	venc[1]->encoder_group = NULL;
}

/* ------------------------------------------------------------------------- */
/* free_packets */

static void free_packets_releases_everything(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	queue_packet(video_packet(0, 0, 0, 0, true, 1), 0);
	queue_packet(audio_packet(0, 0, 0, 2), 0);
	queue_packet(audio_packet(0, 0, 0, 3), 0);
	assert_true(bnum_allocs() > allocs);

	free_packets(out);
	assert_int_equal(out->interleaved_packets.num, 0);
	assert_int_equal(out->interleaved_packets.capacity, 0);
	assert_null(out->interleaved_packets.array);
	assert_int_equal(bnum_allocs(), allocs);

	/* the other interleaver state is left alone */
	out->received_audio = true;
	out->video_offsets[0] = 5;
	free_packets(out);
	assert_true(out->received_audio);
	assert_int_equal(out->video_offsets[0], 5);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(encoder_index_is_the_encoder_slot, setup, teardown),
		cmocka_unit_test_setup_teardown(received_flags_per_video_track_one_for_audio, setup, teardown),
		cmocka_unit_test_setup_teardown(inactive_output_ignores_the_packet, setup, teardown),
		cmocka_unit_test_setup_teardown(video_waits_for_a_keyframe, setup, teardown),
		cmocka_unit_test_setup_teardown(keyframe_wait_discards_other_tracks, setup, teardown),
		cmocka_unit_test_setup_teardown(delayed_packets_are_taken_not_copied, setup, teardown),
		cmocka_unit_test_setup_teardown(start_index_is_the_audio_closest_to_video, setup, teardown),
		cmocka_unit_test_setup_teardown(start_index_keeps_priming_audio, setup, teardown),
		cmocka_unit_test_setup_teardown(start_index_follows_video_track_0_only, setup, teardown),
		cmocka_unit_test_setup_teardown(premature_audio_prunes_through_it, setup, teardown),
		cmocka_unit_test_setup_teardown(premature_prune_needs_every_track, setup, teardown),
		cmocka_unit_test_setup_teardown(premature_prune_uses_the_last_audio_track, setup, teardown),
		cmocka_unit_test_setup_teardown(premature_prune_widens_to_the_audio_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(late_audio_drops_the_first_keyframe, setup, teardown),
		cmocka_unit_test_setup_teardown(premature_prune_can_start_on_a_non_keyframe, setup, teardown),
		cmocka_unit_test_setup_teardown(start_waits_for_every_track, setup, teardown),
		cmocka_unit_test_setup_teardown(start_offsets_and_resort, setup, teardown),
		cmocka_unit_test_setup_teardown(start_leaves_priming_audio_unshifted, setup, teardown),
		cmocka_unit_test_setup_teardown(start_with_only_priming_audio, setup, teardown),
		cmocka_unit_test_setup_teardown(start_needs_audio_past_the_first_video, setup, teardown),
		cmocka_unit_test_setup_teardown(offset_arithmetic, setup, teardown),
		cmocka_unit_test_setup_teardown(insertion_order, setup, teardown),
		cmocka_unit_test_setup_teardown(resort_reinserts_and_raises_highest, setup, teardown),
		cmocka_unit_test_setup_teardown(sends_when_the_opposing_types_are_ahead, setup, teardown),
		cmocka_unit_test_setup_teardown(one_packet_per_call_then_catch_up, setup, teardown),
		cmocka_unit_test_setup_teardown(send_interleaved_times_callbacks_and_counts, setup, teardown),
		cmocka_unit_test_setup_teardown(send_interleaved_caption_decision, setup, teardown),
		cmocka_unit_test_setup_teardown(discard_below_a_dts, setup, teardown),
		cmocka_unit_test_setup_teardown(group_keyframes_are_tracked_until_every_track_has_one, setup, teardown),
		cmocka_unit_test_setup_teardown(free_packets_releases_everything, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
