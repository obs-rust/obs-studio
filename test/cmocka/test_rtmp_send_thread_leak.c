/* The RTMP send thread leaks the packet it is sending when the connection
 * fails (upstream obsproject/obs-studio#9972).
 *
 * send_thread() takes the next encoder packet from its queue, sends the
 * stream headers first if they are not sent yet, then sends the packet.
 * When the headers can't be sent (the server went away) it gives up
 * without releasing the packet it already took. With Stream Delay and
 * "Preserve cutoff point when reconnecting" on, the first packet reaches
 * the send thread seconds after the connection is up, so a server that
 * drops the connection in between leaks one packet per reconnect.
 *
 * send_packet(), send_packet_ex() and send_audio_packet_ex() read what the
 * server sent before they write. When that read fails they return without
 * releasing the packet (or the header data) they were given, so a server
 * that sends a broken message and drops the connection leaks too.
 *
 * The test builds the real rtmp-stream.c into this executable and runs its
 * send thread on one end of a socket pair. The other end plays the server:
 * it is closed (writes fail), or it sends one byte of an RTMP message and
 * is closed (reads fail). The output and encoder are small fakes, so no
 * network, display or encoder is needed. bmem's allocation count shows
 * whether everything the thread was given was freed.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <signal.h>
#include <sys/socket.h>
#include <unistd.h>

/* Route the libobs calls the send thread makes to the fakes below. The rest
 * of rtmp-stream.c links against libobs as usual. */
#define obs_output_get_name fake_output_get_name
#define obs_output_get_video_encoder2 fake_output_get_video_encoder2
#define obs_output_get_audio_encoder fake_output_get_audio_encoder
#define obs_output_set_last_error fake_output_set_last_error
#define obs_output_signal_stop fake_output_signal_stop
#define obs_output_end_data_capture fake_output_end_data_capture
#define obs_encoder_get_extra_data fake_encoder_get_extra_data
#define obs_encoder_video fake_encoder_video
#define video_output_get_info fake_video_output_get_info

#include "rtmp-stream.c"

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

/* ------------------------------------------------------------------------- */
/* fakes */

static int fake_output;        /* only its address is used */
static int fake_encoder;       /* only its address is used */
static int fake_audio_encoder; /* only its address is used */
static int fake_video;         /* only its address is used */
static bool with_audio;
static os_event_t *thread_done;

#define FAKE_OUTPUT ((obs_output_t *)&fake_output)
#define FAKE_ENCODER ((obs_encoder_t *)&fake_encoder)
#define FAKE_AUDIO_ENCODER ((obs_encoder_t *)&fake_audio_encoder)

/* An H.264 header: SPS and PPS in Annex B. */
static uint8_t video_extra[] = {0,    0, 0, 1, 0x67, 0x64, 0,    0x1f, 0xac, 0xd9, 0x40,
				0x50, 0, 0, 0, 1,    0x68, 0xeb, 0xe3, 0xcb, 0x22, 0xc0};
/* An AAC AudioSpecificConfig. */
static uint8_t audio_extra[] = {0x11, 0x90};

const char *fake_output_get_name(const obs_output_t *output)
{
	(void)output;
	return "leak test";
}

obs_encoder_t *fake_output_get_video_encoder2(const obs_output_t *output, size_t idx)
{
	(void)output;
	return idx == 0 ? FAKE_ENCODER : NULL;
}

obs_encoder_t *fake_output_get_audio_encoder(const obs_output_t *output, size_t idx)
{
	(void)output;
	return with_audio && idx == 0 ? FAKE_AUDIO_ENCODER : NULL;
}

void fake_output_set_last_error(obs_output_t *output, const char *message)
{
	(void)output;
	(void)message;
}

void fake_output_signal_stop(obs_output_t *output, int code)
{
	(void)output;
	(void)code;
	os_event_signal(thread_done);
}

void fake_output_end_data_capture(obs_output_t *output)
{
	(void)output;
	os_event_signal(thread_done);
}

bool fake_encoder_get_extra_data(const obs_encoder_t *encoder, uint8_t **extra_data, size_t *size)
{
	bool audio = encoder == FAKE_AUDIO_ENCODER;
	*extra_data = audio ? audio_extra : video_extra;
	*size = audio ? sizeof(audio_extra) : sizeof(video_extra);
	return true;
}

video_t *fake_encoder_video(const obs_encoder_t *encoder)
{
	(void)encoder;
	return (video_t *)&fake_video;
}

const struct video_output_info *fake_video_output_get_info(const video_t *video)
{
	(void)video;
	static const struct video_output_info info = {.colorspace = VIDEO_CS_709};
	return &info;
}

/* ------------------------------------------------------------------------- */
/* helpers */

enum server {
	SERVER_GONE,        /* the server closed the connection: writes fail */
	SERVER_BROKEN_READ, /* it sent part of a message and closed: reads fail */
};

struct conn {
	struct rtmp_stream *stream;
	long allocs_before;
};

static struct conn start_stream(enum server server, bool headers_sent)
{
	struct conn c;
	c.allocs_before = bnum_allocs();

