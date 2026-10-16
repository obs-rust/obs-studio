/*
 * HLG-compatible HDR (HEVC 8.4) recording: the mp4 muxer must carry the encoder's native
 * dvvC configuration record into the HEVC sample description, beside
 * hvcC. mp4_write_dvvC_box writes that record verbatim as a plain ISOBMFF box
 * (8-byte header + opaque payload); it must not reinterpret the bytes.
 *
 * The payload is opaque to the muxer, so these tests pin the box framing: the
 * 32-bit big-endian size field must equal payload + 8, the type must be
 * "dvvC", and the payload must be copied byte-for-byte. A regression in any of
 * those (wrong size, wrong type, dropped/altered bytes) fails here.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <util/array-serializer.h>

#include "mp4-mux.h"

/* mp4-mux.c names its start chapter through the module's locale. */
const char *obs_module_text(const char *val)
{
	return val;
}

/* A representative 24-byte dvvC configuration record (the size VideoToolbox
 * emits for profile 8.4). The muxer treats it as opaque; the exact values are
 * irrelevant to the box framing under test. */
static const uint8_t dvvC_payload[24] = {
	0x01, 0x00, 0x10, 0x0d, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
	0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
};

static void dvvC_box_frames_payload_verbatim(void **state)
{
	UNUSED_PARAMETER(state);

	struct array_output_data aod;
	struct serializer s;
	array_output_serializer_init(&s, &aod);

	size_t written = mp4_write_dvvC_box(&s, dvvC_payload, sizeof(dvvC_payload));

	/* 8-byte box header + 24-byte payload. */
	assert_int_equal(written, 8 + sizeof(dvvC_payload));
	assert_int_equal(aod.bytes.num, 8 + sizeof(dvvC_payload));

	const uint8_t *out = aod.bytes.array;
	/* 32-bit big-endian box size. */
	assert_int_equal(out[0], 0x00);
	assert_int_equal(out[1], 0x00);
	assert_int_equal(out[2], 0x00);
	assert_int_equal(out[3], 8 + sizeof(dvvC_payload));
	/* Box type. */
	assert_memory_equal(out + 4, "dvvC", 4);
	/* Payload copied byte-for-byte, unmodified. */
	assert_memory_equal(out + 8, dvvC_payload, sizeof(dvvC_payload));

	array_output_serializer_free(&aod);
}

static void dvvC_box_size_scales_with_payload(void **state)
{
	UNUSED_PARAMETER(state);

	/* A different payload length must change the size field accordingly,
	 * proving the header is computed from the payload, not hard-coded. */
	static const uint8_t small[4] = {0xde, 0xad, 0xbe, 0xef};

	struct array_output_data aod;
	struct serializer s;
	array_output_serializer_init(&s, &aod);

	size_t written = mp4_write_dvvC_box(&s, small, sizeof(small));

	assert_int_equal(written, 8 + sizeof(small));
	const uint8_t *out = aod.bytes.array;
	assert_int_equal(out[0], 0x00);
	assert_int_equal(out[1], 0x00);
	assert_int_equal(out[2], 0x00);
	assert_int_equal(out[3], 8 + sizeof(small));
	assert_memory_equal(out + 4, "dvvC", 4);
	assert_memory_equal(out + 8, small, sizeof(small));

	array_output_serializer_free(&aod);
}

static void dvvC_box_empty_payload_is_header_only(void **state)
{
	UNUSED_PARAMETER(state);

	/* Boundary: an empty record still yields a well-formed 8-byte box rather
	 * than reading past the (absent) payload. */
	struct array_output_data aod;
	struct serializer s;
	array_output_serializer_init(&s, &aod);

	size_t written = mp4_write_dvvC_box(&s, dvvC_payload, 0);

	assert_int_equal(written, 8);
	assert_int_equal(aod.bytes.num, 8);
	const uint8_t *out = aod.bytes.array;
	assert_int_equal(out[0], 0x00);
	assert_int_equal(out[1], 0x00);
	assert_int_equal(out[2], 0x00);
	assert_int_equal(out[3], 8);
	assert_memory_equal(out + 4, "dvvC", 4);

	array_output_serializer_free(&aod);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(dvvC_box_frames_payload_verbatim),
		cmocka_unit_test(dvvC_box_size_scales_with_payload),
		cmocka_unit_test(dvvC_box_empty_payload_is_header_only),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
