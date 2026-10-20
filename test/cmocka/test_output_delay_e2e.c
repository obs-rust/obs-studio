/* The output stream delay (obs_output_set_delay) end to end through real
 * libobs: obs_startup(), an audio encoder and an encoded output registered
 * here, and a real-time audio thread feeding the encoder. test_output_delay
 * compiles obs-output-delay.c.h into itself, so it never runs the delay that
 * libobs (or libobs-with-rust) ships; this test does.
 *
 * Delays are whole seconds of real time, so the delayed cases use one
 * second, and the checks are orderings and lower bounds only: a packet reaches
 * the output at least the delay after it was encoded, in order and without
 * gaps; the output's start() and stop() run at least the delay after
 * obs_output_start() and obs_output_stop(); obs_output_force_stop() skips the
 * delay; with OBS_OUTPUT_DELAY_PRESERVE a reconnect keeps the queued packets
 * and grows the active delay by the outage, and without it the packets that
 * come due during the outage are dropped.
 *
 * obs_startup() needs a display on Linux, so there the cases skip without one
 * (run under xvfb-run). No video, graphics or network is needed. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define ENCODER_ID "test_output_delay_e2e_audio"
#define OUTPUT_ID "test_output_delay_e2e_output"
#define FRAME_SIZE 1024 /* samples per packet; about 21 ms at 48 kHz */
#define MAX_PACKETS 2048
#define NS_PER_SEC 1000000000ULL
#define WAIT_MS 10000

/* What the encoder and the output saw, written by their threads under
 * mutex. The cases assert on a snapshot (snap), never with mutex held. */
struct record {
	/* Encode time of the packet with pts i * FRAME_SIZE. */
	uint64_t encoded_ns[MAX_PACKETS];
	size_t encoded;
	/* What reached the output's encoded_packet, in arrival order. */
	int64_t delivered_pts[MAX_PACKETS];
	uint64_t delivered_ns[MAX_PACKETS];
	size_t delivered;

	long start_calls;
	uint64_t start_ns;
	long stop_calls;
	uint64_t stop_ns;
	uint64_t stop_ts;
	uint64_t stopping_ns;
};

static pthread_mutex_t mutex;
static struct record rec;
static struct record snap;

static struct {
	os_event_t *packet;
	os_event_t *started;
	os_event_t *stopped;
	os_event_t *deactivated;
	os_event_t *reconnected;
} ev;

static bool obs_up;
static audio_t *audio;
static obs_encoder_t *encoder;
static obs_output_t *output;

static const char *test_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "output delay e2e test";
}

static void test_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *enc)
{
	UNUSED_PARAMETER(settings);
	return enc;
}

static size_t encoder_frame_size(void *data)
{
	UNUSED_PARAMETER(data);
	return FRAME_SIZE;
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	static uint8_t payload;
	UNUSED_PARAMETER(data);

	size_t idx = (size_t)(frame->pts / FRAME_SIZE);
	pthread_mutex_lock(&mutex);
	if (idx < MAX_PACKETS) {
		rec.encoded_ns[idx] = os_gettime_ns();
		rec.encoded = idx + 1;
	}
	pthread_mutex_unlock(&mutex);

	packet->type = OBS_ENCODER_AUDIO;
	packet->data = &payload;
	packet->size = sizeof(payload);
	packet->pts = packet->dts = frame->pts;
	packet->keyframe = true;
	*received = true;
	return true;
}

static void *output_create(obs_data_t *settings, obs_output_t *out)
{
	UNUSED_PARAMETER(settings);
	return out;
}

static bool output_start(void *data)
{
	pthread_mutex_lock(&mutex);
	rec.start_ns = os_gettime_ns();
	rec.start_calls++;
	pthread_mutex_unlock(&mutex);

	bool ok = obs_output_can_begin_data_capture(data, 0) && obs_output_initialize_encoders(data, 0) &&
		  obs_output_begin_data_capture(data, 0);
	os_event_signal(ev.started);
	return ok;
}