	struct rtmp_stream *stream = bzalloc(sizeof(*stream));
	stream->output = FAKE_OUTPUT;
	stream->video_codec[0] = CODEC_H264;
	stream->audio_codec[0] = AUDIO_CODEC_AAC;
	stream->sent_headers = headers_sent;
	dstr_copy(&stream->path, "rtmp://127.0.0.1/live");
	assert_int_equal(pthread_mutex_init(&stream->packets_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&stream->dbr_mutex, NULL), 0);
	assert_int_equal(os_event_init(&stream->stop_event, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_sem_init(&stream->send_sem, 0), 0);
	assert_int_equal(os_event_init(&thread_done, OS_EVENT_TYPE_MANUAL), 0);

	RTMP_Init(&stream->rtmp);
	int sv[2];
	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM, 0, sv), 0);
	stream->rtmp.m_sb.sb_socket = sv[0];
	if (server == SERVER_BROKEN_READ) {
		/* The first byte of a chunk header, then nothing. */
		const char partial = 0x03;
		assert_int_equal(write(sv[1], &partial, 1), 1);
	}
	close(sv[1]);

	c.stream = stream;
	return c;
}

/* An encoder packet the way libobs hands it to an output: its data is
 * reference counted (a long in front of the data, see
 * obs_encoder_packet_release()), and the queue holds one reference. */
static void queue_video_packet(struct rtmp_stream *stream)
{
	static const uint8_t frame[] = {0, 0, 0, 1, 0x65, 0x88, 0x84, 0x00, 0x33, 0xff};
	long *refs = bmalloc(sizeof(long) + sizeof(frame));
	*refs = 1;
	memcpy(refs + 1, frame, sizeof(frame));

	struct encoder_packet packet = {
		.type = OBS_ENCODER_VIDEO,
		.data = (uint8_t *)(refs + 1),
		.size = sizeof(frame),
		.timebase_num = 1,
		.timebase_den = 30,
		.keyframe = true,
	};

	pthread_mutex_lock(&stream->packets_mutex);
	deque_push_back(&stream->packets, &packet, sizeof(packet));
	pthread_mutex_unlock(&stream->packets_mutex);
	os_sem_post(stream->send_sem);
}

/* Runs the send thread until it gives up on the connection, then frees the
 * stream the way rtmp_stream_destroy() does. Returns the number of
 * allocations left over. */
static long run_send_thread(struct conn c)
{
	struct rtmp_stream *stream = c.stream;

	assert_int_equal(pthread_create(&stream->send_thread, NULL, send_thread, stream), 0);
	/* The thread detaches itself when the connection drops. */
	assert_int_equal(os_event_timedwait(thread_done, 10000), 0);
	assert_true(disconnected(stream));

	free_packets(stream);
	deque_free(&stream->packets);
	deque_free(&stream->dbr_frames);
	dstr_free(&stream->path);
	os_sem_destroy(stream->send_sem);
	os_event_destroy(stream->stop_event);
	os_event_destroy(thread_done);
	pthread_mutex_destroy(&stream->dbr_mutex);
	pthread_mutex_destroy(&stream->packets_mutex);
	bfree(stream);

	return bnum_allocs() - c.allocs_before;
}

/* ------------------------------------------------------------------------- */
/* cases */

/* The report: the server is gone when the first packet arrives, so the
 * headers can't be written. */
static void headers_write_fails(void **state)
{
	(void)state;
	with_audio = false;
	struct conn c = start_stream(SERVER_GONE, false);
	queue_video_packet(c.stream);

	long left = run_send_thread(c);
	if (left != 0)
		fail_msg("headers could not be written: %ld allocation(s) leaked", left);
}

/* With an audio track: the first header write already fails. */
static void audio_header_write_fails(void **state)
{
	(void)state;
	with_audio = true;
	struct conn c = start_stream(SERVER_GONE, false);
	queue_video_packet(c.stream);

	long left = run_send_thread(c);
	if (left != 0)
		fail_msg("audio header could not be written: %ld allocation(s) leaked", left);
}

/* The server sent part of a message, then closed: reading it before the
 * header write fails. */
static void headers_read_fails(void **state)
{
	(void)state;
	with_audio = false;
	struct conn c = start_stream(SERVER_BROKEN_READ, false);
	queue_video_packet(c.stream);

	long left = run_send_thread(c);
	if (left != 0)
		fail_msg("server message could not be read before the headers: %ld allocation(s) leaked", left);
}

/* Headers are out; reading the server's message before a packet fails. */
static void packet_read_fails(void **state)
{
	(void)state;
	with_audio = false;
	struct conn c = start_stream(SERVER_BROKEN_READ, true);
	queue_video_packet(c.stream);

	long left = run_send_thread(c);
	if (left != 0)
		fail_msg("server message could not be read before a packet: %ld allocation(s) leaked", left);
}

/* Guard: headers are out and writing a packet fails. send_packet() already
 * released the packet here before the fix. */
static void packet_write_fails(void **state)
{
	(void)state;
	with_audio = false;
	struct conn c = start_stream(SERVER_GONE, true);
	queue_video_packet(c.stream);

	long left = run_send_thread(c);
	assert_int_equal(left, 0);
}

int main(void)
{
	/* Like OBS: a write to a closed connection fails instead of killing
	 * the process. */
	signal(SIGPIPE, SIG_IGN);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(headers_write_fails), cmocka_unit_test(audio_header_write_fails),
		cmocka_unit_test(headers_read_fails),  cmocka_unit_test(packet_read_fails),
		cmocka_unit_test(packet_write_fails),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
