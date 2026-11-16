/* Public CPU image loading characterization before extracting the FFmpeg
 * packet/frame orchestration. No graphics device or decoder mocks. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include <libavcodec/avcodec.h>
#include <libavutil/frame.h>
#include "graphics/graphics.h"
#include "graphics/graphics-internal.h"
#include "util/bmem.h"

static void write_image(const char *path, enum AVCodecID codec, bool truncated)
{
	const AVCodec *encoder = avcodec_find_encoder(codec);
	assert_non_null(encoder);
	AVCodecContext *ctx = avcodec_alloc_context3(encoder);
	assert_non_null(ctx);
	ctx->width = 16;
	ctx->height = 8;
	ctx->time_base = (AVRational){1, 1};
	ctx->pix_fmt = codec == AV_CODEC_ID_PNG ? AV_PIX_FMT_RGBA : AV_PIX_FMT_YUVJ420P;
	assert_int_equal(avcodec_open2(ctx, encoder, NULL), 0);
	AVFrame *frame = av_frame_alloc();
	assert_non_null(frame);
	frame->format = ctx->pix_fmt;
	frame->width = ctx->width;
	frame->height = ctx->height;
	assert_int_equal(av_frame_get_buffer(frame, 0), 0);
	for (int y = 0; y < ctx->height; y++) {
		if (codec == AV_CODEC_ID_PNG) {
			for (int x = 0; x < ctx->width; x++) {
				const uint8_t pixel[] = {32, 64, 96, 255};
				memcpy(frame->data[0] + y * frame->linesize[0] + x * 4, pixel, 4);
			}
		} else {
			memset(frame->data[0] + y * frame->linesize[0], 64, (size_t)ctx->width);
		}
	}
	if (codec != AV_CODEC_ID_PNG) {
		for (int p = 1; p < 3; p++)
			for (int y = 0; y < ctx->height / 2; y++)
				memset(frame->data[p] + y * frame->linesize[p], 128, (size_t)ctx->width / 2);
	}
	AVPacket *packet = av_packet_alloc();
	assert_non_null(packet);
	assert_int_equal(avcodec_send_frame(ctx, frame), 0);
	assert_int_equal(avcodec_receive_packet(ctx, packet), 0);
	/* A PNG through IHDR but without IDAT reaches the decode error path. */
	size_t size = truncated ? 33 : (size_t)packet->size;
	assert_true(size <= (size_t)packet->size);
	FILE *file = fopen(path, "wb");
	assert_non_null(file);
	assert_int_equal(fwrite(packet->data, 1, size, file), size);
	assert_int_equal(fclose(file), 0);
	av_packet_free(&packet);
	av_frame_free(&frame);
	avcodec_free_context(&ctx);
}

static void check_load(enum AVCodecID codec, const char *path, enum gs_color_format expected)
{
	write_image(path, codec, false);
	for (int pass = 0; pass < 2; pass++) {
		enum gs_color_format format = GS_UNKNOWN;
		uint32_t width = 0, height = 0;
		uint8_t *pixels = gs_create_texture_file_data(path, &format, &width, &height);
		assert_non_null(pixels);
		assert_int_equal(format, expected);
		assert_int_equal(width, 16);
		assert_int_equal(height, 8);
		for (size_t i = 0; i < 16 * 8; i++) {
			if (codec == AV_CODEC_ID_PNG) {
				const uint8_t pixel[] = {32, 64, 96, 255};
				assert_memory_equal(pixels + i * 4, pixel, 4);
			} else {
				/* Neutral JPEG is gray; allow integer conversion rounding. */
				for (int c = 0; c < 3; c++)
					assert_true(pixels[i * 4 + c] >= 62 && pixels[i * 4 + c] <= 66);
			}
		}
		bfree(pixels);
	}
	assert_int_equal(remove(path), 0);
}

static void png(void **state)
{
	(void)state;
	check_load(AV_CODEC_ID_PNG, "ffdecode-valid.png", GS_RGBA);
}

static void jpeg(void **state)
{
	(void)state;
	check_load(AV_CODEC_ID_MJPEG, "ffdecode-valid.jpg", GS_BGRA);
}

static void truncated_png(void **state)
{
	(void)state;
	const char *path = "ffdecode-truncated.png";
	write_image(path, AV_CODEC_ID_PNG, true);
	for (int pass = 0; pass < 2; pass++) {
		enum gs_color_format format = GS_UNKNOWN;
		uint32_t width = 123, height = 456;
		assert_null(gs_create_texture_file_data(path, &format, &width, &height));
		/* Failure leaves the caller's outputs untouched: characterized. */
		assert_int_equal(format, GS_UNKNOWN);
		assert_int_equal(width, 123);
		assert_int_equal(height, 456);
	}
	assert_int_equal(remove(path), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(png), cmocka_unit_test(jpeg),
					   cmocka_unit_test(truncated_png)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
