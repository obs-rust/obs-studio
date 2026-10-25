#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <setjmp.h>
#include <sys/eventfd.h>
#include <sys/mman.h>
#include <cmocka.h>

#include <linux/videodev2.h>
#include <libv4l2.h>

#include <libavcodec/avcodec.h>
#include <libavutil/frame.h>

#include <obs-module.h>

#include "v4l2-decoder.h"
#include "v4l2-frame-action.h"
#include "v4l2-helpers.h"

/* One corrupt MJPEG or H.264 frame ends a V4L2 source's capture until OBS is
 * restarted (obsproject/obs-studio#12521 by gNovp, and the closed #9013 and
 * #9434 by es20490446e, where NikolaiSchreier traced it to the capture loop:
 * a decoder error "is treated as unrecoverable and the loop is exited", and
 * skipping the frame might be enough).
 *
 * The log shows "decoder: failed to send frame to codec", "failed to unpack
 * jpeg or h264" and "Stopped capture after N frames". libavcodec returns an
 * error for that one packet only and decodes the next good one, so the
 * capture thread has no reason to stop.
 *
 * The first group of tests feeds real MJPEG and H.264 bytes through the
 * real decoder and the per-buffer decision the capture thread uses. The
 * second group runs the real capture thread from v4l2-input.c against a
 * fake libv4l2 (below), so no camera, kernel module or display is needed. */

#define WIDTH 64
#define HEIGHT 48

/* ------------------------------------------------------------------------ */
/* MJPEG frames, encoded by libavcodec with a solid luma per frame */

struct packet {
	uint8_t *data;
	size_t size;
};

static struct packet encode_mjpeg(uint8_t luma)
{
	struct packet result = {0};
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_MJPEG);
	if (!codec) {
		fprintf(stderr, "libavcodec has no MJPEG encoder\n");
		fail();
	}

	AVCodecContext *ctx = avcodec_alloc_context3(codec);
	ctx->width = WIDTH;
	ctx->height = HEIGHT;
	ctx->pix_fmt = AV_PIX_FMT_YUVJ420P;
	ctx->color_range = AVCOL_RANGE_JPEG;
	ctx->time_base = (AVRational){1, 30};
	ctx->strict_std_compliance = FF_COMPLIANCE_UNOFFICIAL;
	if (avcodec_open2(ctx, codec, NULL) < 0) {
		fprintf(stderr, "cannot open the MJPEG encoder\n");
		fail();
	}

	AVFrame *frame = av_frame_alloc();
	frame->width = WIDTH;
	frame->height = HEIGHT;
	frame->format = ctx->pix_fmt;
	frame->color_range = AVCOL_RANGE_JPEG;
	assert_int_equal(av_frame_get_buffer(frame, 0), 0);
	for (int y = 0; y < HEIGHT; y++)
		memset(frame->data[0] + y * frame->linesize[0], luma, WIDTH);
	for (int p = 1; p < 3; p++) {
		for (int y = 0; y < HEIGHT / 2; y++)
			memset(frame->data[p] + y * frame->linesize[p], 128, WIDTH / 2);
	}

	AVPacket *pkt = av_packet_alloc();
	assert_int_equal(avcodec_send_frame(ctx, frame), 0);
	assert_int_equal(avcodec_receive_packet(ctx, pkt), 0);

	result.size = pkt->size;
	result.data = malloc(pkt->size);
	memcpy(result.data, pkt->data, pkt->size);

	av_packet_free(&pkt);
	av_frame_free(&frame);
	avcodec_free_context(&ctx);
	return result;
}

/* What a damaged USB transfer looks like to the decoder: no valid JPEG */
static struct packet junk_packet(void)
{
	struct packet result = {malloc(3000), 3000};
	uint32_t x = 12521;
	for (size_t i = 0; i < result.size; i++) {
		x = x * 1103515245u + 12345u;
		result.data[i] = (uint8_t)(x >> 16);
	}
	return result;
}

