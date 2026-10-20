#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

/*
 * Characterization tests for the output stream delay in
 * libobs/obs-output-delay.c (#1546): obs_output_set_delay(),
 * obs_output_get_delay(), obs_output_get_active_delay(),
 * obs_output_delay_start(), obs_output_delay_stop(),
 * obs_output_cleanup_delay() and process_delay(), the encoded packet callback
 * that holds packets for the delay. They pin the current C behavior for the
 * Rust port; cases marked "characterized, not endorsed" are odd but real.
 *
 * process_delay() and the start, stop and cleanup functions are not exported
 * from libobs, so the file is compiled into this test, which therefore
 * characterizes the C source only. The output is a zeroed struct obs_output
 * with only the fields the file reads set up; no libobs core is started. The
 * functions of obs-output.c that the file calls (actual start and stop, the
 * data capture checks) are recording stubs below, and the delay callback is
 * a recording stub too. os_gettime_ns() is renamed to a test clock, so no
 * case depends on real time. Held packets are released with libobs's
 * obs_encoder_packet_release(); obs_encoder_packet_create_instance() is not
 * exported, so the test has a copy of it. bnum_allocs() shows whether a held
 * packet is released.
 */

#define os_gettime_ns output_delay_test_clock_ns
#include "libobs/obs-output-delay.c.h"

#define SEC 1000000000ULL

/* ------------------------------------------------------------------------- */
/* test clock */

static uint64_t clock_ns;

uint64_t output_delay_test_clock_ns(void)
{
	return clock_ns;
}

/* ------------------------------------------------------------------------- */
/* recorded calls, in order */

enum event_kind {
	EV_CAN_BEGIN,
	EV_INITIALIZE_ENCODERS,
	EV_BEGIN_CAPTURE,
	EV_ACTUAL_START,
	EV_ACTUAL_STOP,
	EV_PACKET,
	EV_SIGNAL_STARTING,
	EV_SIGNAL_STOPPING,
};

struct event {
	enum event_kind kind;
	/* the delay mutex was held when the call was made */
	bool locked;
	/* EV_INITIALIZE_ENCODERS: output->starting at the call */
	bool starting;
	/* EV_ACTUAL_STOP */
	bool force;
	uint64_t ts;
	/* EV_PACKET */
	int64_t pts;
	uint8_t first_byte;
	const uint8_t *data;
	bool time_valid;
	struct encoder_packet_time packet_time;
};

#define MAX_EVENTS 64

static struct obs_output *out;
static struct event events[MAX_EVENTS];
static size_t num_events;

static bool can_begin_result;
static bool initialize_result;
static bool begin_result;
static bool actual_start_result;

static bool mutex_held(struct obs_output *output)
{
	if (pthread_mutex_trylock(&output->delay_mutex) != 0)
		return true;
	pthread_mutex_unlock(&output->delay_mutex);
	return false;
}

static struct event *record(struct obs_output *output, enum event_kind kind)
{
	assert_ptr_equal(output, out);
	assert_true(num_events < MAX_EVENTS);
	struct event *ev = &events[num_events++];
	memset(ev, 0, sizeof(*ev));
	ev->kind = kind;
	ev->locked = mutex_held(output);
	return ev;
}

/* ------------------------------------------------------------------------- */
/* obs-output.c functions the file calls */

bool obs_output_can_begin_data_capture(const obs_output_t *output, uint32_t flags)
{
	UNUSED_PARAMETER(flags);
	record((struct obs_output *)output, EV_CAN_BEGIN);
	return can_begin_result;
}

bool obs_output_initialize_encoders(obs_output_t *output, uint32_t flags)
{
	UNUSED_PARAMETER(flags);
	record(output, EV_INITIALIZE_ENCODERS)->starting = os_atomic_load_bool(&output->starting);
	return initialize_result;
}

bool obs_output_begin_data_capture(obs_output_t *output, uint32_t flags)
{
	UNUSED_PARAMETER(flags);
	record(output, EV_BEGIN_CAPTURE);
	return begin_result;
}

