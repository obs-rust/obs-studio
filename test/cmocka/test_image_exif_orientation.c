/*
 * Image Source and Image Slide Show must show an image the way its EXIF
 * Orientation tag says (OBS forum thread 195973, "Image rotation in version
 * 32.2.1 broken"): photos rotated in a file manager or taken on a phone in
 * portrait were shown sideways or mirrored.
 *
 * libobs reads the orientation in graphics-ffmpeg.c. FFmpeg 8.0 and earlier
 * put the EXIF tag of a JPEG or WebP in the decoded frame's metadata as
 * "Orientation". FFmpeg 8.1 parses the EXIF block itself (av_exif_* API) for
 * JPEG, WebP and PNG, and exports the orientation as display matrix side
 * data instead, so the metadata lookup finds nothing and the image is shown
 * as stored.
 *
 * Every case encodes a 32x16 image with libavcodec, four solid quadrants
 * (red, green / blue, white), as JPEG, PNG and WebP, inserts an EXIF block
 * with one orientation (1 to 8; 0: none) and loads it through
 * gs_create_texture_file_data(), the CPU decode the image source uses, so no
 * display or graphics module is needed. The decoded size and the colour of
 * each corner must match the orientation. Each case also logs what the
 * decoder exported for the file (metadata tag or display matrix), since that
 * depends on the FFmpeg version. PNG orientations skip before FFmpeg 8.1,
 * which does not read a PNG's EXIF block; WebP skips without libwebp.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavutil/crc.h>
#include <libavutil/display.h>
#include <libavutil/frame.h>

#include <graphics/graphics.h>
#include <util/bmem.h>

#define W 32
#define H 16

enum colour { RED, GREEN, BLUE, WHITE, OTHER };
static const char *const colour_names[] = {"red", "green", "blue", "white", "other"};
static const uint8_t colour_rgb[4][3] = {{255, 0, 0}, {0, 255, 0}, {0, 0, 255}, {255, 255, 255}};

/* Stored image: red | green over blue | white */
static enum colour stored_quadrant(int x, int y)
{
	if (y < H / 2)
		return x < W / 2 ? RED : GREEN;
	return x < W / 2 ? BLUE : WHITE;
}

struct expected {
	uint32_t cx, cy;
	/* top left, top right, bottom left, bottom right */
	enum colour corner[4];
};

/* What a viewer shows for each EXIF orientation (index 0: no EXIF block). */
static const struct expected expected[9] = {
	{W, H, {RED, GREEN, BLUE, WHITE}}, /* no EXIF */
	{W, H, {RED, GREEN, BLUE, WHITE}}, /* 1: as stored */
	{W, H, {GREEN, RED, WHITE, BLUE}}, /* 2: mirrored horizontally */
	{W, H, {WHITE, BLUE, GREEN, RED}}, /* 3: rotated 180 */
	{W, H, {BLUE, WHITE, RED, GREEN}}, /* 4: mirrored vertically */
	{H, W, {RED, BLUE, GREEN, WHITE}}, /* 5: transposed */
	{H, W, {BLUE, RED, WHITE, GREEN}}, /* 6: rotated 90 clockwise */
	{H, W, {WHITE, GREEN, BLUE, RED}}, /* 7: transversed */
	{H, W, {GREEN, WHITE, RED, BLUE}}, /* 8: rotated 90 counterclockwise */
};

enum format { JPEG, PNG, WEBP };
static const char *const format_names[] = {"JPEG", "PNG", "WebP"};
static const char *const format_exts[] = {"jpg", "png", "webp"};

struct image {
	uint8_t *data;
	size_t size;
};

/* FFmpeg 8.1 (libavcodec 62.13.100, the av_exif_* API) is the first to read
 * the EXIF block of a PNG (eXIf chunk) at all. */
static bool ffmpeg_reads_png_exif(void)
{
	return avcodec_version() >= AV_VERSION_INT(62, 13, 100);
}

static const AVCodec *find_encoder(enum format format)
{
	switch (format) {
	case JPEG:
		return avcodec_find_encoder(AV_CODEC_ID_MJPEG);
	case PNG:
		return avcodec_find_encoder(AV_CODEC_ID_PNG);
	case WEBP:
		/* the only WebP encoder; optional in FFmpeg builds */
		return avcodec_find_encoder_by_name("libwebp");
	}
	return NULL;
}