/* A 16x16 grey H.264 baseline frame from x264, as two buffers: the sequence
 * and picture parameter sets alone, then the IDR slice. */
static const uint8_t h264_parameter_sets[] = {0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xc0, 0x0a, 0xdd, 0xec, 0x04,
					      0x40, 0x00, 0x00, 0x03, 0x00, 0x40, 0x00, 0x00, 0x0c, 0x83, 0xc4,
					      0x89, 0xe0, 0x00, 0x00, 0x00, 0x01, 0x68, 0xce, 0x0f, 0x2c, 0x80};
static const uint8_t h264_idr[] = {0x00, 0x00, 0x01, 0x65, 0x88, 0x84, 0x04, 0xbc, 0x98, 0xa0, 0x00, 0x20, 0x2f, 0x80};

/* Four 16x16 H.264 main profile frames from x264 with two B-frames (I B B P,
 * sent as I P B B), one access unit per buffer. The decoder holds the first
 * two back to reorder them, so it has no picture for them (EAGAIN). */
static const uint8_t h264_reorder_au0[] = {0x00, 0x00, 0x00, 0x01, 0x09, 0x10, 0x00, 0x00, 0x00, 0x01, 0x67,
					   0x4d, 0x40, 0x0a, 0xec, 0xaf, 0x60, 0x22, 0x00, 0x00, 0x03, 0x00,
					   0x02, 0x00, 0x00, 0x03, 0x00, 0x78, 0x1e, 0x24, 0x4b, 0x2c, 0x00,
					   0x00, 0x00, 0x01, 0x68, 0xef, 0x8f, 0x2c, 0x80, 0x00, 0x00, 0x01,
					   0x65, 0x88, 0x84, 0x00, 0x25, 0xff, 0xf4, 0x14, 0x6b, 0xf1, 0x85,
					   0x27, 0x56, 0xd1, 0x5f, 0x7e, 0xee, 0xe8, 0x5f, 0x06, 0x01, 0x51};
static const uint8_t h264_reorder_au1[] = {0x00, 0x00, 0x00, 0x01, 0x09, 0x30, 0x00, 0x00, 0x01, 0x41,
					   0x9a, 0x23, 0x18, 0x85, 0x3f, 0xfe, 0x74, 0x67, 0x88, 0xad,
					   0xbb, 0x7c, 0x03, 0xda, 0xe3, 0xbf, 0x7e, 0xbf};
static const uint8_t h264_reorder_au2[] = {0x00, 0x00, 0x00, 0x01, 0x09, 0x50, 0x00, 0x00, 0x01,
					   0x41, 0x9e, 0x41, 0x42, 0x12, 0xff, 0xeb, 0x81};
static const uint8_t h264_reorder_au3[] = {0x00, 0x00, 0x00, 0x01, 0x09, 0x50, 0x00, 0x00, 0x01,
					   0x01, 0x9e, 0x62, 0x44, 0x29, 0xff, 0xec, 0x80};

