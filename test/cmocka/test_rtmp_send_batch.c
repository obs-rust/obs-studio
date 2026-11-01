#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <errno.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <unistd.h>

#include <librtmp/rtmp.h>

/* RTMP_SendPacket splits a message into out-chunk-size chunks, each after a
 * one-byte type-3 header. It wrote every chunk with its own send(): about 37
 * syscalls for a 150 KB keyframe at OBS's 4096-byte chunk size (#3207). The
 * whole chunked message now goes to the socket in one write.
 *
 * The socket is an AF_UNIX SOCK_SEQPACKET pair, so the peer receives one
 * record per send() and the record count is the syscall count. The received
 * stream is then checked chunk by chunk against the body. */

#define CHUNK 4096
#define CHANNEL 0x06

static size_t send_one(size_t body_size, uint32_t timestamp, unsigned char *out, size_t out_cap, size_t *records)
{
	int fds[2];
	assert_int_equal(socketpair(AF_UNIX, SOCK_SEQPACKET, 0, fds), 0);
	int buf = 4 * 1024 * 1024;
	setsockopt(fds[0], SOL_SOCKET, SO_SNDBUF, &buf, sizeof(buf));
	setsockopt(fds[1], SOL_SOCKET, SO_RCVBUF, &buf, sizeof(buf));

	RTMP *rtmp = RTMP_Alloc();
	assert_non_null(rtmp);
	RTMP_Init(rtmp);
	rtmp->m_outChunkSize = CHUNK;
	rtmp->m_sb.sb_socket = fds[0];

	RTMPPacket packet = {0};
	assert_true(RTMPPacket_Alloc(&packet, (uint32_t)body_size));
	for (size_t i = 0; i < body_size; i++)
		packet.m_body[i] = (char)(i * 13 + 5);
	packet.m_nChannel = CHANNEL;
	packet.m_headerType = RTMP_PACKET_SIZE_LARGE;
	packet.m_packetType = RTMP_PACKET_TYPE_VIDEO;
	packet.m_nTimeStamp = timestamp;
	packet.m_nInfoField2 = 1;
	packet.m_nBodySize = (uint32_t)body_size;

	assert_true(RTMP_SendPacket(rtmp, &packet, FALSE));

	size_t len = 0;
	*records = 0;
	for (;;) {
		ssize_t n = recv(fds[1], out + len, out_cap - len, MSG_DONTWAIT);
		if (n < 0) {
			assert_true(errno == EAGAIN || errno == EWOULDBLOCK);
			break;
		}
		len += (size_t)n;
		(*records)++;
	}

	RTMPPacket_Free(&packet);
	rtmp->m_sb.sb_socket = -1;
	RTMP_Free(rtmp);
	close(fds[0]);
	close(fds[1]);
	return len;
}

static void check_stream(const unsigned char *s, size_t len, size_t body_size, uint32_t timestamp)
{
	/* A timestamp of 0xffffff or more follows each header as 4 bytes. */
	const size_t ext = timestamp >= 0xffffff ? 4 : 0;
	/* Type 0 header: fmt 0 and the channel, 3-byte timestamp, 3-byte size,
	 * type, 4-byte little-endian stream id. */
	assert_true(len >= 12);
	assert_int_equal(s[0], CHANNEL);
	assert_int_equal((s[4] << 16) | (s[5] << 8) | s[6], body_size);
	assert_int_equal(s[7], RTMP_PACKET_TYPE_VIDEO);
	size_t pos = 12 + ext;
	size_t done = 0;
	while (done < body_size) {
		if (done > 0) {
			assert_true(pos < len);
			assert_int_equal(s[pos], 0xc0 | CHANNEL);
			pos += 1 + ext;
		}
		size_t n = body_size - done < CHUNK ? body_size - done : CHUNK;
		assert_true(pos + n <= len);
		for (size_t i = 0; i < n; i++)
			assert_int_equal(s[pos + i], (unsigned char)((done + i) * 13 + 5));
		pos += n;
		done += n;
	}
	assert_int_equal(pos, len);
}

static void keyframe_is_one_write(void **state)
{
	(void)state;
	const size_t body = 150000;
	unsigned char *out = malloc(body * 2);
	size_t records;
	size_t len = send_one(body, 1000, out, body * 2, &records);
	check_stream(out, len, body, 1000);
	assert_int_equal(records, 1);
	free(out);
}

static void one_chunk_message_is_one_write(void **state)
{
	(void)state;
	unsigned char out[CHUNK * 2];
	size_t records;
	size_t len = send_one(1000, 1000, out, sizeof(out), &records);
	check_stream(out, len, 1000, 1000);
	assert_int_equal(records, 1);
}

static void exact_chunk_multiple(void **state)
{
	(void)state;
	const size_t body = CHUNK * 3;
	unsigned char *out = malloc(body * 2);
	size_t records;
	size_t len = send_one(body, 1000, out, body * 2, &records);
	check_stream(out, len, body, 1000);
	assert_int_equal(records, 1);
	free(out);
}

/* The buffer holds the 4-byte extended timestamp after every type-3
 * header too. */
static void extended_timestamp(void **state)
{
	(void)state;
	const size_t body = 150000;
	const uint32_t ts = 0x1000000;
	unsigned char *out = malloc(body * 2);
	size_t records;
	size_t len = send_one(body, ts, out, body * 2, &records);
	check_stream(out, len, body, ts);
	assert_int_equal(records, 1);
	free(out);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(keyframe_is_one_write),
		cmocka_unit_test(one_chunk_message_is_one_write),
		cmocka_unit_test(exact_chunk_multiple),
		cmocka_unit_test(extended_timestamp),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
