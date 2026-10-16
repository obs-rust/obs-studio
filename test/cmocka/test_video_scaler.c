/*
 * Characterization test for libobs/media-io/video-scaler-ffmpeg.c.
 *
 * Pins the observable behavior of video_scaler_scale() across output
 * layouts ahead of the guarded direct-output rework: the same scale must
 * produce identical visible bytes whether the destination planes are packed
 * (video_frame_init layout), padded to a larger stride, or 32-aligned at a
 * non-zero offset, and no byte outside each plane's linesize*height extent
 * may be touched (canary check).
 *
 * For odd heights on vertically-subsampled formats the C copy path writes
 * floor(height/2) chroma rows; the last chroma row sws produces is not part
 * of the observable output (characterized, not endorsed). The canary behind
 * the last copied chroma row pins that the row is NOT written.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <media-io/video-frame.h>
#include <media-io/video-scaler.h>
#include <util/bmem.h>

#define CANARY 0xA5
#define SLACK 64
#define PAD_STRIDE 64

/* Packed per-plane strides, matching video_frame_get_linesizes (not
 * exported, so replicated for the formats exercised here). */
static void packed_linesizes(uint32_t ls[MAX_AV_PLANES], enum video_format fmt, uint32_t width)
{
	uint32_t half = (width + 1) / 2;
	memset(ls, 0, sizeof(uint32_t) * MAX_AV_PLANES);
	switch (fmt) {
	case VIDEO_FORMAT_BGR3:
		ls[0] = width * 3;
		break;
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
	case VIDEO_FORMAT_AYUV:
		ls[0] = width * 4;
		break;
	case VIDEO_FORMAT_P416:
		ls[0] = width * 2;
		ls[1] = width * 4;
		break;
	case VIDEO_FORMAT_I420:
	case VIDEO_FORMAT_I422:
		ls[0] = width;
		ls[1] = half;
		ls[2] = half;
		break;
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I010:
		ls[0] = width * 2;
		ls[1] = width;
		ls[2] = width;
		break;
	case VIDEO_FORMAT_I40A:
	case VIDEO_FORMAT_I42A:
		ls[0] = width;
		ls[1] = half;
		ls[2] = half;
		ls[3] = width;
		break;
	case VIDEO_FORMAT_YVYU:
	case VIDEO_FORMAT_YUY2:
	case VIDEO_FORMAT_UYVY:
		ls[0] = width * 2;
		break;
	case VIDEO_FORMAT_P010:
	case VIDEO_FORMAT_P216:
		ls[0] = width * 2;
		ls[1] = width * 2;
		break;
	case VIDEO_FORMAT_I412:
	case VIDEO_FORMAT_YA2L:
		ls[0] = width * 2;
		ls[1] = width * 2;
		ls[2] = width * 2;
		ls[3] = width * 2;
		break;
	case VIDEO_FORMAT_Y800:
		ls[0] = width;
		break;
	case VIDEO_FORMAT_NV12:
	case VIDEO_FORMAT_I444:
	case VIDEO_FORMAT_YUVA:
		ls[0] = width;
		ls[1] = width;
		ls[2] = width;
		ls[3] = fmt == VIDEO_FORMAT_NV12 ? 0 : width;
		break;
	default:
		break;
	}
	if (fmt == VIDEO_FORMAT_NV12) {
		ls[1] = width;
		ls[2] = 0;
		ls[3] = 0;
	}
}

/* Rows the scale writes per plane for the formats exercised here,
 * matching video_scaler's copy path: chroma planes of 420-subsampled
 * formats copy floor(h/2) rows. */