static void assert_luma(const struct obs_source_frame *out, uint8_t luma)
{
	if (!out->data[0]) {
		fprintf(stderr, "no picture where luma %d was expected\n", luma);
		fail();
	}
	int got = out->data[0][(HEIGHT / 2) * out->linesize[0] + WIDTH / 2];
	if (abs(got - luma) > 3) {
		fprintf(stderr, "luma %d where the frame had %d\n", got, luma);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* The per-buffer decision */

struct decode_state {
	struct v4l2_decoder decoder;
	struct obs_source_frame out;
	size_t plane_offsets[MAX_AV_PLANES];
	uint8_t buffer[64 * 1024];
};

static int setup_decoder(void **state, int pixfmt)
{
	struct decode_state *s = calloc(1, sizeof(*s));
	if (v4l2_init_decoder(&s->decoder, pixfmt) < 0) {
		v4l2_destroy_decoder(&s->decoder);
		free(s);
		return -1;
	}
	*state = s;
	return 0;
}

static int setup_mjpeg(void **state)
{
	return setup_decoder(state, V4L2_PIX_FMT_MJPEG);
}

static int setup_h264(void **state)
{
	return setup_decoder(state, V4L2_PIX_FMT_H264);
}

static int teardown_decoder(void **state)
{
	struct decode_state *s = *state;
	v4l2_destroy_decoder(&s->decoder);
	free(s);
	return 0;
}

/* Copies the bytes into the "mapped" buffer, as the driver would, and asks
 * what the capture thread should do with it. */
static enum v4l2_frame_action feed(struct decode_state *s, int pixfmt, const uint8_t *data, size_t size, uint32_t flags)
{
	struct v4l2_buffer buf;
	memset(&buf, 0, sizeof(buf));
	buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	buf.memory = V4L2_MEMORY_MMAP;
	buf.flags = flags;
	buf.bytesused = (uint32_t)size;
	buf.length = sizeof(s->buffer);

	memset(s->buffer, 0, sizeof(s->buffer));
	if (size)
		memcpy(s->buffer, data, size);
	return v4l2_frame_action(&s->out, pixfmt, &buf, s->buffer, s->plane_offsets, &s->decoder);
}

static const char *action_name(enum v4l2_frame_action action)
{
	switch (action) {
	case V4L2_FRAME_OUTPUT:
		return "output";
	case V4L2_FRAME_SKIP:
		return "skip";
	case V4L2_FRAME_DROP:
		return "drop";
	default:
		return "stop capture";
	}
}

static void expect_action(enum v4l2_frame_action got, enum v4l2_frame_action want, const char *what)
{
	if (got != want) {
		fprintf(stderr, "%s: capture would %s, expected %s\n", what, action_name(got), action_name(want));
		fail();
	}
}

static void corrupt_mjpeg_frame_is_dropped(void **state)
{
	struct decode_state *s = *state;
	static const uint8_t lumas[] = {40, 120, 200};
	struct packet good[3];
	struct packet junk = junk_packet();

	for (size_t i = 0; i < 3; i++)
		good[i] = encode_mjpeg(lumas[i]);

	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, good[0].data, good[0].size, 0), V4L2_FRAME_OUTPUT,
		      "first good frame");
	assert_luma(&s->out, lumas[0]);

	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, junk.data, junk.size, 0), V4L2_FRAME_DROP, "corrupt frame");

	for (size_t i = 1; i < 3; i++) {
		expect_action(feed(s, V4L2_PIX_FMT_MJPEG, good[i].data, good[i].size, 0), V4L2_FRAME_OUTPUT,
			      "good frame after the corrupt one");
		assert_luma(&s->out, lumas[i]);
	}

	for (size_t i = 0; i < 3; i++)
		free(good[i].data);
	free(junk.data);
}

/* A burst of bad frames, as when the USB link is saturated for a moment */
static void burst_of_corrupt_frames_is_dropped(void **state)
{
	struct decode_state *s = *state;
	struct packet junk = junk_packet();
	struct packet good = encode_mjpeg(90);

	for (int i = 0; i < 10; i++)
		expect_action(feed(s, V4L2_PIX_FMT_MJPEG, junk.data, junk.size, 0), V4L2_FRAME_DROP,
			      "corrupt frame in a burst");

	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, good.data, good.size, 0), V4L2_FRAME_OUTPUT,
		      "good frame after the burst");
	assert_luma(&s->out, 90);

	free(good.data);
	free(junk.data);
}

/* bytesused 0: libavcodec rejects an empty packet with data (EINVAL) */
static void empty_buffer_is_skipped(void **state)
{
	struct decode_state *s = *state;
	struct packet good = encode_mjpeg(160);

	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, NULL, 0, 0), V4L2_FRAME_SKIP, "empty buffer");
	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, good.data, good.size, 0), V4L2_FRAME_OUTPUT,
		      "good frame after the empty buffer");
	assert_luma(&s->out, 160);

	free(good.data);
}

/* Control: buffers the driver flags as damaged were already skipped
 * (obsproject/obs-studio#12093). */