bool obs_output_actual_start(obs_output_t *output)
{
	record(output, EV_ACTUAL_START);
	return actual_start_result;
}

void obs_output_actual_stop(obs_output_t *output, bool force, uint64_t ts)
{
	struct event *ev = record(output, EV_ACTUAL_STOP);
	ev->force = force;
	ev->ts = ts;
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

/* The delay callback owns the packet it is given (interleave_packets() and
 * default_encoded_callback() release it while a delay is active). */
static void delayed_packet(void *data, struct encoder_packet *packet, struct encoder_packet_time *packet_time)
{
	struct event *ev = record(data, EV_PACKET);
	ev->pts = packet->pts;
	ev->data = packet->data;
	ev->first_byte = packet->size ? packet->data[0] : 0;
	ev->time_valid = packet_time != NULL;
	if (packet_time)
		ev->packet_time = *packet_time;
	obs_encoder_packet_release(packet);
}

static void on_starting(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	record(calldata_ptr(cd, "output"), EV_SIGNAL_STARTING);
}

static void on_stopping(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	record(calldata_ptr(cd, "output"), EV_SIGNAL_STOPPING);
}

/* ------------------------------------------------------------------------- */
/* log capture */

static char last_log[512];
static int last_log_level;
static size_t num_logs;

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	UNUSED_PARAMETER(p);
	vsnprintf(last_log, sizeof(last_log), msg, args);
	last_log_level = lvl;
	num_logs++;
}

/* ------------------------------------------------------------------------- */
/* fixture */

static long allocs_before_setup;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	allocs_before_setup = bnum_allocs();

	out = bzalloc(sizeof(*out));
	out->context.name = "delayed output";
	out->info.flags = OBS_OUTPUT_ENCODED | OBS_OUTPUT_VIDEO;
	out->delay_callback = delayed_packet;
	assert_int_equal(pthread_mutex_init(&out->delay_mutex, NULL), 0);
	deque_init(&out->delay_data);

	out->context.signals = signal_handler_create();
	assert_true(signal_handler_add(out->context.signals, "void starting(ptr output)"));
	assert_true(signal_handler_add(out->context.signals, "void stopping(ptr output)"));
	signal_handler_connect(out->context.signals, "starting", on_starting, NULL);
	signal_handler_connect(out->context.signals, "stopping", on_stopping, NULL);

	num_events = 0;
	clock_ns = 1000 * SEC;
	can_begin_result = true;
	initialize_result = true;
	begin_result = true;
	actual_start_result = true;

	last_log[0] = 0;
	last_log_level = -1;
	num_logs = 0;
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	base_set_log_handler(NULL, NULL);

	obs_output_cleanup_delay(out);
	deque_free(&out->delay_data);
	signal_handler_destroy(out->context.signals);
	pthread_mutex_destroy(&out->delay_mutex);
	bfree(out);
	out = NULL;

	/* every packet the delay held has been released */
	assert_int_equal(bnum_allocs(), allocs_before_setup);
	return 0;
}

/* Delay running and capturing, as hook_data_capture() and
 * begin_delayed_capture() in obs-output.c leave it. */
static void start_capturing(uint64_t delay_ns)
{
	out->active_delay_ns = delay_ns;
	os_atomic_set_bool(&out->delay_active, true);
	os_atomic_set_bool(&out->delay_capturing, true);
}

static uint8_t payloads[64][4];

/* What an encoder hands the output: data the encoder owns. */
static struct encoder_packet make_packet(int64_t pts)
{
	uint8_t *payload = payloads[pts & 63];
	payload[0] = (uint8_t)(0xA0 + pts);
	payload[1] = 1;
	payload[2] = 2;
	payload[3] = 3;
	return (struct encoder_packet){
		.data = payload,
		.size = 4,
		.pts = pts,
		.dts = pts,
		.timebase_num = 1,
		.timebase_den = 30,
		.type = OBS_ENCODER_VIDEO,
		.keyframe = pts == 0,
	};
}

static void send_packet_at(uint64_t t, int64_t pts)
{
	struct encoder_packet pkt = make_packet(pts);
	clock_ns = t;
	process_delay(out, &pkt, NULL);
	/* the caller's packet is copied, not taken */
	assert_ptr_equal(pkt.data, payloads[pts & 63]);
}

