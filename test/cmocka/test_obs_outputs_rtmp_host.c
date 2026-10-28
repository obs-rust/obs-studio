#include <string.h>
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/socket.h>

#include "plugins/obs-outputs/rtmp-stream-host-posix.c.h"

#ifdef TEST_HAVE_MBEDTLS
#include "plugins/obs-outputs/rtmp-send-result.h"
#endif

/*
 * The RTMP output's POSIX host (obs-rust/obs-studio#851): socket errors map
 * to the same locale keys, sockets go non-blocking and report pending bytes
 * through ioctl, the last error is errno, and the Windows-only socket loop
 * is refused. The Windows host (WSA errors, ioctlsocket, send thread) is
 * compiled by the cross check.
 */

const char *obs_module_text(const char *key)
{
	return key;
}

static void test_error_text(void **state)
{
	(void)state;
	assert_string_equal(rtmp_host_socket_error_text(ETIMEDOUT), "ConnectionTimedOut");
	assert_string_equal(rtmp_host_socket_error_text(ECONNRESET), "ConnectionReset");
	assert_string_equal(rtmp_host_socket_error_text(EHOSTUNREACH), "NoRoute");
	assert_null(rtmp_host_socket_error_text(0));
}

static void test_root_cert_error(void **state)
{
	(void)state;
	const char *msg = rtmp_host_root_cert_error();
	assert_non_null(strstr(msg, "Failed to load root certificates for a secure TLS connection."));
}

static void test_socket_calls(void **state)
{
	(void)state;
	int fds[2];
	int size = -1;

	assert_int_equal(socketpair(AF_UNIX, SOCK_STREAM, 0, fds), 0);
	assert_int_equal(rtmp_host_set_nonblocking((uintptr_t)fds[0]), 0);
	assert_true(fcntl(fds[0], F_GETFL) & O_NONBLOCK);

	assert_int_equal(write(fds[1], "abc", 3), 3);
	assert_true(rtmp_host_bytes_available((uintptr_t)fds[0], &size) >= 0);
	assert_int_equal(size, 3);

	close(fds[0]);
	close(fds[1]);

	assert_true(rtmp_host_bytes_available((uintptr_t)fds[0], &size) < 0);
	assert_int_equal(rtmp_host_last_socket_error(), EBADF);
	assert_int_equal(rtmp_host_set_nonblocking((uintptr_t)fds[0]), EBADF);
}

static void test_no_socket_loop(void **state)
{
	(void)state;
	assert_false(rtmp_host_has_socket_loop());
	assert_false(rtmp_host_start_socket_loop(NULL));
	rtmp_host_log_sndbuf_size(NULL);
	rtmp_host_log_interface_type(NULL);
}

#ifdef TEST_HAVE_MBEDTLS
/*
 * The socket loop's send rules (obs-rust/obs-studio#2220). The loop is inside
 * `#ifdef _WIN32`, but these decisions are not: an RTMPS stream sends through
 * mbedtls_ssl_write(), which answers a full transport with
 * MBEDTLS_ERR_SSL_WANT_WRITE and a pending handshake with
 * MBEDTLS_ERR_SSL_WANT_READ instead of -1/EAGAIN.
 */

/* The two library errors the loop depends on, pinned so an Mbed TLS change
 * cannot silently move them out from under the classification. */
static void test_send_result_mbedtls_codes(void **state)
{
	(void)state;
	assert_int_equal(MBEDTLS_ERR_SSL_WANT_READ, -0x6900);
	assert_int_equal(MBEDTLS_ERR_SSL_WANT_WRITE, -0x6880);
}

/* A send that accepted bytes is success, whatever the socket error was. */
static void test_send_result_wrote(void **state)
{
	(void)state;
	assert_int_equal(rtmp_send_result_of(1, false), RTMP_SEND_WROTE);
	assert_int_equal(rtmp_send_result_of(16384, false), RTMP_SEND_WROTE);
	assert_int_equal(rtmp_send_result_of(512, true), RTMP_SEND_WROTE);

	struct rtmp_send_action action = rtmp_send_action_of(RTMP_SEND_WROTE);
	assert_false(action.clear_can_write);
	assert_false(action.fatal);
}