static void error_flagged_buffer_is_skipped(void **state)
{
	struct decode_state *s = *state;
	struct packet good = encode_mjpeg(70);

	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, good.data, good.size, V4L2_BUF_FLAG_ERROR), V4L2_FRAME_SKIP,
		      "buffer flagged with V4L2_BUF_FLAG_ERROR");
	expect_action(feed(s, V4L2_PIX_FMT_MJPEG, good.data, good.size, 0), V4L2_FRAME_OUTPUT, "good frame");
	assert_luma(&s->out, 70);

	free(good.data);
}

/* While the decoder holds frames back to reorder them (EAGAIN) there is no
 * new picture, so nothing may be output: out would still point at the
 * decoder's previous picture, which it has released, or at nothing. */
static void h264_buffer_without_new_picture_is_skipped(void **state)
{
	struct decode_state *s = *state;

	expect_action(feed(s, V4L2_PIX_FMT_H264, h264_reorder_au0, sizeof(h264_reorder_au0), 0), V4L2_FRAME_SKIP,
		      "H.264 I frame held for reordering");
	expect_action(feed(s, V4L2_PIX_FMT_H264, h264_reorder_au1, sizeof(h264_reorder_au1), 0), V4L2_FRAME_SKIP,
		      "H.264 P frame held for reordering");
	expect_action(feed(s, V4L2_PIX_FMT_H264, h264_reorder_au2, sizeof(h264_reorder_au2), 0), V4L2_FRAME_OUTPUT,
		      "H.264 B frame");
	assert_non_null(s->out.data[0]);
	expect_action(feed(s, V4L2_PIX_FMT_H264, h264_reorder_au3, sizeof(h264_reorder_au3), 0), V4L2_FRAME_OUTPUT,
		      "H.264 B frame");
	assert_non_null(s->out.data[0]);
}

/* libavcodec rejects a buffer with only the parameter sets ("no frame!"),
 * but keeps them for the slice that follows. */
static void h264_parameter_sets_alone_are_dropped(void **state)
{
	struct decode_state *s = *state;

	expect_action(feed(s, V4L2_PIX_FMT_H264, h264_parameter_sets, sizeof(h264_parameter_sets), 0), V4L2_FRAME_DROP,
		      "H.264 parameter sets without a slice");
	expect_action(feed(s, V4L2_PIX_FMT_H264, h264_idr, sizeof(h264_idr), 0), V4L2_FRAME_OUTPUT, "H.264 IDR slice");
	assert_non_null(s->out.data[0]);
}

static void h264_corrupt_frame_is_dropped(void **state)
{
	struct decode_state *s = *state;
	struct packet junk = junk_packet();
	uint8_t frame[sizeof(h264_parameter_sets) + sizeof(h264_idr)];

	memcpy(frame, h264_parameter_sets, sizeof(h264_parameter_sets));
	memcpy(frame + sizeof(h264_parameter_sets), h264_idr, sizeof(h264_idr));

	expect_action(feed(s, V4L2_PIX_FMT_H264, frame, sizeof(frame), 0), V4L2_FRAME_OUTPUT, "H.264 frame");
	expect_action(feed(s, V4L2_PIX_FMT_H264, junk.data, junk.size, 0), V4L2_FRAME_DROP, "corrupt H.264 frame");
	expect_action(feed(s, V4L2_PIX_FMT_H264, frame, sizeof(frame), 0), V4L2_FRAME_OUTPUT,
		      "H.264 frame after the corrupt one");

	free(junk.data);
}

/* Control: raw formats point the frame's planes into the buffer */
static void raw_buffer_is_output_in_place(void **state)
{
	(void)state;
	static struct decode_state s;
	const uint8_t yuyv[32] = {0};
	s.plane_offsets[1] = 100;

	expect_action(feed(&s, V4L2_PIX_FMT_YUYV, yuyv, sizeof(yuyv), 0), V4L2_FRAME_OUTPUT, "YUYV buffer");
	assert_ptr_equal(s.out.data[0], s.buffer);
	assert_ptr_equal(s.out.data[1], s.buffer + 100);
}