static void scale_rows(uint32_t rows[MAX_AV_PLANES], enum video_format fmt, uint32_t height)
{
	memset(rows, 0, sizeof(uint32_t) * MAX_AV_PLANES);
	switch (fmt) {
	case VIDEO_FORMAT_I420:
	case VIDEO_FORMAT_I010:
		rows[0] = height;
		rows[1] = height >> 1;
		rows[2] = height >> 1;
		break;
	case VIDEO_FORMAT_NV12:
	case VIDEO_FORMAT_P010:
		rows[0] = height;
		rows[1] = height >> 1;
		break;
	case VIDEO_FORMAT_I40A:
		rows[0] = height;
		rows[1] = height >> 1;
		rows[2] = height >> 1;
		rows[3] = height;
		break;
	case VIDEO_FORMAT_I422:
	case VIDEO_FORMAT_I444:
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I412:
		rows[0] = height;
		rows[1] = height;
		rows[2] = height;
		break;
	case VIDEO_FORMAT_I42A:
	case VIDEO_FORMAT_YUVA:
	case VIDEO_FORMAT_YA2L:
		rows[0] = height;
		rows[1] = height;
		rows[2] = height;
		rows[3] = height;
		break;
	default:
		rows[0] = height;
		break;
	}
}

/* Per-plane heights of the allocated buffer, matching video_frame_init
 * (ceil for subsampled planes). */
static void plane_rows(uint32_t rows[MAX_AV_PLANES], enum video_format fmt, uint32_t height)
{
	memset(rows, 0, sizeof(uint32_t) * MAX_AV_PLANES);
	switch (fmt) {
	case VIDEO_FORMAT_I420:
	case VIDEO_FORMAT_I010:
		rows[0] = height;
		rows[1] = (height + 1) >> 1;
		rows[2] = (height + 1) >> 1;
		break;
	case VIDEO_FORMAT_NV12:
	case VIDEO_FORMAT_P010:
		rows[0] = height;
		rows[1] = (height + 1) >> 1;
		break;
	case VIDEO_FORMAT_I40A:
		rows[0] = height;
		rows[1] = (height + 1) >> 1;
		rows[2] = (height + 1) >> 1;
		rows[3] = height;
		break;
	default:
		scale_rows(rows, fmt, height);
		break;
	}
}

static void fill_pattern(struct video_frame *f, enum video_format fmt, uint32_t height)
{
	uint32_t rows[MAX_AV_PLANES];
	plane_rows(rows, fmt, height);
	for (int p = 0; p < MAX_AV_PLANES; p++) {
		if (!f->data[p] || !f->linesize[p])
			continue;
		for (uint32_t y = 0; y < rows[p]; y++) {
			uint8_t *row = f->data[p] + (size_t)y * f->linesize[p];
			for (uint32_t x = 0; x < f->linesize[p]; x++)
				row[x] = (uint8_t)(x * 7 + y * 31 + p * 97);
		}
	}
}

/* Destination image held in independently allocated planes, each trailed by
 * a canary region. `linesize` is the stride video_scaler_scale is told. */
struct canary_dst {
	uint8_t *base[MAX_AV_PLANES];
	uint8_t *data[MAX_AV_PLANES];
	uint32_t linesize[MAX_AV_PLANES];
	uint32_t height[MAX_AV_PLANES];
};

enum dst_layout { LAYOUT_PACKED, LAYOUT_PADDED, LAYOUT_MISALIGNED };

static void alloc_canary_dst(struct canary_dst *d, enum video_format fmt, uint32_t width, uint32_t height,
			     enum dst_layout layout, int misalign)
{
	uint32_t packed[MAX_AV_PLANES] = {0};
	uint32_t prows[MAX_AV_PLANES] = {0};
	packed_linesizes(packed, fmt, width);
	plane_rows(prows, fmt, height);
	memset(d, 0, sizeof(*d));
	for (int p = 0; p < MAX_AV_PLANES; p++) {
		if (!packed[p] || !prows[p])
			continue;
		d->linesize[p] = packed[p];
		if (layout == LAYOUT_PADDED)
			d->linesize[p] += PAD_STRIDE;
		d->height[p] = prows[p];
		size_t extent = (size_t)d->linesize[p] * prows[p];
		d->base[p] = bmalloc(extent + misalign + SLACK);
		memset(d->base[p], CANARY, extent + misalign + SLACK);
		d->data[p] = d->base[p] + misalign;
	}
}

