/* obs-qsv11: on Linux the QSV encoder opened its DRM render node without
 * O_CLOEXEC (Initialize()) and kept it open while encoding, so a process
 * OBS started meanwhile got a copy: a recording's obs-ffmpeg-mux, started
 * with os_process_pipe_create(), held the encoder's GPU device for as long
 * as it ran. The adapter probe (check_adapter()) opened nodes the same way.
 * Initialize() also left the node open when vaGetDisplayDRM() failed
 * (obs-rust/obs-studio#2305).
 *
 * The real common_utils_linux.cpp is compiled in, and vaGetDisplayDRM() is
 * wrapped (--wrap): the wrapper records the FD_CLOEXEC flag of the
 * descriptor it is given, the one just opened, counts the copies a child
 * started at that moment holds, and refuses the display, as libva does for
 * a node without a driver. The "device" is a temporary file, so no GPU is
 * needed. */

/* The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
 * with libstdc++. */
#include "plugins/obs-qsv11/common_utils_linux.cpp.hpp"

#include <errno.h>
#include <limits.h>
#include <util/pipe.h>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

static char device_path[PATH_MAX];
static int displays;
static int inheritable_displays;
static int inherited_copies;

/* How many of pid's descriptors refer to path. */
static int fds_on(const char *pid, const char *path)
{
	char fd_dir[64];
	snprintf(fd_dir, sizeof(fd_dir), "/proc/%s/fd", pid);

	DIR *dir = opendir(fd_dir);
	if (!dir) {
		fprintf(stderr, "cannot read %s: %s\n", fd_dir, strerror(errno));
		fail();
	}

	int count = 0;
	struct dirent *entry;
	while ((entry = readdir(dir)) != NULL) {
		if (entry->d_name[0] == '.') {
			continue;
		}

		char link[PATH_MAX + 64];
		char target[PATH_MAX];
		snprintf(link, sizeof(link), "%s/%s", fd_dir, entry->d_name);
		ssize_t n = readlink(link, target, sizeof(target) - 1);
		if (n < 0) {
			continue;
		}
		target[n] = 0;
		if (strcmp(target, path) == 0) {
			count++;
		}
	}
	closedir(dir);
	return count;
}

/* Copies of the device a process started now gets. */
static int copies_in_child(void)
{
	os_process_pipe_t *pp = os_process_pipe_create("echo $$ >&2; exec cat >/dev/null", "w");
	if (!pp) {
		fprintf(stderr, "cannot start the child\n");
		fail();
	}

	char line[32] = {0};
	size_t len = 0;
	while (len < sizeof(line) - 1) {
		uint8_t c;
		if (os_process_pipe_read_err(pp, &c, 1) != 1 || c == '\n') {
			break;
		}
		line[len++] = (char)c;
	}
	if (atoi(line) <= 0) {
		os_process_pipe_destroy(pp);
		fprintf(stderr, "the child did not report its pid ('%s')\n", line);
		fail();
	}

	int copies = fds_on(line, device_path);
	os_process_pipe_destroy(pp);
	return copies;
}

extern "C" {
VADisplay __wrap_vaGetDisplayDRM(int fd);

VADisplay __wrap_vaGetDisplayDRM(int fd)
{
	displays++;
	if (!(fcntl(fd, F_GETFD) & FD_CLOEXEC)) {
		inheritable_displays++;
	}
	inherited_copies += copies_in_child();
	return NULL;
}
}

static int setup(void **state)
{
	(void)state;
	const char *dir = getenv("TMPDIR");
	snprintf(device_path, sizeof(device_path), "%s/qsv-open-cloexec-XXXXXX", dir && *dir ? dir : "/tmp");
	int fd = mkstemp(device_path);
	if (fd < 0) {
		return -1;
	}
	close(fd);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	unlink(device_path);
	return 0;
}

static void reset_records(void)
{
	displays = 0;
	inheritable_displays = 0;
	inherited_copies = 0;
}

static void check_close_on_exec(const char *what)
{
	if (displays < 1) {
		fprintf(stderr, "%s opened no device\n", what);
		fail();
	}
	if (inheritable_displays || inherited_copies) {
		fprintf(stderr,
			"%s: %d of %d device opens lack O_CLOEXEC; a process started meanwhile held %d copies\n", what,
			inheritable_displays, displays, inherited_copies);
		fail();
	}
}

/* Closes, and counts, the descriptors of the device this process holds:
 * what a failed open left behind. */
static int close_left_open(void)
{
	int left = 0;
	for (int fd = 3; fd < 1024; fd++) {
		char link[64];
		char target[PATH_MAX];
		snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
		ssize_t n = readlink(link, target, sizeof(target) - 1);
		if (n < 0) {
			continue;
		}
		target[n] = 0;
		if (strcmp(target, device_path) == 0) {
			close(fd);
			left++;
		}
	}
	return left;
}

static mfxStatus open_encoder(void)
{
	reset_records();
	default_h264_device = device_path;

	mfxVersion ver = {};
	mfxSession session = nullptr;
	void *data = nullptr;
	mfxStatus sts = Initialize(ver, &session, nullptr, nullptr, false, QSV_CODEC_AVC, &data);

	default_h264_device = nullptr;
	assert_null(data);
	return sts;
}

/* The encoder opens its device while a recording may start its muxer */
static void encoder_device_is_close_on_exec(void **state)
{
	(void)state;
	mfxStatus sts = open_encoder();
	close_left_open();

	assert_int_equal(sts, MFX_ERR_DEVICE_FAILED);
	check_close_on_exec("Initialize()");
}

/* A refused display leaves nothing open */
static void encoder_closes_device_without_display(void **state)
{
	(void)state;
	mfxStatus sts = open_encoder();
	int left = close_left_open();

	assert_int_equal(sts, MFX_ERR_DEVICE_FAILED);
	assert_int_equal(displays, 1);
	if (left) {
		fprintf(stderr, "Initialize() failed without a display but left %d descriptors of the device open\n",
			left);
		fail();
	}
}

/* The adapter probe at module load */
static void probe_device_is_close_on_exec(void **state)
{
	(void)state;
	reset_records();
	struct adapter_info probed[MAX_ADAPTERS] = {};

	assert_true(check_adapter(probed, device_path, 0));
	int left = close_left_open();

	assert_false(probed[0].is_intel);
	check_close_on_exec("check_adapter()");
	assert_int_equal(left, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(encoder_device_is_close_on_exec),
		cmocka_unit_test(encoder_closes_device_without_display),
		cmocka_unit_test(probe_device_is_close_on_exec),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
