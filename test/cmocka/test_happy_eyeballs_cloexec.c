/* shared/happy-eyeballs: the connector created its sockets without
 * close-on-exec, so a process OBS started while streaming got a copy of the
 * RTMP connection: a recording started during a stream gave its
 * obs-ffmpeg-mux, started with os_process_pipe_create(), the stream's TCP
 * socket, which then stayed open in the muxer until the recording ended
 * (obs-rust/obs-studio#2305).
 *
 * - connected_socket_is_close_on_exec connects to a loopback listener
 *   through the real connector and checks FD_CLOEXEC on the socket it
 *   returns. It runs on every POSIX host (macOS has no SOCK_CLOEXEC).
 * - connected_socket_is_not_inherited starts a child with
 *   os_process_pipe_create() while the connection is open, as a recording
 *   starts its muxer during a stream, and reads /proc/<pid>/fd. Linux. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <util/c99defs.h>
#include <util/pipe.h>
#include <happy-eyeballs.h>

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <unistd.h>
#include <dirent.h>
#include <netinet/in.h>
#include <arpa/inet.h>

#ifndef INVALID_SOCKET
#define INVALID_SOCKET -1
#endif

struct connection {
	SOCKET server;
	SOCKET accepted;
	SOCKET fd;
	struct happy_eyeballs_ctx *ctx;
};

/* A connection from the connector to a loopback listener. */
static void connect_loopback(struct connection *c)
{
	c->server = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
	assert_true(c->server != INVALID_SOCKET);

	struct sockaddr_in in;
	memset(&in, 0, sizeof(in));
	in.sin_family = AF_INET;
	in.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
	socklen_t len = sizeof(in);
	assert_int_equal(bind(c->server, (struct sockaddr *)&in, len), 0);
	assert_int_equal(listen(c->server, 4), 0);
	assert_int_equal(getsockname(c->server, (struct sockaddr *)&in, &len), 0);

	c->ctx = NULL;
	assert_int_equal(happy_eyeballs_create(&c->ctx), 0);
	assert_int_equal(happy_eyeballs_connect(c->ctx, "127.0.0.1", ntohs(in.sin_port)), 0);
	assert_int_equal(happy_eyeballs_timedwait_default(c->ctx), 0);
	assert_int_equal(happy_eyeballs_get_error_code(c->ctx), 0);

	c->fd = happy_eyeballs_get_socket_fd(c->ctx);
	assert_true(c->fd != INVALID_SOCKET);
	c->accepted = accept(c->server, NULL, NULL);
	assert_true(c->accepted != INVALID_SOCKET);
}

static void disconnect(struct connection *c)
{
	close(c->accepted);
	close(c->fd);
	close(c->server);
	assert_int_equal(happy_eyeballs_destroy(c->ctx), 0);
}

static void connected_socket_is_close_on_exec(void **state)
{
	UNUSED_PARAMETER(state);
	struct connection c;
	connect_loopback(&c);

	int flags = fcntl(c.fd, F_GETFD);
	disconnect(&c);

	assert_true(flags >= 0);
	if (!(flags & FD_CLOEXEC)) {
		fprintf(stderr,
			"the connector's socket lacks FD_CLOEXEC; every process OBS starts meanwhile gets a copy\n");
		fail();
	}
}

#ifdef __linux__
static os_process_pipe_t *start_child(pid_t *pid)
{
	os_process_pipe_t *pp = os_process_pipe_create("echo $$ >&2; exec cat >/dev/null", "w");
	if (!pp) {
		fprintf(stderr, "cannot start the child\n");
		fail();
	}

	char line[32] = {0};
	size_t len = 0;
	while (len < sizeof(line) - 1) {
		uint8_t ch;
		if (os_process_pipe_read_err(pp, &ch, 1) != 1 || ch == '\n')
			break;
		line[len++] = (char)ch;
	}
	*pid = (pid_t)atoi(line);
	if (*pid <= 0) {
		fprintf(stderr, "the child did not report its pid ('%s')\n", line);
		fail();
	}
	return pp;
}

/* How many of pid's descriptors refer to target ("socket:[inode]"). */
static int fds_on(pid_t pid, const char *target)
{
	char fd_dir[64];
	snprintf(fd_dir, sizeof(fd_dir), "/proc/%d/fd", (int)pid);

	DIR *dir = opendir(fd_dir);
	if (!dir) {
		fprintf(stderr, "cannot read %s: %s\n", fd_dir, strerror(errno));
		fail();
	}

	int count = 0;
	struct dirent *entry;
	while ((entry = readdir(dir)) != NULL) {
		if (entry->d_name[0] == '.')
			continue;

		char link[PATH_MAX];
		char found[PATH_MAX];
		snprintf(link, sizeof(link), "%s/%s", fd_dir, entry->d_name);
		ssize_t n = readlink(link, found, sizeof(found) - 1);
		if (n < 0)
			continue;
		found[n] = 0;
		if (strcmp(found, target) == 0)
			count++;
	}
	closedir(dir);
	return count;
}

/* A recording starts its muxer while a stream is connected */
static void connected_socket_is_not_inherited(void **state)
{
	UNUSED_PARAMETER(state);
	struct connection c;
	connect_loopback(&c);

	char link[64];
	char socket_name[PATH_MAX];
	snprintf(link, sizeof(link), "/proc/self/fd/%d", (int)c.fd);
	ssize_t n = readlink(link, socket_name, sizeof(socket_name) - 1);
	assert_true(n > 0);
	socket_name[n] = 0;

	pid_t pid;
	os_process_pipe_t *child = start_child(&pid);
	int held = fds_on(pid, socket_name);
	os_process_pipe_destroy(child);
	disconnect(&c);

	if (held) {
		fprintf(stderr, "a process started during the connection holds %d copies of its socket (%s)\n", held,
			socket_name);
		fail();
	}
}
#else
static void connected_socket_is_not_inherited(void **state)
{
	UNUSED_PARAMETER(state);
	fprintf(stderr, "skipped: needs /proc/<pid>/fd\n");
	skip();
}
#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(connected_socket_is_close_on_exec),
		cmocka_unit_test(connected_socket_is_not_inherited),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
