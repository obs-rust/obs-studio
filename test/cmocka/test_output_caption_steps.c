#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-output-caption.h"

/*
 * Characterization of the caption plan of add_caption() in
 * libobs/obs-output.c, moved to libobs/obs-output-caption.c before its Rust
 * port (#3074):
 * - obs_output_caption_begin(): the carrier the encoder's codec gets, and
 *   the priority gate;
 * - obs_output_caption_hevc_sei_header(): the HEVC Annex B start code and
 *   the suffix SEI's NAL unit header;
 * - obs_output_caption_cc_keep(): the CEA-608 triplet filter.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. add_caption() keeps the
 * caption queue, the libcaption calls and the new packet; the existing
 * test_output_caption_codec and test_output_caption_queue see those through
 * the real libobs.
 *
 * Not covered: an HEVC packet shorter than its start code and 2 header
 * bytes, which the C reads past (#3074).
 */

/* A value obs_output_caption_begin() never gives, to see that it sets
 * *carrier. */
#define UNSET 0xA5A5A5A5u

/* ------------------------------------------------------------------------ */
/* obs_output_caption_begin() */

static uint32_t carrier_of(const char *codec, bool hevc)
{
	uint32_t carrier = UNSET;
	assert_true(obs_output_caption_begin(codec, hevc, 0, &carrier));
	return carrier;
}

/* h264, av1 and (with ENABLE_HEVC) hevc carry captions. */
static void test_carriers(void **state)
{
	(void)state;

	assert_int_equal(carrier_of("h264", false), OBS_OUTPUT_CAPTION_AVC);
	assert_int_equal(carrier_of("h264", true), OBS_OUTPUT_CAPTION_AVC);
	assert_int_equal(carrier_of("av1", false), OBS_OUTPUT_CAPTION_AV1);
	assert_int_equal(carrier_of("av1", true), OBS_OUTPUT_CAPTION_AV1);
	assert_int_equal(carrier_of("hevc", true), OBS_OUTPUT_CAPTION_HEVC);
}

/* Any other codec, and hevc in a build without ENABLE_HEVC, gets none: the
 * caption still leaves the queue (test_output_caption_codec). The names
 * match exactly, case and length included. */
static void test_no_carrier(void **state)
{
	(void)state;

	static const char *const codecs[] = {"",    "prores", "aac",  "H264", "h264 ", "h26",  "h2640", "avc",
					     "AV1", "av",     "av10", "HEVC", "hevc ", "h265", "x264"};

	assert_int_equal(carrier_of("hevc", false), OBS_OUTPUT_CAPTION_NONE);
	for (size_t i = 0; i < sizeof(codecs) / sizeof(codecs[0]); i++) {
		assert_int_equal(carrier_of(codecs[i], false), OBS_OUTPUT_CAPTION_NONE);
		assert_int_equal(carrier_of(codecs[i], true), OBS_OUTPUT_CAPTION_NONE);
	}
}

/* A packet of priority above 1 takes no caption, and the caption stays
 * queued; priority 1 and below take one. Negative priorities count as
 * below 1 (characterized, not endorsed: encoders give 0 to 3). *carrier is
 * set either way. */
static void test_priority_gate(void **state)
{
	(void)state;

	static const struct {
		int priority;
		bool takes;
	} cases[] = {
		{INT_MIN, true}, {-1, true}, {0, true}, {1, true}, {2, false}, {3, false}, {INT_MAX, false},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		uint32_t carrier = UNSET;
		assert_int_equal(obs_output_caption_begin("h264", true, cases[i].priority, &carrier), cases[i].takes);
		assert_int_equal(carrier, OBS_OUTPUT_CAPTION_AVC);

		carrier = UNSET;
		assert_int_equal(obs_output_caption_begin("prores", true, cases[i].priority, &carrier), cases[i].takes);
		assert_int_equal(carrier, OBS_OUTPUT_CAPTION_NONE);
	}
}