static void output_stop(void *data, uint64_t ts)
{
	pthread_mutex_lock(&mutex);
	rec.stop_ns = os_gettime_ns();
	rec.stop_ts = ts;
	rec.stop_calls++;
	pthread_mutex_unlock(&mutex);

	obs_output_end_data_capture(data);
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	uint64_t now = os_gettime_ns();

	pthread_mutex_lock(&mutex);
	if (rec.delivered < MAX_PACKETS) {
		rec.delivered_pts[rec.delivered] = packet->pts;
		rec.delivered_ns[rec.delivered] = now;
		rec.delivered++;
	}
	pthread_mutex_unlock(&mutex);
	os_event_signal(ev.packet);
}

static bool audio_input(void *data, uint64_t start, uint64_t end, uint64_t *timestamp, uint32_t mixers,
			struct audio_output_data *mixes)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(end);
	UNUSED_PARAMETER(mixers);
	UNUSED_PARAMETER(mixes); /* audio_output_open supplies zeroed silence. */
	*timestamp = start;
	return true;
}

static void on_stopping(void *data, calldata_t *cd)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(cd);
	pthread_mutex_lock(&mutex);
	rec.stopping_ns = os_gettime_ns();
	pthread_mutex_unlock(&mutex);
}

static void on_event(void *data, calldata_t *cd)
{
	UNUSED_PARAMETER(cd);
	os_event_signal(*(os_event_t **)data);
}

/* Keep warnings and errors only. */
static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	UNUSED_PARAMETER(p);
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	base_set_log_handler(log_handler, NULL);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed (no display?): cases skip\n");
		return 0;
	}

	struct obs_encoder_info enc = {
		.id = ENCODER_ID,
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = test_name,
		.create = encoder_create,
		.destroy = test_destroy,
		.encode = encoder_encode,
		.get_frame_size = encoder_frame_size,
	};
	obs_register_encoder(&enc);
	struct obs_output_info out = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED,
		.get_name = test_name,
		.create = output_create,
		.destroy = test_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&out);

	struct audio_output_info ai = {
		.name = "delay test audio",
		.samples_per_sec = 48000,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.speakers = SPEAKERS_STEREO,
		.input_callback = audio_input,
	};
	assert_int_equal(audio_output_open(&audio, &ai), AUDIO_OUTPUT_SUCCESS);

	assert_int_equal(pthread_mutex_init(&mutex, NULL), 0);
	assert_int_equal(os_event_init(&ev.packet, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&ev.started, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&ev.stopped, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&ev.deactivated, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&ev.reconnected, OS_EVENT_TYPE_AUTO), 0);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;
	audio_output_close(audio);
	os_event_destroy(ev.packet);
	os_event_destroy(ev.started);
	os_event_destroy(ev.stopped);
	os_event_destroy(ev.deactivated);
	os_event_destroy(ev.reconnected);
	pthread_mutex_destroy(&mutex);
	obs_shutdown();
	return 0;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;

	pthread_mutex_lock(&mutex);
	rec.encoded = rec.delivered = 0;
	rec.start_calls = rec.stop_calls = 0;
	rec.start_ns = rec.stop_ns = rec.stop_ts = rec.stopping_ns = 0;
	pthread_mutex_unlock(&mutex);
	os_event_reset(ev.packet);
	os_event_reset(ev.started);
	os_event_reset(ev.stopped);
	os_event_reset(ev.deactivated);
	os_event_reset(ev.reconnected);

	encoder = obs_audio_encoder_create(ENCODER_ID, "delay test encoder", NULL, 0, NULL);
	output = obs_output_create(OUTPUT_ID, "delay test output", NULL, NULL);
	assert_non_null(encoder);
	assert_non_null(output);
	obs_encoder_set_audio(encoder, audio);
	obs_output_set_audio_encoder(output, encoder, 0);

	signal_handler_t *sh = obs_output_get_signal_handler(output);
	signal_handler_connect(sh, "stopping", on_stopping, NULL);
	signal_handler_connect(sh, "stop", on_event, &ev.stopped);
	signal_handler_connect(sh, "deactivate", on_event, &ev.deactivated);
	signal_handler_connect(sh, "reconnect_success", on_event, &ev.reconnected);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;
	if (obs_output_active(output))
		obs_output_force_stop(output);
	obs_output_release(output);
	obs_encoder_release(encoder);
	output = NULL;
	encoder = NULL;
	return 0;
}

static void snapshot(void)
{
	pthread_mutex_lock(&mutex);
	snap = rec;
	pthread_mutex_unlock(&mutex);
}

