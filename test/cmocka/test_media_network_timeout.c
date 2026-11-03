#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavformat/avformat.h>
#include <libavutil/dict.h>
#include <libavutil/opt.h>

#include <util/c99defs.h>
#include <util/platform.h>
#include <util/threading.h>

#include <media-playback/media-playback.h>
#include <media-playback/network-timeout.h>

/* obsproject/obs-studio#12958, #11537: a Media Source playing an RTSP camera
 * never reconnects once the camera goes silent (loses power, drops off the
 * network) without closing the connection. The media thread blocks in
 * av_read_frame() forever, so it never stops and obs-ffmpeg-source never
 * schedules a reconnect. */

#define TEST_RTSP_URL "rtsp://127.0.0.1:554/cam"

static bool rtsp_demuxer_knows_option(const char *key)
{
	const AVInputFormat *rtsp = av_find_input_format("rtsp");
	const AVClass *avformat_class = avformat_get_class();

	assert_non_null(rtsp);
	assert_non_null(rtsp->priv_class);

	/* AV_OPT_SEARCH_FAKE_OBJ takes a pointer to an AVClass pointer and
	 * only reads it. */
	const AVClass *rtsp_class = rtsp->priv_class;
	return av_opt_find((void *)&rtsp_class, key, NULL, 0, AV_OPT_SEARCH_FAKE_OBJ) != NULL ||
	       av_opt_find((void *)&avformat_class, key, NULL, 0, AV_OPT_SEARCH_FAKE_OBJ) != NULL;
}

/* Every option set for an RTSP input must exist in the linked FFmpeg;
 * avformat_open_input() silently leaves unknown ones unused. */
static void rtsp_timeout_options_are_known_to_ffmpeg(void **state)
{
	UNUSED_PARAMETER(state);
	AVDictionary *opts = NULL;
	const AVDictionaryEntry *entry = NULL;

	mp_media_set_network_timeouts(&opts, TEST_RTSP_URL);
	assert_true(av_dict_count(opts) > 0);

	while ((entry = av_dict_iterate(opts, entry))) {
		if (!rtsp_demuxer_knows_option(entry->key)) {
			/* fail_msg() expands to cm_print_error(), which the
			 * Windows cmocka library does not export. */
			fprintf(stderr, "FFmpeg's RTSP demuxer has no option '%s'\n", entry->key);
			fail();
		}
	}

	av_dict_free(&opts);
}

/* The RTSP demuxer's socket I/O timeout (microseconds) must be set. */
static void rtsp_socket_timeout_is_set(void **state)
{
	UNUSED_PARAMETER(state);
	AVDictionary *opts = NULL;

	mp_media_set_network_timeouts(&opts, TEST_RTSP_URL);

	const AVDictionaryEntry *timeout = av_dict_get(opts, "timeout", NULL, 0);
	if (!timeout) {
		fprintf(stderr, "no 'timeout' option is set for an RTSP input\n");
		fail();
	}
	assert_true(strtoll(timeout->value, NULL, 10) > 0);

	av_dict_free(&opts);
}

/* A timeout from the source's "FFmpeg Options" field wins. Passes today;
 * guards the fix against clobbering the user's workaround. */
static void user_timeout_is_kept(void **state)
{
	UNUSED_PARAMETER(state);
	AVDictionary *opts = NULL;

	av_dict_set(&opts, "timeout", "2000000", 0);
	mp_media_set_network_timeouts(&opts, TEST_RTSP_URL);

	const AVDictionaryEntry *timeout = av_dict_get(opts, "timeout", NULL, 0);
	assert_non_null(timeout);
	assert_string_equal(timeout->value, "2000000");

	av_dict_free(&opts);
}

#ifndef _WIN32
#include <arpa/inet.h>
#include <netinet/in.h>
#include <poll.h>
#include <sys/socket.h>
#include <unistd.h>

/* A camera that accepts the session, then goes silent: it answers OPTIONS,
 * DESCRIBE, SETUP and PLAY on 127.0.0.1 and afterwards sends nothing and
 * never closes the connection, like one that lost power. */