/* ------------------------------------------------------------------------ */
/* Fake libv4l2: one capture device whose frames come from a script. The
 * device fd is an eventfd, readable while frames are left, so select() in
 * the capture thread behaves as with a camera. */

#define FAKE_BUFFERS 4
#define FAKE_BUFFER_SIZE (64 * 1024)
#define MAX_OUTPUTS 32

struct fake_frame {
	const uint8_t *data;
	size_t size;
	int dqbuf_errno; /* nonzero: VIDIOC_DQBUF fails like this */
};

static struct {
	pthread_mutex_t mutex;
	pthread_cond_t cond;
	int fd;
	uint8_t *memory[FAKE_BUFFERS];
	bool queued[FAKE_BUFFERS];
	const struct fake_frame *script;
	size_t script_size;
	size_t next;
	uint32_t sequence;
	bool streamoff;

	size_t outputs;
	int output_luma[MAX_OUTPUTS];
} fake = {.mutex = PTHREAD_MUTEX_INITIALIZER, .cond = PTHREAD_COND_INITIALIZER, .fd = -1};

static void fake_reset(const struct fake_frame *script, size_t size)
{
	pthread_mutex_lock(&fake.mutex);
	for (size_t i = 0; i < FAKE_BUFFERS; i++) {
		if (!fake.memory[i])
			fake.memory[i] = malloc(FAKE_BUFFER_SIZE);
		fake.queued[i] = false;
	}
	fake.script = script;
	fake.script_size = size;
	fake.next = 0;
	fake.sequence = 0;
	fake.streamoff = false;
	fake.outputs = 0;
	pthread_mutex_unlock(&fake.mutex);
}

static void fake_drain(void)
{
	uint64_t value;
	while (read(fake.fd, &value, sizeof(value)) > 0)
		;
}

int v4l2_open(const char *file, int oflag, ...)
{
	(void)file;
	(void)oflag;
	fake.fd = eventfd(1, EFD_NONBLOCK | EFD_CLOEXEC);
	return fake.fd;
}

int v4l2_close(int fd)
{
	if (fd == fake.fd)
		fake.fd = -1;
	return close(fd);
}

void *v4l2_mmap(void *start, size_t length, int prot, int flags, int fd, int64_t offset)
{
	(void)start;
	(void)length;
	(void)prot;
	(void)flags;
	(void)fd;
	size_t index = (size_t)offset / FAKE_BUFFER_SIZE;
	return index < FAKE_BUFFERS ? fake.memory[index] : MAP_FAILED;
}

int v4l2_munmap(void *start, size_t length)
{
	(void)start;
	(void)length;
	return 0;
}

static int fake_dqbuf(struct v4l2_buffer *buf)
{
	if (fake.next >= fake.script_size) {
		fake_drain();
		errno = EAGAIN;
		return -1;
	}

	const struct fake_frame *frame = &fake.script[fake.next];
	if (frame->dqbuf_errno) {
		fake.next++;
		errno = frame->dqbuf_errno;
		return -1;
	}

	for (uint32_t i = 0; i < FAKE_BUFFERS; i++) {
		if (!fake.queued[i])
			continue;

		fake.queued[i] = false;
		fake.next++;
		memcpy(fake.memory[i], frame->data, frame->size);
		buf->index = i;
		buf->bytesused = (uint32_t)frame->size;
		buf->length = FAKE_BUFFER_SIZE;
		buf->flags = 0;
		buf->sequence = fake.sequence++;
		buf->timestamp.tv_sec = 1000;
		buf->timestamp.tv_usec = buf->sequence * 33333;
		if (fake.next >= fake.script_size)
			fake_drain();
		return 0;
	}

	errno = EAGAIN;
	return -1;
}