static size_t delivered_count(void)
{
	pthread_mutex_lock(&mutex);
	size_t n = rec.delivered;
	pthread_mutex_unlock(&mutex);
	return n;
}

static long start_calls(void)
{
	pthread_mutex_lock(&mutex);
	long n = rec.start_calls;
	pthread_mutex_unlock(&mutex);
	return n;
}

static long stop_calls(void)
{
	pthread_mutex_lock(&mutex);
	long n = rec.stop_calls;
	pthread_mutex_unlock(&mutex);
	return n;
}

static void wait_event(os_event_t *event, const char *what)
{
	if (os_event_timedwait(event, WAIT_MS) != 0)
		fail_msg("timed out waiting for %s", what);
}

static void wait_delivered(size_t n)
{
	while (delivered_count() < n)
		wait_event(ev.packet, "a delivered packet");
}

/* In the last snapshot, every delivered packet came at least the delay after
 * it was encoded, in pts order, starting from the first one encoded. Returns
 * how many packets are missing in between. */
static size_t check_delivered_order(uint64_t delay_ns)
{
	size_t missing = 0;
	assert_true(snap.delivered > 0);
	assert_int_equal(snap.delivered_pts[0], 0);
	for (size_t i = 0; i < snap.delivered; i++) {
		int64_t pts = snap.delivered_pts[i];
		size_t idx = (size_t)(pts / FRAME_SIZE);
		if (i > 0 && pts <= snap.delivered_pts[i - 1])
			fail_msg("packet %zu has pts %lld after %lld (out of order)", i, (long long)pts,
				 (long long)snap.delivered_pts[i - 1]);
		if (i > 0)
			missing += (size_t)((pts - snap.delivered_pts[i - 1]) / FRAME_SIZE) - 1;
		assert_true(pts % FRAME_SIZE == 0 && idx < snap.encoded);
		uint64_t latency = snap.delivered_ns[i] - snap.encoded_ns[idx];
		if (latency < delay_ns)
			fail_msg("packet %zu reached the output %llu ms after it was encoded, before the %llu ms delay",
				 i, (unsigned long long)(latency / 1000000), (unsigned long long)(delay_ns / 1000000));
	}
	return missing;
}

/* As check_delivered_order, with no packet missing either. */
static void check_delivered(uint64_t delay_ns)
{
	size_t missing = check_delivered_order(delay_ns);
	if (missing)
		fail_msg("%zu packets never reached the output", missing);
}

static void delayed_start_packets_and_stop(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_output_set_delay(output, 1, 0);
	assert_int_equal(obs_output_get_delay(output), 1);
	assert_int_equal(obs_output_get_active_delay(output), 0);

	uint64_t t0 = os_gettime_ns();
	assert_true(obs_output_start(output));
	/* The output's own start() waits in the queue for the delay. */
	assert_int_equal(start_calls(), 0);
	assert_int_equal(obs_output_get_active_delay(output), 1);
	wait_event(ev.started, "the delayed start");
	snapshot();
	assert_true(snap.start_ns - t0 >= NS_PER_SEC);

	wait_delivered(10);
	snapshot();
	assert_true(snap.delivered_ns[0] >= snap.start_ns);

	uint64_t before = os_gettime_ns();
	obs_output_stop(output);
	uint64_t after = os_gettime_ns();
	/* "stopping" comes at once, the output's stop() after the delay, with
	 * the time the stop was asked for. */
	snapshot();
	assert_true(snap.stopping_ns >= before && snap.stopping_ns <= after);
	assert_int_equal(snap.stop_calls, 0);
	wait_event(ev.stopped, "the delayed stop");
	wait_event(ev.deactivated, "deactivate");
	assert_int_equal(obs_output_get_active_delay(output), 0);
	snapshot();
	assert_int_equal(snap.stop_calls, 1);
	assert_true(snap.stop_ns - before >= NS_PER_SEC);
	assert_true(snap.stop_ts >= before && snap.stop_ts <= after);

	check_delivered(NS_PER_SEC);
	/* The packets queued before the stop were all sent first; none queued
	 * after it was. 250 ms covers the gap between encoding a packet and
	 * queueing it. */
	size_t last = snap.delivered - 1;
	assert_true(snap.encoded_ns[last] <= after);
	for (size_t i = last + 1; i < snap.encoded; i++) {
		if (snap.encoded_ns[i] + 250000000ULL < before)
			fail_msg("packet %zu, encoded before the stop, never reached the output", i);
	}
}