static size_t queued(void)
{
	return out->delay_data.size / sizeof(struct delay_data);
}

static struct delay_data queued_at(size_t idx)
{
	struct delay_data dd;
	memcpy(&dd, deque_data(&out->delay_data, idx * sizeof(dd)), sizeof(dd));
	return dd;
}

static size_t count_events(enum event_kind kind)
{
	size_t n = 0;
	for (size_t i = 0; i < num_events; i++)
		n += events[i].kind == kind;
	return n;
}

/* ------------------------------------------------------------------------- */
/* settings */

static void set_delay_stores_seconds_and_flags(void **state)
{
	UNUSED_PARAMETER(state);

	obs_output_set_delay(out, 20, OBS_OUTPUT_DELAY_PRESERVE);
	assert_int_equal(out->delay_sec, 20);
	assert_int_equal(out->delay_flags, OBS_OUTPUT_DELAY_PRESERVE);
	assert_int_equal(obs_output_get_delay(out), 20);

	/* The setting is only stored: the running delay and its flags are
	 * copied from it when data capture is hooked (obs-output.c). */
	assert_int_equal(out->active_delay_ns, 0);
	assert_int_equal(out->delay_cur_flags, 0);
	assert_int_equal(obs_output_get_active_delay(out), 0);

	/* flags are stored as given, unknown bits too */
	obs_output_set_delay(out, 0, 0xF0);
	assert_int_equal(obs_output_get_delay(out), 0);
	assert_int_equal(out->delay_flags, 0xF0);

	obs_output_set_delay(out, UINT32_MAX, 0);
	assert_int_equal(obs_output_get_delay(out), UINT32_MAX);
	assert_int_equal(num_events, 0);
	assert_int_equal(num_logs, 0);
}

static void set_delay_ignored_on_raw_output(void **state)
{
	UNUSED_PARAMETER(state);

	obs_output_set_delay(out, 5, OBS_OUTPUT_DELAY_PRESERVE);
	out->info.flags = OBS_OUTPUT_VIDEO;

	obs_output_set_delay(out, 30, 0);
	assert_int_equal(out->delay_sec, 5);
	assert_int_equal(out->delay_flags, OBS_OUTPUT_DELAY_PRESERVE);
	assert_int_equal(num_logs, 1);
	assert_int_equal(last_log_level, LOG_WARNING);
	assert_string_equal(last_log, "Output 'delayed output': Tried to use obs_output_set_delay on a raw output");

	/* the getters do not check the flag */
	assert_int_equal(obs_output_get_delay(out), 5);
}

static void getters_on_null_output(void **state)
{
	UNUSED_PARAMETER(state);

	obs_output_set_delay(NULL, 10, 0);
	assert_int_equal(last_log_level, LOG_DEBUG);
	assert_string_equal(last_log, "obs_output_set_delay: Null 'output' parameter");

	/* Characterized, not endorsed: both getters log under the setter's
	 * name. */
	last_log[0] = 0;
	assert_int_equal(obs_output_get_delay(NULL), 0);
	assert_string_equal(last_log, "obs_output_set_delay: Null 'output' parameter");
	last_log[0] = 0;
	assert_int_equal(obs_output_get_active_delay(NULL), 0);
	assert_string_equal(last_log, "obs_output_set_delay: Null 'output' parameter");
	assert_int_equal(num_logs, 3);
}

static void active_delay_is_whole_seconds(void **state)
{
	UNUSED_PARAMETER(state);

	out->active_delay_ns = 2 * SEC + SEC / 2;
	assert_int_equal(obs_output_get_active_delay(out), 2);
	out->active_delay_ns = SEC - 1;
	assert_int_equal(obs_output_get_active_delay(out), 0);
	out->active_delay_ns = 30 * SEC;
	assert_int_equal(obs_output_get_active_delay(out), 30);

	/* Characterized, not endorsed: the cast to uint32_t wraps past
	 * UINT32_MAX seconds. 18446744073 s mod 2^32 = 1266874889. */
	out->active_delay_ns = UINT64_MAX;
	assert_int_equal(obs_output_get_active_delay(out), 1266874889u);

	/* independent of the stored setting */
	obs_output_set_delay(out, 7, 0);
	out->active_delay_ns = 3 * SEC;
	assert_int_equal(obs_output_get_active_delay(out), 3);
	assert_int_equal(obs_output_get_delay(out), 7);
}

