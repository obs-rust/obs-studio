#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <cmocka.h>

#include <pipewire/pipewire.h>

#include <util/base.h>
#include <util/dstr.h>

#include "pipewire.h"

/* A PipeWire client without its configuration crashed OBS at launch: the
 * obs-rust AppImage pointed PIPEWIRE_CONFIG_DIR at a directory that was not
 * packaged, so pw_context_new() failed (no client.conf) and returned NULL,
 * and obs_pipewire_connect_fd() passed that NULL context on to
 * pw_context_connect_fd(), which dereferenced it. Any scene with a PipeWire
 * screen capture crashed while it loaded.
 *
 * obs_pipewire_connect_fd() has to fail cleanly instead: return NULL, log
 * why, and close the portal's PipeWire fd it was given, so that a capture
 * stays empty instead of taking OBS down.
 *
 * The failure cases need no PipeWire daemon: the configuration is read
 * before connecting, and a socket with no daemon behind it stands in for the
 * portal's fd. They always run, CI included. Only the control case needs a
 * daemon: test_pipewire_context_fail.sh starts a private one (own runtime
 * dir and minimal configuration, no D-Bus, no session manager, no devices)
 * when pipewire is installed, and the case skips without it. No display is
 * needed. */

static struct dstr log_text;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	char buf[1024];

	UNUSED_PARAMETER(lvl);
	UNUSED_PARAMETER(p);

	vsnprintf(buf, sizeof(buf), msg, args);
	dstr_cat(&log_text, buf);
	dstr_cat(&log_text, "\n");
}

/* A connected socket to the private daemon: what the portal's
 * OpenPipeWireRemote hands OBS. */
static int connect_private_daemon(void)
{
	const char *runtime_dir = getenv("PIPEWIRE_RUNTIME_DIR");
	struct sockaddr_un addr = {.sun_family = AF_UNIX};
	int fd;

	if (!runtime_dir)
		return -1;

	snprintf(addr.sun_path, sizeof(addr.sun_path), "%s/pipewire-0", runtime_dir);

	fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
	if (fd < 0)
		return -1;

	if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
		close(fd);
		return -1;
	}

	return fd;
}

static bool fd_is_open(int fd)
{
	return fcntl(fd, F_GETFD) != -1 || errno != EBADF;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);

	dstr_free(&log_text);
	unsetenv("PIPEWIRE_CONFIG_DIR");
	return 0;
}

static void connects_with_its_configuration(void **state)
{
	obs_pipewire *obs_pw;
	int fd;

	UNUSED_PARAMETER(state);

	fd = connect_private_daemon();
	if (fd < 0)
		skip();

	obs_pw = obs_pipewire_connect_fd(fd, NULL, NULL);
	assert_non_null(obs_pw);

	obs_pipewire_destroy(obs_pw);
}

static void missing_configuration_fails_cleanly(void **state)
{
	obs_pipewire *obs_pw;
	int fds[2];
	int fd;

	UNUSED_PARAMETER(state);

	/* The configuration is read before the fd is used. */
	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, fds), 0);
	fd = fds[0];

	/* Like the AppImage: the configuration directory does not exist. */
	setenv("PIPEWIRE_CONFIG_DIR", "/nonexistent/obs-test-pipewire-config", 1);

	obs_pw = obs_pipewire_connect_fd(fd, NULL, NULL);
	assert_null(obs_pw);
	assert_false(fd_is_open(fd));
	assert_non_null(strstr(log_text.array ? log_text.array : "", "Error creating PipeWire context"));
	close(fds[1]);
}

static void unusable_fd_fails_cleanly(void **state)
{
	obs_pipewire *obs_pw;
	int fds[2];

	UNUSED_PARAMETER(state);

	/* A socket with no PipeWire daemon behind it. */
	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0, fds), 0);
	close(fds[1]);

	obs_pw = obs_pipewire_connect_fd(fds[0], NULL, NULL);
	if (obs_pw) {
		/* PipeWire may accept the fd and only notice the dead peer later;
		 * that is reported through the core error path, not here. */
		obs_pipewire_destroy(obs_pw);
		return;
	}

	assert_false(fd_is_open(fds[0]));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(connects_with_its_configuration, setup),
		cmocka_unit_test_setup(missing_configuration_fails_cleanly, setup),
		cmocka_unit_test_setup(unusable_fd_fails_cleanly, setup),
	};

	pw_init(NULL, NULL);
	base_set_log_handler(log_handler, NULL);

	return cmocka_run_group_tests(tests, NULL, NULL);
}
