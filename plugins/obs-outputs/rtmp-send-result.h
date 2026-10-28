/* How the RTMP socket loop reads a send result, and which length a retry has
 * to repeat (obs-rust/obs-studio#2220).
 *
 * The loop treats anything but -1/EAGAIN and 0 as "no error", which is only
 * true for a plain socket: mbedtls_ssl_write() reports a full transport or a
 * pending handshake read as negative library errors, so an RTMPS stream spins
 * or corrupts data. These decisions are platform-neutral and free of Winsock,
 * so Linux Tier 2 pins them (test/cmocka/test_obs_outputs_rtmp_host.c) while
 * the Windows loop stays the only caller. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include <mbedtls/error.h>
#include <mbedtls/ssl.h>

enum rtmp_send_result {
	/* The transport accepted `ret` bytes. */
	RTMP_SEND_WROTE,
	/* Nothing was accepted and the transport is full: wait for FD_WRITE. */
	RTMP_SEND_WAIT_WRITE,
	/* TLS needs a read before it can write (handshake, renegotiation):
	 * wait for FD_READ. */
	RTMP_SEND_WAIT_READ,
	/* Connection closed, or an error that ends the stream. */
	RTMP_SEND_FAILED,
};

/* What the loop does with a result: stop writing until the socket reports an
 * event, or shut the stream down. */
struct rtmp_send_action {
	bool clear_can_write;
	bool fatal;
};

static inline struct rtmp_send_action rtmp_send_action_of(enum rtmp_send_result result)
{
	switch (result) {
	case RTMP_SEND_WAIT_WRITE:
	case RTMP_SEND_WAIT_READ:
		return (struct rtmp_send_action){.clear_can_write = true, .fatal = false};
	case RTMP_SEND_FAILED:
		return (struct rtmp_send_action){.clear_can_write = true, .fatal = true};
	case RTMP_SEND_WROTE:
		break;
	}
	return (struct rtmp_send_action){.clear_can_write = false, .fatal = false};
}

/* `socket_would_block` is the caller's WSAEWOULDBLOCK answer for ret == -1,
 * which only a plain socket produces; a TLS session returns a library error
 * instead, so WSAGetLastError() means nothing for it. */
static inline enum rtmp_send_result rtmp_send_result_of(int ret, bool socket_would_block)
{
	if (ret > 0)
		return RTMP_SEND_WROTE;
	if (ret == MBEDTLS_ERR_SSL_WANT_WRITE)
		return RTMP_SEND_WAIT_WRITE;
	if (ret == MBEDTLS_ERR_SSL_WANT_READ)
		return RTMP_SEND_WAIT_READ;
	if (ret == -1 && socket_would_block)
		return RTMP_SEND_WAIT_WRITE;
	return RTMP_SEND_FAILED;
}

/* The reason a TLS send failed, for the log: a library error code means
 * nothing to the user, and WSAGetLastError() has no answer for it. */
static inline const char *rtmp_send_tls_error_text(int ret, char *buffer, size_t size)
{
	mbedtls_strerror(ret, buffer, size);
	return buffer;
}

/* mbedtls_ssl_write() must be retried with the length of the call that
 * returned MBEDTLS_ERR_SSL_WANT_WRITE. It keeps the unflushed record in
 * out_left and, once that record is flushed, reports the *new* call's length,
 * so the caller's memmove would drop bytes the record never carried. The loop
 * recomputes its length from write_buf_len on every pass (and from
 * min(latency_packet_size, write_buf_len) in low latency mode), so the first
 * length is remembered here until a send actually completes. */
struct rtmp_send_retry {
	size_t len; /* length of the call in flight; 0 when none */
};

static inline size_t rtmp_send_retry_length(struct rtmp_send_retry *retry, size_t want)
{
	if (!retry->len)
		retry->len = want;
	return retry->len;
}

static inline void rtmp_send_retry_done(struct rtmp_send_retry *retry)
{
	retry->len = 0;
}