/* ------------------------------------------------------------------------- */
/* packets */

static void packets_held_then_released_in_order(void **state)
{
	UNUSED_PARAMETER(state);
	const uint64_t t0 = 1000 * SEC;
	start_capturing(SEC);

	send_packet_at(t0, 0);
	send_packet_at(t0 + SEC / 2, 1);
	assert_int_equal(num_events, 0);
	assert_int_equal(queued(), 2);

	/* each entry keeps the time it arrived */
	assert_int_equal(queued_at(0).msg, DELAY_MSG_PACKET);
	assert_int_equal(queued_at(0).ts, t0);
	assert_int_equal(queued_at(1).ts, t0 + SEC / 2);
	assert_false(queued_at(0).packet_time_valid);

	/* held while elapsed == delay: the comparison is strict */
	send_packet_at(t0 + SEC, 2);
	assert_int_equal(num_events, 0);

	/* released once more than the delay has passed */
	send_packet_at(t0 + SEC + 1, 3);
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].kind, EV_PACKET);
	assert_int_equal(events[0].pts, 0);
	assert_false(events[0].time_valid);
	assert_int_equal(queued(), 3);

	/* the callback gets the held copy, not the encoder's buffer */
	assert_ptr_not_equal(events[0].data, payloads[0]);
	assert_int_equal(events[0].first_byte, 0xA0);

	/* one call releases every packet that is due, oldest first, but never
	 * the one it just queued */
	send_packet_at(t0 + 10 * SEC, 4);
	assert_int_equal(num_events, 4);
	for (size_t i = 1; i < 4; i++) {
		assert_int_equal(events[i].kind, EV_PACKET);
		assert_int_equal(events[i].pts, (int64_t)i);
		assert_int_equal(events[i].first_byte, 0xA0 + i);
		assert_false(events[i].locked);
	}
	assert_int_equal(queued(), 1);
	assert_int_equal(queued_at(0).packet.pts, 4);
}

static void packet_time_is_copied(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(SEC);

	struct encoder_packet pkt = make_packet(7);
	struct encoder_packet_time pt = {.pts = 7, .cts = 11, .fer = 13, .ferc = 17};
	clock_ns = 2000 * SEC;
	process_delay(out, &pkt, &pt);
	assert_true(queued_at(0).packet_time_valid);

	/* changing the caller's copy afterwards does not matter */
	pt.cts = 99;

	send_packet_at(2000 * SEC + 2 * SEC, 8);
	assert_int_equal(num_events, 1);
	assert_true(events[0].time_valid);
	assert_int_equal(events[0].packet_time.pts, 7);
	assert_int_equal(events[0].packet_time.cts, 11);
	assert_int_equal(events[0].packet_time.fer, 13);
	assert_int_equal(events[0].packet_time.ferc, 17);
}

static void zero_delay_still_waits_for_the_next_packet(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(0);

	/* Characterized, not endorsed: with no delay left (cleanup sets 0) a
	 * packet is still held, because its own elapsed time is 0, and 0 > 0
	 * is false. A later packet at the same time does not release it
	 * either. */
	send_packet_at(5 * SEC, 0);
	send_packet_at(5 * SEC, 1);
	assert_int_equal(num_events, 0);
	assert_int_equal(queued(), 2);

	send_packet_at(5 * SEC + 1, 2);
	assert_int_equal(num_events, 2);
	assert_int_equal(events[0].pts, 0);
	assert_int_equal(events[1].pts, 1);
	assert_int_equal(queued(), 1);
}