#ifdef LIBV4L_HAVE_POSIX_IOCTL
int v4l2_ioctl(int fd, int request, ...)
#else
int v4l2_ioctl(int fd, unsigned long int request, ...)
#endif
{
	va_list args;
	va_start(args, request);
	void *arg = va_arg(args, void *);
	va_end(args);

	if (fd != fake.fd) {
		errno = EBADF;
		return -1;
	}

	int result = 0;
	pthread_mutex_lock(&fake.mutex);

	switch ((unsigned long)request) {
	case VIDIOC_G_INPUT:
		*(int *)arg = 0;
		break;
	case VIDIOC_S_INPUT:
	case VIDIOC_S_FMT:
	case VIDIOC_S_PARM:
	case VIDIOC_LOG_STATUS:
		break;
	case VIDIOC_ENUMINPUT:
		((struct v4l2_input *)arg)->capabilities = 0;
		break;
	case VIDIOC_G_FMT: {
		struct v4l2_format *fmt = arg;
		fmt->fmt.pix.width = WIDTH;
		fmt->fmt.pix.height = HEIGHT;
		fmt->fmt.pix.pixelformat = V4L2_PIX_FMT_MJPEG;
		fmt->fmt.pix.bytesperline = 0;
		break;
	}
	case VIDIOC_G_PARM: {
		struct v4l2_streamparm *par = arg;
		par->parm.capture.timeperframe.numerator = 1;
		par->parm.capture.timeperframe.denominator = 30;
		break;
	}
	case VIDIOC_REQBUFS:
		((struct v4l2_requestbuffers *)arg)->count = FAKE_BUFFERS;
		break;
	case VIDIOC_QUERYBUF: {
		struct v4l2_buffer *buf = arg;
		buf->length = FAKE_BUFFER_SIZE;
		buf->m.offset = buf->index * FAKE_BUFFER_SIZE;
		break;
	}
	case VIDIOC_QBUF:
		fake.queued[((struct v4l2_buffer *)arg)->index] = true;
		break;
	case VIDIOC_DQBUF:
		result = fake_dqbuf(arg);
		break;
	case VIDIOC_STREAMON:
		break;
	case VIDIOC_STREAMOFF:
		fake.streamoff = true;
		pthread_cond_broadcast(&fake.cond);
		break;
	default:
		errno = ENOTTY;
		result = -1;
	}

	pthread_mutex_unlock(&fake.mutex);
	return result;
}

/* The source handed to the plugin is NULL: frames go here instead of into
 * libobs, which is not started. */
void obs_source_output_video(obs_source_t *source, const struct obs_source_frame *frame)
{
	(void)source;
	pthread_mutex_lock(&fake.mutex);
	if (frame && fake.outputs < MAX_OUTPUTS)
		fake.output_luma[fake.outputs] =
			frame->data[0] ? frame->data[0][(HEIGHT / 2) * frame->linesize[0] + WIDTH / 2] : -1;
	fake.outputs++;
	pthread_cond_broadcast(&fake.cond);
	pthread_mutex_unlock(&fake.mutex);
}

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

extern struct obs_source_info v4l2_input;

static void *start_source(void)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", "fake-camera");
	obs_data_set_int(settings, "input", 0);
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_MJPEG);
	obs_data_set_int(settings, "standard", -1);
	obs_data_set_int(settings, "dv_timing", -1);
	obs_data_set_int(settings, "resolution", v4l2_pack_tuple(WIDTH, HEIGHT));
	obs_data_set_int(settings, "framerate", v4l2_pack_tuple(1, 30));
	obs_data_set_int(settings, "color_range", VIDEO_RANGE_DEFAULT);
	obs_data_set_bool(settings, "buffering", true);
	obs_data_set_bool(settings, "auto_reset", false);
	obs_data_set_int(settings, "timeout_frames", 5);

	void *data = v4l2_input.create(settings, NULL);
	obs_data_release(settings);
	return data;
}