static void put_pixel(AVFrame *frame, enum format format, int x, int y, const uint8_t *rgb)
{
	if (format == JPEG) {
		const double r = rgb[0], g = rgb[1], b = rgb[2];
		const double yy = 0.299 * r + 0.587 * g + 0.114 * b;
		const double cb = 128.0 + (b - yy) * 0.564;
		const double cr = 128.0 + (r - yy) * 0.713;
		frame->data[0][y * frame->linesize[0] + x] = (uint8_t)(yy + 0.5);
		frame->data[1][y * frame->linesize[1] + x] = (uint8_t)(cb < 0 ? 0 : cb > 255 ? 255 : cb + 0.5);
		frame->data[2][y * frame->linesize[2] + x] = (uint8_t)(cr < 0 ? 0 : cr > 255 ? 255 : cr + 0.5);
	} else if (format == PNG) {
		uint8_t *p = frame->data[0] + y * frame->linesize[0] + x * 3;
		memcpy(p, rgb, 3);
	} else {
		/* AV_PIX_FMT_RGB32: one native-endian 0xAARRGGBB word per pixel */
		const uint32_t argb = 0xFF000000u | ((uint32_t)rgb[0] << 16) | ((uint32_t)rgb[1] << 8) | rgb[2];
		memcpy(frame->data[0] + y * frame->linesize[0] + x * 4, &argb, 4);
	}
}

static struct image encode_quadrants(enum format format)
{
	struct image result = {0};
	const AVCodec *codec = find_encoder(format);
	if (!codec) {
		fprintf(stderr, "libavcodec has no %s encoder\n", format_names[format]);
		fail();
	}

	AVCodecContext *ctx = avcodec_alloc_context3(codec);
	AVDictionary *opts = NULL;
	ctx->width = W;
	ctx->height = H;
	ctx->time_base = (AVRational){1, 30};
	if (format == JPEG) {
		/* 4:4:4 and 8x8 aligned quadrants keep the colours apart */
		ctx->pix_fmt = AV_PIX_FMT_YUVJ444P;
		ctx->color_range = AVCOL_RANGE_JPEG;
		ctx->flags |= AV_CODEC_FLAG_QSCALE;
		ctx->global_quality = FF_QP2LAMBDA * 2;
		ctx->strict_std_compliance = FF_COMPLIANCE_UNOFFICIAL;
	} else if (format == PNG) {
		ctx->pix_fmt = AV_PIX_FMT_RGB24;
	} else {
		ctx->pix_fmt = AV_PIX_FMT_RGB32;
		av_dict_set(&opts, "lossless", "1", 0);
	}
	if (avcodec_open2(ctx, codec, &opts) < 0) {
		fprintf(stderr, "cannot open the %s encoder\n", format_names[format]);
		fail();
	}
	av_dict_free(&opts);

	AVFrame *frame = av_frame_alloc();
	frame->width = W;
	frame->height = H;
	frame->format = ctx->pix_fmt;
	frame->color_range = ctx->color_range;
	frame->quality = ctx->global_quality;
	assert_int_equal(av_frame_get_buffer(frame, 0), 0);
	for (int y = 0; y < H; y++) {
		for (int x = 0; x < W; x++)
			put_pixel(frame, format, x, y, colour_rgb[stored_quadrant(x, y)]);
	}

	AVPacket *pkt = av_packet_alloc();
	assert_int_equal(avcodec_send_frame(ctx, frame), 0);
	assert_int_equal(avcodec_send_frame(ctx, NULL), 0);
	assert_int_equal(avcodec_receive_packet(ctx, pkt), 0);

	result.size = pkt->size;
	result.data = malloc(pkt->size);
	memcpy(result.data, pkt->data, pkt->size);

	av_packet_free(&pkt);
	av_frame_free(&frame);
	avcodec_free_context(&ctx);
	return result;
}

/* A TIFF block with one IFD0 entry: Orientation. This is the payload of the
 * JPEG APP1 segment (after "Exif\0\0"), the PNG eXIf chunk and the WebP EXIF
 * chunk alike. */