static void free_canary_dst(struct canary_dst *d)
{
	for (int p = 0; p < MAX_AV_PLANES; p++)
		if (d->base[p])
			bfree(d->base[p]);
}

/* Visible bytes plus canary slack behind the last row. Returns false and
 * prints context on mismatch. */
static bool dst_matches(const struct canary_dst *a, const struct canary_dst *b, enum video_format fmt, uint32_t width,
			uint32_t height)
{
	uint32_t rows[MAX_AV_PLANES] = {0};
	uint32_t packed[MAX_AV_PLANES] = {0};
	scale_rows(rows, fmt, height);
	packed_linesizes(packed, fmt, width);
	for (int p = 0; p < MAX_AV_PLANES; p++) {
		if (!rows[p])
			continue;
		uint32_t row_bytes = packed[p];
		if (row_bytes > a->linesize[p] || row_bytes > b->linesize[p])
			row_bytes = a->linesize[p] < b->linesize[p] ? a->linesize[p] : b->linesize[p];
		for (uint32_t y = 0; y < rows[p]; y++) {
			const uint8_t *ra = a->data[p] + (size_t)y * a->linesize[p];
			const uint8_t *rb = b->data[p] + (size_t)y * b->linesize[p];
			if (memcmp(ra, rb, row_bytes) != 0) {
				print_error("plane %d row %u diverges\n", p, y);
				return false;
			}
		}
	}
	return true;
}

static bool canary_intact(const struct canary_dst *d)
{
	for (int p = 0; p < MAX_AV_PLANES; p++) {
		if (!d->base[p])
			continue;
		const uint8_t *canary = d->base[p] + (d->data[p] - d->base[p]) + (size_t)d->linesize[p] * d->height[p];
		for (int i = 0; i < SLACK; i++) {
			if (canary[i] != CANARY) {
				print_error("plane %d canary byte %d clobbered (0x%02x)\n", p, i, canary[i]);
				return false;
			}
		}
	}
	return true;
}

/* Bytes beyond the copied extent inside the buffer (e.g. the ceil-allocated
 * final chroma row on odd heights) must be untouched: the C copy path never
 * writes them. */
static bool tail_rows_untouched(const struct canary_dst *d, enum video_format fmt, uint32_t width, uint32_t height)
{
	uint32_t copied[MAX_AV_PLANES] = {0};
	uint32_t alloc_rows[MAX_AV_PLANES] = {0};
	uint32_t packed[MAX_AV_PLANES] = {0};
	scale_rows(copied, fmt, height);
	plane_rows(alloc_rows, fmt, height);
	packed_linesizes(packed, fmt, width);
	for (int p = 0; p < MAX_AV_PLANES; p++) {
		if (!d->base[p])
			continue;
		for (uint32_t y = copied[p]; y < alloc_rows[p]; y++) {
			const uint8_t *row = d->data[p] + (size_t)y * d->linesize[p];
			for (uint32_t x = 0; x < packed[p] && x < d->linesize[p]; x++) {
				if (row[x] != CANARY) {
					print_error("plane %d uncopied tail row %u written\n", p, y);
					return false;
				}
			}
		}
	}
	return true;
}