static void packets_dropped_unless_delay_is_capturing(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(SEC);

	send_packet_at(10 * SEC, 0);
	long held_allocs = bnum_allocs();

	/* delay active, capture ended (a stop, or a reconnect without
	 * preserve): due packets are released, not delivered */
	os_atomic_set_bool(&out->delay_capturing, false);
	send_packet_at(12 * SEC, 1);
	assert_int_equal(num_events, 0);
	assert_int_equal(queued(), 1);
	assert_int_equal(bnum_allocs(), held_allocs);

	/* delay no longer active: the same */
	os_atomic_set_bool(&out->delay_active, false);
	os_atomic_set_bool(&out->delay_capturing, true);
	send_packet_at(14 * SEC, 2);
	assert_int_equal(num_events, 0);
	assert_int_equal(queued(), 1);
	assert_int_equal(bnum_allocs(), held_allocs);

	/* the check is made when the packet leaves, not when it arrives */
	os_atomic_set_bool(&out->delay_active, true);
	send_packet_at(16 * SEC, 3);
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].pts, 2);
}

static void clock_going_backwards_releases_at_once(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(30 * SEC);

	send_packet_at(100 * SEC, 0);

	/* Characterized, not endorsed: the elapsed time is an unsigned
	 * difference, so an earlier time wraps to a huge one and the packet is
	 * released at once. os_gettime_ns() is monotonic, so it is not reached
	 * in practice. */
	send_packet_at(100 * SEC - 1, 1);
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].pts, 0);
	assert_int_equal(queued(), 1);
}

/* ------------------------------------------------------------------------- */
/* start and stop */

static void delay_start_first_start(void **state)
{
	UNUSED_PARAMETER(state);
	obs_output_set_delay(out, 1, 0);
	clock_ns = 50 * SEC;

	assert_true(obs_output_delay_start(out));

	/* checks, then `starting`, then the encoders, then data capture; no
	 * "starting" signal of its own (obs-output.c's begin emits it) */
	assert_int_equal(num_events, 3);
	assert_int_equal(events[0].kind, EV_CAN_BEGIN);
	assert_int_equal(events[1].kind, EV_INITIALIZE_ENCODERS);
	assert_true(events[1].starting);
	assert_int_equal(events[2].kind, EV_BEGIN_CAPTURE);
	for (size_t i = 0; i < num_events; i++)
		assert_false(events[i].locked);

	assert_true(os_atomic_load_bool(&out->starting));
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 1);

	/* the real start is queued behind the delay */
	assert_int_equal(queued(), 1);
	assert_int_equal(queued_at(0).msg, DELAY_MSG_START);
	assert_int_equal(queued_at(0).ts, 50 * SEC);
}

static void delay_start_refused_before_queueing(void **state)
{
	UNUSED_PARAMETER(state);

	can_begin_result = false;
	assert_false(obs_output_delay_start(out));
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].kind, EV_CAN_BEGIN);
	assert_false(os_atomic_load_bool(&out->starting));
	assert_int_equal(queued(), 0);
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 0);

	/* encoders that fail to initialize clear `starting` again */
	num_events = 0;
	can_begin_result = true;
	initialize_result = false;
	assert_false(obs_output_delay_start(out));
	assert_int_equal(num_events, 2);
	assert_int_equal(events[1].kind, EV_INITIALIZE_ENCODERS);
	assert_true(events[1].starting);
	assert_false(os_atomic_load_bool(&out->starting));
	assert_int_equal(queued(), 0);
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 0);
}

static void delay_start_failed_capture_cleans_up(void **state)
{
	UNUSED_PARAMETER(state);
	out->active_delay_ns = SEC;
	begin_result = false;

	assert_false(obs_output_delay_start(out));
	assert_int_equal(num_events, 3);
	assert_int_equal(events[2].kind, EV_BEGIN_CAPTURE);

	/* the queued start is dropped without running, and the delay reset */
	assert_int_equal(queued(), 0);
	assert_int_equal(count_events(EV_ACTUAL_START), 0);
	assert_int_equal(out->active_delay_ns, 0);
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 0);

	/* Characterized, not endorsed: unlike a failed encoder initialization,
	 * this path leaves `starting` set. */
	assert_true(os_atomic_load_bool(&out->starting));
}

