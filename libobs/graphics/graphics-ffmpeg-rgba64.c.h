#include "libobs/build/libobs-graphics-ffmpeg-rgba64.pch.h"

/* Portable on every compiler; GCC, Clang and MSVC turn it into one byte swap. */
#define obs_bswap16(v) ((uint16_t)(((uint16_t)(v) << 8) | ((uint16_t)(v) >> 8)))

void gs_ffmpeg_image_rgba64be_rows(uint8_t *dst, const uint8_t *src, int cx, int cy, size_t src_linesize,
				   int alpha_mode)
{
	const size_t dst_linesize = (size_t)cx * 4;
	const size_t src_min_line = (dst_linesize * 2) < src_linesize ? (dst_linesize * 2) : src_linesize;
	const size_t row_elements = src_min_line >> 3;
	uint16_t value[4];
	float f[4];
	if (alpha_mode == GS_FFMPEG_RGBA64_ALPHA_STRAIGHT) {
		for (int y = 0; y < cy; y++) {
			for (size_t x = 0; x < row_elements; ++x) {
				memcpy(value, src, sizeof(value));
				f[0] = (float)obs_bswap16(value[0]) / 65535.0f;
				f[1] = (float)obs_bswap16(value[1]) / 65535.0f;
				f[2] = (float)obs_bswap16(value[2]) / 65535.0f;
				f[3] = (float)obs_bswap16(value[3]) / 65535.0f;
				gs_float4_to_u8x4(dst, f);
				dst += sizeof(*dst) * 4;
				src += sizeof(value);
			}

			src += src_linesize - src_min_line;
		}
	} else if (alpha_mode == GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB) {
		for (int y = 0; y < cy; y++) {
			for (size_t x = 0; x < row_elements; ++x) {
				memcpy(value, src, sizeof(value));
				f[0] = (float)obs_bswap16(value[0]) / 65535.0f;
				f[1] = (float)obs_bswap16(value[1]) / 65535.0f;
				f[2] = (float)obs_bswap16(value[2]) / 65535.0f;
				f[3] = (float)obs_bswap16(value[3]) / 65535.0f;
				gs_float3_srgb_nonlinear_to_linear(f);
				gs_premultiply_float4(f);
				gs_float3_srgb_linear_to_nonlinear(f);
				gs_float4_to_u8x4(dst, f);
				dst += sizeof(*dst) * 4;
				src += sizeof(value);
			}

			src += src_linesize - src_min_line;
		}
	} else if (alpha_mode == GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY) {
		for (int y = 0; y < cy; y++) {
			for (size_t x = 0; x < row_elements; ++x) {
				memcpy(value, src, sizeof(value));
				f[0] = (float)obs_bswap16(value[0]) / 65535.0f;
				f[1] = (float)obs_bswap16(value[1]) / 65535.0f;
				f[2] = (float)obs_bswap16(value[2]) / 65535.0f;
				f[3] = (float)obs_bswap16(value[3]) / 65535.0f;
				gs_premultiply_float4(f);
				gs_float4_to_u8x4(dst, f);
				dst += sizeof(*dst) * 4;
				src += sizeof(value);
			}

			src += src_linesize - src_min_line;
		}
	}
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef obs_bswap16
