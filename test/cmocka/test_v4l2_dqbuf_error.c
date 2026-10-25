#include <errno.h>
#include <inttypes.h>
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

#include <obs-module.h>
#include <util/platform.h>

#include "v4l2-helpers.h"

/* A V4L2 source freezes on its last frame for good when the camera fails
 * without being unplugged (obs-rust/obs-studio#2264, upstream
 * obsproject/obs-studio#9434 "Reload frozen webcams").
 *
 * The capture thread left its loop on the first VIDIOC_DQBUF error other
 * than EAGAIN, and nothing restarted it: only udev's remove/add events, a
 * settings change or a replug reopen the device. A camera whose firmware
 * hangs, a driver that fails with EIO or ENODEV after resume, or a stream
 * another process pulled stays registered, so no udev event comes. The
 * source also kept the device open, so the driver could not recover it.
 *
 * These tests run the real capture thread from v4l2-input.c against a fake
 * libv4l2 (below), so no camera, kernel module or display is needed.
 * test_v4l2_vivid_stall covers the same with the kernel's vivid driver. */

#define WIDTH 32
#define HEIGHT 16

/* ------------------------------------------------------------------------ */
/* Fake libv4l2: one YUYV capture device whose frames come from a script.
 * The device fd is an eventfd, readable while frames are left, so select()
 * in the capture thread behaves as with a camera. Every open gets a new fd,
 * as with a real device. */

#define FAKE_BUFFERS 4
#define FAKE_BUFFER_SIZE (WIDTH * HEIGHT * 2)
#define MAX_OUTPUTS 64
#define MAX_OPENS 64

struct fake_frame {
	uint8_t value;   /* every byte of the picture */
	int dqbuf_errno; /* nonzero: VIDIOC_DQBUF fails like this */
};

static struct {
	pthread_mutex_t mutex;
	pthread_cond_t cond;
	int fd;
	uint8_t *memory[FAKE_BUFFERS];
	bool queued[FAKE_BUFFERS];
	int mapped;
	const struct fake_frame *script;
	size_t script_size;
	size_t next;
	uint32_t sequence;
	size_t streamoffs;

	/* the camera at the node, as VIDIOC_QUERYCAP reports it */
	const char *card;

	/* the next open_failures opens fail with open_errno */
	size_t open_failures;
	int open_errno;
	size_t opens;
	uint64_t open_ns[MAX_OPENS];

	size_t outputs;
	uint8_t output_value[MAX_OUTPUTS];
	uint64_t output_ns[MAX_OUTPUTS];
} fake = {.mutex = PTHREAD_MUTEX_INITIALIZER, .cond = PTHREAD_COND_INITIALIZER, .fd = -1};

static void fake_reset(const struct fake_frame *script, size_t size)
{
	pthread_mutex_lock(&fake.mutex);
	for (size_t i = 0; i < FAKE_BUFFERS; i++) {
		if (!fake.memory[i])
			fake.memory[i] = malloc(FAKE_BUFFER_SIZE);
		fake.queued[i] = false;
	}
	fake.mapped = 0;
	fake.script = script;
	fake.script_size = size;
	fake.next = 0;
	fake.sequence = 0;
	fake.streamoffs = 0;
	fake.card = "Camera A";
	fake.open_failures = 0;
	fake.open_errno = 0;
	fake.opens = 0;
	fake.outputs = 0;
	pthread_mutex_unlock(&fake.mutex);
}

/* The next n opens fail with err */
static void fake_fail_opens(size_t n, int err)
{
	pthread_mutex_lock(&fake.mutex);
	fake.open_failures = n;
	fake.open_errno = err;
	pthread_mutex_unlock(&fake.mutex);
}

/* Another camera takes the node (NULL: the source's camera again) */
static void fake_set_camera(const char *card)
{
	pthread_mutex_lock(&fake.mutex);
	fake.card = card ? card : "Camera A";
	pthread_mutex_unlock(&fake.mutex);
}