static void delay_start_while_delay_active(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(SEC);
	can_begin_result = false;
	initialize_result = false;
	begin_result = false;

	/* A restart while the delay still runs (stopped, then started again
	 * before the stop left the queue): no checks, no new capture, just a
	 * queued start, a reference and the "starting" signal. */
	clock_ns = 60 * SEC;
	assert_true(obs_output_delay_start(out));
	clock_ns = 61 * SEC;
	assert_true(obs_output_delay_start(out));

	assert_int_equal(num_events, 2);
	assert_int_equal(events[0].kind, EV_SIGNAL_STARTING);
	assert_int_equal(events[1].kind, EV_SIGNAL_STARTING);
	assert_false(events[0].locked);
	assert_false(os_atomic_load_bool(&out->starting));
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 2);
	assert_int_equal(queued(), 2);
	assert_int_equal(queued_at(0).msg, DELAY_MSG_START);
	assert_int_equal(queued_at(0).ts, 60 * SEC);
	assert_int_equal(queued_at(1).ts, 61 * SEC);
}

static void queued_start_runs_after_the_delay(void **state)
{
	UNUSED_PARAMETER(state);
	obs_output_set_delay(out, 2, 0);
	clock_ns = 100 * SEC;
	assert_true(obs_output_delay_start(out));
	/* what obs-output.c's hook_data_capture() does on begin */
	start_capturing(2 * SEC);
	num_events = 0;

	send_packet_at(100 * SEC + 1, 0);
	send_packet_at(101 * SEC, 1);
	assert_int_equal(num_events, 0);

	/* The start leaves the queue first, then the packets behind it; the
	 * start's return value is not used. */
	actual_start_result = false;
	send_packet_at(102 * SEC + 2, 2);
	assert_int_equal(num_events, 2);
	assert_int_equal(events[0].kind, EV_ACTUAL_START);
	assert_false(events[0].locked);
	assert_int_equal(events[1].kind, EV_PACKET);
	assert_int_equal(events[1].pts, 0);

	/* start and stop entries run whatever the delay state */
	clock_ns = 103 * SEC;
	os_atomic_set_bool(&out->delay_active, false);
	obs_output_delay_stop(out);
	num_events = 0;
	send_packet_at(200 * SEC, 3);
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].kind, EV_ACTUAL_STOP);
	assert_int_equal(queued(), 1);
}

static void delay_stop_queues_a_stop_with_its_time(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(5 * SEC);

	send_packet_at(10 * SEC, 0);
	clock_ns = 11 * SEC;
	obs_output_delay_stop(out);

	/* "stopping" is signalled at once, the stop itself is queued */
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].kind, EV_SIGNAL_STOPPING);
	assert_false(events[0].locked);
	assert_int_equal(queued(), 2);
	assert_int_equal(queued_at(1).msg, DELAY_MSG_STOP);
	assert_int_equal(queued_at(1).ts, 11 * SEC);

	/* packets that arrive after the stop are queued behind it */
	send_packet_at(12 * SEC, 1);
	assert_int_equal(queued(), 3);

	/* the packet before the stop is delivered first; the stop is not
	 * forced and carries the time it was asked for */
	send_packet_at(16 * SEC + 1, 2);
	assert_int_equal(num_events, 3);
	assert_int_equal(events[1].kind, EV_PACKET);
	assert_int_equal(events[1].pts, 0);
	assert_int_equal(events[2].kind, EV_ACTUAL_STOP);
	assert_false(events[2].force);
	assert_int_equal(events[2].ts, 11 * SEC);
	assert_false(events[2].locked);

	/* The stub stop leaves the delay capturing, so the next packet is
	 * still delivered; the real stop ends the capture. */
	send_packet_at(17 * SEC + 1, 3);
	assert_int_equal(num_events, 4);
	assert_int_equal(events[3].kind, EV_PACKET);
	assert_int_equal(events[3].pts, 1);

	/* stopping does not take a restart reference */
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 0);
}