struct silent_camera {
	int listen_fd;
	int port;
	pthread_t thread;
	volatile bool quit;
};

#define SILENT_CAMERA_SDP                   \
	"v=0\r\n"                           \
	"o=- 0 0 IN IP4 127.0.0.1\r\n"      \
	"s=silent camera\r\n"               \
	"c=IN IP4 0.0.0.0\r\n"              \
	"t=0 0\r\n"                         \
	"m=video 0 RTP/AVP 96\r\n"          \
	"a=rtpmap:96 H264/90000\r\n"        \
	"a=fmtp:96 packetization-mode=1\r\n" \
	"a=control:track1\r\n"

static void silent_camera_reply(int fd, const char *request, int port)
{
	char reply[2048];
	const char *cseq_header = strstr(request, "CSeq:");
	int cseq = cseq_header ? atoi(cseq_header + 5) : 0;
	int len;

	if (strncmp(request, "OPTIONS", 7) == 0) {
		len = snprintf(reply, sizeof(reply),
			       "RTSP/1.0 200 OK\r\nCSeq: %d\r\n"
			       "Public: OPTIONS, DESCRIBE, SETUP, PLAY, TEARDOWN\r\n\r\n",
			       cseq);
	} else if (strncmp(request, "DESCRIBE", 8) == 0) {
		len = snprintf(reply, sizeof(reply),
			       "RTSP/1.0 200 OK\r\nCSeq: %d\r\n"
			       "Content-Base: rtsp://127.0.0.1:%d/cam/\r\n"
			       "Content-Type: application/sdp\r\n"
			       "Content-Length: %zu\r\n\r\n%s",
			       cseq, port, strlen(SILENT_CAMERA_SDP), SILENT_CAMERA_SDP);
	} else if (strncmp(request, "SETUP", 5) == 0) {
		len = snprintf(reply, sizeof(reply),
			       "RTSP/1.0 200 OK\r\nCSeq: %d\r\n"
			       "Transport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n"
			       "Session: 12345678;timeout=60\r\n\r\n",
			       cseq);
	} else if (strncmp(request, "PLAY", 4) == 0) {
		len = snprintf(reply, sizeof(reply),
			       "RTSP/1.0 200 OK\r\nCSeq: %d\r\n"
			       "Session: 12345678\r\nRange: npt=0.000-\r\n\r\n",
			       cseq);
	} else {
		/* Keep-alives after PLAY go unanswered: the camera is gone. */
		return;
	}

	if (send(fd, reply, (size_t)len, 0) != len)
		fprintf(stderr, "silent camera: short send\n");
}

static void *silent_camera_thread(void *data)
{
	struct silent_camera *camera = data;
	struct pollfd listen_poll = {.fd = camera->listen_fd, .events = POLLIN};
	int client_fd = -1;

	while (!os_atomic_load_bool(&camera->quit) && client_fd < 0) {
		if (poll(&listen_poll, 1, 100) > 0)
			client_fd = accept(camera->listen_fd, NULL, NULL);
	}

	char request[4096];
	size_t used = 0;
	bool playing = false;

	while (!os_atomic_load_bool(&camera->quit) && client_fd >= 0) {
		struct pollfd client_poll = {.fd = client_fd, .events = POLLIN};
		if (poll(&client_poll, 1, 100) <= 0)
			continue;

		ssize_t got = recv(client_fd, request + used, sizeof(request) - 1 - used, 0);
		if (got <= 0)
			break;
		used += (size_t)got;
		request[used] = '\0';

		char *end;
		while ((end = strstr(request, "\r\n\r\n"))) {
			*end = '\0';
			if (!playing)
				silent_camera_reply(client_fd, request, camera->port);
			if (strncmp(request, "PLAY", 4) == 0)
				playing = true;

			size_t consumed = (size_t)(end + 4 - request);
			memmove(request, request + consumed, used - consumed + 1);
			used -= consumed;
		}
		if (used == sizeof(request) - 1)
			used = 0;
	}

	if (client_fd >= 0)
		close(client_fd);
	return NULL;
}