/* TLS out of buffer space must stop the loop until FD_WRITE, not spin. */
static void test_send_result_tls_would_block(void **state)
{
	(void)state;
	assert_int_equal(rtmp_send_result_of(MBEDTLS_ERR_SSL_WANT_WRITE, false), RTMP_SEND_WAIT_WRITE);

	struct rtmp_send_action action = rtmp_send_action_of(RTMP_SEND_WAIT_WRITE);
	assert_true(action.clear_can_write);
	assert_false(action.fatal);
}

/* A handshake or renegotiation read also stops the loop, until FD_READ. */
static void test_send_result_tls_wants_read(void **state)
{
	(void)state;
	assert_int_equal(rtmp_send_result_of(MBEDTLS_ERR_SSL_WANT_READ, false), RTMP_SEND_WAIT_READ);

	struct rtmp_send_action action = rtmp_send_action_of(RTMP_SEND_WAIT_READ);
	assert_true(action.clear_can_write);
	assert_false(action.fatal);
}

/* A fatal TLS send logs a reason, not a Winsock error number. */
static void test_send_result_error_text(void **state)
{
	(void)state;
	char buffer[128];

	const char *text = rtmp_send_tls_error_text(MBEDTLS_ERR_SSL_WANT_WRITE, buffer, sizeof(buffer));
	assert_ptr_equal(text, buffer);
	assert_true(strlen(text) > 0);
}

/* A plain socket keeps the old rule: -1 with WSAEWOULDBLOCK waits for FD_WRITE. */
static void test_send_result_plain_socket(void **state)
{
	(void)state;
	assert_int_equal(rtmp_send_result_of(-1, true), RTMP_SEND_WAIT_WRITE);
}

/* Everything else ends the stream: a closed connection, an unclassified -1,
 * and any other Mbed TLS error. */
static void test_send_result_fatal(void **state)
{
	(void)state;
	assert_int_equal(rtmp_send_result_of(0, false), RTMP_SEND_FAILED);
	assert_int_equal(rtmp_send_result_of(-1, false), RTMP_SEND_FAILED);
	assert_int_equal(rtmp_send_result_of(MBEDTLS_ERR_SSL_FATAL_ALERT_MESSAGE, false), RTMP_SEND_FAILED);
	assert_int_equal(rtmp_send_result_of(MBEDTLS_ERR_SSL_INTERNAL_ERROR, false), RTMP_SEND_FAILED);

	struct rtmp_send_action action = rtmp_send_action_of(RTMP_SEND_FAILED);
	assert_true(action.fatal);
}

/* After WANT_WRITE the retry repeats the first call's length, even though the
 * loop recomputes its length from a write buffer that kept growing. The next
 * send after a completed one uses the new length. */
static void test_send_retry_repeats_first_length(void **state)
{
	(void)state;
	struct rtmp_send_retry retry = {0};

	assert_int_equal(rtmp_send_retry_length(&retry, 1000), 1000);
	assert_int_equal(rtmp_send_retry_length(&retry, 4000), 1000);
	assert_int_equal(rtmp_send_retry_length(&retry, 8000), 1000);

	rtmp_send_retry_done(&retry);
	assert_int_equal(rtmp_send_retry_length(&retry, 8000), 8000);
}
#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_error_text),
		cmocka_unit_test(test_root_cert_error),
		cmocka_unit_test(test_socket_calls),
		cmocka_unit_test(test_no_socket_loop),
#ifdef TEST_HAVE_MBEDTLS
		cmocka_unit_test(test_send_result_mbedtls_codes),
		cmocka_unit_test(test_send_result_wrote),
		cmocka_unit_test(test_send_result_tls_would_block),
		cmocka_unit_test(test_send_result_tls_wants_read),
		cmocka_unit_test(test_send_result_error_text),
		cmocka_unit_test(test_send_result_plain_socket),
		cmocka_unit_test(test_send_result_fatal),
		cmocka_unit_test(test_send_retry_repeats_first_length),
#endif
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