static void cleanup_releases_everything(void **state)
{
	UNUSED_PARAMETER(state);
	obs_output_set_delay(out, 4, OBS_OUTPUT_DELAY_PRESERVE);
	out->delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
	start_capturing(4 * SEC);
	os_atomic_set_long(&out->delay_restart_refs, 3);

	send_packet_at(10 * SEC, 0);
	assert_true(obs_output_delay_start(out));
	send_packet_at(10 * SEC, 1);
	obs_output_delay_stop(out);
	send_packet_at(10 * SEC, 2);
	assert_int_equal(queued(), 5);
	num_events = 0;

	obs_output_cleanup_delay(out);

	/* nothing runs: no start, no stop, no packet */
	assert_int_equal(num_events, 0);
	assert_int_equal(queued(), 0);
	assert_int_equal(out->active_delay_ns, 0);
	assert_int_equal(os_atomic_load_long(&out->delay_restart_refs), 0);

	/* the rest of the delay state is left alone */
	assert_true(os_atomic_load_bool(&out->delay_active));
	assert_true(os_atomic_load_bool(&out->delay_capturing));
	assert_int_equal(out->delay_sec, 4);
	assert_int_equal(out->delay_flags, OBS_OUTPUT_DELAY_PRESERVE);
	assert_int_equal(out->delay_cur_flags, OBS_OUTPUT_DELAY_PRESERVE);
	assert_int_equal(obs_output_get_active_delay(out), 0);

	/* the queue still works afterwards (teardown checks no packet leaks) */
	obs_output_cleanup_delay(out);
	send_packet_at(11 * SEC, 3);
	assert_int_equal(queued(), 1);
}

/* ------------------------------------------------------------------------- */
/* preserve on disconnect */

static void preserve_while_reconnecting_grows_the_delay(void **state)
{
	UNUSED_PARAMETER(state);
	out->delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
	start_capturing(10 * SEC);

	send_packet_at(100 * SEC, 0);
	send_packet_at(105 * SEC, 1);

	/* While reconnecting, nothing leaves the queue, and the delay becomes
	 * the age of the oldest entry: the stream picks up where it was cut,
	 * later by the time the outage took. */
	os_atomic_set_bool(&out->reconnecting, true);
	send_packet_at(115 * SEC, 2);
	assert_int_equal(num_events, 0);
	assert_int_equal(out->active_delay_ns, 15 * SEC);
	send_packet_at(140 * SEC, 3);
	assert_int_equal(out->active_delay_ns, 40 * SEC);
	assert_int_equal(obs_output_get_active_delay(out), 40);
	assert_int_equal(queued(), 4);

	/* back online: the grown delay stays */
	os_atomic_set_bool(&out->reconnecting, false);
	send_packet_at(140 * SEC, 4);
	assert_int_equal(num_events, 0);
	send_packet_at(140 * SEC + 1, 5);
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].pts, 0);
	assert_int_equal(out->active_delay_ns, 40 * SEC);

	/* Characterized, not endorsed: it never shrinks back to the 10 s that
	 * was set; the next packets keep the 40 s spacing. */
	send_packet_at(145 * SEC + 1, 6);
	assert_int_equal(num_events, 2);
	assert_int_equal(events[1].pts, 1);
	assert_int_equal(out->active_delay_ns, 40 * SEC);
}

static void preserve_holds_even_when_not_capturing(void **state)
{
	UNUSED_PARAMETER(state);
	out->delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
	start_capturing(SEC);

	/* obs_output_signal_stop() ends the capture before it reconnects;
	 * with preserve the packets are kept, not dropped */
	send_packet_at(10 * SEC, 0);
	os_atomic_set_bool(&out->delay_capturing, false);
	os_atomic_set_bool(&out->reconnecting, true);
	long held_allocs = bnum_allocs();
	send_packet_at(20 * SEC, 1);
	assert_int_equal(queued(), 2);
	assert_true(bnum_allocs() > held_allocs);
	assert_int_equal(out->active_delay_ns, 10 * SEC);

	/* reconnected and capturing again: delivered in order */
	os_atomic_set_bool(&out->reconnecting, false);
	os_atomic_set_bool(&out->delay_capturing, true);
	send_packet_at(25 * SEC, 2);
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].pts, 0);
	assert_int_equal(queued(), 2);
}

