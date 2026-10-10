#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include "flv-mux.h"

/* The enhanced RTMP audio tags that rtmp-stream and flv-output build for
 * every audio track, plus the empty packet regression: an empty packet used
 * to return without setting *output, and both callers then bfree()d an
 * uninitialized pointer. */

static struct encoder_packet audio_packet(uint8_t *data, size_t size)
{
	struct encoder_packet packet = {0};
	packet.type = OBS_ENCODER_AUDIO;
	packet.timebase_num = 1;
	packet.timebase_den = 1000;
	packet.pts = 1234;
	packet.dts = 1234;
	packet.data = data;
	packet.size = size;
	return packet;
}

static void test_audio_frames_tag(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t payload[] = {0x21, 0x10, 0x05};
	struct encoder_packet packet = audio_packet(payload, sizeof(payload));
	/* tag type, data size (payload + 5), timestamp 1234 ms, extended
	 * timestamp, stream id, AUDIO_HEADER_EX | FRAMES, fourcc, payload,
	 * previous tag size (11 + 8) */
	const uint8_t want[] = {0x08, 0,   0,   8,   0,    0x04, 0xd2, 0, 0, 0, 0, 0x91,
				'm',  'p', '4', 'a', 0x21, 0x10, 0x05, 0, 0, 0, 19};
	uint8_t *out = NULL;
	size_t size = 0;

	flv_packet_audio_frames(&packet, AUDIO_CODEC_AAC, 0, &out, &size, 0);
	assert_int_equal(size, sizeof(want));
	assert_memory_equal(out, want, sizeof(want));
	bfree(out);
}

static void test_audio_multitrack_start_tag(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t payload[] = {0x12, 0x10};
	struct encoder_packet packet = audio_packet(payload, sizeof(payload));
	/* a track other than 0: AUDIO_HEADER_EX | MULTITRACK, then
	 * ONE_TRACK | SEQ_START, the fourcc and the track index */
	const uint8_t want[] = {0x08, 0,   0,   9,   0,   0x04, 0xd2, 0,    0, 0, 0, 0x95,
				0x00, 'm', 'p', '4', 'a', 2,    0x12, 0x10, 0, 0, 0, 20};
	uint8_t *out = NULL;
	size_t size = 0;

	flv_packet_audio_start(&packet, AUDIO_CODEC_AAC, &out, &size, 2);
	assert_int_equal(size, sizeof(want));
	assert_memory_equal(out, want, sizeof(want));
	bfree(out);
}

static void test_empty_audio_packet(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t payload[] = {0x21};
	uint8_t garbage;
	uint8_t *out;
	size_t size;

	/* NULL data on a secondary track */
	struct encoder_packet packet = audio_packet(NULL, 0);
	out = &garbage;
	size = 99;
	flv_packet_audio_frames(&packet, AUDIO_CODEC_AAC, 0, &out, &size, 1);
	assert_null(out);
	assert_int_equal(size, 0);
	bfree(out);

	/* data with size 0, header path */
	packet = audio_packet(payload, 0);
	out = &garbage;
	size = 99;
	flv_packet_audio_start(&packet, AUDIO_CODEC_AAC, &out, &size, 0);
	assert_null(out);
	assert_int_equal(size, 0);
	bfree(out);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_audio_frames_tag),
		cmocka_unit_test(test_audio_multitrack_start_tag),
		cmocka_unit_test(test_empty_audio_packet),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