/* Waits until the capture output `outputs` frames or stopped. */
static void wait_for_capture(size_t outputs)
{
	struct timespec deadline;
	clock_gettime(CLOCK_REALTIME, &deadline);
	deadline.tv_sec += 10;

	pthread_mutex_lock(&fake.mutex);
	while (fake.outputs < outputs && !fake.streamoff) {
		if (pthread_cond_timedwait(&fake.cond, &fake.mutex, &deadline) == ETIMEDOUT)
			break;
	}
	pthread_mutex_unlock(&fake.mutex);
}

static void capture_continues_after_corrupt_frame(void **state)
{
	(void)state;
	static const uint8_t lumas[] = {30, 80, 130, 180, 230};
	struct packet good[5];
	struct packet junk = junk_packet();

	for (size_t i = 0; i < 5; i++)
		good[i] = encode_mjpeg(lumas[i]);

	const struct fake_frame script[] = {
		{good[0].data, good[0].size, 0}, {junk.data, junk.size, 0},       {good[1].data, good[1].size, 0},
		{good[2].data, good[2].size, 0}, {good[3].data, good[3].size, 0}, {good[4].data, good[4].size, 0},
	};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	wait_for_capture(5);

	pthread_mutex_lock(&fake.mutex);
	size_t outputs = fake.outputs;
	bool stopped = fake.streamoff;
	int luma[5];
	memcpy(luma, fake.output_luma, sizeof(luma));
	pthread_mutex_unlock(&fake.mutex);

	v4l2_input.destroy(source);

	if (stopped && outputs < 5) {
		fprintf(stderr, "capture stopped after %zu of 5 good frames (the corrupt frame was #2)\n", outputs);
		fail();
	}
	if (outputs != 5) {
		fprintf(stderr, "capture output %zu frames for 5 good frames\n", outputs);
		fail();
	}
	for (size_t i = 0; i < 5; i++) {
		if (abs(luma[i] - lumas[i]) > 3) {
			fprintf(stderr, "frame %zu has luma %d, the camera sent %d\n", i, luma[i], lumas[i]);
			fail();
		}
	}

	for (size_t i = 0; i < 5; i++)
		free(good[i].data);
	free(junk.data);
}

/* Control: a device error (ENODEV) still stops the stream; the capture
 * thread then closes the device and opens it again after a delay, which
 * test_v4l2_dqbuf_error covers (obs-rust/obs-studio#2264). */
static void capture_stops_when_device_is_gone(void **state)
{
	(void)state;
	struct packet good = encode_mjpeg(100);

	const struct fake_frame script[] = {
		{good.data, good.size, 0},
		{NULL, 0, ENODEV},
		{good.data, good.size, 0},
	};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	wait_for_capture(SIZE_MAX);

	pthread_mutex_lock(&fake.mutex);
	size_t outputs = fake.outputs;
	bool stopped = fake.streamoff;
	pthread_mutex_unlock(&fake.mutex);

	v4l2_input.destroy(source);

	assert_true(stopped);
	assert_int_equal(outputs, 1);

	free(good.data);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(corrupt_mjpeg_frame_is_dropped, setup_mjpeg, teardown_decoder),
		cmocka_unit_test_setup_teardown(burst_of_corrupt_frames_is_dropped, setup_mjpeg, teardown_decoder),
		cmocka_unit_test_setup_teardown(empty_buffer_is_skipped, setup_mjpeg, teardown_decoder),
		cmocka_unit_test_setup_teardown(error_flagged_buffer_is_skipped, setup_mjpeg, teardown_decoder),
		cmocka_unit_test_setup_teardown(h264_buffer_without_new_picture_is_skipped, setup_h264,
						teardown_decoder),
		cmocka_unit_test_setup_teardown(h264_parameter_sets_alone_are_dropped, setup_h264, teardown_decoder),
		cmocka_unit_test_setup_teardown(h264_corrupt_frame_is_dropped, setup_h264, teardown_decoder),
		cmocka_unit_test(raw_buffer_is_output_in_place),
		cmocka_unit_test(capture_continues_after_corrupt_frame),
		cmocka_unit_test(capture_stops_when_device_is_gone),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
