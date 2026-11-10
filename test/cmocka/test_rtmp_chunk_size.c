#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <librtmp/rtmp.h>

/* Set Chunk Size messages from the server. RTMP 1.0 (5.4.1) allows 1 to
 * 0x7FFFFFFF. A size of 0 never lets RTMP_ReadPacket complete a packet, and
 * sizes with bit 31 set turn negative in the int m_inChunkSize and move the
 * read offset backwards, so those are ignored. */

static int setup(void **state)
{
	RTMP *rtmp = RTMP_Alloc();
	assert_non_null(rtmp);
	RTMP_Init(rtmp);
	*state = rtmp;
	return 0;
}

static int teardown(void **state)
{
	RTMP_Free(*state);
	return 0;
}

static void receive_chunk_size(RTMP *rtmp, uint32_t size, uint32_t body_size)
{
	char body[4] = {(char)(size >> 24), (char)(size >> 16), (char)(size >> 8), (char)size};
	RTMPPacket packet = {0};
	packet.m_packetType = RTMP_PACKET_TYPE_CHUNK_SIZE;
	packet.m_nBodySize = body_size;
	packet.m_body = body;
	assert_int_equal(RTMP_ClientPacket(rtmp, &packet), 0);
}

static void valid_sizes(void **state)
{
	RTMP *rtmp = *state;

	assert_int_equal(rtmp->m_inChunkSize, RTMP_DEFAULT_CHUNKSIZE);
	receive_chunk_size(rtmp, 4096, 4);
	assert_int_equal(rtmp->m_inChunkSize, 4096);
	receive_chunk_size(rtmp, 1, 4);
	assert_int_equal(rtmp->m_inChunkSize, 1);
	receive_chunk_size(rtmp, 0x7fffffff, 4);
	assert_int_equal(rtmp->m_inChunkSize, 0x7fffffff);
}

static void short_message_is_ignored(void **state)
{
	RTMP *rtmp = *state;

	receive_chunk_size(rtmp, 4096, 3);
	assert_int_equal(rtmp->m_inChunkSize, RTMP_DEFAULT_CHUNKSIZE);
}

static void invalid_sizes_are_ignored(void **state)
{
	RTMP *rtmp = *state;

	receive_chunk_size(rtmp, 4096, 4);
	receive_chunk_size(rtmp, 0, 4);
	assert_int_equal(rtmp->m_inChunkSize, 4096);
	receive_chunk_size(rtmp, 0x80000000, 4);
	assert_int_equal(rtmp->m_inChunkSize, 4096);
	receive_chunk_size(rtmp, 0xffffffff, 4);
	assert_int_equal(rtmp->m_inChunkSize, 4096);
}

/* Window Acknowledgement Size and Set Peer Bandwidth carry a 32-bit value.
 * A message shorter than that is ignored. The body buffer is longer than
 * the size the message declares, so reading past the declared size would
 * change the stored value. */
static void receive_bandwidth(RTMP *rtmp, uint8_t type, uint32_t body_size)
{
	char body[5] = {0, 0, 0x10, 0, 2};
	RTMPPacket packet = {0};
	packet.m_packetType = type;
	packet.m_nBodySize = body_size;
	packet.m_body = body;
	assert_int_equal(RTMP_ClientPacket(rtmp, &packet), 0);
}

static void short_bandwidth_messages_are_ignored(void **state)
{
	RTMP *rtmp = *state;
	int server_bw = rtmp->m_nServerBW;
	int client_bw = rtmp->m_nClientBW;

	receive_bandwidth(rtmp, RTMP_PACKET_TYPE_SERVER_BW, 3);
	assert_int_equal(rtmp->m_nServerBW, server_bw);
	receive_bandwidth(rtmp, RTMP_PACKET_TYPE_CLIENT_BW, 0);
	assert_int_equal(rtmp->m_nClientBW, client_bw);

	receive_bandwidth(rtmp, RTMP_PACKET_TYPE_SERVER_BW, 4);
	assert_int_equal(rtmp->m_nServerBW, 0x1000);
	receive_bandwidth(rtmp, RTMP_PACKET_TYPE_CLIENT_BW, 5);
	assert_int_equal(rtmp->m_nClientBW, 0x1000);
	assert_int_equal(rtmp->m_nClientBW2, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(valid_sizes, setup, teardown),
		cmocka_unit_test_setup_teardown(short_message_is_ignored, setup, teardown),
		cmocka_unit_test_setup_teardown(invalid_sizes_are_ignored, setup, teardown),
		cmocka_unit_test_setup_teardown(short_bandwidth_messages_are_ignored, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