/* Nothing more to read until the next open */
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
	int result;

	pthread_mutex_lock(&fake.mutex);
	if (fake.opens < MAX_OPENS)
		fake.open_ns[fake.opens] = os_gettime_ns();
	fake.opens++;
	if (fake.open_failures) {
		fake.open_failures--;
		errno = fake.open_errno;
		result = -1;
	} else {
		fake.fd = eventfd(1, EFD_NONBLOCK | EFD_CLOEXEC);
		result = fake.fd;
	}
	pthread_cond_broadcast(&fake.cond);
	pthread_mutex_unlock(&fake.mutex);
	return result;
}

int v4l2_close(int fd)
{
	pthread_mutex_lock(&fake.mutex);
	if (fd == fake.fd)
		fake.fd = -1;
	pthread_cond_broadcast(&fake.cond);
	pthread_mutex_unlock(&fake.mutex);
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
	if (index >= FAKE_BUFFERS)
		return MAP_FAILED;
	pthread_mutex_lock(&fake.mutex);
	fake.mapped++;
	pthread_mutex_unlock(&fake.mutex);
	return fake.memory[index];
}

int v4l2_munmap(void *start, size_t length)
{
	(void)start;
	(void)length;
	pthread_mutex_lock(&fake.mutex);
	fake.mapped--;
	pthread_mutex_unlock(&fake.mutex);
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
		memset(fake.memory[i], frame->value, FAKE_BUFFER_SIZE);
		buf->index = i;
		buf->bytesused = FAKE_BUFFER_SIZE;
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

	int result = 0;
	pthread_mutex_lock(&fake.mutex);

	if (fd != fake.fd) {
		pthread_mutex_unlock(&fake.mutex);
		errno = EBADF;
		return -1;
	}

	switch ((unsigned long)request) {
	case VIDIOC_QUERYCAP: {
		struct v4l2_capability *cap = arg;
		memset(cap, 0, sizeof(*cap));
		snprintf((char *)cap->card, sizeof(cap->card), "%s", fake.card);
		snprintf((char *)cap->bus_info, sizeof(cap->bus_info), "usb-fake-1");
		cap->capabilities = V4L2_CAP_VIDEO_CAPTURE | V4L2_CAP_STREAMING;
		break;
	}
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
		fmt->fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
		fmt->fmt.pix.bytesperline = WIDTH * 2;
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
		fake.streamoffs++;
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
	if (frame && fake.outputs < MAX_OUTPUTS) {
		fake.output_value[fake.outputs] = frame->data[0] ? frame->data[0][0] : 0;
		fake.output_ns[fake.outputs] = os_gettime_ns();
	}
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
	obs_data_set_int(settings, "pixelformat", V4L2_PIX_FMT_YUYV);
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

/* What the fake saw, copied under its lock */
struct seen {
	size_t outputs;
	size_t opens;
	size_t streamoffs;
	bool open;
	int mapped;
};

static struct seen seen_locked(void)
{
	struct seen s = {fake.outputs, fake.opens, fake.streamoffs, fake.fd != -1, fake.mapped};
	return s;
}

static struct seen seen(void)
{
	pthread_mutex_lock(&fake.mutex);
	struct seen s = seen_locked();
	pthread_mutex_unlock(&fake.mutex);
	return s;
}

/* Waits up to timeout_ms until done() holds; returns whether it does */
static bool wait_until(bool (*done)(const struct seen *s, size_t n), size_t n, int timeout_ms)
{
	struct timespec deadline;
	clock_gettime(CLOCK_REALTIME, &deadline);
	deadline.tv_sec += timeout_ms / 1000;
	deadline.tv_nsec += (long)(timeout_ms % 1000) * 1000000L;
	if (deadline.tv_nsec >= 1000000000L) {
		deadline.tv_sec++;
		deadline.tv_nsec -= 1000000000L;
	}

	pthread_mutex_lock(&fake.mutex);
	struct seen s = seen_locked();
	while (!done(&s, n)) {
		if (pthread_cond_timedwait(&fake.cond, &fake.mutex, &deadline) == ETIMEDOUT) {
			s = seen_locked();
			break;
		}
		s = seen_locked();
	}
	pthread_mutex_unlock(&fake.mutex);
	return done(&s, n);
}

static bool outputs_reached(const struct seen *s, size_t n)
{
	return s->outputs >= n;
}

static bool opens_reached(const struct seen *s, size_t n)
{
	return s->opens >= n;
}

static bool streamoffs_reached(const struct seen *s, size_t n)
{
	return s->streamoffs >= n;
}

static bool device_released(const struct seen *s, size_t n)
{
	(void)n;
	return !s->open && s->mapped == 0;
}

static void expect_values(size_t count, const uint8_t *want)
{
	pthread_mutex_lock(&fake.mutex);
	uint8_t got[MAX_OUTPUTS];
	memcpy(got, fake.output_value, sizeof(got));
	pthread_mutex_unlock(&fake.mutex);

	for (size_t i = 0; i < count; i++) {
		if (got[i] != want[i]) {
			fprintf(stderr, "frame %zu shows %d, the camera sent %d\n", i, got[i], want[i]);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */

/* VIDIOC_DQBUF fails once with EIO (a driver error after resume, a firmware
 * hiccup), the device stays: the capture resumes after a short wait, on the
 * device opened again. Before the fix it showed frame 3 forever. */
static void capture_resumes_after_a_dqbuf_error(void **state)
{
	(void)state;
	static const struct fake_frame script[] = {{1, 0}, {2, 0}, {3, 0}, {0, EIO}, {4, 0}, {5, 0}, {6, 0}};
	static const uint8_t want[] = {1, 2, 3, 4, 5, 6};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	bool resumed = wait_until(outputs_reached, 6, 10000);
	struct seen s = seen();
	uint64_t gap_ns = 0;
	if (resumed) {
		pthread_mutex_lock(&fake.mutex);
		gap_ns = fake.output_ns[3] - fake.output_ns[2];
		pthread_mutex_unlock(&fake.mutex);
	}
	v4l2_input.destroy(source);

	if (!resumed) {
		fprintf(stderr,
			"after the VIDIOC_DQBUF error the source output %zu of 6 frames in 10 s "
			"(device opened %zu times): frozen on its last frame\n",
			s.outputs, s.opens);
		fail();
	}
	expect_values(6, want);
	assert_int_equal(s.opens, 2);
	/* It waits before reopening: no busy loop on a failing device */
	if (gap_ns < 500000000ULL) {
		fprintf(stderr, "reopened %" PRIu64 " ms after the error, expected a wait\n", gap_ns / 1000000);
		fail();
	}
}

/* While it waits, the source lets go of the device: the stream is stopped,
 * the buffers unmapped and the device closed, so a driver that recovers
 * once nobody has it open (vivid does) or another program can take it.
 * Before the fix the source kept it open until it was destroyed. */
static void device_is_released_after_the_error(void **state)
{
	(void)state;
	static const struct fake_frame script[] = {{1, 0}, {0, ENODEV}};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	assert_true(wait_until(outputs_reached, 1, 5000));
	/* the device does not come back */
	fake_fail_opens(SIZE_MAX, ENODEV);
	bool stopped = wait_until(streamoffs_reached, 1, 5000);
	bool released = wait_until(device_released, 0, 2000);
	struct seen s = seen();
	v4l2_input.destroy(source);

	assert_true(stopped);
	if (!released) {
		fprintf(stderr, "2 s after the error the source still has the device %s (%d buffers mapped)\n",
			s.open ? "open" : "closed", s.mapped);
		fail();
	}
}

/* The device cannot be opened at first (ENOENT, ENODEV while the driver
 * recovers): the source keeps trying, and captures again once it opens. */
static void capture_resumes_once_the_device_opens_again(void **state)
{
	(void)state;
	static const struct fake_frame script[] = {{1, 0}, {0, ENODEV}, {2, 0}, {3, 0}};
	static const uint8_t want[] = {1, 2, 3};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	assert_true(wait_until(outputs_reached, 1, 5000));
	/* the first reopen fails, the second works */
	fake_fail_opens(1, ENOENT);

	bool resumed = wait_until(outputs_reached, 3, 15000);
	struct seen s = seen();
	v4l2_input.destroy(source);

	if (!resumed) {
		fprintf(stderr, "the device opened again, but the source output %zu of 3 frames (opened %zu times)\n",
			s.outputs, s.opens);
		fail();
	}
	expect_values(3, want);
	assert_int_equal(s.opens, 3);
}

/* Destroying the source while it waits to reopen the device returns at
 * once, and the device is not opened again. */
static void destroy_while_waiting_returns_promptly(void **state)
{
	(void)state;
	static const struct fake_frame script[] = {{1, 0}, {0, ENODEV}};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	assert_true(wait_until(outputs_reached, 1, 5000));
	fake_fail_opens(SIZE_MAX, ENODEV);
	/* the first reopen failed: it now waits longer before the next */
	bool retried = wait_until(opens_reached, 2, 10000);
	struct seen before = seen();

	uint64_t start = os_gettime_ns();
	v4l2_input.destroy(source);
	uint64_t took_ms = (os_gettime_ns() - start) / 1000000;
	size_t opens_after = seen().opens;

	if (!retried) {
		fprintf(stderr, "the device was opened %zu times in 10 s after the error: never reopened\n",
			before.opens);
		fail();
	}
	if (took_ms > 500) {
		fprintf(stderr, "destroying the source while it waits took %" PRIu64 " ms\n", took_ms);
		fail();
	}
	assert_int_equal(opens_after, before.opens);
}

/* While the camera is away another camera takes its node (the numbers
 * follow the order cameras appear, obs-rust/obs-studio#2262): the reopen
 * does not capture it, and the camera is captured once it is back. */
static void another_camera_at_the_node_is_not_captured(void **state)
{
	(void)state;
	static const struct fake_frame script[] = {{1, 0}, {0, ENODEV}, {2, 0}, {3, 0}};
	static const uint8_t want[] = {1, 2, 3};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	assert_true(wait_until(outputs_reached, 1, 5000));
	fake_set_camera("Camera B");

	/* the first reopen (after 1 s) finds Camera B */
	bool tried = wait_until(opens_reached, 2, 5000);
	os_sleep_ms(300);
	struct seen with_other = seen();
	fake_set_camera(NULL);
	bool resumed = wait_until(outputs_reached, 3, 10000);
	struct seen s = seen();
	v4l2_input.destroy(source);

	assert_true(tried);
	if (with_other.outputs != 1 || with_other.open) {
		fprintf(stderr,
			"with another camera at the node the source output %zu frames (1 before) and has it %s\n",
			with_other.outputs, with_other.open ? "open" : "closed");
		fail();
	}
	if (!resumed) {
		fprintf(stderr, "the camera came back, but the source output %zu of 3 frames\n", s.outputs);
		fail();
	}
	expect_values(3, want);
}

/* The wait before each reopen: 1 s, doubling to at most 10 s */
static void reopen_delay_doubles_to_a_cap(void **state)
{
	(void)state;
	assert_int_equal(v4l2_reopen_delay_ms(0), 1000);
	assert_int_equal(v4l2_reopen_delay_ms(1), 2000);
	assert_int_equal(v4l2_reopen_delay_ms(2), 4000);
	assert_int_equal(v4l2_reopen_delay_ms(3), 8000);
	assert_int_equal(v4l2_reopen_delay_ms(4), 10000);
	assert_int_equal(v4l2_reopen_delay_ms(31), 10000);
	assert_int_equal(v4l2_reopen_delay_ms(32), 10000);
	assert_int_equal(v4l2_reopen_delay_ms(UINT32_MAX), 10000);
}

/* Control: stopping the source ends the capture without reopening */
static void stopping_the_source_does_not_reopen(void **state)
{
	(void)state;
	static const struct fake_frame script[] = {{1, 0}, {2, 0}};
	fake_reset(script, sizeof(script) / sizeof(script[0]));

	void *source = start_source();
	assert_true(wait_until(outputs_reached, 2, 5000));
	v4l2_input.destroy(source);
	os_sleep_ms(200);

	struct seen s = seen();
	assert_int_equal(s.opens, 1);
	assert_false(s.open);
	assert_int_equal(s.mapped, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(capture_resumes_after_a_dqbuf_error),
		cmocka_unit_test(device_is_released_after_the_error),
		cmocka_unit_test(capture_resumes_once_the_device_opens_again),
		cmocka_unit_test(destroy_while_waiting_returns_promptly),
		cmocka_unit_test(stopping_the_source_does_not_reopen),
		cmocka_unit_test(another_camera_at_the_node_is_not_captured),
		cmocka_unit_test(reopen_delay_doubles_to_a_cap),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
