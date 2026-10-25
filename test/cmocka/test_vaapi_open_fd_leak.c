/* obs-ffmpeg: vaapi_open_device() left the device it opened open when
 * vaGetDisplayDRM() refused it, and returned NULL without closing it. The
 * callers only close what a display was made for, so every failed open
 * (the encoder probe at startup, the encoder's properties) leaked one
 * descriptor of the render node (obs-rust/obs-studio#2305).
 *
 * The real vaapi-utils.c is compiled in, and vaGetDisplayDRM() is wrapped
 * (--wrap) to refuse the display, as libva does for a node without a
 * driver. The "device" is a temporary file, so no GPU is needed. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "plugins/obs-ffmpeg/vaapi-utils.c.h"

#include <limits.h>

static char device_path[PATH_MAX];
static int displays;

VADisplay __wrap_vaGetDisplayDRM(int fd);

VADisplay __wrap_vaGetDisplayDRM(int fd)
{
	(void)fd;
	displays++;
	return NULL;
}

static int setup(void **state)
{
	(void)state;
	const char *dir = getenv("TMPDIR");
	snprintf(device_path, sizeof(device_path), "%s/vaapi-open-fd-leak-XXXXXX", dir && *dir ? dir : "/tmp");
	int fd = mkstemp(device_path);
	if (fd < 0)
		return -1;
	close(fd);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	unlink(device_path);
	return 0;
}

/* Closes, and counts, the descriptors of the device this process holds. */
static int close_left_open(void)
{
	int left = 0;
	for (int fd = 3; fd < 1024; fd++) {
		char link[64];
		char target[PATH_MAX];
		snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
		ssize_t n = readlink(link, target, sizeof(target) - 1);
		if (n < 0)
			continue;
		target[n] = 0;
		if (strcmp(target, device_path) == 0) {
			close(fd);
			left++;
		}
	}
	return left;
}

static void refused_display_leaves_nothing_open(void **state)
{
	(void)state;
	displays = 0;
	int fd = -1;

	VADisplay dpy = vaapi_open_device(&fd, device_path, "test");
	int left = close_left_open();

	assert_null(dpy);
	assert_int_equal(displays, 1);
	if (left) {
		fprintf(stderr,
			"vaapi_open_device() failed without a display but left %d descriptors of the device open\n",
			left);
		fail();
	}
	assert_int_equal(fd, -1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(refused_display_leaves_nothing_open),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
