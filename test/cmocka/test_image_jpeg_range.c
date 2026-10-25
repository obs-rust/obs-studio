/*
 * Decoding a JPEG must not make swscale log "deprecated pixel format used,
 * make sure you did set range correctly" (obs-rust/obs-studio#2334). An
 * Image Slide Show over JPEG files logged that line once per slide.
 *
 * The MJPEG decoder outputs the deprecated full-range AV_PIX_FMT_YUVJ*
 * formats. graphics-ffmpeg handed them to sws_getContext() as they came,
 * and swscale warns once for every context it creates from one. The image
 * decode creates a context per image, so every decode warned again.
 *
 * Every case encodes a 64x16 JPEG with libavcodec in one YUVJ layout (4:2:0,
 * the usual camera and baseline JPEG, 4:2:2 and 4:4:4), made of four 16x16
 * solid blocks of known colour, and loads it twice through
 * gs_create_texture_file_data(), the CPU decode the image source uses, with
 * an av_log callback counting the warning. It must not appear.
 *
 * The decoded colours must stay what they are: full range, BT.601. A fix
 * that only renamed YUVJ to YUV would read the planes as limited range and
 * darken every JPEG (0x103050 becomes 72 36 0 instead of 79 47 15 BGR).
 * Each block's centre must be within 2 of the colour the decoder's own
 * Y/Cb/Cr give through the full-range BT.601 equations, and within 4 of the
 * colour encoded. Both checks hold before the fix too.
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
#include <libavutil/frame.h>
#include <libavutil/log.h>
#include <libavutil/pixdesc.h>

#include <graphics/graphics.h>
#include <util/bmem.h>

#define BLOCK 16
#define BLOCKS 4
#define W (BLOCK * BLOCKS)
#define H BLOCK

/* One colour per block. They are far enough from their limited-range
 * reading that a darkened decode cannot pass. */
static const uint8_t block_rgb[BLOCKS][3] = {
	{0x10, 0x30, 0x50}, /* the colour of the research: 79 47 15 BGR */
	{200, 100, 50},
	{32, 160, 96},
	{240, 224, 208},
};

static int deprecated_warnings;

static void count_av_log(void *avcl, int level, const char *fmt, va_list vl)
{
	(void)avcl;
	char line[1024];
	va_list copy;
	va_copy(copy, vl);
	vsnprintf(line, sizeof(line), fmt, copy);
	va_end(copy);
	if (strstr(line, "deprecated pixel format")) {
		deprecated_warnings++;
		fprintf(stderr, "av_log level %d: %s", level, line);
	}
}

struct image {
	uint8_t *data;
	size_t size;
};

static void rgb_to_ycbcr(const uint8_t *rgb, uint8_t *ycc)
{
	const double r = rgb[0], g = rgb[1], b = rgb[2];
	const double y = 0.299 * r + 0.587 * g + 0.114 * b;
	const double v[3] = {y, 128.0 + (b - y) * 0.564, 128.0 + (r - y) * 0.713};
	for (int i = 0; i < 3; i++)
		ycc[i] = (uint8_t)(v[i] < 0 ? 0 : v[i] > 255 ? 255 : v[i] + 0.5);
}

