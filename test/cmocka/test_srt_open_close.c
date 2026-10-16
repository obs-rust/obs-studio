#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <signal.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#if defined(__GNUC__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-function"
#endif
#include "obs-ffmpeg-srt.h"
#if defined(__GNUC__)
#pragma GCC diagnostic pop
#endif

/* An SRT stream whose connection fails (nobody listening, wrong passphrase,
 * refused streamid) or whose srt:// URL is invalid.
 *
 * libsrt counts srt_startup() calls process-wide: every srt_startup() needs
 * exactly one srt_cleanup(), and when the count drops to zero libsrt breaks
 * every SRT socket in the process. libsrt_open() releases its reference
 * itself when it fails, and connect_mpegts_url() then calls libsrt_close() on
 * the same context, which released it a second time. Each failed attempt
 * therefore dropped a reference that belongs to somebody else: an SRT Media
 * Source (FFmpeg's libsrt protocol uses the same library) lost its
 * connection.
 *
 * The test counts the output's srt_startup()/srt_cleanup() calls (linker
 * --wrap) and mirrors the output's open + close. It also holds its own libsrt
 * reference and an idle listening socket that stands in for the other SRT
 * user, and checks that the socket is still alive. That second check only
 * means something on a libsrt that counts references: 1.5.4 does not
 * (Haivision/srt#3098), and breaks every socket on any srt_cleanup(), so
 * there the test reports it and checks the count only.
 *
 * The stream's server runs in a child process forked before this process
 * touches libsrt. When libsrt tears its global state down (any srt_cleanup()
 * on 1.5.4, the extra one before the fix), its garbage collector can deadlock
 * with a receive thread that is handling a handshake for a listener in the
 * same process; with the server elsewhere, only the idle stand-in is here. */

#define GOOD_PASSPHRASE "correct-passphrase-1"
#define BAD_PASSPHRASE "wrong-passphrase-1"

int __real_srt_startup(void);
int __real_srt_cleanup(void);

static int startup_calls;
static int cleanup_calls;
static bool libsrt_counts_references;

/* Default visibility: libavformat in the same link also imports srt_startup()
 * and srt_cleanup(), and the linker refuses to resolve a DSO's reference to a
 * hidden __wrap_ symbol. Only this executable's calls are redirected. */
__attribute__((visibility("default"))) int __wrap_srt_startup(void)
{
	startup_calls++;
	return __real_srt_startup();
}

__attribute__((visibility("default"))) int __wrap_srt_cleanup(void)
{
	cleanup_calls++;
	return __real_srt_cleanup();
}

struct fixture {
	SRTSOCKET bystander;
	int port;
};

static pid_t server_pid = -1;
static int server_port;

static SRTSOCKET listen_on_loopback(const char *passphrase, int *port)
{
	struct sockaddr_in addr = {0};
	int len = sizeof(addr);
	int yes = 1;

	SRTSOCKET s = srt_create_socket();
	if (s == SRT_INVALID_SOCK)
		return SRT_INVALID_SOCK;
	if (passphrase)
		srt_setsockopt(s, 0, SRTO_PASSPHRASE, passphrase, (int)strlen(passphrase));
	srt_setsockopt(s, 0, SRTO_REUSEADDR, &yes, sizeof(yes));

	addr.sin_family = AF_INET;
	addr.sin_port = 0;
	inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);
	if (srt_bind(s, (struct sockaddr *)&addr, sizeof(addr)) < 0 || srt_listen(s, 4) < 0 ||
	    srt_getsockname(s, (struct sockaddr *)&addr, &len) < 0) {
		fprintf(stderr, "listener setup failed: %s\n", srt_getlasterror_str());
		srt_close(s);
		return SRT_INVALID_SOCK;
	}
	if (port)
		*port = ntohs(addr.sin_port);
	return s;
}

/* The child: an SRT listener with GOOD_PASSPHRASE that accepts every
 * connection and keeps it until the test kills the process. */
