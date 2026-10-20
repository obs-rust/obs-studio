/*
 * A process started through os_process_pipe_create(2) gets its three
 * standard streams and nothing else of ours (obsproject/obs-studio#4596).
 *
 * The recording's ffmpeg-mux is such a child. When it inherited every
 * inheritable handle of OBS, it also got the UDP socket of an SRT Media
 * Source in listener mode (libsrt leaves its sockets inheritable on Windows).
 * After the sender went away the source closed its socket and opened a new
 * one on the same port, but the old one stayed open in ffmpeg-mux: on Windows
 * the sender's packets kept going to it, so the source did not reconnect
 * until the recording stopped.
 *
 * The test does the same with a socket of its own: it binds a UDP port,
 * starts a child that stays alive until its stdin closes, closes the socket
 * and binds the port again. The port is free again only if the child did not
 * inherit the socket.
 */

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
typedef SOCKET sock_t;
#define close_sock closesocket
#define LONG_CHILD "cmd /c more > NUL"
#else
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>
typedef int sock_t;
#define INVALID_SOCKET (-1)
#define close_sock close
#define LONG_CHILD "cat > /dev/null"
#endif

#include <util/pipe.h>

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
#ifdef _WIN32
	WSADATA wsa;
	return WSAStartup(MAKEWORD(2, 2), &wsa);
#else
	return 0;
#endif
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
#ifdef _WIN32
	WSACleanup();
#endif
	return 0;
}

/* A UDP socket bound to 127.0.0.1:port (0: any free port), created the way
 * libsrt creates its socket on Windows: inheritable, no close-on-exec. */
static sock_t bind_udp(uint16_t port)
{
	struct sockaddr_in addr;
	memset(&addr, 0, sizeof(addr));
	addr.sin_family = AF_INET;
	addr.sin_port = htons(port);
	addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);

	sock_t s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
	assert_true(s != INVALID_SOCKET);
	if (bind(s, (struct sockaddr *)&addr, sizeof(addr)) != 0) {
		close_sock(s);
		return INVALID_SOCKET;
	}
	return s;
}

static uint16_t local_port(sock_t s)
{
	struct sockaddr_in addr;
	socklen_t len = sizeof(addr);
	assert_int_equal(getsockname(s, (struct sockaddr *)&addr, &len), 0);
	return ntohs(addr.sin_port);
}

static void check_port_released(os_process_pipe_t *(*spawn)(void))
{
	sock_t s = bind_udp(0);
	assert_true(s != INVALID_SOCKET);
	uint16_t port = local_port(s);

	os_process_pipe_t *pp = spawn();
	assert_non_null(pp);

	/* The child is alive (it waits for stdin) while the port is reused. */
	close_sock(s);
	sock_t again = bind_udp(port);
	bool released = again != INVALID_SOCKET;
	if (released)
		close_sock(again);

	assert_int_equal(os_process_pipe_destroy(pp), 0);
	assert_true(released);
}

static os_process_pipe_t *spawn_create(void)
{
	return os_process_pipe_create(LONG_CHILD, "w");
}

static os_process_pipe_t *spawn_create_read(void)
{
	/* Read mode: the child lives until the parent stops reading. */
#ifdef _WIN32
	return os_process_pipe_create("cmd /c ping -n 3 127.0.0.1 > NUL", "r");
#else
	return os_process_pipe_create("sleep 1", "r");
#endif
}

static os_process_pipe_t *spawn_create2(void)
{
#ifdef _WIN32
	os_process_args_t *args = os_process_args_create("cmd");
	os_process_args_add_arg(args, "/c");
	os_process_args_add_arg(args, "more > NUL");
#else
	os_process_args_t *args = os_process_args_create("/bin/sh");
	os_process_args_add_arg(args, "-c");
	os_process_args_add_arg(args, LONG_CHILD);
#endif
	os_process_pipe_t *pp = os_process_pipe_create2(args, "w");
	os_process_args_destroy(args);
	return pp;
}

static void test_create_write_mode_does_not_inherit_socket(void **state)
{
	UNUSED_PARAMETER(state);
	check_port_released(spawn_create);
}

static void test_create_read_mode_does_not_inherit_socket(void **state)
{
	UNUSED_PARAMETER(state);
	check_port_released(spawn_create_read);
}

static void test_create2_does_not_inherit_socket(void **state)
{
	UNUSED_PARAMETER(state);
	check_port_released(spawn_create2);
}

/* The standard streams still reach the child: what it writes to stdout comes
 * back through the pipe, and what it writes to stderr through the error pipe. */
static void test_stdio_still_connected(void **state)
{
	UNUSED_PARAMETER(state);
#ifdef _WIN32
	os_process_pipe_t *pp = os_process_pipe_create("cmd /c (echo out) & (1>&2 echo err)", "r");
#else
	os_process_pipe_t *pp = os_process_pipe_create("printf out; printf err 1>&2", "r");
#endif
	assert_non_null(pp);
	char out[16] = {0};
	char err[16] = {0};
	size_t n = os_process_pipe_read(pp, (uint8_t *)out, sizeof(out) - 1);
	size_t e = os_process_pipe_read_err(pp, (uint8_t *)err, sizeof(err) - 1);
	assert_int_equal(os_process_pipe_destroy(pp), 0);
	assert_true(n >= 3);
	assert_true(e >= 3);
	assert_memory_equal(out, "out", 3);
	assert_memory_equal(err, "err", 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_create_write_mode_does_not_inherit_socket),
		cmocka_unit_test(test_create_read_mode_does_not_inherit_socket),
		cmocka_unit_test(test_create2_does_not_inherit_socket),
		cmocka_unit_test(test_stdio_still_connected),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