static void force_stop_skips_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_output_set_delay(output, 1, 0);
	assert_true(obs_output_start(output));
	wait_event(ev.started, "the delayed start");
	wait_delivered(5);

	obs_output_force_stop(output);
	/* No wait for the delay: stop() has run when force_stop returns. */
	snapshot();
	assert_int_equal(snap.stop_calls, 1);
	assert_int_equal(snap.stop_ts, 0);
	wait_event(ev.deactivated, "deactivate");
	assert_false(obs_output_active(output));
	assert_int_equal(obs_output_get_active_delay(output), 0);

	/* The packets still queued (about a second of them) are dropped, not
	 * sent. */
	size_t n = delivered_count();
	os_sleep_ms(300);
	assert_int_equal(delivered_count(), n);
	snapshot();
	check_delivered(NS_PER_SEC);
}

static void no_delay_passes_through(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	assert_int_equal(obs_output_get_delay(output), 0);
	assert_true(obs_output_start(output));
	assert_int_equal(start_calls(), 1);
	assert_int_equal(obs_output_get_active_delay(output), 0);
	wait_delivered(3);
	obs_output_stop(output);
	assert_int_equal(stop_calls(), 1);
	wait_event(ev.deactivated, "deactivate");
	snapshot();
	check_delivered(0);
}

static void preserve_holds_queue_through_reconnect(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_output_set_delay(output, 1, OBS_OUTPUT_DELAY_PRESERVE);
	obs_output_set_reconnect_settings(output, 3, 2);
	assert_true(obs_output_start(output));
	wait_event(ev.started, "the delayed start");
	wait_delivered(10);

	/* A dropped connection, as a network output reports it. The output's
	 * start() runs again after the 2 s retry wait. */
	uint64_t disconnect = os_gettime_ns();
	obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);
	assert_true(obs_output_reconnecting(output));
	wait_event(ev.reconnected, "reconnect_success");
	assert_int_equal(start_calls(), 2);
	assert_true(os_gettime_ns() - disconnect >= 2 * NS_PER_SEC);
	size_t at_reconnect = delivered_count();
	wait_delivered(at_reconnect + 10);

	/* The queue was held for the outage, so the delay grew by it. */
	assert_true(obs_output_get_active_delay(output) >= 2);
	/* Packets encoded before the outage still reached the output after it,
	 * and none went missing. */
	snapshot();
	check_delivered(NS_PER_SEC);
	assert_true(snap.encoded_ns[at_reconnect] < disconnect);
	assert_int_equal(snap.stop_calls, 0);

	obs_output_force_stop(output);
	wait_event(ev.deactivated, "deactivate");
}

static void no_preserve_drops_due_packets_through_reconnect(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_output_set_delay(output, 1, 0);
	obs_output_set_reconnect_settings(output, 3, 1);
	assert_true(obs_output_start(output));
	wait_event(ev.started, "the delayed start");
	wait_delivered(10);

	/* Without OBS_OUTPUT_DELAY_PRESERVE the queue keeps moving while the
	 * output reconnects (1 s retry wait), and the packets that come due are
	 * dropped, not sent, since the output is not capturing. */
	obs_output_signal_stop(output, OBS_OUTPUT_DISCONNECTED);
	assert_true(obs_output_reconnecting(output));
	wait_event(ev.reconnected, "reconnect_success");
	assert_int_equal(start_calls(), 2);
	wait_delivered(delivered_count() + 10);

	assert_int_equal(obs_output_get_active_delay(output), 1);
	snapshot();
	assert_true(check_delivered_order(NS_PER_SEC) > 0);
	assert_int_equal(snap.stop_calls, 0);

	obs_output_force_stop(output);
	wait_event(ev.deactivated, "deactivate");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(delayed_start_packets_and_stop, setup, teardown),
		cmocka_unit_test_setup_teardown(force_stop_skips_delay, setup, teardown),
		cmocka_unit_test_setup_teardown(no_delay_passes_through, setup, teardown),
		cmocka_unit_test_setup_teardown(preserve_holds_queue_through_reconnect, setup, teardown),
		cmocka_unit_test_setup_teardown(no_preserve_drops_due_packets_through_reconnect, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