/* ------------------------------------------------------------------------ */
/* obs_output_caption_hevc_sei_header() */

static void check_header(const uint8_t *data, uint8_t first, uint8_t second)
{
	uint8_t header[2] = {0xEE, 0xEE};
	assert_true(obs_output_caption_hevc_sei_header(data, header));
	assert_int_equal(header[0], first);
	assert_int_equal(header[1], second);
}

/* After a 3-byte or a 4-byte start code, the packet's own 2-byte header
 * gives the SEI's: NAL type 40 (0x50 in the first byte), the first byte's
 * last bit (the top bit of nuh_layer_id) and the whole second byte kept. */
static void test_hevc_header(void **state)
{
	(void)state;

	/* 3-byte start code, an IDR_W_RADL slice (type 19). */
	static const uint8_t idr3[] = {0x00, 0x00, 0x01, 0x26, 0x01, 0xAF};
	check_header(idr3, 0x50, 0x01);
	/* 4-byte start code, a VPS (type 32). */
	static const uint8_t vps4[] = {0x00, 0x00, 0x00, 0x01, 0x40, 0x01};
	check_header(vps4, 0x50, 0x01);
	/* The first byte's last bit set, a temporal id in the second byte. */
	static const uint8_t layer3[] = {0x00, 0x00, 0x01, 0x03, 0x2B};
	check_header(layer3, 0x51, 0x2B);
	static const uint8_t layer4[] = {0x00, 0x00, 0x00, 0x01, 0xFF, 0xFF};
	check_header(layer4, 0x51, 0xFF);
	/* A forbidden_zero_bit of 1 in the packet does not carry over. */
	static const uint8_t forbidden[] = {0x00, 0x00, 0x01, 0x80, 0x07};
	check_header(forbidden, 0x50, 0x07);
}

/* The 3-byte start code is tried first: 00 00 01 00 01 is a 3-byte start
 * code followed by the header 00 01, not a 4-byte one (characterized, not
 * endorsed: such a header is a forbidden NAL type 0). */
static void test_hevc_three_byte_first(void **state)
{
	(void)state;

	static const uint8_t data[] = {0x00, 0x00, 0x01, 0x00, 0x01, 0x42};
	check_header(data, 0x50, 0x01);
}

/* Without either start code at the start of the packet there is no caption,
 * and the header is left as it was. */
static void test_hevc_no_start_code(void **state)
{
	(void)state;

	static const uint8_t cases[][6] = {
		{0x00, 0x01, 0x40, 0x01, 0x0C, 0x01}, {0x00, 0x00, 0x02, 0x40, 0x01, 0x0C},
		{0x00, 0x00, 0x00, 0x00, 0x01, 0x40}, {0x01, 0x00, 0x00, 0x01, 0x40, 0x01},
		{0x00, 0x00, 0x00, 0x02, 0x40, 0x01}, {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF},
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		uint8_t header[2] = {0xEE, 0xEE};
		assert_false(obs_output_caption_hevc_sei_header(cases[i], header));
		assert_int_equal(header[0], 0xEE);
		assert_int_equal(header[1], 0xEE);
	}
}

/* ------------------------------------------------------------------------ */
/* obs_output_caption_cc_keep() */

static void check_cc(uint8_t b0, uint8_t b1, uint8_t b2, bool keep)
{
	const uint8_t triplet[3] = {b0, b1, b2};
	uint16_t cc_data = 0xBEEF;
	assert_int_equal(obs_output_caption_cc_keep(triplet, &cc_data), keep);
	assert_int_equal(cc_data, keep ? (uint16_t)(b1 << 8 | b2) : 0xBEEF);
}

/* Only cc_type 0 (CEA-608 field 1) is kept; the other bits of the first
 * byte are not looked at, so cc_valid 0 is kept too (characterized, not
 * endorsed). */