static void run_server(int port_fd)
{
	int port = 0;

	prctl(PR_SET_PDEATHSIG, SIGKILL);
	SRTSOCKET l = __real_srt_startup() < 0 ? SRT_INVALID_SOCK : listen_on_loopback(GOOD_PASSPHRASE, &port);
	if (write(port_fd, &port, sizeof(port)) != (ssize_t)sizeof(port) || l == SRT_INVALID_SOCK)
		_exit(1);
	close(port_fd);
	for (;;) {
		if (srt_accept(l, NULL, NULL) == SRT_INVALID_SOCK)
			sleep(1);
	}
}

static bool start_server(void)
{
	int fds[2];

	if (pipe(fds) < 0)
		return false;
	fflush(NULL);
	server_pid = fork();
	if (server_pid == 0) {
		close(fds[0]);
		run_server(fds[1]);
	}
	close(fds[1]);
	bool ok = server_pid > 0 && read(fds[0], &server_port, sizeof(server_port)) == (ssize_t)sizeof(server_port) &&
		  server_port > 0;
	close(fds[0]);
	if (!ok)
		fprintf(stderr, "could not start the SRT server process\n");
	return ok;
}

static void stop_server(void)
{
	if (server_pid > 0) {
		kill(server_pid, SIGKILL);
		waitpid(server_pid, NULL, 0);
		server_pid = -1;
	}
}

/* A second, balanced srt_startup()/srt_cleanup() pair must leave an open socket
 * alone. libsrt 1.5.4 breaks it anyway. */
static int probe_libsrt(void **state)
{
	(void)state;
	if (__real_srt_startup() < 0)
		return -1;
	SRTSOCKET s = listen_on_loopback(NULL, NULL);
	if (s == SRT_INVALID_SOCK)
		return -1;
	__real_srt_startup();
	__real_srt_cleanup();
	libsrt_counts_references = srt_getsockstate(s) == SRTS_LISTENING;
	srt_close(s);
	__real_srt_cleanup();
	if (!libsrt_counts_references)
		fprintf(stderr,
			"libsrt %x breaks every socket on any srt_cleanup() (Haivision/srt#3098): checking the "
			"output's srt_startup()/srt_cleanup() count only, not the other SRT user's socket\n",
			srt_getversion());
	return 0;
}

static int setup(void **state)
{
	static struct fixture f;

	if (__real_srt_startup() < 0)
		return -1;
	f.bystander = listen_on_loopback(NULL, NULL);
	if (f.bystander == SRT_INVALID_SOCK)
		return -1;
	f.port = server_port;
	startup_calls = 0;
	cleanup_calls = 0;
	*state = &f;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	srt_close(f->bystander);
	__real_srt_cleanup();
	return 0;
}

/* A UDP port nobody listens on: bind one, read its number, close it. */
static int closed_udp_port(void)
{
	struct sockaddr_in addr = {0};
	socklen_t len = sizeof(addr);
	int fd = socket(AF_INET, SOCK_DGRAM, 0);
	assert_true(fd >= 0);
	addr.sin_family = AF_INET;
	inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);
	assert_int_equal(bind(fd, (struct sockaddr *)&addr, sizeof(addr)), 0);
	assert_int_equal(getsockname(fd, (struct sockaddr *)&addr, &len), 0);
	close(fd);
	return ntohs(addr.sin_port);
}

/* Mirrors connect_mpegts_url() in obs-ffmpeg-mpegts.c: allocate the contexts,
 * hand over the passphrase, open, and close on failure (a stream that opened
 * is closed by close_mpegts_url() when it stops). */
static int open_like_mpegts_output(const char *url, const char *passphrase)
{
	URLContext *uc = av_mallocz(sizeof(URLContext) + strlen(url) + 1);
	assert_non_null(uc);
	uc->url = (char *)(uintptr_t)url;
	uc->max_packet_size = SRT_LIVE_DEFAULT_PAYLOAD_SIZE;
	uc->priv_data = av_mallocz(sizeof(SRTContext));
	assert_non_null(uc->priv_data);
	if (passphrase)
		((SRTContext *)uc->priv_data)->passphrase = av_strdup(passphrase);

	int err = libsrt_open(uc, uc->url);
	libsrt_close(uc);

	av_freep(&uc->priv_data);
	av_freep(&uc);
	return err;
}