static void run_scale_case(enum video_format fmt, uint32_t width, uint32_t height)
{
	struct video_scale_info dst = {
		.format = fmt,
		.width = width,
		.height = height,
		.range = VIDEO_RANGE_DEFAULT,
		.colorspace = VIDEO_CS_DEFAULT,
	};
	struct video_scale_info src = {
		.format = VIDEO_FORMAT_BGRA,
		.width = 640,
		.height = 360,
		.range = VIDEO_RANGE_DEFAULT,
		.colorspace = VIDEO_CS_DEFAULT,
	};
	struct video_frame in;
	video_frame_init(&in, src.format, src.width, src.height);
	fill_pattern(&in, src.format, src.height);

	video_scaler_t *scaler;
	assert_int_equal(video_scaler_create(&scaler, &dst, &src, VIDEO_SCALE_FAST_BILINEAR), VIDEO_SCALER_SUCCESS);

	/* padded-stride destination is the reference layout: it always takes
	 * the same path as today (scratch + per-row copy). */
	struct canary_dst ref;
	alloc_canary_dst(&ref, fmt, width, height, LAYOUT_PADDED, 0);
	assert_true(video_scaler_scale(scaler, ref.data, ref.linesize, (const uint8_t *const *)in.data,
				       (const uint32_t *)in.linesize));
	assert_true(canary_intact(&ref));

	/* packed, aligned, and misaligned layouts must produce identical
	 * visible bytes and leave canaries and uncopied tail rows intact. */
	const enum dst_layout layouts[] = {LAYOUT_PACKED, LAYOUT_MISALIGNED};
	const int misaligns[] = {0, 16};
	for (size_t i = 0; i < sizeof(layouts) / sizeof(layouts[0]); i++) {
		struct canary_dst out;
		alloc_canary_dst(&out, fmt, width, height, layouts[i], misaligns[i]);
		assert_true(video_scaler_scale(scaler, out.data, out.linesize, (const uint8_t *const *)in.data,
					       (const uint32_t *)in.linesize));
		assert_true(canary_intact(&out));
		assert_true(tail_rows_untouched(&out, fmt, width, height));
		assert_true(dst_matches(&out, &ref, fmt, width, height));
		free_canary_dst(&out);
	}

	/* repeated calls through the same scaler must not diverge */
	struct canary_dst again;
	alloc_canary_dst(&again, fmt, width, height, LAYOUT_PADDED, 0);
	for (int i = 0; i < 4; i++) {
		memset(again.data[0], CANARY, (size_t)again.linesize[0] * again.height[0]);
		assert_true(video_scaler_scale(scaler, again.data, again.linesize, (const uint8_t *const *)in.data,
					       (const uint32_t *)in.linesize));
		assert_true(dst_matches(&again, &ref, fmt, width, height));
	}

	free_canary_dst(&again);
	free_canary_dst(&ref);
	video_scaler_destroy(scaler);
	video_frame_free(&in);
}

static void scaler_even_sizes(void **state)
{
	(void)state;
	const enum video_format fmts[] = {VIDEO_FORMAT_I420, VIDEO_FORMAT_NV12, VIDEO_FORMAT_I444, VIDEO_FORMAT_YUVA,
					  VIDEO_FORMAT_P010, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_Y800};
	const uint32_t sizes[][2] = {{128, 64}, {322, 242}, {160, 120}};
	for (size_t f = 0; f < sizeof(fmts) / sizeof(fmts[0]); f++)
		for (size_t s = 0; s < sizeof(sizes) / sizeof(sizes[0]); s++)
			run_scale_case(fmts[f], sizes[s][0], sizes[s][1]);
}

static void scaler_odd_geometry(void **state)
{
	(void)state;
	const enum video_format fmts[] = {VIDEO_FORMAT_I420, VIDEO_FORMAT_NV12, VIDEO_FORMAT_P010, VIDEO_FORMAT_I40A,
					  VIDEO_FORMAT_BGRA};
	const uint32_t sizes[][2] = {{161, 121}, {160, 121}, {161, 122}};
	for (size_t f = 0; f < sizeof(fmts) / sizeof(fmts[0]); f++)
		for (size_t s = 0; s < sizeof(sizes) / sizeof(sizes[0]); s++)
			run_scale_case(fmts[f], sizes[s][0], sizes[s][1]);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scaler_even_sizes),
		cmocka_unit_test(scaler_odd_geometry),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
