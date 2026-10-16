/* SPDX-License-Identifier: GPL-2.0-or-later */
/* A disjoint padded destination forces the legacy scratch-copy reference.
 * Aliased calls must produce the same visible bytes and leave every byte
 * outside the destination unchanged, including input-only storage. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <media-io/video-frame.h>
#include <media-io/video-scaler.h>
#include <util/bmem.h>

#define GUARD_BYTE 0xA5
#define EXTRA_BYTES 256

enum alias_layout { ALIAS_EXACT, ALIAS_OUTPUT_AFTER, ALIAS_OUTPUT_BEFORE, ALIAS_CHROMA };

static uint32_t source_rows(enum video_format format, size_t plane, uint32_t height)
{
	if ((format == VIDEO_FORMAT_I420 || format == VIDEO_FORMAT_NV12 || format == VIDEO_FORMAT_P010) &&
	    (plane == 1 || plane == 2))
		return (height + 1) / 2;
	return height;
}

static void alias_case(enum video_format format, enum alias_layout layout)
{
	const struct video_scale_info src = {
		.format = format,
		.width = 160,
		.height = 120,
		.range = VIDEO_RANGE_DEFAULT,
		.colorspace = VIDEO_CS_DEFAULT,
	};
	const struct video_scale_info dst = {
		.format = VIDEO_FORMAT_BGRA,
		.width = 320,
		.height = 240,
		.range = VIDEO_RANGE_DEFAULT,
		.colorspace = VIDEO_CS_DEFAULT,
	};
	struct video_frame original;
	video_frame_init(&original, src.format, src.width, src.height);
	size_t source_extent = 0;
	size_t offsets[MAX_AV_PLANES] = {0};
	for (size_t p = 0; p < MAX_AV_PLANES; ++p) {
		if (!original.data[p])
			continue;
		offsets[p] = (size_t)(original.data[p] - original.data[0]);
		const size_t end = offsets[p] + (size_t)original.linesize[p] * source_rows(format, p, src.height);
		if (end > source_extent)
			source_extent = end;
	}

	const size_t input_offset = EXTRA_BYTES;
	size_t output_offset = input_offset;
	if (layout == ALIAS_OUTPUT_AFTER)
		output_offset += 128;
	else if (layout == ALIAS_OUTPUT_BEFORE)
		output_offset -= 128;
	else if (layout == ALIAS_CHROMA) {
		assert_non_null(original.data[1]);
		output_offset += offsets[1];
	}
	const uint32_t row_bytes = dst.width * 4;
	const size_t output_bytes = (size_t)row_bytes * dst.height;
	const size_t shared_bytes = input_offset + source_extent + output_bytes + EXTRA_BYTES;
	uint8_t *shared = bmalloc(shared_bytes);
	uint8_t *snapshot = bmalloc(shared_bytes);
	uint8_t *output[MAX_AV_PLANES] = {shared + output_offset};
	const uint8_t *input[MAX_AV_PLANES] = {0};
	for (size_t p = 0; p < MAX_AV_PLANES; ++p)
		if (original.data[p])
			input[p] = shared + input_offset + offsets[p];
	const uint32_t output_stride[MAX_AV_PLANES] = {row_bytes};

	/* Use independent contexts so cached swscale state cannot couple the
	 * reference and tested calls. Padded stride always selects scratch. */
	video_scaler_t *reference_scaler = NULL;
	video_scaler_t *alias_scaler = NULL;
	assert_int_equal(video_scaler_create(&reference_scaler, &dst, &src, VIDEO_SCALE_FAST_BILINEAR),
			 VIDEO_SCALER_SUCCESS);
	assert_int_equal(video_scaler_create(&alias_scaler, &dst, &src, VIDEO_SCALE_FAST_BILINEAR),
			 VIDEO_SCALER_SUCCESS);
	const uint32_t reference_stride[MAX_AV_PLANES] = {row_bytes + 64};
	const size_t reference_bytes = (size_t)reference_stride[0] * dst.height;
	uint8_t *reference[MAX_AV_PLANES] = {bmalloc(reference_bytes + EXTRA_BYTES)};

	/* Distinct consecutive frames catch a guard that depends on stale data. */
	for (unsigned frame = 0; frame < 3; ++frame) {
		memset(shared, GUARD_BYTE, shared_bytes);
		memset(reference[0], GUARD_BYTE, reference_bytes + EXTRA_BYTES);
		for (size_t p = 0; p < MAX_AV_PLANES; ++p) {
			if (!original.data[p])
				continue;
			const size_t bytes = (size_t)original.linesize[p] * source_rows(format, p, src.height);
			for (size_t i = 0; i < bytes; ++i)
				original.data[p][i] =
					(uint8_t)(i * 7 + (i / original.linesize[p]) * 31 + p * 97 + frame * 43);
			memcpy(shared + input_offset + offsets[p], original.data[p], bytes);
		}
		memcpy(snapshot, shared, shared_bytes);
		assert_true(video_scaler_scale(reference_scaler, reference, reference_stride,
					       (const uint8_t *const *)original.data, original.linesize));
		assert_true(video_scaler_scale(alias_scaler, output, output_stride, input, original.linesize));
		for (uint32_t y = 0; y < dst.height; ++y)
			assert_memory_equal(output[0] + (size_t)y * row_bytes,
					    reference[0] + (size_t)y * reference_stride[0], row_bytes);
		assert_memory_equal(shared, snapshot, output_offset);
		const size_t after = output_offset + output_bytes;
		assert_memory_equal(shared + after, snapshot + after, shared_bytes - after);
		for (size_t i = 0; i < EXTRA_BYTES; ++i)
			assert_int_equal(reference[0][reference_bytes + i], GUARD_BYTE);
	}

	bfree(reference[0]);
	video_scaler_destroy(alias_scaler);
	video_scaler_destroy(reference_scaler);
	bfree(snapshot);
	bfree(shared);
	video_frame_free(&original);
}

static void scaler_alias_bgra(void **state)
{
	(void)state;
	alias_case(VIDEO_FORMAT_BGRA, ALIAS_EXACT);
	alias_case(VIDEO_FORMAT_BGRA, ALIAS_OUTPUT_AFTER);
	alias_case(VIDEO_FORMAT_BGRA, ALIAS_OUTPUT_BEFORE);
}

static void scaler_alias_planar(void **state)
{
	(void)state;
	const enum video_format formats[] = {VIDEO_FORMAT_I420, VIDEO_FORMAT_NV12, VIDEO_FORMAT_P010,
					     VIDEO_FORMAT_YUVA};
	for (size_t f = 0; f < sizeof(formats) / sizeof(formats[0]); ++f) {
		alias_case(formats[f], ALIAS_EXACT);
		alias_case(formats[f], ALIAS_OUTPUT_AFTER);
		alias_case(formats[f], ALIAS_OUTPUT_BEFORE);
		alias_case(formats[f], ALIAS_CHROMA);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scaler_alias_bgra),
		cmocka_unit_test(scaler_alias_planar),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