static void expect_balanced_and_bystander_alive(struct fixture *f, const char *after)
{
	bool ok = true;
	if (startup_calls != cleanup_calls) {
		fprintf(stderr, "%s: the output called srt_startup() %d time(s) and srt_cleanup() %d time(s)\n", after,
			startup_calls, cleanup_calls);
		ok = false;
	}
	SRT_SOCKSTATUS st = srt_getsockstate(f->bystander);
	if (libsrt_counts_references && st != SRTS_LISTENING) {
		fprintf(stderr,
			"%s: another SRT user's socket was broken (state %d, expected SRTS_LISTENING %d): the output "
			"released a libsrt reference it did not hold\n",
			after, (int)st, (int)SRTS_LISTENING);
		ok = false;
	}
	if (!ok)
		fail();
}

static void accepted_stream_leaves_other_srt_users_alone(void **state)
{
	struct fixture *f = *state;
	char url[256];
	snprintf(url, sizeof(url), "srt://127.0.0.1:%d?mode=caller", f->port);

	int err = open_like_mpegts_output(url, GOOD_PASSPHRASE);
	assert_int_equal(err, OBS_OUTPUT_SUCCESS);
	assert_int_equal(startup_calls, 1);
	expect_balanced_and_bystander_alive(f, "after a normal connect and stop");
}

static void rejected_passphrase_leaves_other_srt_users_alone(void **state)
{
	struct fixture *f = *state;
	char url[256];
	snprintf(url, sizeof(url), "srt://127.0.0.1:%d?mode=caller", f->port);

	int err = open_like_mpegts_output(url, BAD_PASSPHRASE);
	assert_true(err < 0);
	assert_int_equal(startup_calls, 1);
	expect_balanced_and_bystander_alive(f, "after a rejected passphrase");
}

static void nobody_listening_leaves_other_srt_users_alone(void **state)
{
	struct fixture *f = *state;
	char url[256];
	snprintf(url, sizeof(url), "srt://127.0.0.1:%d?mode=caller&connect_timeout=500", closed_udp_port());

	int err = open_like_mpegts_output(url, NULL);
	assert_true(err < 0);
	assert_int_equal(startup_calls, 1);
	expect_balanced_and_bystander_alive(f, "after connecting to a port nobody listens on");
}

static void invalid_url_leaves_other_srt_users_alone(void **state)
{
	struct fixture *f = *state;
	char url[256];
	snprintf(url, sizeof(url), "srt://127.0.0.1:%d?mode=bogus", f->port);

	int err = open_like_mpegts_output(url, NULL);
	assert_true(err < 0);
	assert_int_equal(startup_calls, 1);
	expect_balanced_and_bystander_alive(f, "after an invalid URL");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(accepted_stream_leaves_other_srt_users_alone, setup, teardown),
		cmocka_unit_test_setup_teardown(rejected_passphrase_leaves_other_srt_users_alone, setup, teardown),
		cmocka_unit_test_setup_teardown(nobody_listening_leaves_other_srt_users_alone, setup, teardown),
		cmocka_unit_test_setup_teardown(invalid_url_leaves_other_srt_users_alone, setup, teardown),
	};

	if (!start_server())
		return 1;
	int rc = cmocka_run_group_tests(tests, probe_libsrt, NULL);
	stop_server();

	/* Every result is in; leave without running libraries' static
	 * destructors. Where FFmpeg links one libsrt build and this test (like
	 * obs-ffmpeg) another, as on Ubuntu (libavformat: libsrt-gnutls, here:
	 * libsrt-openssl), both copies end up in the process and export the same
	 * C++ globals. Once srt_startup() has run, libsrt 1.5.4's packet-filter
	 * registry (srt::PacketFilter::filters) is destroyed by both copies at
	 * exit and the second destructor crashes, whatever this test did. */
	fflush(stdout);
	fflush(stderr);
	_exit(rc);
}