#define TIFF_SIZE 26
static void make_tiff(uint8_t tiff[TIFF_SIZE], int orientation)
{
	static const uint8_t head[] = {
		'I',  'I',  0x2A, 0x00, /* TIFF, little endian */
		0x08, 0x00, 0x00, 0x00, /* IFD0 at offset 8 */
		0x01, 0x00,             /* one entry */
		0x12, 0x01,             /* tag 0x0112 Orientation */
		0x03, 0x00,             /* SHORT */
		0x01, 0x00, 0x00, 0x00, /* count 1 */
	};
	memcpy(tiff, head, sizeof(head));
	tiff[18] = (uint8_t)orientation; /* value, then padding */
	tiff[19] = 0;
	memset(tiff + 20, 0, TIFF_SIZE - 20); /* padding, no next IFD */
}

static void put_be32(uint8_t *p, uint32_t v)
{
	p[0] = (uint8_t)(v >> 24);
	p[1] = (uint8_t)(v >> 16);
	p[2] = (uint8_t)(v >> 8);
	p[3] = (uint8_t)v;
}

static void put_le(uint8_t *p, uint32_t v, int bytes)
{
	for (int i = 0; i < bytes; i++)
		p[i] = (uint8_t)(v >> (8 * i));
}

static uint32_t get_le32(const uint8_t *p)
{
	return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

/* JPEG: an APP1 EXIF segment right after SOI. */
static struct image jpeg_with_exif(const struct image *plain, const uint8_t tiff[TIFF_SIZE])
{
	struct image result;
	assert_true(plain->size > 2 && plain->data[0] == 0xFF && plain->data[1] == 0xD8);
	result.size = plain->size + 4 + 6 + TIFF_SIZE;
	result.data = malloc(result.size);
	uint8_t *p = result.data;
	memcpy(p, plain->data, 2); /* SOI */
	p += 2;
	*p++ = 0xFF;
	*p++ = 0xE1; /* APP1 */
	*p++ = 0x00;
	*p++ = 2 + 6 + TIFF_SIZE; /* length */
	memcpy(p, "Exif\0\0", 6);
	p += 6;
	memcpy(p, tiff, TIFF_SIZE);
	p += TIFF_SIZE;
	memcpy(p, plain->data + 2, plain->size - 2);
	return result;
}

/* PNG: an eXIf chunk right after IHDR. */
static struct image png_with_exif(const struct image *plain, const uint8_t tiff[TIFF_SIZE])
{
	static const uint8_t signature[8] = {0x89, 'P', 'N', 'G', '\r', '\n', 0x1A, '\n'};
	const size_t ihdr_end = 8 + 4 + 4 + 13 + 4;
	assert_true(plain->size > ihdr_end && memcmp(plain->data, signature, 8) == 0 &&
		    memcmp(plain->data + 12, "IHDR", 4) == 0);

	struct image result;
	result.size = plain->size + 4 + 4 + TIFF_SIZE + 4;
	result.data = malloc(result.size);
	uint8_t *p = result.data;
	memcpy(p, plain->data, ihdr_end);
	p += ihdr_end;
	put_be32(p, TIFF_SIZE);
	p += 4;
	uint8_t *type = p;
	memcpy(p, "eXIf", 4);
	p += 4;
	memcpy(p, tiff, TIFF_SIZE);
	p += TIFF_SIZE;
	const uint32_t crc = av_crc(av_crc_get_table(AV_CRC_32_IEEE_LE), UINT32_MAX, type, 4 + TIFF_SIZE) ^ UINT32_MAX;
	put_be32(p, crc);
	p += 4;
	memcpy(p, plain->data + ihdr_end, plain->size - ihdr_end);
	return result;
}

/* WebP: the extended format, a VP8X chunk with the EXIF flag before the image
 * chunk and an EXIF chunk after it. */
static struct image webp_with_exif(const struct image *plain, const uint8_t tiff[TIFF_SIZE])
{
	assert_true(plain->size > 20 && memcmp(plain->data, "RIFF", 4) == 0 && memcmp(plain->data + 8, "WEBP", 4) == 0);
	/* libwebp writes a simple file (one VP8 or VP8L chunk) for a still image */
	assert_true(memcmp(plain->data + 12, "VP8X", 4) != 0);

	const size_t chunks = plain->size - 12;
	const size_t exif_chunk = 8 + TIFF_SIZE; /* TIFF_SIZE is even: no pad byte */
	struct image result;
	result.size = 12 + 18 + chunks + exif_chunk;
	result.data = malloc(result.size);
	uint8_t *p = result.data;
	memcpy(p, "RIFF", 4);
	put_le(p + 4, (uint32_t)(result.size - 8), 4);
	memcpy(p + 8, "WEBP", 4);
	p += 12;
	memcpy(p, "VP8X", 4);
	put_le(p + 4, 10, 4);
	p[8] = 0x08; /* EXIF metadata */
	p[9] = p[10] = p[11] = 0;
	put_le(p + 12, W - 1, 3);
	put_le(p + 15, H - 1, 3);
	p += 18;
	memcpy(p, plain->data + 12, chunks);
	assert_true(get_le32(plain->data + 4) == chunks + 4);
	p += chunks;
	memcpy(p, "EXIF", 4);
	put_le(p + 4, TIFF_SIZE, 4);
	memcpy(p + 8, tiff, TIFF_SIZE);
	return result;
}

/* The encoded image with an EXIF block holding the given orientation;
 * orientation 0 leaves the image as it is. */
static struct image with_exif(enum format format, const struct image *plain, int orientation)
{
	if (!orientation) {
		struct image result = {malloc(plain->size), plain->size};
		memcpy(result.data, plain->data, plain->size);
		return result;
	}

	uint8_t tiff[TIFF_SIZE];
	make_tiff(tiff, orientation);
	switch (format) {
	case JPEG:
		return jpeg_with_exif(plain, tiff);
	case PNG:
		return png_with_exif(plain, tiff);
	case WEBP:
		break;
	}
	return webp_with_exif(plain, tiff);
}

/* Logs how this FFmpeg exports the orientation of the file. */
static void describe_decoder_output(enum format format, const struct image *image, int orientation)
{
	static const enum AVCodecID ids[] = {AV_CODEC_ID_MJPEG, AV_CODEC_ID_PNG, AV_CODEC_ID_WEBP};
	const AVCodec *codec = avcodec_find_decoder(ids[format]);
	AVCodecContext *ctx = avcodec_alloc_context3(codec);
	AVPacket *pkt = av_packet_alloc();
	AVFrame *frame = av_frame_alloc();
	if (avcodec_open2(ctx, codec, NULL) < 0)
		goto done;

	pkt->data = image->data;
	pkt->size = (int)image->size;
	if (avcodec_send_packet(ctx, pkt) < 0 || avcodec_receive_frame(ctx, frame) < 0) {
		fprintf(stderr, "%s orientation %d: the decoder rejected the file\n", format_names[format],
			orientation);
		goto done;
	}

	const AVDictionaryEntry *tag = av_dict_get(frame->metadata, "Orientation", NULL, AV_DICT_MATCH_CASE);
	const AVFrameSideData *sd = av_frame_get_side_data(frame, AV_FRAME_DATA_DISPLAYMATRIX);
	if (sd && sd->size >= 9 * sizeof(int32_t)) {
		const int32_t *m = (const int32_t *)sd->data;
		fprintf(stderr,
			"%s orientation %d (%s): metadata Orientation=%s, display matrix [%d %d; %d %d] rotation %.0f\n",
			format_names[format], orientation, LIBAVCODEC_IDENT, tag ? tag->value : "(none)", m[0] / 65536,
			m[1] / 65536, m[3] / 65536, m[4] / 65536, av_display_rotation_get(m));
	} else {
		fprintf(stderr, "%s orientation %d (%s): metadata Orientation=%s, no display matrix\n",
			format_names[format], orientation, LIBAVCODEC_IDENT, tag ? tag->value : "(none)");
	}

done:
	av_frame_free(&frame);
	pkt->data = NULL;
	pkt->size = 0;
	av_packet_free(&pkt);
	avcodec_free_context(&ctx);
}

static enum colour classify(const uint8_t *bgra)
{
	const int b = bgra[0], g = bgra[1], r = bgra[2];
	for (int c = RED; c <= WHITE; c++) {
		if (abs(r - colour_rgb[c][0]) < 60 && abs(g - colour_rgb[c][1]) < 60 && abs(b - colour_rgb[c][2]) < 60)
			return (enum colour)c;
	}
	return OTHER;
}

static void check_orientation(enum format format, int orientation)
{
	if (format == WEBP && !find_encoder(WEBP)) {
		fprintf(stderr, "libavcodec has no WebP encoder (libwebp), skipping\n");
		skip();
	}
	if (format == PNG && orientation > 1 && !ffmpeg_reads_png_exif()) {
		fprintf(stderr, "%s cannot read the EXIF block of a PNG (needs FFmpeg 8.1), skipping\n",
			LIBAVCODEC_IDENT);
		skip();
	}

	struct image plain = encode_quadrants(format);
	struct image image = with_exif(format, &plain, orientation);
	describe_decoder_output(format, &image, orientation);

	char path[64];
	snprintf(path, sizeof(path), "test_image_exif_orientation_%d.%s", orientation, format_exts[format]);
	FILE *f = fopen(path, "wb");
	assert_non_null(f);
	assert_int_equal(fwrite(image.data, 1, image.size, f), image.size);
	fclose(f);
	free(image.data);
	free(plain.data);

	enum gs_color_format color_format = GS_UNKNOWN;
	uint32_t cx = 0, cy = 0;
	uint8_t *data = gs_create_texture_file_data(path, &color_format, &cx, &cy);
	remove(path);
	if (!data) {
		fprintf(stderr, "%s orientation %d: libobs could not load the image\n", format_names[format],
			orientation);
		fail();
	}
	assert_true(color_format == GS_BGRA || color_format == GS_BGRX);

	const struct expected *want = &expected[orientation];
	const uint32_t xs[4] = {1, cx - 2, 1, cx - 2};
	const uint32_t ys[4] = {1, 1, cy - 2, cy - 2};
	static const char *const corner_names[4] = {"top left", "top right", "bottom left", "bottom right"};

	bool ok = cx == want->cx && cy == want->cy;
	enum colour got[4] = {OTHER, OTHER, OTHER, OTHER};
	if (ok) {
		for (int i = 0; i < 4; i++) {
			got[i] = classify(data + ((size_t)ys[i] * cx + xs[i]) * 4);
			if (got[i] != want->corner[i])
				ok = false;
		}
	}

	if (!ok) {
		fprintf(stderr, "%s orientation %d: decoded %ux%u, expected %ux%u\n", format_names[format], orientation,
			cx, cy, want->cx, want->cy);
		if (cx == want->cx && cy == want->cy) {
			for (int i = 0; i < 4; i++)
				fprintf(stderr, "  %s: %s, expected %s\n", corner_names[i], colour_names[got[i]],
					colour_names[want->corner[i]]);
		}
		bfree(data);
		fail();
	}

	bfree(data);
}

#define ORIENTATION_TEST(fmt, name, n)                     \
	static void test_##name##_orientation_##n(void **state) \
	{                                                      \
		(void)state;                                   \
		check_orientation(fmt, n);                     \
	}

#define FORMAT_TESTS(fmt, name)          \
	ORIENTATION_TEST(fmt, name, 0)   \
	ORIENTATION_TEST(fmt, name, 1)   \
	ORIENTATION_TEST(fmt, name, 2)   \
	ORIENTATION_TEST(fmt, name, 3)   \
	ORIENTATION_TEST(fmt, name, 4)   \
	ORIENTATION_TEST(fmt, name, 5)   \
	ORIENTATION_TEST(fmt, name, 6)   \
	ORIENTATION_TEST(fmt, name, 7)   \
	ORIENTATION_TEST(fmt, name, 8)

FORMAT_TESTS(JPEG, jpeg)
FORMAT_TESTS(PNG, png)
FORMAT_TESTS(WEBP, webp)

#define FORMAT_CASES(name)                                                                 \
	cmocka_unit_test(test_##name##_orientation_0), cmocka_unit_test(test_##name##_orientation_1), \
		cmocka_unit_test(test_##name##_orientation_2), cmocka_unit_test(test_##name##_orientation_3), \
		cmocka_unit_test(test_##name##_orientation_4), cmocka_unit_test(test_##name##_orientation_5), \
		cmocka_unit_test(test_##name##_orientation_6), cmocka_unit_test(test_##name##_orientation_7), \
		cmocka_unit_test(test_##name##_orientation_8)

int main(void)
{
	const struct CMUnitTest tests[] = {
		FORMAT_CASES(jpeg),
		FORMAT_CASES(png),
		FORMAT_CASES(webp),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