static struct image encode_blocks(enum AVPixelFormat pix_fmt)
{
	struct image result = {0};
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_MJPEG);
	if (!codec) {
		fprintf(stderr, "libavcodec has no MJPEG encoder\n");
		fail();
	}

	AVCodecContext *ctx = avcodec_alloc_context3(codec);
	ctx->width = W;
	ctx->height = H;
	ctx->time_base = (AVRational){1, 30};
	ctx->pix_fmt = pix_fmt;
	ctx->color_range = AVCOL_RANGE_JPEG;
	ctx->flags |= AV_CODEC_FLAG_QSCALE;
	ctx->global_quality = FF_QP2LAMBDA * 2;
	ctx->strict_std_compliance = FF_COMPLIANCE_UNOFFICIAL;
	if (avcodec_open2(ctx, codec, NULL) < 0) {
		fprintf(stderr, "cannot open the MJPEG encoder for %s\n", av_get_pix_fmt_name(pix_fmt));
		fail();
	}

	AVFrame *frame = av_frame_alloc();
	frame->width = W;
	frame->height = H;
	frame->format = pix_fmt;
	frame->color_range = AVCOL_RANGE_JPEG;
	frame->quality = ctx->global_quality;
	assert_int_equal(av_frame_get_buffer(frame, 0), 0);

	const AVPixFmtDescriptor *desc = av_pix_fmt_desc_get(pix_fmt);
	for (int y = 0; y < H; y++) {
		for (int x = 0; x < W; x++) {
			uint8_t ycc[3];
			rgb_to_ycbcr(block_rgb[x / BLOCK], ycc);
			frame->data[0][y * frame->linesize[0] + x] = ycc[0];
			const int cx = x >> desc->log2_chroma_w, cy = y >> desc->log2_chroma_h;
			frame->data[1][cy * frame->linesize[1] + cx] = ycc[1];
			frame->data[2][cy * frame->linesize[2] + cx] = ycc[2];
		}
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

static int clamp_round(double v)
{
	return v < 0 ? 0 : v > 255 ? 255 : (int)(v + 0.5);
}

/* The colour the decoder's own samples give at each block's centre, by the
 * full-range BT.601 equations. Also checks the decoder outputs the YUVJ
 * format this case is about. */
static void reference_rgb(const struct image *image, enum AVPixelFormat pix_fmt, int rgb[BLOCKS][3])
{
	const AVCodec *codec = avcodec_find_decoder(AV_CODEC_ID_MJPEG);
	AVCodecContext *ctx = avcodec_alloc_context3(codec);
	AVPacket *pkt = av_packet_alloc();
	AVFrame *frame = av_frame_alloc();
	assert_int_equal(avcodec_open2(ctx, codec, NULL), 0);

	pkt->data = image->data;
	pkt->size = (int)image->size;
	assert_int_equal(avcodec_send_packet(ctx, pkt), 0);
	assert_int_equal(avcodec_receive_frame(ctx, frame), 0);
	fprintf(stderr, "%s: the decoder outputs %s, range %s\n", av_get_pix_fmt_name(pix_fmt),
		av_get_pix_fmt_name(frame->format), av_color_range_name(frame->color_range));
	assert_int_equal(frame->format, pix_fmt);

	const AVPixFmtDescriptor *desc = av_pix_fmt_desc_get(frame->format);
	for (int b = 0; b < BLOCKS; b++) {
		const int x = b * BLOCK + BLOCK / 2, y = H / 2;
		const int cx = x >> desc->log2_chroma_w, cy = y >> desc->log2_chroma_h;
		const double yy = frame->data[0][y * frame->linesize[0] + x];
		const double cb = frame->data[1][cy * frame->linesize[1] + cx] - 128.0;
		const double cr = frame->data[2][cy * frame->linesize[2] + cx] - 128.0;
		rgb[b][0] = clamp_round(yy + 1.402 * cr);
		rgb[b][1] = clamp_round(yy - 0.344136 * cb - 0.714136 * cr);
		rgb[b][2] = clamp_round(yy + 1.772 * cb);
	}

	av_frame_free(&frame);
	pkt->data = NULL;
	pkt->size = 0;
	av_packet_free(&pkt);
	avcodec_free_context(&ctx);
}

static bool near(int got, int want, int tolerance)
{
	return got >= want - tolerance && got <= want + tolerance;
}

static void check_jpeg(enum AVPixelFormat pix_fmt)
{
	const char *name = av_get_pix_fmt_name(pix_fmt);
	struct image image = encode_blocks(pix_fmt);
	int want[BLOCKS][3];
	reference_rgb(&image, pix_fmt, want);

	char path[64];
	snprintf(path, sizeof(path), "test_image_jpeg_range_%s.jpg", name);
	FILE *f = fopen(path, "wb");
	assert_non_null(f);
	assert_int_equal(fwrite(image.data, 1, image.size, f), image.size);
	fclose(f);
	free(image.data);

	av_log_set_callback(count_av_log);
	deprecated_warnings = 0;

	bool ok = true;
	for (int pass = 0; pass < 2; pass++) {
		enum gs_color_format color_format = GS_UNKNOWN;
		uint32_t cx = 0, cy = 0;
		uint8_t *data = gs_create_texture_file_data(path, &color_format, &cx, &cy);
		if (!data) {
			fprintf(stderr, "%s: libobs could not load the image\n", name);
			ok = false;
			break;
		}
		if (cx != W || cy != H || (color_format != GS_BGRA && color_format != GS_BGRX)) {
			fprintf(stderr, "%s: decoded %ux%u format %d, expected %dx%d BGRA/BGRX\n", name, cx, cy,
				(int)color_format, W, H);
			bfree(data);
			ok = false;
			break;
		}

		const uint32_t crc = av_crc(av_crc_get_table(AV_CRC_32_IEEE_LE), 0, data, (size_t)cx * cy * 4);
		fprintf(stderr, "%s decode %d: pixels crc32 %08x\n", name, pass + 1, crc);
		for (int b = 0; b < BLOCKS; b++) {
			const uint8_t *p = data + ((size_t)(H / 2) * cx + (size_t)(b * BLOCK + BLOCK / 2)) * 4;
			const int r = p[2], g = p[1], bl = p[0];
			const bool pass_ref = near(r, want[b][0], 2) && near(g, want[b][1], 2) &&
					      near(bl, want[b][2], 2);
			const bool pass_src = near(r, block_rgb[b][0], 4) && near(g, block_rgb[b][1], 4) &&
					      near(bl, block_rgb[b][2], 4);
			fprintf(stderr,
				"  block %d: BGR %3d %3d %3d, full-range reference %3d %3d %3d, encoded %3d %3d "
				"%3d%s\n",
				b, bl, g, r, want[b][2], want[b][1], want[b][0], block_rgb[b][2], block_rgb[b][1],
				block_rgb[b][0], pass_ref && pass_src ? "" : "  WRONG");
			if (!pass_ref || !pass_src)
				ok = false;
		}
		bfree(data);
	}

	av_log_set_callback(av_log_default_callback);
	remove(path);

	fprintf(stderr, "%s: %d \"deprecated pixel format\" warnings over 2 decodes\n", name, deprecated_warnings);
	if (!ok || deprecated_warnings != 0)
		fail();
}

static void test_yuvj420p(void **state)
{
	(void)state;
	check_jpeg(AV_PIX_FMT_YUVJ420P);
}

static void test_yuvj422p(void **state)
{
	(void)state;
	check_jpeg(AV_PIX_FMT_YUVJ422P);
}

static void test_yuvj444p(void **state)
{
	(void)state;
	check_jpeg(AV_PIX_FMT_YUVJ444P);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_yuvj420p),
		cmocka_unit_test(test_yuvj422p),
		cmocka_unit_test(test_yuvj444p),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