static void test_cc_type(void **state)
{
	(void)state;

	/* 0x94 0x2C: erase displayed memory, both bytes of odd parity. */
	check_cc(0xFC, 0x94, 0x2C, true);
	check_cc(0xFD, 0x94, 0x2C, false);
	check_cc(0xFE, 0x94, 0x2C, false);
	check_cc(0xFF, 0x94, 0x2C, false);
	check_cc(0x00, 0x94, 0x2C, true);
	check_cc(0xF8, 0x94, 0x2C, true);
	check_cc(0x03, 0x94, 0x2C, false);
}

/* Padding (0x80 0x80) and 0x00 0x00 are dropped; 0x80 with another byte is
 * not padding. 0x00 0x00 also fails the parity test below, so the C's
 * separate test for 0 never decides. */
static void test_cc_padding_and_zero(void **state)
{
	(void)state;

	check_cc(0xFC, 0x80, 0x80, false);
	check_cc(0xFC, 0x00, 0x00, false);
	check_cc(0xFC, 0x80, 0x01, true);
	check_cc(0xFC, 0x01, 0x80, true);
}

/* Both bytes must have odd parity (bit 7 is the parity bit). */
static void test_cc_parity(void **state)
{
	(void)state;

	/* 0x14 has two bits set, 0x2D four. */
	check_cc(0xFC, 0x14, 0x2C, false);
	check_cc(0xFC, 0x94, 0x2D, false);
	check_cc(0xFC, 0x14, 0x2D, false);
	/* 'H' 'i' with their parity bits: 0xC8 0xE9. */
	check_cc(0xFC, 0xC8, 0xE9, true);
	check_cc(0xFC, 0x48, 0xE9, false);
	check_cc(0xFC, 0xC8, 0x69, false);
	/* 0x00 with an odd byte: 0 has even parity. */
	check_cc(0xFC, 0x00, 0x01, false);
	check_cc(0xFC, 0x01, 0x00, false);
	check_cc(0xFC, 0x01, 0x01, true);
}

/* A byte with an odd number of bits set. */
static bool odd_parity(uint8_t b)
{
	bool odd = false;
	for (; b; b &= (uint8_t)(b - 1))
		odd = !odd;
	return odd;
}

/* Every triplet: kept exactly when cc_type is 0, cc_data is neither 0x8080
 * nor 0 and both bytes have odd parity. */
static void test_cc_every_triplet(void **state)
{
	(void)state;

	size_t kept = 0;
	for (uint32_t t = 0; t < (1u << 24); t++) {
		const uint8_t b0 = (uint8_t)(t >> 16), b1 = (uint8_t)(t >> 8), b2 = (uint8_t)t;
		const uint16_t word = (uint16_t)(b1 << 8 | b2);
		const bool odd = odd_parity(b1) && odd_parity(b2);
		const bool expect = (b0 & 0x3) == 0 && word != 0x8080 && word != 0 && odd;
		const uint8_t triplet[3] = {b0, b1, b2};
		uint16_t cc_data = 0xBEEF;
		const bool keep = obs_output_caption_cc_keep(triplet, &cc_data);
		if (keep != expect || cc_data != (keep ? word : 0xBEEF)) {
			fprintf(stderr, "triplet %02x %02x %02x: keep %d cc_data %04x\n", b0, b1, b2, keep, cc_data);
			fail();
		}
		kept += keep;
	}
	/* 64 first bytes with cc_type 0, 128 * 128 odd pairs, less 0x8080. */
	assert_int_equal(kept, 64 * (128 * 128 - 1));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_carriers),
		cmocka_unit_test(test_no_carrier),
		cmocka_unit_test(test_priority_gate),
		cmocka_unit_test(test_hevc_header),
		cmocka_unit_test(test_hevc_three_byte_first),
		cmocka_unit_test(test_hevc_no_start_code),
		cmocka_unit_test(test_cc_type),
		cmocka_unit_test(test_cc_padding_and_zero),
		cmocka_unit_test(test_cc_parity),
		cmocka_unit_test(test_cc_every_triplet),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