static void silent_camera_start(struct silent_camera *camera)
{
	struct sockaddr_in addr = {0};
	socklen_t addr_len = sizeof(addr);

	addr.sin_family = AF_INET;
	addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
	addr.sin_port = 0;

	os_atomic_set_bool(&camera->quit, false);
	camera->listen_fd = socket(AF_INET, SOCK_STREAM, 0);
	assert_true(camera->listen_fd >= 0);
	assert_int_equal(bind(camera->listen_fd, (struct sockaddr *)&addr, sizeof(addr)), 0);
	assert_int_equal(listen(camera->listen_fd, 4), 0);
	assert_int_equal(getsockname(camera->listen_fd, (struct sockaddr *)&addr, &addr_len), 0);
	camera->port = ntohs(addr.sin_port);
	assert_int_equal(pthread_create(&camera->thread, NULL, silent_camera_thread, camera), 0);
}

static void silent_camera_stop(struct silent_camera *camera)
{
	os_atomic_set_bool(&camera->quit, true);
	pthread_join(camera->thread, NULL);
	close(camera->listen_fd);
}

static void ignore_video(void *opaque, struct obs_source_frame *frame)
{
	UNUSED_PARAMETER(opaque);
	UNUSED_PARAMETER(frame);
}

static void ignore_audio(void *opaque, struct obs_source_audio *audio)
{
	UNUSED_PARAMETER(opaque);
	UNUSED_PARAMETER(audio);
}

static void signal_stopped(void *opaque)
{
	os_event_signal(opaque);
}

/* The default reconnect delay is 10 s; a camera that went silent must be
 * given up on well within twice that so it is retried soon after it is
 * back. */
#define SILENT_CAMERA_STOP_DEADLINE_MS 20000

/* End to end through media-playback and FFmpeg's RTSP demuxer: the media
 * thread must stop (calling stop_cb, which makes obs-ffmpeg-source
 * reconnect) when the camera goes silent. */
static void silent_rtsp_camera_stops_media(void **state)
{
	UNUSED_PARAMETER(state);
	struct silent_camera camera = {0};
	os_event_t *stopped = NULL;
	char url[64];
	char ffmpeg_options[] = "rtsp_transport=tcp";

	silent_camera_start(&camera);
	snprintf(url, sizeof(url), "rtsp://127.0.0.1:%d/cam", camera.port);
	assert_int_equal(os_event_init(&stopped, OS_EVENT_TYPE_MANUAL), 0);

	struct mp_media_info info = {
		.opaque = stopped,
		.v_cb = ignore_video,
		.v_preload_cb = ignore_video,
		.v_seek_cb = ignore_video,
		.a_cb = ignore_audio,
		.stop_cb = signal_stopped,
		.path = url,
		.ffmpeg_options = ffmpeg_options,
		.buffering = 2 * 1024 * 1024,
		.speed = 100,
		.is_local_file = false,
	};
	media_playback_t *media = media_playback_create(&info);
	assert_non_null(media);
	media_playback_play(media, false, false);

	int waited = os_event_timedwait(stopped, SILENT_CAMERA_STOP_DEADLINE_MS);

	/* Destroying the media interrupts a blocked read, so this returns
	 * either way. */
	media_playback_destroy(media);
	silent_camera_stop(&camera);
	os_event_destroy(stopped);

	if (waited != 0) {
		fprintf(stderr, "media did not stop within %d ms of the camera going silent\n",
			SILENT_CAMERA_STOP_DEADLINE_MS);
		fail();
	}
}
#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(rtsp_timeout_options_are_known_to_ffmpeg),
		cmocka_unit_test(rtsp_socket_timeout_is_set),
		cmocka_unit_test(user_timeout_is_kept),
#ifndef _WIN32
		cmocka_unit_test(silent_rtsp_camera_stops_media),
#endif
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