static void reconnecting_without_preserve_keeps_releasing(void **state)
{
	UNUSED_PARAMETER(state);
	start_capturing(SEC);

	send_packet_at(10 * SEC, 0);
	send_packet_at(10 * SEC, 1);
	os_atomic_set_bool(&out->reconnecting, true);
	os_atomic_set_bool(&out->delay_capturing, false);

	/* the delay is unchanged and due packets are dropped */
	send_packet_at(20 * SEC, 2);
	assert_int_equal(num_events, 0);
	assert_int_equal(queued(), 1);
	assert_int_equal(out->active_delay_ns, SEC);
}

static void preserve_follows_the_running_flags(void **state)
{
	UNUSED_PARAMETER(state);

	/* Preserve is read from delay_cur_flags, which obs-output.c copies
	 * from the setting when capture is hooked; changing the setting while
	 * the delay runs does not change it. */
	obs_output_set_delay(out, 1, OBS_OUTPUT_DELAY_PRESERVE);
	out->delay_cur_flags = 0;
	start_capturing(SEC);
	os_atomic_set_bool(&out->reconnecting, true);
	send_packet_at(10 * SEC, 0);
	send_packet_at(12 * SEC, 1);
	assert_int_equal(num_events, 1);
	assert_int_equal(out->active_delay_ns, SEC);

	/* and the other way round */
	obs_output_set_delay(out, 1, 0);
	out->delay_cur_flags = OBS_OUTPUT_DELAY_PRESERVE;
	send_packet_at(20 * SEC, 2);
	assert_int_equal(num_events, 1);
	assert_int_equal(out->active_delay_ns, 8 * SEC);

	/* only the preserve bit counts */
	out->delay_cur_flags = ~(uint32_t)OBS_OUTPUT_DELAY_PRESERVE;
	send_packet_at(28 * SEC, 3);
	assert_int_equal(num_events, 2);
	assert_int_equal(events[1].pts, 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(set_delay_stores_seconds_and_flags, setup, teardown),
		cmocka_unit_test_setup_teardown(set_delay_ignored_on_raw_output, setup, teardown),
		cmocka_unit_test_setup_teardown(getters_on_null_output, setup, teardown),
		cmocka_unit_test_setup_teardown(active_delay_is_whole_seconds, setup, teardown),
		cmocka_unit_test_setup_teardown(packets_held_then_released_in_order, setup, teardown),
		cmocka_unit_test_setup_teardown(packet_time_is_copied, setup, teardown),
		cmocka_unit_test_setup_teardown(zero_delay_still_waits_for_the_next_packet, setup, teardown),
		cmocka_unit_test_setup_teardown(packets_dropped_unless_delay_is_capturing, setup, teardown),
		cmocka_unit_test_setup_teardown(clock_going_backwards_releases_at_once, setup, teardown),
		cmocka_unit_test_setup_teardown(delay_start_first_start, setup, teardown),
		cmocka_unit_test_setup_teardown(delay_start_refused_before_queueing, setup, teardown),
		cmocka_unit_test_setup_teardown(delay_start_failed_capture_cleans_up, setup, teardown),
		cmocka_unit_test_setup_teardown(delay_start_while_delay_active, setup, teardown),
		cmocka_unit_test_setup_teardown(queued_start_runs_after_the_delay, setup, teardown),
		cmocka_unit_test_setup_teardown(delay_stop_queues_a_stop_with_its_time, setup, teardown),
		cmocka_unit_test_setup_teardown(cleanup_releases_everything, setup, teardown),
		cmocka_unit_test_setup_teardown(preserve_while_reconnecting_grows_the_delay, setup, teardown),
		cmocka_unit_test_setup_teardown(preserve_holds_even_when_not_capturing, setup, teardown),
		cmocka_unit_test_setup_teardown(reconnecting_without_preserve_keeps_releasing, setup, teardown),
		cmocka_unit_test_setup_teardown(preserve_follows_the_running_flags, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
