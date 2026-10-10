#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-avc.h>

/* Characterization of the exported functions in obs-avc.c, through libobs
 * only (test_avc.c compiles its own copy of obs-avc.c for UBSan). */

/* A 1080p High-profile SPS as x264 writes it (4:2:0, 8-bit), with
 * emulation-prevention bytes, and a PPS. */
static const uint8_t sps_high[] = {0x67, 0x64, 0x00, 0x28, 0xac, 0xd9, 0x40, 0x78, 0x02, 0x27, 0xe5, 0xc0, 0x44, 0x00,
				   0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03, 0x00, 0xc8, 0x3c, 0x60, 0xc9, 0x20};
static const uint8_t pps[] = {0x68, 0xeb, 0xe3, 0xcb};

static size_t put_nal(uint8_t *dst, const uint8_t *nal, size_t size)
{
	static const uint8_t code[] = {0, 0, 0, 1};
	memcpy(dst, code, sizeof(code));
	memcpy(dst + sizeof(code), nal, size);
	return sizeof(code) + size;
}

static void test_avc_keyframe(void **state)
{
	UNUSED_PARAMETER(state);

	const uint8_t sps_idr[] = {0, 0, 0, 1, 0x67, 1, 0, 0, 1, 0x65, 2};
	const uint8_t slice_idr[] = {0, 0, 1, 0x41, 1, 0, 0, 1, 0x65, 2};
	const uint8_t sps_only[] = {0, 0, 1, 0x67, 1, 2, 3};

	assert_true(obs_avc_keyframe(sps_idr, sizeof(sps_idr)));
	/* the first slice decides */
	assert_false(obs_avc_keyframe(slice_idr, sizeof(slice_idr)));
	assert_false(obs_avc_keyframe(sps_only, sizeof(sps_only)));
}

static void test_avc_parse_packet(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t data[] = {0, 0, 0, 1, 0x06, 0xaa, 0, 0, 0, 1, 0x65, 0x88, 0x84};
	const uint8_t avcc[] = {0, 0, 0, 2, 0x06, 0xaa, 0, 0, 0, 3, 0x65, 0x88, 0x84};
	struct encoder_packet src = {0};
	struct encoder_packet out = {0};

	src.data = data;
	src.size = sizeof(data);
	src.pts = 42;
	src.keyframe = false;
	src.priority = 0;
	src.drop_priority = -1;

	obs_parse_avc_packet(&out, &src);

	/* units become 32-bit big-endian sizes; header bytes decide the rest */
	assert_int_equal(out.size, sizeof(avcc));
	assert_memory_equal(out.data, avcc, sizeof(avcc));
	assert_true(out.keyframe);
	assert_int_equal(out.priority, 3);
	assert_int_equal(out.drop_priority, 3);
	assert_int_equal(out.pts, 42);

	/* a long reference count of 1 sits in front of the data */
	long ref;
	memcpy(&ref, out.data - sizeof(long), sizeof(long));
	assert_int_equal(ref, 1);
	bfree(out.data - sizeof(long));
}

static void test_avc_packet_priority(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t data[] = {0, 0, 1, 0x06, 0, 0, 0, 1, 0x41, 0};
	struct encoder_packet packet = {0};

	packet.data = data;
	packet.size = sizeof(data);
	packet.priority = 0;
	assert_int_equal(obs_parse_avc_packet_priority(&packet), 2);

	packet.priority = 5;
	assert_int_equal(obs_parse_avc_packet_priority(&packet), 5);
}

static void test_avc_parse_header_high_profile(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t data[64];
	size_t size = put_nal(data, sps_high, sizeof(sps_high));
	size += put_nal(data + size, pps, sizeof(pps));

	uint8_t *header = NULL;
	size_t header_size = obs_parse_avc_header(&header, data, size);

	const uint8_t head[] = {0x01, 0x64, 0x00, 0x28, 0xff, 0xe1, 0x00, sizeof(sps_high)};
	const uint8_t tail[] = {0xfd, 0xf8, 0xf8, 0x00};
	assert_int_equal(header_size, sizeof(head) + sizeof(sps_high) + 3 + sizeof(pps) + sizeof(tail));
	assert_memory_equal(header, head, sizeof(head));
	assert_memory_equal(header + sizeof(head), sps_high, sizeof(sps_high));
	assert_memory_equal(header + header_size - sizeof(tail), tail, sizeof(tail));
	bfree(header);
}

static void test_avc_parse_header_other_inputs(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t *header = (uint8_t *)&header;
	const uint8_t tiny[] = {0, 0, 1, 0x67, 1, 2};
	const uint8_t no_pps[] = {0, 0, 0, 1, 0x67, 0x42, 0, 0x1f, 0x11};
	const uint8_t avcc[] = {1, 0x42, 0, 0x1f, 0xff, 0xe1, 0};

	/* 6 bytes or fewer, or no PPS: 0, and *header is untouched */
	assert_int_equal(obs_parse_avc_header(&header, tiny, sizeof(tiny)), 0);
	assert_int_equal(obs_parse_avc_header(&header, no_pps, sizeof(no_pps)), 0);
	assert_ptr_equal(header, (uint8_t *)&header);

	/* not Annex B: copied as is */
	assert_int_equal(obs_parse_avc_header(&header, avcc, sizeof(avcc)), sizeof(avcc));
	assert_memory_equal(header, avcc, sizeof(avcc));
	bfree(header);
}

static void test_avc_extract_headers(void **state)
{
	UNUSED_PARAMETER(state);

	const uint8_t sei[] = {0x06, 1, 2};
	const uint8_t idr[] = {0x65, 7, 7};
	uint8_t data[64];
	size_t size = put_nal(data, sps_high, sizeof(sps_high));
	size_t sei_at = size;
	size += put_nal(data + size, sei, sizeof(sei));
	size_t pps_at = size;
	size += put_nal(data + size, pps, sizeof(pps));
	size_t idr_at = size;
	size += put_nal(data + size, idr, sizeof(idr));

	uint8_t *packet, *header, *sei_data;
	size_t packet_size, header_size, sei_size;
	obs_extract_avc_headers(data, size, &packet, &packet_size, &header, &header_size, &sei_data, &sei_size);

	/* each unit keeps its start code */
	assert_int_equal(header_size, sei_at + (idr_at - pps_at));
	assert_memory_equal(header, data, sei_at);
	assert_memory_equal(header + sei_at, data + pps_at, idr_at - pps_at);
	assert_int_equal(sei_size, pps_at - sei_at);
	assert_memory_equal(sei_data, data + sei_at, sei_size);
	assert_int_equal(packet_size, size - idr_at);
	assert_memory_equal(packet, data + idr_at, packet_size);

	bfree(packet);
	bfree(header);
	bfree(sei_data);

	/* no units at all: every output is NULL */
	const uint8_t none[] = {1, 2, 3};
	obs_extract_avc_headers(none, sizeof(none), &packet, &packet_size, &header, &header_size, &sei_data, &sei_size);
	assert_null(packet);
	assert_null(header);
	assert_null(sei_data);
	assert_int_equal(packet_size + header_size + sei_size, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_avc_keyframe),
		cmocka_unit_test(test_avc_parse_packet),
		cmocka_unit_test(test_avc_packet_priority),
		cmocka_unit_test(test_avc_parse_header_high_profile),
		cmocka_unit_test(test_avc_parse_header_other_inputs),
		cmocka_unit_test(test_avc_extract_headers),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
